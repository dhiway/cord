// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Crash-safe content and provider metadata storage.

#[allow(dead_code)]
pub(crate) mod bucket_mmr;
pub mod streaming;

pub use streaming::{
	BeginStreaming, IngressPermit, IntegritySummary, ProgressAck, StreamingDescriptor,
	StreamingFault, StreamingReceipt, StreamingStore,
};

use std::{
	collections::BTreeMap,
	fs,
	io::Read,
	path::{Path, PathBuf},
	sync::RwLock,
	time::{SystemTime, UNIX_EPOCH},
};

use blake2::{digest::consts::U32, Blake2b, Digest as _};
use codec::{Decode, Encode};
use orbis_storage_runtime_api::{
	CheckpointDutyInfo, DeletionDutyInfo, MAX_CHECKPOINT_DUTY_PAGE_SIZE,
};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, H256};

use crate::{
	merkle, AgreementAuthorization, CheckpointDuty, CheckpointDutyBatch, CheckpointDutyPageRequest,
	CheckpointDutyRole, CheckpointDutyScanCursor, DeletionDuty, DeletionDutyBatch,
	DeletionDutyPageRequest, DeletionDutyScanCursor, MAX_STREAMING_OPERATIONS, PROTOCOL_VERSION,
};

const INDEX_FILE: &str = "provider-index-v6.json";
const INDEX_TEMP_PREFIX: &str = "provider-index-v6.tmp-";
const LEGACY_INDEX_FILE: &str = "provider-index-v5.json";
const BLOBS_DIR: &str = "blobs";
const MAX_BUCKET_BYTES: usize = 255;
const MAX_KEY_BYTES: usize = 1024;
const CHUNK_BYTES: usize = 256 * 1024;
const MAX_PROVIDER_INDEX_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PROVIDER_INDEX_RECORDS: usize = MAX_STREAMING_OPERATIONS;
const MAX_PROVIDER_INDEX_LEAVES: usize = MAX_PROVIDER_INDEX_RECORDS * 2;
const MAX_PROVIDER_ROOT_ARTIFACTS: usize = 64;
const MAX_PROVIDER_INDEX_TEMP_ARTIFACTS: usize = 1;
const MAX_PROVIDER_BLOB_TEMP_ARTIFACTS: usize = 1;

/// A validated content write ready for persistence.
#[derive(Clone, Debug)]
pub(crate) struct CommitInput {
	/// Canonical raw-content commitment (`blake2b-256(bytes)`).
	pub commitment: [u8; 32],
	/// Agreement authorization from finalized Orbis state.
	pub authorization: AgreementAuthorization,
	/// Optional S3 bucket identifier used only as an off-chain lookup key.
	pub bucket: Option<String>,
	/// Optional S3/Drive object key used only as an off-chain lookup key.
	pub key: Option<String>,
	/// Content bytes.
	pub bytes: Vec<u8>,
}

/// Public provider registration profile.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeProfile {
	/// Provider account encoded as lowercase hex.
	pub provider: String,
	/// Public HTTP endpoint advertised by the runtime provider record.
	pub endpoint: String,
	/// Service-key public key encoded as lowercase hex.
	pub service_key: String,
	/// Operator-readable region (not chain authority).
	pub region: Option<String>,
}

/// Immutable content record. Deletion is represented by a tombstone so proof history is stable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentRecord {
	/// Hex commitment and blob filename.
	pub commitment: String,
	/// Runtime agreement id.
	pub agreement_id: String,
	/// Runtime container reference.
	pub container_ref: String,
	/// Finalized block at which the agreement was authorized.
	pub authorized_at: String,
	/// Agreement expiry block observed at authorization.
	pub expires_at: u32,
	/// Optional bucket lookup key.
	pub bucket: Option<String>,
	/// Optional object lookup key.
	pub key: Option<String>,
	/// Content length.
	pub bytes: u64,
	/// Append-only proof leaf index.
	pub leaf_index: u64,
	/// Appended tombstone leaf index after deletion.
	pub tombstone_leaf_index: Option<u64>,
	/// Wall-clock creation time used for operations only, never consensus.
	pub created_unix_ms: u64,
	/// Tombstone marker.
	pub deleted: bool,
}

#[derive(Serialize)]
struct ContentLeaf<'a> {
	commitment: &'a str,
	agreement_id: &'a str,
	container_ref: &'a str,
	authorized_at: &'a str,
	expires_at: u32,
	bucket: &'a Option<String>,
	key: &'a Option<String>,
	bytes: u64,
	leaf_index: u64,
	created_unix_ms: u64,
	deleted: bool,
}

/// Crash-recoverable deletion transition retained until its acknowledgement is fsynced.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PendingDeletion {
	/// Normalized raw-content commitment.
	pub commitment: String,
	/// Canonical runtime agreement id.
	pub agreement_id: String,
	/// Finalized block hash which authorized deletion.
	pub authorized_at: String,
	/// Provider root immediately after appending the tombstone leaf.
	pub tombstone_root: String,
	/// Canonical tombstone leaf appended by the corresponding provider-root submission.
	pub tombstone_leaf: String,
	/// Monotonic provider root sequence (equal to the covered leaf count).
	pub root_sequence: u64,
	/// Index of the canonical deletion leaf.
	pub leaf_index: u64,
	/// Total leaves covered by `tombstone_root`.
	pub leaf_count: u64,
	/// Bounded leaf-to-root sibling hashes for duplicate-last Merkle verification.
	pub inclusion_proof: Vec<String>,
}

/// Crash-recoverable append-only root update retained until it is durably queued.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PendingRootSubmission {
	/// Exact next provider root sequence.
	pub sequence: u64,
	/// Exact leaf values appended by this update (one per local atomic transition).
	pub appended_leaves: Vec<String>,
	/// Locally derived root used to audit the finalized runtime result.
	pub expected_root: String,
	/// Locally derived leaf count used to audit the finalized runtime result.
	pub expected_leaf_count: u64,
}

/// Durable watermark for the last completely installed finalized checkpoint-duty snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointDutyWatermark {
	/// Finalized block hash used to read the snapshot.
	pub finalized_hash: String,
	/// Finalized block number corresponding to `finalized_hash`.
	pub finalized_number: u32,
	/// Governed finalized checkpoint which fixed the snapshot.
	pub snapshot_checkpoint: u32,
	/// Last fully visited runtime cursor, or `None` for an empty snapshot.
	pub cursor: Option<CheckpointDutyScanCursor>,
}

/// Exact duty inventory from the latest completely installed finalized scan.
///
/// Unlike the pending-duty journal, this inventory is replaced as one durable unit whenever a
/// terminal page is installed. Its three finalized coordinates bind every contained duty to one
/// fixed runtime view.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckpointDutyInventory {
	/// Finalized block hash used to read the complete inventory.
	pub(crate) finalized_hash: String,
	/// Finalized block number corresponding to `finalized_hash`.
	pub(crate) finalized_number: u32,
	/// Governed finalized checkpoint which fixed the inventory.
	pub(crate) snapshot_checkpoint: u32,
	/// Exact duties returned by the complete scan, in canonical bucket order.
	pub(crate) duties: Vec<CheckpointDuty>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointDutyDiscoveryCursor {
	finalized_hash: String,
	finalized_number: u32,
	snapshot_checkpoint: u32,
	after_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointDutyIntake {
	finalized_hash: String,
	finalized_number: u32,
	provider: String,
	snapshot_checkpoint: u32,
	next_cursor: CheckpointDutyScanCursor,
	duties: Vec<CheckpointDuty>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeletionDutyIntake {
	finalized_hash: String,
	finalized_number: u32,
	provider: String,
	snapshot_checkpoint: u32,
	requested_cursor: Option<DeletionDutyScanCursor>,
	next_cursor: Option<DeletionDutyScanCursor>,
	page_tail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeletionDutyWatermark {
	finalized_hash: String,
	finalized_number: u32,
	snapshot_checkpoint: u32,
	last_manifest: Option<String>,
}

/// Bounded provider statistics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderStats {
	/// Number of live objects.
	pub live_objects: u64,
	/// Number of tombstoned objects.
	pub deleted_objects: u64,
	/// Bytes currently stored.
	pub stored_bytes: u64,
	/// Configured capacity.
	pub capacity_bytes: u64,
	/// Remaining local capacity.
	pub available_bytes: u64,
	/// Append-only proof root.
	pub root: String,
	/// Exact append-log leaf count covered by `root`.
	pub proof_leaf_count: u64,
}

/// Verifiable fixed-size content chunk and its content-local Merkle proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChunkProof {
	/// Zero-based chunk index.
	pub index: usize,
	/// Total number of content chunks.
	pub chunks: usize,
	/// Hex chunk hash.
	pub chunk_hash: String,
	/// Hex Merkle root over all chunk hashes.
	pub root: String,
	/// Hex sibling hashes ordered from leaf to root.
	pub proof: Vec<String>,
	/// Raw chunk bytes, omitted from the derived JSON representation.
	#[serde(skip)]
	pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedState {
	version: u16,
	profile: NodeProfile,
	capacity_bytes: u64,
	records: BTreeMap<String, ContentRecord>,
	leaf_hashes: Vec<String>,
	root_frontier: Vec<Option<String>>,
	root_history: Vec<String>,
	root_index: BTreeMap<String, u64>,
	root_sequence: u64,
	pending_roots: BTreeMap<u64, PendingRootSubmission>,
	pending_deletions: BTreeMap<String, PendingDeletion>,
	checkpoint_duty_watermark: Option<CheckpointDutyWatermark>,
	checkpoint_duty_intake: Option<CheckpointDutyIntake>,
	pending_checkpoint_duties: BTreeMap<String, CheckpointDuty>,
	checkpoint_duty_inventory: Option<CheckpointDutyInventory>,
	checkpoint_duty_discovery_cursor: Option<CheckpointDutyDiscoveryCursor>,
	#[serde(default)]
	deletion_duty_intake: Option<DeletionDutyIntake>,
	#[serde(default)]
	deletion_duty_watermark: Option<DeletionDutyWatermark>,
	#[serde(default)]
	pending_manifest_deletions: BTreeMap<String, DeletionDuty>,
}

/// Storage failures. Callers map these to stable HTTP status codes.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
	/// Invalid input or attempted invariant violation.
	#[error("invalid content request: {0}")]
	Invalid(String),
	/// Content was not found or has been deleted.
	#[error("content not found")]
	NotFound,
	/// Capacity would be exceeded.
	#[error("provider capacity exceeded")]
	Capacity,
	/// Local filesystem or serialization failure.
	#[error("provider store I/O failed: {0}")]
	Io(String),
}

/// Crash-safe filesystem content store.
///
/// Blob files and the JSON index are written through a same-directory temporary followed by an
/// atomic rename. A process-local write lock serializes commit/delete/profile transitions.
pub struct DiskStore {
	root: PathBuf,
	state: RwLock<PersistedState>,
	_root_guard: Option<ProviderRootGuard>,
	blob_directory: Option<fs::File>,
}

pub(crate) struct PreparedDiskStore {
	root: PathBuf,
	state: PersistedState,
	initial_index: Option<Vec<u8>>,
	index_temps: Vec<crate::bounded_io::PreparedRegularFile>,
	blob_temps: Vec<crate::bounded_io::PreparedRegularFile>,
	canonical_blobs: Vec<PreparedBlobGuard>,
	blobs_missing: bool,
	blobs_identity: Option<crate::bounded_io::FileIdentity>,
	root_guard: PreparedProviderRootGuard,
	index_guard: PreparedProviderIndexGuard,
}

struct PreparedBlobGuard {
	file: crate::bounded_io::PreparedRegularFile,
	commitment: String,
	expected_bytes: u64,
}

pub(crate) struct ArmedDiskStore {
	prepared: Option<PreparedDiskStore>,
	root_guard: Option<ProviderRootGuard>,
}

struct ProviderRootGuard {
	directory: crate::bounded_io::LockedDirectory,
}

enum PreparedProviderRootGuard {
	Missing(crate::bounded_io::PreparedDirectoryPath),
	Existing(ProviderRootGuard),
	Consumed,
}

enum PreparedProviderIndexGuard {
	Missing,
	Present {
		identity: crate::bounded_io::FileIdentity,
		length: u64,
		bytes: Vec<u8>,
	},
}

impl ProviderRootGuard {
	fn validate(&self) -> Result<(), StoreError> {
		self.directory.validate_path_identity().map_err(io_error)
	}
}

impl PreparedProviderIndexGuard {
	fn validate(&self, directory: &fs::File) -> Result<(), StoreError> {
		match self {
			Self::Missing => {
				if crate::bounded_io::entry_missing_at(directory, INDEX_FILE.as_ref())
					.map_err(io_error)?
				{
					Ok(())
				} else {
					Err(StoreError::Io(
					"provider index appeared after startup validation".into(),
					))
				}
			},
			Self::Present { identity, length, bytes } => {
				let current = crate::bounded_io::read_regular_file_snapshot_at(
					directory,
					INDEX_FILE.as_ref(),
					MAX_PROVIDER_INDEX_BYTES,
				)
				.map_err(io_error)?;
				if current.identity != *identity
					|| current.length != *length
					|| current.bytes != *bytes
				{
					return Err(StoreError::Io(
						"provider index changed after startup validation".into(),
					));
				}
				Ok(())
			},
		}
	}
}

impl PreparedDiskStore {
	pub(crate) fn prepared_root_directory(&self) -> Option<&crate::bounded_io::LockedDirectory> {
		match &self.root_guard {
			PreparedProviderRootGuard::Existing(guard) => Some(&guard.directory),
			PreparedProviderRootGuard::Missing(_) | PreparedProviderRootGuard::Consumed => None,
		}
	}

	pub(crate) fn arm(mut self) -> Result<ArmedDiskStore, StoreError> {
		let prepared_root = std::mem::replace(
			&mut self.root_guard,
			PreparedProviderRootGuard::Consumed,
		);
		let root_guard = match prepared_root {
			PreparedProviderRootGuard::Existing(guard) => {
				guard.validate()?;
				guard
			},
			PreparedProviderRootGuard::Missing(path) => {
				let directory = path.create_and_lock().map_err(io_error)?;
				ProviderRootGuard { directory }
			},
			PreparedProviderRootGuard::Consumed =>
				return Err(StoreError::Io("provider root plan was already armed".into())),
		};
		let armed = ArmedDiskStore {
			prepared: Some(self),
			root_guard: Some(root_guard),
		};
		let prepared = armed.prepared.as_ref().expect("armed plan contains prepared state");
		let root_guard = armed.root_guard.as_ref().expect("armed plan contains root guard");
		root_guard.validate()?;
		prepared.index_guard.validate(root_guard.directory.file())?;
		Ok(armed)
	}
}

impl ArmedDiskStore {
	#[allow(dead_code)]
	pub(crate) fn root_directory(&self) -> Result<&crate::bounded_io::LockedDirectory, StoreError> {
		self.root_guard
			.as_ref()
			.map(|guard| &guard.directory)
			.ok_or_else(|| StoreError::Io("provider root guard is not armed".into()))
	}

	pub(crate) fn apply(mut self) -> Result<DiskStore, StoreError> {
		let prepared = self
			.prepared
			.take()
			.ok_or_else(|| StoreError::Io("provider disk plan was already applied".into()))?;
		let root_guard = self
			.root_guard
			.take()
			.ok_or_else(|| StoreError::Io("provider root guard is not armed".into()))?;
		root_guard.validate()?;
		prepared.index_guard.validate(root_guard.directory.file())?;
		crate::bounded_io::validate_prepared_regular_files_at(
			root_guard.directory.file(),
			&prepared.index_temps,
		)
		.map_err(io_error)?;
		let prepared_blob_directory = if prepared.blobs_missing {
			None
		} else {
			let directory = crate::bounded_io::open_directory_at(
				root_guard.directory.file(),
				BLOBS_DIR.as_ref(),
			)
			.map_err(io_error)?;
			let identity = crate::bounded_io::file_identity(
				&directory.metadata().map_err(io_error)?,
			);
			if prepared.blobs_identity != Some(identity) {
				return Err(StoreError::Io(
					"provider blob directory changed after startup validation".into(),
				))
			}
			crate::bounded_io::validate_prepared_regular_files_at(
				&directory,
				&prepared.blob_temps,
			)
			.map_err(io_error)?;
			validate_prepared_blobs(&directory, &prepared.canonical_blobs)?;
			Some(directory)
		};
		let blob_directory = match prepared_blob_directory {
			Some(directory) => directory,
			None => crate::bounded_io::create_directory_at(
				root_guard.directory.file(),
				BLOBS_DIR.as_ref(),
			)
			.map_err(io_error)?,
		};
		crate::bounded_io::remove_validated_temp_artifacts_at(
			root_guard.directory.file(),
			&prepared.index_temps,
		)
		.map_err(io_error)?;
		crate::bounded_io::remove_validated_temp_artifacts_at(
			&blob_directory,
			&prepared.blob_temps,
		)
			.map_err(io_error)?;
		let mut store = DiskStore {
			root: prepared.root,
			state: RwLock::new(prepared.state),
			_root_guard: Some(root_guard),
			blob_directory: Some(blob_directory),
		};
		if let Some(bytes) = prepared.initial_index {
			let temporary = atomic_temp_path(Path::new(INDEX_FILE));
			crate::bounded_io::write_atomic_at(
				store._root_guard.as_ref().expect("root guard transferred").directory.file(),
				INDEX_FILE.as_ref(),
				temporary.file_name().ok_or_else(|| {
					StoreError::Io("provider index temp name is invalid".into())
				})?,
				&bytes,
			)
			.map_err(io_error)?;
		}
		store
			._root_guard
			.as_mut()
			.expect("root guard transferred")
			.directory
			.preserve_owned();
		Ok(store)
	}
}

impl DiskStore {
	pub(crate) fn root_directory(&self) -> Result<&crate::bounded_io::LockedDirectory, StoreError> {
		self._root_guard
			.as_ref()
			.map(|guard| &guard.directory)
			.ok_or_else(|| StoreError::Io("provider root guard is unavailable".into()))
	}

	fn root_file(&self) -> Result<&fs::File, StoreError> {
		Ok(self.root_directory()?.file())
	}

	fn blob_file(&self) -> Result<&fs::File, StoreError> {
		self.blob_directory
			.as_ref()
			.ok_or_else(|| StoreError::Io("provider blob directory capability is unavailable".into()))
	}

	fn persist_state(&self, state: &PersistedState) -> Result<(), StoreError> {
		persist_state_at(self.root_file()?, state)
	}

	/// Reject non-directory and symlink provider roots before any child startup validation.
	pub(crate) fn validate_root(root: &Path) -> Result<(), StoreError> {
		optional_owned_directory_exists(root).map(|_| ())
	}

	/// Open or create a provider store. Existing protocol/capacity/provider identity must match.
	#[cfg(any(test, feature = "evidence", feature = "test-seams"))]
	pub fn open(
		root: impl AsRef<Path>,
		profile: NodeProfile,
		capacity_bytes: u64,
	) -> Result<Self, StoreError> {
		Self::prepare_open(root, profile, capacity_bytes)?.arm()?.apply()
	}

	pub(crate) fn prepare_open(
		root: impl AsRef<Path>,
		profile: NodeProfile,
		capacity_bytes: u64,
	) -> Result<PreparedDiskStore, StoreError> {
		if capacity_bytes == 0 {
			return Err(StoreError::Invalid("capacity must be non-zero".into()));
		}
		validate_profile(&profile)?;
		let root = root.as_ref().to_path_buf();
		let root_path = crate::bounded_io::prepare_directory_path(&root).map_err(io_error)?;
		let root_exists = !root_path.is_missing().map_err(io_error)?;
		let root_guard = if root_exists {
			PreparedProviderRootGuard::Existing(ProviderRootGuard {
				directory: root_path.lock_existing().map_err(io_error)?,
			})
		} else {
			PreparedProviderRootGuard::Missing(root_path)
		};
		let root_directory = match &root_guard {
			PreparedProviderRootGuard::Existing(guard) => Some(guard.directory.file()),
			_ => None,
		};
		let (index_temps, root_artifacts) =
			if let Some(directory) = root_directory {
				collect_index_temps_at(directory)?
			} else {
				(Vec::new(), 0)
			};
		let index_exists = root_directory
			.map(|directory| {
				crate::bounded_io::regular_file_exists_at(directory, INDEX_FILE.as_ref())
					.map_err(io_error)
			})
			.transpose()?
			.unwrap_or(false);
		let legacy_exists = root_directory
			.map(|directory| {
				crate::bounded_io::regular_file_exists_at(directory, LEGACY_INDEX_FILE.as_ref())
					.map_err(io_error)
			})
			.transpose()?
			.unwrap_or(false);
		if !index_exists && legacy_exists {
			return Err(StoreError::Invalid(
				"provider protocol v5 state is unsupported; initialize a clean data path".into(),
			));
		}
		let (state, index_guard) = if index_exists {
			let snapshot = crate::bounded_io::read_regular_file_snapshot_at(
				root_directory.expect("existing index has root directory"),
				INDEX_FILE.as_ref(),
				MAX_PROVIDER_INDEX_BYTES,
			)
			.map_err(io_error)?;
			let existing: PersistedState =
				serde_json::from_slice(&snapshot.bytes).map_err(io_error)?;
			validate_persisted_state_bounds(&existing)?;
			if existing.version != PROTOCOL_VERSION {
				return Err(StoreError::Invalid(format!(
					"unsupported persisted protocol version {}",
					existing.version
				)));
			}
			if existing.profile.provider != profile.provider
				|| existing.profile.service_key != profile.service_key
			{
				return Err(StoreError::Invalid(
					"configured provider identity does not match persisted store".into(),
				));
			}
			if existing.capacity_bytes != capacity_bytes {
				return Err(StoreError::Invalid(
					"capacity changes require authenticated PUT /node".into(),
				));
			}
			(
				existing,
				PreparedProviderIndexGuard::Present {
					identity: snapshot.identity,
					length: snapshot.length,
					bytes: snapshot.bytes,
				},
			)
		} else {
			(PersistedState {
				version: PROTOCOL_VERSION,
				profile,
				capacity_bytes,
				records: BTreeMap::new(),
				leaf_hashes: Vec::new(),
				root_frontier: Vec::new(),
				root_history: Vec::new(),
				root_index: BTreeMap::new(),
				root_sequence: 0,
				pending_roots: BTreeMap::new(),
				pending_deletions: BTreeMap::new(),
				checkpoint_duty_watermark: None,
				checkpoint_duty_intake: None,
				pending_checkpoint_duties: BTreeMap::new(),
				checkpoint_duty_inventory: None,
				checkpoint_duty_discovery_cursor: None,
				deletion_duty_intake: None,
				deletion_duty_watermark: None,
				pending_manifest_deletions: BTreeMap::new(),
			}, PreparedProviderIndexGuard::Missing)
		};
			let store = Self {
				root: root.clone(),
				state: RwLock::new(state),
				_root_guard: None,
				blob_directory: None,
			};
		store.verify_index()?;
		let state = store
			.state
			.into_inner()
			.map_err(|_| StoreError::Io("provider index validation lock was poisoned".into()))?;
		let (blobs_exists, blobs_identity, blob_temps, canonical_blobs) = validate_blob_namespace_at(
			root_directory,
			&state,
			index_exists,
		)?;
		let recovered_artifacts = root_artifacts
			.checked_sub(index_temps.len())
			.and_then(|count| count.checked_add(usize::from(!blobs_exists)))
			.and_then(|count| count.checked_add(usize::from(!index_exists)))
			.ok_or_else(|| StoreError::Io("provider root artifact count overflow".into()))?;
		if recovered_artifacts > MAX_PROVIDER_ROOT_ARTIFACTS {
			return Err(StoreError::Io("provider root contains too many durable artifacts".into()));
		}
		let initial_index = (!index_exists).then(|| encode_persisted_state(&state)).transpose()?;
		if let PreparedProviderRootGuard::Existing(guard) = &root_guard {
			guard.validate()?;
		}
		Ok(PreparedDiskStore {
			root,
			state,
			initial_index,
			index_temps,
			blob_temps,
			canonical_blobs,
			blobs_missing: !blobs_exists,
			blobs_identity,
			root_guard,
			index_guard,
		})
	}

	/// Return the provider data root for co-located private durable kernels.
	pub(crate) fn root(&self) -> &Path {
		&self.root
	}

	/// Compute the canonical off-chain content commitment.
	pub fn content_commitment(bytes: &[u8]) -> [u8; 32] {
		sp_crypto_hashing::blake2_256(bytes)
	}

	/// Return the public node profile.
	pub fn profile(&self) -> Result<NodeProfile, StoreError> {
		Ok(self.read_state()?.profile.clone())
	}

	/// Atomically update mutable public profile fields and capacity.
	pub fn update_profile(
		&self,
		profile: NodeProfile,
		capacity_bytes: u64,
	) -> Result<(), StoreError> {
		validate_profile(&profile)?;
		let mut state = self.write_state()?;
		if profile.provider != state.profile.provider
			|| profile.service_key != state.profile.service_key
		{
			return Err(StoreError::Invalid(
				"provider and service_key are immutable for an initialized store".into(),
			));
		}
		let stored = retained_blob_bytes(&state)?;
		if capacity_bytes < stored || capacity_bytes == 0 {
			return Err(StoreError::Invalid("capacity is below stored bytes".into()));
		}
		let mut next = state.clone();
		next.profile = profile;
		next.capacity_bytes = capacity_bytes;
		self.persist_state(&next)?;
		*state = next;
		Ok(())
	}

	/// Atomically commit a finalized-authorized object.
	pub(crate) fn commit(&self, input: CommitInput) -> Result<ContentRecord, StoreError> {
		validate_locator(input.bucket.as_deref(), input.key.as_deref())?;
		if Self::content_commitment(&input.bytes) != input.commitment {
			return Err(StoreError::Invalid("content commitment mismatch".into()));
		}
		if input.authorization.bytes != input.bytes.len() as u64 {
			return Err(StoreError::Invalid("authorization byte count mismatch".into()));
		}
		let commitment = hex::encode(input.commitment);
		let mut state = self.write_state()?;
		if let Some(record) = state.records.get(&commitment) {
			if record.deleted {
				return Err(StoreError::Invalid("deleted commitment cannot be recommitted".into()));
			}
			if record.agreement_id != input.authorization.agreement_id {
				return Err(StoreError::Invalid(
					"commitment already belongs to another agreement".into(),
				));
			}
			return Ok(record.clone());
		}
		if retained_blob_bytes(&state)?
			.checked_add(input.bytes.len() as u64)
			.filter(|total| *total <= state.capacity_bytes)
			.is_none()
		{
			return Err(StoreError::Capacity);
		}
		let mut next = state.clone();
		let record = ContentRecord {
			commitment: commitment.clone(),
			agreement_id: input.authorization.agreement_id,
			container_ref: input.authorization.container_ref,
			authorized_at: input.authorization.finalized_hash,
			expires_at: input.authorization.expires_at,
			bucket: input.bucket,
			key: input.key,
			bytes: input.bytes.len() as u64,
			leaf_index: next.leaf_hashes.len() as u64,
			tombstone_leaf_index: None,
			created_unix_ms: now_ms()?,
			deleted: false,
		};
		let leaf = record_leaf(&record, false)?;
		let blob_directory = self.blob_file()?;
		let temporary_path = atomic_temp_path(Path::new(&commitment));
		let temporary = temporary_path
			.file_name()
			.ok_or_else(|| StoreError::Io("durable blob temp name is invalid".into()))?;
		crate::bounded_io::remove_optional_regular_file_at(blob_directory, temporary)
			.map_err(io_error)?;
		crate::bounded_io::write_atomic_at(
			blob_directory,
			commitment.as_ref(),
			temporary,
			&input.bytes,
		)
		.map_err(io_error)?;
		let encoded_leaf = hex::encode(leaf);
		next.leaf_hashes.push(encoded_leaf.clone());
		journal_root_append(&mut next, encoded_leaf)?;
		next.records.insert(commitment, record.clone());
		if let Err(error) = self.persist_state(&next) {
			let _ = crate::bounded_io::remove_optional_regular_file_at(
				blob_directory,
				record.commitment.as_ref(),
			);
			return Err(error);
		}
		*state = next;
		Ok(record)
	}

	/// Read verified content bytes. Local corruption is rejected rather than returned.
	pub fn read(&self, commitment: &str) -> Result<Vec<u8>, StoreError> {
		let normalized = normalize_hash(commitment)?;
		let state = self.read_state()?;
		let record = state
			.records
			.get(&normalized)
			.filter(|record| !record.deleted)
			.ok_or(StoreError::NotFound)?;
		let bytes = crate::bounded_io::read_regular_file_at(
			self.blob_file()?,
			normalized.as_ref(),
			record.bytes,
		)
		.map_err(io_error)?;
		if bytes.len() as u64 != record.bytes
			|| hex::encode(Self::content_commitment(&bytes)) != normalized
		{
			return Err(StoreError::Io("stored blob failed commitment verification".into()));
		}
		Ok(bytes)
	}

	/// Return one record, including tombstones.
	pub fn record(&self, commitment: &str) -> Result<ContentRecord, StoreError> {
		let normalized = normalize_hash(commitment)?;
		self.read_state()?.records.get(&normalized).cloned().ok_or(StoreError::NotFound)
	}

	/// Check live existence for a bounded list of commitments.
	pub fn exists(&self, commitments: &[String]) -> Result<BTreeMap<String, bool>, StoreError> {
		if commitments.len() > 100 {
			return Err(StoreError::Invalid("at most 100 commitments may be checked".into()));
		}
		let state = self.read_state()?;
		commitments
			.iter()
			.map(|value| {
				let normalized = normalize_hash(value)?;
				let exists = state.records.get(&normalized).is_some_and(|record| !record.deleted);
				Ok((normalized, exists))
			})
			.collect()
	}

	/// Persist a tombstone and pending-deletion journal before any content bytes are removed.
	pub(crate) fn prepare_delete(
		&self,
		commitment: &str,
		authorization: &AgreementAuthorization,
	) -> Result<(ContentRecord, PendingDeletion), StoreError> {
		let normalized = normalize_hash(commitment)?;
		let mut state = self.write_state()?;
		let record = state.records.get(&normalized).ok_or(StoreError::NotFound)?;
		if record.agreement_id != authorization.agreement_id {
			return Err(StoreError::Invalid("agreement does not own commitment".into()));
		}
		if record.deleted {
			let pending = state.pending_deletions.get(&normalized).cloned().ok_or_else(|| {
				StoreError::Invalid("deletion acknowledgement was already durably queued".into())
			})?;
			return Ok((record.clone(), pending));
		}
		let mut next = state.clone();
		let next_record = next.records.get_mut(&normalized).expect("record was checked above");
		next_record.deleted = true;
		next_record.tombstone_leaf_index = Some(next.leaf_hashes.len() as u64);
		let result = next_record.clone();
		let tombstone = deletion_leaf(&result, &next.profile.provider)?;
		let encoded_tombstone = hex::encode(tombstone);
		next.leaf_hashes.push(encoded_tombstone.clone());
		let root_update = journal_root_append(&mut next, encoded_tombstone.clone())?;
		let leaves = decode_leaves(&next)?;
		let leaf_index = result
			.tombstone_leaf_index
			.expect("tombstone index was assigned before hashing");
		let leaf_count = leaves.len() as u64;
		let tombstone_root = hex::encode(merkle::root(&leaves));
		let inclusion_proof = merkle::proof(&leaves, leaf_index as usize)
			.ok_or_else(|| StoreError::Io("tombstone proof index is invalid".into()))?
			.into_iter()
			.map(hex::encode)
			.collect();
		debug_assert!(merkle::verify(
			tombstone,
			leaf_index as usize,
			leaf_count as usize,
			&merkle::proof(&leaves, leaf_index as usize).expect("index was checked"),
			merkle::root(&leaves),
		));
		let pending = PendingDeletion {
			commitment: normalized.clone(),
			agreement_id: authorization.agreement_id.clone(),
			authorized_at: authorization.finalized_hash.clone(),
			tombstone_root,
			tombstone_leaf: encoded_tombstone,
			root_sequence: root_update.sequence,
			leaf_index,
			leaf_count,
			inclusion_proof,
		};
		next.pending_deletions.insert(normalized, pending.clone());
		self.persist_state(&next)?;
		*state = next;
		Ok((result, pending))
	}

	/// Remove bytes only after the deletion submission has been durably appended to the outbox.
	pub(crate) fn complete_delete(&self, commitment: &str) -> Result<(), StoreError> {
		let normalized = normalize_hash(commitment)?;
		let mut state = self.write_state()?;
		if !state.pending_deletions.contains_key(&normalized) {
			return Ok(());
		}
		crate::bounded_io::remove_optional_regular_file_at(
			self.blob_file()?,
			normalized.as_ref(),
		)
		.map_err(io_error)?;
		let mut next = state.clone();
		next.pending_deletions.remove(&normalized);
		self.persist_state(&next)?;
		*state = next;
		Ok(())
	}

	/// Return deletion journal entries which must be re-enqueued after a crash.
	pub(crate) fn pending_deletions(&self) -> Result<Vec<PendingDeletion>, StoreError> {
		Ok(self.read_state()?.pending_deletions.values().cloned().collect())
	}

	/// Return root append journal entries in exact sequence order.
	pub(crate) fn pending_root_submissions(
		&self,
	) -> Result<Vec<PendingRootSubmission>, StoreError> {
		Ok(self.read_state()?.pending_roots.values().cloned().collect())
	}

	/// Atomically stage one page or install a terminal finalized checkpoint-duty snapshot.
	///
	/// Non-terminal pages retain the fixed finalized hash, exact next cursor, and accumulated
	/// duties for restart. A terminal page atomically replaces the current exact inventory, merges
	/// the complete snapshot into pending work, advances the watermark and clears staging. Existing
	/// pending duties are never removed by intake.
	pub fn stage_checkpoint_duty_page(
		&self,
		batch: CheckpointDutyBatch,
	) -> Result<bool, StoreError> {
		let mut state = self.write_state()?;
		validate_checkpoint_duty_page(&batch, &state.profile)?;
		let accumulated_duties = state
			.checkpoint_duty_intake
			.as_ref()
			.map_or(0, |intake| intake.duties.len())
			.checked_add(batch.duties.len())
			.ok_or(StoreError::Capacity)?;
		if accumulated_duties > MAX_PROVIDER_INDEX_RECORDS {
			return Err(StoreError::Capacity);
		}
		if checkpoint_duty_coordinates_regress(
			batch.snapshot_checkpoint,
			batch.finalized_number,
			state.checkpoint_duty_watermark.as_ref(),
			state.checkpoint_duty_inventory.as_ref(),
		) {
			return Err(StoreError::Invalid(
				"checkpoint duty snapshot or finalized height regressed".into(),
			));
		}
		let mut next = state.clone();
		let mut accumulated = match next.checkpoint_duty_intake.take() {
			Some(intake) => {
				if batch.finalized_hash != intake.finalized_hash
					|| batch.finalized_number != intake.finalized_number
					|| batch.provider != intake.provider
					|| batch.snapshot_checkpoint != intake.snapshot_checkpoint
					|| batch.requested_cursor.as_ref() != Some(&intake.next_cursor)
				{
					return Err(StoreError::Invalid(
						"checkpoint duty page does not resume the durable fixed snapshot".into(),
					));
				}
				intake.duties
			},
			None => {
				if batch.requested_cursor.is_some() {
					return Err(StoreError::Invalid(
						"checkpoint duty resume cursor has no durable staging record".into(),
					));
				}
				Vec::new()
			},
		};
		if let (Some(previous), Some(first)) = (accumulated.last(), batch.duties.first()) {
			if normalize_hash(&previous.bucket_id)? >= normalize_hash(&first.bucket_id)? {
				return Err(StoreError::Invalid(
					"checkpoint duty page did not advance canonical bucket order".into(),
				));
			}
		}
		accumulated.extend(batch.duties);
		validate_checkpoint_duty_collection(
			&batch.finalized_hash,
			&batch.provider,
			batch.snapshot_checkpoint,
			&accumulated,
			&state.profile,
		)?;
		if let Some(next_cursor) = batch.next_cursor {
			next.checkpoint_duty_intake = Some(CheckpointDutyIntake {
				finalized_hash: batch.finalized_hash,
				finalized_number: batch.finalized_number,
				provider: batch.provider,
				snapshot_checkpoint: batch.snapshot_checkpoint,
				next_cursor,
				duties: accumulated,
			});
			self.persist_state(&next)?;
			*state = next;
			return Ok(false);
		}
		for duty in &accumulated {
			let key = normalize_hash(&duty.duty_id)?;
			if let Some(existing) = next.pending_checkpoint_duties.get(&key) {
				if existing != duty {
					return Err(StoreError::Invalid(
						"checkpoint duty id replay changed its payload".into(),
					));
				}
			} else {
				next.pending_checkpoint_duties.insert(key, duty.clone());
			}
		}
		let cursor = accumulated.last().map(|duty| CheckpointDutyScanCursor {
			snapshot_checkpoint: batch.snapshot_checkpoint,
			last_key: duty.bucket_id.clone(),
		});
		let exact_view = match &state.checkpoint_duty_inventory {
			Some(current) => {
				let exact = normalize_hash(&current.finalized_hash)?
					== normalize_hash(&batch.finalized_hash)?
					&& current.finalized_number == batch.finalized_number
					&& current.snapshot_checkpoint == batch.snapshot_checkpoint;
				if exact
					&& (current.duties != accumulated
						|| state.checkpoint_duty_watermark.as_ref().map(|item| &item.cursor)
							!= Some(&cursor))
				{
					return Err(StoreError::Invalid(
						"checkpoint duty fixed-view replay changed its inventory".into(),
					));
				}
				exact
			},
			None => false,
		};
		if let Some(previous) = &state.checkpoint_duty_watermark {
			if exact_view && terminal_cursor_key(&cursor)? < terminal_cursor_key(&previous.cursor)?
			{
				return Err(StoreError::Invalid("checkpoint duty cursor regressed".into()));
			}
		}
		next.checkpoint_duty_watermark = Some(CheckpointDutyWatermark {
			finalized_hash: batch.finalized_hash.clone(),
			finalized_number: batch.finalized_number,
			snapshot_checkpoint: batch.snapshot_checkpoint,
			cursor,
		});
		if !exact_view {
			next.checkpoint_duty_inventory = Some(CheckpointDutyInventory {
				finalized_hash: batch.finalized_hash,
				finalized_number: batch.finalized_number,
				snapshot_checkpoint: batch.snapshot_checkpoint,
				duties: accumulated,
			});
			next.checkpoint_duty_discovery_cursor = None;
		}
		next.checkpoint_duty_intake = None;
		self.persist_state(&next)?;
		*state = next;
		Ok(true)
	}

	/// Return the exact fixed-hash request needed to resume a staged duty scan after restart.
	pub fn checkpoint_duty_resume_request(
		&self,
	) -> Result<Option<CheckpointDutyPageRequest>, StoreError> {
		Ok(self.read_state()?.checkpoint_duty_intake.as_ref().map(|intake| {
			CheckpointDutyPageRequest {
				finalized_hash: intake.finalized_hash.clone(),
				finalized_number: intake.finalized_number,
				provider: intake.provider.clone(),
				snapshot_checkpoint: intake.snapshot_checkpoint,
				cursor: intake.next_cursor.clone(),
			}
		}))
	}

	/// Return the last atomically installed checkpoint-duty snapshot watermark.
	pub fn checkpoint_duty_watermark(&self) -> Result<Option<CheckpointDutyWatermark>, StoreError> {
		Ok(self.read_state()?.checkpoint_duty_watermark.clone())
	}

	/// Return the exact latest fully installed duty inventory.
	pub(crate) fn checkpoint_duty_inventory(
		&self,
	) -> Result<Option<CheckpointDutyInventory>, StoreError> {
		Ok(self.read_state()?.checkpoint_duty_inventory.clone())
	}

	/// Durably reserve a bounded round-robin slice of replica duties from the exact current
	/// inventory. Reopening the store continues strictly after the last reserved duty.
	pub(crate) fn reserve_checkpoint_replica_duties(
		&self,
		inventory: &CheckpointDutyInventory,
		limit: usize,
	) -> Result<Vec<CheckpointDuty>, StoreError> {
		if limit == 0 {
			return Ok(Vec::new());
		}
		let mut state = self.write_state()?;
		if state.checkpoint_duty_inventory.as_ref() != Some(inventory) {
			return Err(StoreError::Invalid("checkpoint duty inventory changed".into()));
		}
		let mut eligible = inventory
			.duties
			.iter()
			.filter(|duty| duty.role == CheckpointDutyRole::Replica)
			.map(|duty| Ok((checkpoint_duty_order_key(duty)?, duty.clone())))
			.collect::<Result<Vec<_>, StoreError>>()?;
		eligible.sort_by(|left, right| left.0.cmp(&right.0));
		if eligible.is_empty() {
			return Ok(Vec::new());
		}
		let after = state
			.checkpoint_duty_discovery_cursor
			.as_ref()
			.filter(|cursor| {
				cursor.finalized_hash == inventory.finalized_hash
					&& cursor.finalized_number == inventory.finalized_number
					&& cursor.snapshot_checkpoint == inventory.snapshot_checkpoint
			})
			.map(|cursor| cursor.after_key.as_str());
		let start = after
			.and_then(|after| eligible.iter().position(|(key, _)| key.as_str() > after))
			.unwrap_or(0);
		let selected = eligible
			.iter()
			.cycle()
			.skip(start)
			.take(eligible.len().min(limit))
			.cloned()
			.collect::<Vec<_>>();
		let next_cursor = CheckpointDutyDiscoveryCursor {
			finalized_hash: inventory.finalized_hash.clone(),
			finalized_number: inventory.finalized_number,
			snapshot_checkpoint: inventory.snapshot_checkpoint,
			after_key: selected.last().expect("selection is non-empty").0.clone(),
		};
		let mut next = state.clone();
		next.checkpoint_duty_discovery_cursor = Some(next_cursor);
		self.persist_state(&next)?;
		*state = next;
		Ok(selected.into_iter().map(|(_, duty)| duty).collect())
	}

	/// Return all durable checkpoint duties awaiting later signing/quorum processing.
	pub fn pending_checkpoint_duties(&self) -> Result<Vec<CheckpointDuty>, StoreError> {
		Ok(self.read_state()?.pending_checkpoint_duties.values().cloned().collect())
	}

	/// Atomically stage one bounded page without accumulating the complete runtime snapshot.
	pub fn stage_deletion_duty_page(&self, batch: DeletionDutyBatch) -> Result<bool, StoreError> {
		let mut state = self.write_state()?;
		validate_deletion_duty_page(&batch, &state.profile)?;
		if !state.pending_manifest_deletions.is_empty() {
			return Err(StoreError::Invalid(
				"manifest deletion page cannot advance before its staged duties are handed off"
					.into(),
			));
		}
		if let Some(previous) = &state.deletion_duty_watermark {
			if batch.snapshot_checkpoint < previous.snapshot_checkpoint
				|| batch.finalized_number < previous.finalized_number
			{
				return Err(StoreError::Invalid(
					"manifest deletion duty snapshot or finalized height regressed".into(),
				));
			}
		}
		match &state.deletion_duty_intake {
			Some(intake) => {
				if batch.finalized_hash != intake.finalized_hash
					|| batch.finalized_number != intake.finalized_number
					|| batch.provider != intake.provider
					|| batch.snapshot_checkpoint != intake.snapshot_checkpoint
					|| batch.requested_cursor != intake.next_cursor
				{
					return Err(StoreError::Invalid(
						"manifest deletion page does not resume the durable fixed snapshot".into(),
					));
				}
			},
			None => {
				if batch.requested_cursor.is_some() {
					return Err(StoreError::Invalid(
						"manifest deletion resume cursor has no durable staging record".into(),
					));
				}
			},
		}
		let page_tail = batch.duties.last().map(|duty| duty.manifest.clone());
		let last_manifest = page_tail
			.clone()
			.or_else(|| batch.requested_cursor.as_ref().map(|cursor| cursor.last_manifest.clone()));
		let mut next = state.clone();
		for duty in &batch.duties {
			next.pending_manifest_deletions
				.insert(normalize_hash(&duty.manifest)?, duty.clone());
		}
		let terminal = batch.next_cursor.is_none();
		if terminal && batch.duties.is_empty() {
			next.deletion_duty_watermark = Some(DeletionDutyWatermark {
				finalized_hash: batch.finalized_hash,
				finalized_number: batch.finalized_number,
				snapshot_checkpoint: batch.snapshot_checkpoint,
				last_manifest,
			});
			next.deletion_duty_intake = None;
			self.persist_state(&next)?;
			*state = next;
			return Ok(true);
		}
		next.deletion_duty_intake = Some(DeletionDutyIntake {
			finalized_hash: batch.finalized_hash.clone(),
			finalized_number: batch.finalized_number,
			provider: batch.provider,
			snapshot_checkpoint: batch.snapshot_checkpoint,
			requested_cursor: batch.requested_cursor,
			next_cursor: batch.next_cursor,
			page_tail,
		});
		/* The watermark advances only after every duty in the terminal page is durable in the
		 * acknowledgement outbox. */
		self.persist_state(&next)?;
		*state = next;
		Ok(terminal)
	}

	fn finalize_manifest_deletion_page(next: &mut PersistedState) {
		let Some(intake) = next.deletion_duty_intake.clone() else { return };
		if intake.next_cursor.is_some() {
			return;
		}
		next.deletion_duty_watermark = Some(DeletionDutyWatermark {
			finalized_hash: intake.finalized_hash.clone(),
			finalized_number: intake.finalized_number,
			snapshot_checkpoint: intake.snapshot_checkpoint,
			last_manifest: intake.page_tail.clone().or_else(|| {
				intake.requested_cursor.as_ref().map(|cursor| cursor.last_manifest.clone())
			}),
		});
		next.deletion_duty_intake = None;
	}

	/// Return the exact fixed-hash request needed to resume deletion intake after restart.
	pub fn deletion_duty_resume_request(
		&self,
	) -> Result<Option<DeletionDutyPageRequest>, StoreError> {
		let state = self.read_state()?;
		if !state.pending_manifest_deletions.is_empty() {
			return Ok(None);
		}
		Ok(state.deletion_duty_intake.as_ref().and_then(|intake| {
			intake.next_cursor.clone().map(|cursor| DeletionDutyPageRequest {
				finalized_hash: intake.finalized_hash.clone(),
				finalized_number: intake.finalized_number,
				provider: intake.provider.clone(),
				snapshot_checkpoint: intake.snapshot_checkpoint,
				cursor,
			})
		}))
	}

	/// Return the bounded staged-page work slice in deterministic manifest-key order.
	pub fn pending_manifest_deletions(
		&self,
		limit: usize,
	) -> Result<Vec<DeletionDuty>, StoreError> {
		Ok(self
			.read_state()?
			.pending_manifest_deletions
			.values()
			.take(limit)
			.cloned()
			.collect())
	}

	/// Complete exact work only after its acknowledgement is durable in the signer outbox.
	pub fn complete_manifest_deletion(
		&self,
		manifest: &str,
		duty_fingerprint: &str,
	) -> Result<(), StoreError> {
		let key = normalize_hash(manifest)?;
		let mut state = self.write_state()?;
		let Some(existing) = state.pending_manifest_deletions.get(&key) else { return Ok(()) };
		if normalize_hash(&existing.duty_fingerprint)? != normalize_hash(duty_fingerprint)? {
			return Err(StoreError::Invalid(
				"manifest deletion completion changed its duty fingerprint".into(),
			));
		}
		let mut next = state.clone();
		next.pending_manifest_deletions.remove(&key);
		if next.pending_manifest_deletions.is_empty() {
			Self::finalize_manifest_deletion_page(&mut next);
		}
		self.persist_state(&next)?;
		*state = next;
		Ok(())
	}

	/// Remove a root journal entry only after it has been durably appended to the outbox.
	pub(crate) fn complete_root_submission(&self, sequence: u64) -> Result<(), StoreError> {
		let mut state = self.write_state()?;
		if !state.pending_roots.contains_key(&sequence) {
			return Ok(());
		}
		let mut next = state.clone();
		next.pending_roots.remove(&sequence);
		self.persist_state(&next)?;
		*state = next;
		Ok(())
	}

	/// List live records in one bucket with bounded pagination.
	pub fn bucket_records(
		&self,
		bucket: Option<&str>,
		cursor: usize,
		limit: usize,
	) -> Result<(Vec<ContentRecord>, Option<usize>), StoreError> {
		if limit == 0 || limit > 100 {
			return Err(StoreError::Invalid("limit must be in 1..=100".into()));
		}
		let state = self.read_state()?;
		let values: Vec<_> = state
			.records
			.values()
			.filter(|record| {
				!record.deleted
					&& bucket.map_or(true, |name| record.bucket.as_deref() == Some(name))
			})
			.skip(cursor)
			.take(limit + 1)
			.cloned()
			.collect();
		let has_more = values.len() > limit;
		let items = values.into_iter().take(limit).collect();
		Ok((items, has_more.then_some(cursor + limit)))
	}

	/// Return storage statistics and the current proof root.
	pub fn stats(&self) -> Result<ProviderStats, StoreError> {
		let state = self.read_state()?;
		let stored_bytes = stored_bytes(&state);
		Ok(ProviderStats {
			live_objects: state.records.values().filter(|record| !record.deleted).count() as u64,
			deleted_objects: state.records.values().filter(|record| record.deleted).count() as u64,
			stored_bytes,
			capacity_bytes: state.capacity_bytes,
			available_bytes: state.capacity_bytes.saturating_sub(stored_bytes),
			root: current_root(&state)?,
			proof_leaf_count: state.leaf_hashes.len() as u64,
		})
	}

	/// Return a Merkle inclusion proof for a content record.
	pub fn proof(&self, commitment: &str) -> Result<Vec<String>, StoreError> {
		let record = self.record(commitment)?;
		let state = self.read_state()?;
		let leaves = decode_leaves(&state)?;
		let index = record.tombstone_leaf_index.unwrap_or(record.leaf_index);
		merkle::proof(&leaves, index as usize)
			.ok_or_else(|| StoreError::Io("record proof index is invalid".into()))
			.map(|proof| proof.into_iter().map(hex::encode).collect())
	}

	/// Build a fixed-size content chunk proof from commitment-verified bytes.
	pub fn chunk_proof(&self, commitment: &str, index: usize) -> Result<ChunkProof, StoreError> {
		let bytes = self.read(commitment)?;
		let chunks: Vec<&[u8]> =
			if bytes.is_empty() { vec![&[]] } else { bytes.chunks(CHUNK_BYTES).collect() };
		let selected = chunks
			.get(index)
			.ok_or_else(|| StoreError::Invalid("chunk index is outside content".into()))?;
		let leaves: Vec<[u8; 32]> = chunks
			.iter()
			.enumerate()
			.map(|(position, chunk)| {
				[b"orbis/content-chunk/v1".as_slice(), &(position as u64).to_le_bytes(), chunk]
					.concat()
			})
			.map(|input| sp_crypto_hashing::blake2_256(&input))
			.collect();
		Ok(ChunkProof {
			index,
			chunks: chunks.len(),
			chunk_hash: hex::encode(leaves[index]),
			root: hex::encode(merkle::root(&leaves)),
			proof: merkle::proof(&leaves, index)
				.expect("selected index was checked")
				.into_iter()
				.map(hex::encode)
				.collect(),
			bytes: selected.to_vec(),
		})
	}

	/// Return Merkle-mountain peaks for replica synchronization.
	pub fn peaks(&self) -> Result<Vec<String>, StoreError> {
		let state = self.read_state()?;
		Ok(merkle::peaks(&decode_leaves(&state)?).into_iter().map(hex::encode).collect())
	}

	/// Return a bounded slice of leaf hashes for authenticated replica synchronization.
	pub fn leaf_nodes(&self, start: usize, limit: usize) -> Result<Vec<String>, StoreError> {
		if limit == 0 || limit > 1024 {
			return Err(StoreError::Invalid("limit must be in 1..=1024".into()));
		}
		Ok(self.read_state()?.leaf_hashes.iter().skip(start).take(limit).cloned().collect())
	}

	fn verify_index(&self) -> Result<(), StoreError> {
		let state = self.read_state()?;
		verify_checkpoint_duty_state(&state)?;
		verify_deletion_duty_state(&state)?;
		if state.root_sequence != state.leaf_hashes.len() as u64 {
			return Err(StoreError::Io("root sequence and proof-leaf counts differ".into()));
		}
		let leaves = decode_leaves(&state)?;
		let (frontier, history) = merkle::accumulate(&leaves)
			.ok_or_else(|| StoreError::Io("proof frontier reconstruction failed".into()))?;
		if encode_frontier(&frontier) != state.root_frontier
			|| history.iter().map(hex::encode).collect::<Vec<_>>() != state.root_history
		{
			return Err(StoreError::Io("persisted proof frontier/history is invalid".into()));
		}
		let expected_index: BTreeMap<_, _> = history
			.iter()
			.enumerate()
			.map(|(index, root)| (hex::encode(root), index as u64 + 1))
			.collect();
		if state.root_index != expected_index {
			return Err(StoreError::Io("persisted proof root index is invalid".into()));
		}
		let tombstones = state.records.values().filter(|record| record.deleted).count();
		if state.records.len().saturating_add(tombstones) != state.leaf_hashes.len() {
			return Err(StoreError::Io("record and proof-leaf counts differ".into()));
		}
		for record in state.records.values() {
			let expected = record_leaf(record, false)?;
			let actual = state.leaf_hashes.get(record.leaf_index as usize).ok_or_else(|| {
				StoreError::Io("record leaf index is outside the proof log".into())
			})?;
			if hex::encode(expected) != *actual {
				return Err(StoreError::Io("record proof leaf mismatch".into()));
			}
			if record.deleted {
				let index = record
					.tombstone_leaf_index
					.ok_or_else(|| StoreError::Io("deleted record has no tombstone leaf".into()))?;
				let actual = state.leaf_hashes.get(index as usize).ok_or_else(|| {
					StoreError::Io("tombstone leaf index is outside the proof log".into())
				})?;
				if hex::encode(deletion_leaf(record, &state.profile.provider)?) != *actual {
					return Err(StoreError::Io("record tombstone leaf mismatch".into()));
				}
			}
		}
		for (commitment, pending) in &state.pending_deletions {
			let record = state
				.records
				.get(commitment)
				.ok_or_else(|| StoreError::Io("pending deletion has no content record".into()))?;
			if !record.deleted || record.agreement_id != pending.agreement_id {
				return Err(StoreError::Io("pending deletion does not match tombstone".into()));
			}
			if let Some(root) = state.pending_roots.get(&pending.root_sequence) {
				if root.appended_leaves != vec![pending.tombstone_leaf.clone()] {
					return Err(StoreError::Io(
						"pending deletion root append does not match tombstone".into(),
					));
				}
			}
		}
		for (sequence, pending) in &state.pending_roots {
			if *sequence != pending.sequence
				|| pending.appended_leaves.len() != 1
				|| pending.expected_leaf_count == 0
				|| pending.expected_leaf_count > state.leaf_hashes.len() as u64
			{
				return Err(StoreError::Io("pending provider root journal is invalid".into()));
			}
			let index = pending.expected_leaf_count as usize - 1;
			if state.leaf_hashes[index] != pending.appended_leaves[0]
				|| state.root_history.get(index) != Some(&pending.expected_root)
			{
				return Err(StoreError::Io(
					"pending provider root journal does not match proof log".into(),
				));
			}
		}
		Ok(())
	}

	fn read_state(&self) -> Result<std::sync::RwLockReadGuard<'_, PersistedState>, StoreError> {
		self.state.read().map_err(|_| StoreError::Io("store read lock poisoned".into()))
	}

	fn write_state(&self) -> Result<std::sync::RwLockWriteGuard<'_, PersistedState>, StoreError> {
		self.state
			.write()
			.map_err(|_| StoreError::Io("store write lock poisoned".into()))
	}
}

fn validate_profile(profile: &NodeProfile) -> Result<(), StoreError> {
	if profile.provider.is_empty() || profile.endpoint.is_empty() || profile.service_key.is_empty()
	{
		return Err(StoreError::Invalid("provider, endpoint and service_key are required".into()));
	}
	if profile.endpoint.len() > 256
		|| profile.service_key.len() > 256
		|| profile.provider.len() > 128
	{
		return Err(StoreError::Invalid("node profile field exceeds bound".into()));
	}
	Ok(())
}

fn record_leaf(record: &ContentRecord, deleted: bool) -> Result<[u8; 32], StoreError> {
	merkle::hash_leaf(&ContentLeaf {
		commitment: &record.commitment,
		agreement_id: &record.agreement_id,
		container_ref: &record.container_ref,
		authorized_at: &record.authorized_at,
		expires_at: record.expires_at,
		bucket: &record.bucket,
		key: &record.key,
		bytes: record.bytes,
		leaf_index: record.leaf_index,
		created_unix_ms: record.created_unix_ms,
		deleted,
	})
	.map_err(io_error)
}

/// Canonical runtime-verifiable deletion leaf. No local locator or wall-clock field participates.
fn deletion_leaf(record: &ContentRecord, provider: &str) -> Result<[u8; 32], StoreError> {
	let agreement = decode_h256(&record.agreement_id)?;
	let content = decode_h256(&record.commitment)?;
	let raw = hex::decode(provider.trim_start_matches("0x"))
		.map_err(|_| StoreError::Invalid("provider is not hex".into()))?;
	let provider = AccountId32::new(
		raw.try_into()
			.map_err(|_| StoreError::Invalid("provider must be exactly 32 bytes".into()))?,
	);
	Ok(sp_crypto_hashing::blake2_256(
		&(b"orbis/provider-deletion-leaf/v1", agreement, content, &provider, 0u64).encode(),
	))
}

fn decode_h256(value: &str) -> Result<H256, StoreError> {
	let raw = hex::decode(value.trim_start_matches("0x"))
		.map_err(|_| StoreError::Invalid("hash is not hex".into()))?;
	let raw: [u8; 32] = raw
		.try_into()
		.map_err(|_| StoreError::Invalid("hash must be exactly 32 bytes".into()))?;
	Ok(H256::from(raw))
}

fn validate_locator(bucket: Option<&str>, key: Option<&str>) -> Result<(), StoreError> {
	if bucket.is_some_and(|value| value.is_empty() || value.len() > MAX_BUCKET_BYTES) {
		return Err(StoreError::Invalid("bucket length is outside 1..=255".into()));
	}
	if key.is_some_and(|value| value.is_empty() || value.len() > MAX_KEY_BYTES) {
		return Err(StoreError::Invalid("key length is outside 1..=1024".into()));
	}
	Ok(())
}

fn normalize_hash(value: &str) -> Result<String, StoreError> {
	let value = value.strip_prefix("0x").unwrap_or(value).to_ascii_lowercase();
	let raw =
		hex::decode(&value).map_err(|_| StoreError::Invalid("commitment is not hex".into()))?;
	if raw.len() != 32 {
		return Err(StoreError::Invalid("commitment must be exactly 32 bytes".into()));
	}
	Ok(value)
}

fn decode_leaves(state: &PersistedState) -> Result<Vec<[u8; 32]>, StoreError> {
	decode_leaves_prefix(state, state.leaf_hashes.len())
}

fn decode_leaves_prefix(state: &PersistedState, len: usize) -> Result<Vec<[u8; 32]>, StoreError> {
	state
		.leaf_hashes
		.iter()
		.take(len)
		.map(|value| {
			let raw = hex::decode(value).map_err(io_error)?;
			raw.try_into().map_err(|_| StoreError::Io("proof leaf is not 32 bytes".into()))
		})
		.collect()
}

fn journal_root_append(
	state: &mut PersistedState,
	leaf: String,
) -> Result<PendingRootSubmission, StoreError> {
	let leaf_count = state.leaf_hashes.len() as u64;
	let node = decode_leaf(&leaf)?;
	let mut frontier = decode_frontier(&state.root_frontier)?;
	if !merkle::append_frontier(&mut frontier, leaf_count.saturating_sub(1), node) {
		return Err(StoreError::Io("persisted proof frontier cannot append leaf".into()));
	}
	let root = merkle::frontier_root(&frontier, leaf_count)
		.ok_or_else(|| StoreError::Io("proof frontier cannot derive root".into()))?;
	state.root_frontier = encode_frontier(&frontier);
	let encoded_root = hex::encode(root);
	state.root_history.push(encoded_root.clone());
	state.root_index.insert(encoded_root, leaf_count);
	state.root_sequence = state
		.root_sequence
		.checked_add(1)
		.ok_or_else(|| StoreError::Invalid("provider root sequence overflow".into()))?;
	let pending = PendingRootSubmission {
		sequence: state.root_sequence,
		appended_leaves: vec![leaf],
		expected_root: hex::encode(root),
		expected_leaf_count: leaf_count,
	};
	state.pending_roots.insert(pending.sequence, pending.clone());
	Ok(pending)
}

fn decode_leaf(value: &str) -> Result<[u8; 32], StoreError> {
	let raw = hex::decode(value).map_err(io_error)?;
	raw.try_into().map_err(|_| StoreError::Io("proof leaf is not 32 bytes".into()))
}

fn decode_frontier(values: &[Option<String>]) -> Result<Vec<Option<[u8; 32]>>, StoreError> {
	values
		.iter()
		.map(|value| value.as_deref().map(decode_leaf).transpose())
		.collect()
}

fn encode_frontier(values: &[Option<[u8; 32]>]) -> Vec<Option<String>> {
	values.iter().map(|value| value.map(hex::encode)).collect()
}

fn current_root(state: &PersistedState) -> Result<String, StoreError> {
	state
		.root_history
		.last()
		.cloned()
		.or_else(|| Some(hex::encode(merkle::root(&[]))))
		.ok_or_else(|| StoreError::Io("proof root is unavailable".into()))
}

fn stored_bytes(state: &PersistedState) -> u64 {
	state
		.records
		.values()
		.filter(|record| !record.deleted)
		.fold(0u64, |total, record| total.saturating_add(record.bytes))
}

fn retained_blob_bytes(state: &PersistedState) -> Result<u64, StoreError> {
	state.records.iter().try_fold(0u64, |total, (commitment, record)| {
		if !record.deleted || state.pending_deletions.contains_key(commitment) {
			total
				.checked_add(record.bytes)
				.ok_or_else(|| StoreError::Io("provider retained-byte total overflow".into()))
		} else {
			Ok(total)
		}
	})
}

fn validate_checkpoint_duty_page(
	batch: &CheckpointDutyBatch,
	profile: &NodeProfile,
) -> Result<(), StoreError> {
	if batch.duties.len() > MAX_CHECKPOINT_DUTY_PAGE_SIZE as usize {
		return Err(StoreError::Invalid("checkpoint duty page exceeds the runtime bound".into()));
	}
	if let Some(cursor) = &batch.requested_cursor {
		if cursor.snapshot_checkpoint != batch.snapshot_checkpoint {
			return Err(StoreError::Invalid(
				"checkpoint duty request cursor belongs to another snapshot".into(),
			));
		}
		normalize_hash(&cursor.last_key)?;
	}
	if batch.next_cursor.is_some() && batch.next_cursor == batch.requested_cursor {
		return Err(StoreError::Invalid("checkpoint duty cursor did not advance".into()));
	}
	match (&batch.next_cursor, batch.duties.last()) {
		(None, _) => {},
		(Some(cursor), Some(last)) => {
			if cursor.snapshot_checkpoint != batch.snapshot_checkpoint
				|| normalize_hash(&cursor.last_key)? != normalize_hash(&last.bucket_id)?
			{
				return Err(StoreError::Invalid(
					"checkpoint duty cursor does not bind the installed snapshot tail".into(),
				));
			}
		},
		(Some(_), None) => {
			return Err(StoreError::Invalid("checkpoint duty page advanced an empty cursor".into()))
		},
	}
	validate_checkpoint_duty_collection(
		&batch.finalized_hash,
		&batch.provider,
		batch.snapshot_checkpoint,
		&batch.duties,
		profile,
	)
}

fn validate_checkpoint_duty_collection(
	finalized_hash: &str,
	provider: &str,
	snapshot_checkpoint: u32,
	duties: &[CheckpointDuty],
	profile: &NodeProfile,
) -> Result<(), StoreError> {
	normalize_hash(finalized_hash)?;
	let expected_provider = normalize_hash(&profile.provider)?;
	let expected_key = normalize_hash(&profile.service_key)?;
	if normalize_hash(provider)? != expected_provider {
		return Err(StoreError::Invalid("checkpoint duty page belongs to another provider".into()));
	}
	let mut previous_bucket = None;
	let mut ids = BTreeMap::new();
	for duty in duties {
		let duty_id = normalize_hash(&duty.duty_id)?;
		let bucket = normalize_hash(&duty.bucket_id)?;
		if duty.snapshot_checkpoint != snapshot_checkpoint {
			return Err(StoreError::Invalid(
				"checkpoint duty does not belong to the installed snapshot".into(),
			));
		}
		if normalize_hash(&duty.provider)? != expected_provider {
			return Err(StoreError::Invalid(
				"checkpoint duty is addressed to another provider".into(),
			));
		}
		if normalize_hash(&duty.service_key)? != expected_key {
			return Err(StoreError::Invalid("checkpoint duty uses another service key".into()));
		}
		normalize_hash(&duty.snapshot_hash)?;
		let encoded = hex::decode(duty.encoded_duty.trim_start_matches("0x")).map_err(|error| {
			StoreError::Invalid(format!("invalid checkpoint duty SCALE: {error}"))
		})?;
		if normalize_hash(&duty.duty_fingerprint)?
			!= hex::encode(sp_crypto_hashing::blake2_256(&encoded))
		{
			return Err(StoreError::Invalid("checkpoint duty fingerprint mismatch".into()));
		}
		let provider_raw = hex::decode(expected_provider.as_bytes()).map_err(io_error)?;
		let provider = AccountId32::new(
			provider_raw
				.try_into()
				.map_err(|_| StoreError::Invalid("provider profile is not 32 bytes".into()))?,
		);
		let service_key: [u8; 32] = hex::decode(expected_key.as_bytes())
			.map_err(io_error)?
			.try_into()
			.map_err(|_| StoreError::Invalid("service key profile is not 32 bytes".into()))?;
		let mut input = &encoded[..];
		let decoded =
			CheckpointDutyInfo::<AccountId32, H256, u32>::decode(&mut input).map_err(|error| {
				StoreError::Invalid(format!("invalid checkpoint duty SCALE: {error}"))
			})?;
		if !input.is_empty() {
			return Err(StoreError::Invalid("checkpoint duty SCALE has trailing bytes".into()));
		}
		let projected = crate::chain::validate_checkpoint_duty(
			decoded,
			&provider,
			service_key,
			snapshot_checkpoint,
		)
		.map_err(|error| StoreError::Invalid(error.to_string()))?;
		if &projected != duty {
			return Err(StoreError::Invalid(
				"checkpoint duty typed projection does not match exact runtime SCALE".into(),
			));
		}
		if ids.insert(duty_id, ()).is_some() {
			return Err(StoreError::Invalid("duplicate checkpoint duty id in batch".into()));
		}
		if previous_bucket.as_ref().is_some_and(|previous| previous >= &bucket) {
			return Err(StoreError::Invalid(
				"checkpoint duties are not in canonical bucket order".into(),
			));
		}
		previous_bucket = Some(bucket);
	}
	Ok(())
}

fn validate_deletion_duty_page(
	batch: &DeletionDutyBatch,
	profile: &NodeProfile,
) -> Result<(), StoreError> {
	if batch.duties.len() > orbis_storage_runtime_api::MAX_DELETION_DUTY_PAGE_SIZE as usize {
		return Err(StoreError::Invalid("manifest deletion page exceeds the runtime bound".into()));
	}
	normalize_hash(&batch.finalized_hash)?;
	let expected_provider = normalize_hash(&profile.provider)?;
	if normalize_hash(&batch.provider)? != expected_provider {
		return Err(StoreError::Invalid(
			"manifest deletion page belongs to another provider".into(),
		));
	}
	if let Some(cursor) = &batch.requested_cursor {
		if cursor.snapshot_checkpoint != batch.snapshot_checkpoint {
			return Err(StoreError::Invalid(
				"manifest deletion request cursor belongs to another snapshot".into(),
			));
		}
		normalize_hash(&cursor.last_manifest)?;
	}
	if batch.next_cursor.is_some() && batch.next_cursor == batch.requested_cursor {
		return Err(StoreError::Invalid("manifest deletion cursor did not advance".into()));
	}
	match (&batch.next_cursor, batch.duties.last()) {
		(None, _) => {},
		(Some(cursor), Some(last)) => {
			if cursor.snapshot_checkpoint != batch.snapshot_checkpoint
				|| normalize_hash(&cursor.last_manifest)? != normalize_hash(&last.manifest)?
			{
				return Err(StoreError::Invalid(
					"manifest deletion cursor does not bind the page tail".into(),
				));
			}
		},
		(Some(_), None) => {
			return Err(StoreError::Invalid(
				"manifest deletion page advanced an empty cursor".into(),
			))
		},
	}
	let provider_raw: [u8; 32] = hex::decode(&expected_provider)
		.map_err(io_error)?
		.try_into()
		.map_err(|_| StoreError::Invalid("provider profile is not 32 bytes".into()))?;
	let provider = AccountId32::new(provider_raw);
	let mut manifests = std::collections::BTreeSet::new();
	for duty in &batch.duties {
		let manifest = normalize_hash(&duty.manifest)?;
		if !manifests.insert(manifest) {
			return Err(StoreError::Invalid(
				"manifest deletion duties contain a duplicate manifest".into(),
			));
		}
		if duty.snapshot_checkpoint != batch.snapshot_checkpoint
			|| normalize_hash(&duty.provider)? != expected_provider
		{
			return Err(StoreError::Invalid(
				"manifest deletion duty has the wrong snapshot or provider".into(),
			));
		}
		normalize_hash(&duty.bucket_id)?;
		normalize_hash(&duty.provider_commitment)?;
		let encoded = hex::decode(duty.encoded_duty.strip_prefix("0x").ok_or_else(|| {
			StoreError::Invalid("manifest deletion SCALE is not prefixed".into())
		})?)
		.map_err(|error| {
			StoreError::Invalid(format!("invalid manifest deletion SCALE: {error}"))
		})?;
		if normalize_hash(&duty.duty_fingerprint)?
			!= hex::encode(sp_crypto_hashing::blake2_256(&encoded))
		{
			return Err(StoreError::Invalid("manifest deletion duty fingerprint mismatch".into()));
		}
		let mut input = &encoded[..];
		let decoded =
			DeletionDutyInfo::<AccountId32, H256, u32>::decode(&mut input).map_err(|error| {
				StoreError::Invalid(format!("invalid manifest deletion SCALE: {error}"))
			})?;
		if !input.is_empty() {
			return Err(StoreError::Invalid("manifest deletion SCALE has trailing bytes".into()));
		}
		let projected =
			crate::chain::validate_deletion_duty(decoded, &provider, batch.snapshot_checkpoint)
				.map_err(|error| StoreError::Invalid(error.to_string()))?;
		if &projected != duty {
			return Err(StoreError::Invalid(
				"manifest deletion typed projection does not match runtime SCALE".into(),
			));
		}
	}
	Ok(())
}

fn verify_deletion_duty_state(state: &PersistedState) -> Result<(), StoreError> {
	if state.pending_manifest_deletions.len()
		> orbis_storage_runtime_api::MAX_DELETION_DUTY_PAGE_SIZE as usize
	{
		return Err(StoreError::Io("manifest deletion pending page exceeds its bound".into()));
	}
	if let Some(intake) = &state.deletion_duty_intake {
		normalize_hash(&intake.finalized_hash)?;
		if normalize_hash(&intake.provider)? != normalize_hash(&state.profile.provider)? {
			return Err(StoreError::Io("manifest deletion intake provider is invalid".into()));
		}
		for cursor in intake.requested_cursor.iter().chain(intake.next_cursor.iter()) {
			if cursor.snapshot_checkpoint != intake.snapshot_checkpoint {
				return Err(StoreError::Io("manifest deletion intake cursor is invalid".into()));
			}
			normalize_hash(&cursor.last_manifest)?;
		}
		if let Some(tail) = &intake.page_tail {
			normalize_hash(tail)?;
		}
		if state
			.pending_manifest_deletions
			.values()
			.any(|duty| duty.snapshot_checkpoint != intake.snapshot_checkpoint)
		{
			return Err(StoreError::Io(
				"manifest deletion pending duty belongs to another intake snapshot".into(),
			));
		}
	}
	if state.deletion_duty_intake.is_none() && !state.pending_manifest_deletions.is_empty() {
		return Err(StoreError::Io("manifest deletion duties have no page intake".into()));
	}
	if let Some(watermark) = &state.deletion_duty_watermark {
		normalize_hash(&watermark.finalized_hash)?;
		if let Some(last_manifest) = &watermark.last_manifest {
			normalize_hash(last_manifest)?;
		}
	}
	for (key, duty) in &state.pending_manifest_deletions {
		if key != &normalize_hash(&duty.manifest)? {
			return Err(StoreError::Io(
				"manifest deletion pending key does not match its duty".into(),
			));
		}
		validate_deletion_duty_page(
			&DeletionDutyBatch {
				finalized_hash: format!("0x{}", "00".repeat(32)),
				finalized_number: 0,
				provider: state.profile.provider.clone(),
				snapshot_checkpoint: duty.snapshot_checkpoint,
				requested_cursor: None,
				next_cursor: None,
				duties: vec![duty.clone()],
			},
			&state.profile,
		)
		.map_err(|_| StoreError::Io("pending manifest deletion duty is invalid".into()))?;
	}
	Ok(())
}

fn terminal_cursor_key(
	cursor: &Option<CheckpointDutyScanCursor>,
) -> Result<Option<String>, StoreError> {
	cursor.as_ref().map(|cursor| normalize_hash(&cursor.last_key)).transpose()
}

fn checkpoint_duty_order_key(duty: &CheckpointDuty) -> Result<String, StoreError> {
	Ok(format!("{}:{}", normalize_hash(&duty.bucket_id)?, normalize_hash(&duty.duty_id)?))
}

fn checkpoint_duty_coordinates_regress(
	snapshot_checkpoint: u32,
	finalized_number: u32,
	watermark: Option<&CheckpointDutyWatermark>,
	inventory: Option<&CheckpointDutyInventory>,
) -> bool {
	watermark.is_some_and(|installed| {
		snapshot_checkpoint < installed.snapshot_checkpoint
			|| finalized_number < installed.finalized_number
	}) || inventory.is_some_and(|installed| {
		snapshot_checkpoint < installed.snapshot_checkpoint
			|| finalized_number < installed.finalized_number
	})
}

fn verify_checkpoint_duty_state(state: &PersistedState) -> Result<(), StoreError> {
	if let Some(intake) = &state.checkpoint_duty_intake {
		validate_checkpoint_duty_collection(
			&intake.finalized_hash,
			&intake.provider,
			intake.snapshot_checkpoint,
			&intake.duties,
			&state.profile,
		)
		.map_err(|_| StoreError::Io("checkpoint duty intake is invalid".into()))?;
		let tail = intake
			.duties
			.last()
			.ok_or_else(|| StoreError::Io("checkpoint duty intake has no page tail".into()))?;
		if intake.next_cursor.snapshot_checkpoint != intake.snapshot_checkpoint
			|| normalize_hash(&intake.next_cursor.last_key)? != normalize_hash(&tail.bucket_id)?
		{
			return Err(StoreError::Io(
				"checkpoint duty intake cursor does not match its tail".into(),
			));
		}
		if checkpoint_duty_coordinates_regress(
			intake.snapshot_checkpoint,
			intake.finalized_number,
			state.checkpoint_duty_watermark.as_ref(),
			state.checkpoint_duty_inventory.as_ref(),
		) {
			return Err(StoreError::Io(
				"checkpoint duty intake regresses behind its installed inventory".into(),
			));
		}
	}
	match (&state.checkpoint_duty_watermark, &state.checkpoint_duty_inventory) {
		(None, None) => {},
		(Some(watermark), Some(inventory)) => {
			if normalize_hash(&watermark.finalized_hash)?
				!= normalize_hash(&inventory.finalized_hash)?
				|| watermark.finalized_number != inventory.finalized_number
				|| watermark.snapshot_checkpoint != inventory.snapshot_checkpoint
			{
				return Err(StoreError::Io(
					"checkpoint duty inventory is not bound to its watermark".into(),
				));
			}
			let terminal = inventory.duties.last().map(|duty| CheckpointDutyScanCursor {
				snapshot_checkpoint: inventory.snapshot_checkpoint,
				last_key: duty.bucket_id.clone(),
			});
			if terminal_cursor_key(&terminal)? != terminal_cursor_key(&watermark.cursor)? {
				return Err(StoreError::Io(
					"checkpoint duty inventory tail does not match its watermark".into(),
				));
			}
			validate_checkpoint_duty_collection(
				&inventory.finalized_hash,
				&state.profile.provider,
				inventory.snapshot_checkpoint,
				&inventory.duties,
				&state.profile,
			)
			.map_err(|_| StoreError::Io("checkpoint duty inventory is invalid".into()))?;
			for duty in &inventory.duties {
				let key = normalize_hash(&duty.duty_id)?;
				if state.pending_checkpoint_duties.get(&key) != Some(duty) {
					return Err(StoreError::Io(
						"checkpoint duty inventory is absent from the pending journal".into(),
					));
				}
			}
		},
		_ => {
			return Err(StoreError::Io(
				"checkpoint duty inventory and watermark presence differ".into(),
			))
		},
	}
	if let Some(cursor) = &state.checkpoint_duty_discovery_cursor {
		let inventory = state.checkpoint_duty_inventory.as_ref().ok_or_else(|| {
			StoreError::Io("checkpoint duty discovery cursor has no inventory".into())
		})?;
		if normalize_hash(&cursor.finalized_hash)? != normalize_hash(&inventory.finalized_hash)?
			|| cursor.finalized_number != inventory.finalized_number
			|| cursor.snapshot_checkpoint != inventory.snapshot_checkpoint
			|| !inventory.duties.iter().any(|duty| {
				duty.role == CheckpointDutyRole::Replica
					&& checkpoint_duty_order_key(duty).ok().as_deref()
						== Some(cursor.after_key.as_str())
			}) {
			return Err(StoreError::Io(
				"checkpoint duty discovery cursor is outside its inventory".into(),
			));
		}
	}
	Ok(())
}

fn now_ms() -> Result<u64, StoreError> {
	Ok(SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_err(|error| StoreError::Io(error.to_string()))?
		.as_millis() as u64)
}

fn persist_state_at(root: &fs::File, state: &PersistedState) -> Result<(), StoreError> {
	let bytes = encode_persisted_state(state)?;
	let temporary = atomic_temp_path(Path::new(INDEX_FILE));
	crate::bounded_io::write_atomic_at(
		root,
		INDEX_FILE.as_ref(),
		temporary
			.file_name()
			.ok_or_else(|| StoreError::Io("provider index temp name is invalid".into()))?,
		&bytes,
	)
	.map_err(io_error)
}

fn encode_persisted_state(state: &PersistedState) -> Result<Vec<u8>, StoreError> {
	validate_persisted_state_bounds(state)?;
	let bytes = serde_json::to_vec(state).map_err(io_error)?;
	if bytes.len() as u64 > MAX_PROVIDER_INDEX_BYTES {
		return Err(StoreError::Capacity);
	}
	Ok(bytes)
}

fn validate_persisted_state_bounds(state: &PersistedState) -> Result<(), StoreError> {
	if state.records.len() > MAX_PROVIDER_INDEX_RECORDS ||
		state.leaf_hashes.len() > MAX_PROVIDER_INDEX_LEAVES ||
		state.root_history.len() > MAX_PROVIDER_INDEX_LEAVES ||
		state.root_index.len() > MAX_PROVIDER_INDEX_LEAVES ||
		state.pending_roots.len() > MAX_PROVIDER_INDEX_LEAVES ||
		state.pending_deletions.len() > MAX_PROVIDER_INDEX_RECORDS ||
		state.pending_checkpoint_duties.len() > MAX_PROVIDER_INDEX_RECORDS ||
		state
			.checkpoint_duty_intake
			.as_ref()
			.is_some_and(|intake| intake.duties.len() > MAX_PROVIDER_INDEX_RECORDS) ||
		state
			.checkpoint_duty_inventory
			.as_ref()
			.is_some_and(|inventory| inventory.duties.len() > MAX_PROVIDER_INDEX_RECORDS) ||
		state.pending_manifest_deletions.len() > MAX_PROVIDER_INDEX_RECORDS
	{
		return Err(StoreError::Capacity);
	}
	if retained_blob_bytes(state)? > state.capacity_bytes {
		return Err(StoreError::Capacity);
	}
	Ok(())
}

fn collect_index_temps_at(
	directory: &fs::File,
) -> Result<(Vec<crate::bounded_io::PreparedRegularFile>, usize), StoreError> {
	let mut visited = 0usize;
	let mut temps = Vec::new();
	for item in crate::bounded_io::list_directory(directory).map_err(io_error)? {
		visited = visited
			.checked_add(1)
			.ok_or_else(|| StoreError::Io("provider root artifact count overflow".into()))?;
		if visited > MAX_PROVIDER_ROOT_ARTIFACTS {
			return Err(StoreError::Io("provider root contains too many durable artifacts".into()));
		}
		let name = item.name.to_string_lossy().into_owned();
		if let Some(process_id) = name.strip_prefix(INDEX_TEMP_PREFIX) {
			if item.file_type != rustix::fs::FileType::RegularFile {
				return Err(StoreError::Io("provider index temp artifact set is invalid".into()));
			}
			if process_id.is_empty() || !process_id.bytes().all(|byte| byte.is_ascii_digit()) {
				return Err(StoreError::Io("provider index temp artifact set is invalid".into()));
			}
			temps.push(item.into_regular_guard().map_err(io_error)?);
			if temps.len() > MAX_PROVIDER_INDEX_TEMP_ARTIFACTS {
				return Err(StoreError::Io("provider index temp artifact set is invalid".into()));
			}
		}
	}
	Ok((temps, visited))
}

fn validate_blob_namespace_at(
	root_directory: Option<&fs::File>,
	state: &PersistedState,
	index_exists: bool,
) -> Result<(
	bool,
	Option<crate::bounded_io::FileIdentity>,
	Vec<crate::bounded_io::PreparedRegularFile>,
	Vec<PreparedBlobGuard>,
), StoreError> {
	for (key, record) in &state.records {
		if normalize_hash(key)? != *key || record.commitment != *key {
			return Err(StoreError::Io("provider blob record key is not canonical".into()));
		}
	}
	if retained_blob_bytes(state)? > state.capacity_bytes {
		return Err(StoreError::Io("provider retained blob bytes exceed configured capacity".into()));
	}
	let exists = root_directory
		.map(|root| crate::bounded_io::entry_missing_at(root, BLOBS_DIR.as_ref()).map(|missing| !missing))
		.transpose()
		.map_err(io_error)?
		.unwrap_or(false);
	if !exists {
		if state.records.values().any(|record| !record.deleted) {
			return Err(StoreError::Io("provider live blob directory is missing".into()));
		}
		return Ok((false, None, Vec::new(), Vec::new()));
	}
	let directory_handle = crate::bounded_io::open_directory_at(
		root_directory.expect("existing blob directory has provider root"),
		BLOBS_DIR.as_ref(),
	)
	.map_err(io_error)?;
	let directory_identity = crate::bounded_io::file_identity(
		&directory_handle.metadata().map_err(io_error)?,
	);
	let entries = crate::bounded_io::list_directory(&directory_handle).map_err(io_error)?;
	if !index_exists {
		if !entries.is_empty() {
			return Err(StoreError::Io(
				"provider blobs exist without a canonical provider index".into(),
			));
		}
		return Ok((true, Some(directory_identity), Vec::new(), Vec::new()));
	}
	let max_entries = state
		.records
		.len()
		.checked_add(MAX_PROVIDER_BLOB_TEMP_ARTIFACTS)
		.ok_or_else(|| StoreError::Io("provider blob artifact count overflow".into()))?;
	let mut visited = 0usize;
	let mut temps = Vec::new();
	let mut canonical = Vec::new();
	let mut seen = BTreeMap::new();
	for item in entries {
		visited = visited
			.checked_add(1)
			.ok_or_else(|| StoreError::Io("provider blob artifact count overflow".into()))?;
		if visited > max_entries {
			return Err(StoreError::Io(
				"provider blob namespace contains too many artifacts".into(),
			));
		}
		if item.file_type != rustix::fs::FileType::RegularFile {
			return Err(StoreError::Io("provider blob artifact is not a regular file".into()));
		}
		let name = item.name.to_string_lossy().into_owned();
		if let Some((commitment, process_id)) = name.rsplit_once(".tmp-") {
			if normalize_hash(commitment).ok().as_deref() != Some(commitment)
				|| process_id.is_empty()
				|| !process_id.bytes().all(|byte| byte.is_ascii_digit())
			{
				return Err(StoreError::Io("provider blob temp artifact name is invalid".into()));
			}
			temps.push(item.into_regular_guard().map_err(io_error)?);
			if temps.len() > MAX_PROVIDER_BLOB_TEMP_ARTIFACTS {
				return Err(StoreError::Io("provider blob temp artifact set is invalid".into()));
			}
			continue;
		}
		if normalize_hash(&name)? != name {
			return Err(StoreError::Io("provider blob artifact name is invalid".into()));
		}
		let record = state
			.records
			.get(&name)
			.ok_or_else(|| StoreError::Io("provider blob has no canonical index record".into()))?;
		if record.deleted && !state.pending_deletions.contains_key(&name) {
			return Err(StoreError::Io("completed deletion retained provider blob bytes".into()));
		}
		let guard = prepare_blob_guard(&directory_handle, item, record)?;
		canonical.push(guard);
		if seen.insert(name, ()).is_some() {
			return Err(StoreError::Io("provider blob namespace contains a duplicate".into()));
		}
	}
	for record in state.records.values().filter(|record| !record.deleted) {
		if !seen.contains_key(&record.commitment) {
			return Err(StoreError::Io("provider live blob is missing".into()));
		}
	}
	Ok((true, Some(directory_identity), temps, canonical))
}

fn prepare_blob_guard(
	directory: &fs::File,
	entry: crate::bounded_io::DirectoryEntry,
	record: &ContentRecord,
) -> Result<PreparedBlobGuard, StoreError> {
	let guard = entry.into_regular_guard().map_err(io_error)?;
	let file = crate::bounded_io::open_prepared_regular_file_at(directory, &guard)
		.map_err(io_error)?;
	verify_blob_contents(file, record.bytes, &record.commitment)?;
	Ok(PreparedBlobGuard {
		file: guard,
		commitment: record.commitment.clone(),
		expected_bytes: record.bytes,
	})
}

fn validate_prepared_blobs(
	directory: &fs::File,
	blobs: &[PreparedBlobGuard],
) -> Result<(), StoreError> {
	for blob in blobs {
		let file = crate::bounded_io::open_prepared_regular_file_at(directory, &blob.file)
			.map_err(io_error)?;
		verify_blob_contents(file, blob.expected_bytes, &blob.commitment)?;
	}
	Ok(())
}

fn verify_blob_contents(
	file: fs::File,
	expected_bytes: u64,
	expected_commitment: &str,
) -> Result<(), StoreError> {
	let metadata = file.metadata().map_err(io_error)?;
	if !metadata.is_file() || metadata.len() != expected_bytes {
		return Err(StoreError::Io("provider blob length does not match its record".into()));
	}
	let limit = expected_bytes
		.checked_add(1)
		.ok_or_else(|| StoreError::Io("provider blob read bound overflow".into()))?;
	let mut reader = file.take(limit);
	let mut hash = Blake2b::<U32>::new();
	let mut total = 0u64;
	let mut buffer = [0u8; 64 * 1024];
	loop {
		let read = reader.read(&mut buffer).map_err(io_error)?;
		if read == 0 {
			break;
		}
		total = total
			.checked_add(read as u64)
			.ok_or_else(|| StoreError::Io("provider blob length overflow".into()))?;
		hash.update(&buffer[..read]);
	}
	if total != expected_bytes || hex::encode(hash.finalize()) != expected_commitment {
		return Err(StoreError::Io("provider blob failed commitment verification".into()));
	}
	Ok(())
}

fn optional_owned_directory_exists(path: &Path) -> Result<bool, StoreError> {
	match fs::symlink_metadata(path) {
		Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
			Err(StoreError::Io("provider owned namespace is not a directory".into()))
		},
		Ok(_) => Ok(true),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
		Err(error) => Err(io_error(error)),
	}
}

fn atomic_temp_path(path: &Path) -> PathBuf {
	path.with_extension(format!("tmp-{}", std::process::id()))
}

fn io_error(error: impl std::fmt::Display) -> StoreError {
	StoreError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;
	use orbis_storage_runtime_api::{
		CheckpointDutyMode as RuntimeCheckpointDutyMode,
		CheckpointDutyPhase as RuntimeCheckpointDutyPhase, DeletionDutyInfo, ProviderDutyAuthority,
		ProviderDutyRole, RESPONSE_VERSION,
	};

	fn profile() -> NodeProfile {
		NodeProfile {
			provider: "01".repeat(32),
			endpoint: "http://127.0.0.1:8080".into(),
			service_key: "02".repeat(32),
			region: None,
		}
	}

	fn authorization(commitment: [u8; 32], bytes: u64) -> AgreementAuthorization {
		AgreementAuthorization {
			finalized_hash: "0x11".into(),
			agreement_id: format!("0x{}", hex::encode(commitment)),
			provider: profile().provider,
			container_ref: format!("0x{}", "03".repeat(32)),
			bytes,
			expires_at: 100,
		}
	}

	fn duty(id: u8, bucket: u8, snapshot: u32) -> CheckpointDuty {
		duty_with_role(id, bucket, snapshot, CheckpointDutyRole::Primary)
	}

	fn replica_duty(id: u8, bucket: u8, snapshot: u32) -> CheckpointDuty {
		duty_with_role(id, bucket, snapshot, CheckpointDutyRole::Replica)
	}

	fn duty_with_role(
		id: u8,
		bucket: u8,
		snapshot: u32,
		role: CheckpointDutyRole,
	) -> CheckpointDuty {
		let provider = AccountId32::new([1; 32]);
		let other = AccountId32::new([3; 32]);
		let (primary, replica) = if role == CheckpointDutyRole::Primary {
			(provider.clone(), other)
		} else {
			(other, provider.clone())
		};
		let authority = |account: AccountId32, role, order, key, may_sign| ProviderDutyAuthority {
			provider: account,
			role,
			order,
			active_service_key_version: 1,
			active_service_key: [key; 32],
			endpoint_hash: H256::repeat_byte(key),
			organization_sla_eligible: true,
			overdue_challenge: false,
			eligible: true,
			may_sign,
			may_initiate: false,
			exclusion: None,
			initiation_exclusion: None,
			confirmed_checkpoint: None,
		};
		crate::chain::validate_checkpoint_duty(
			CheckpointDutyInfo {
				response_version: RESPONSE_VERSION,
				commons_genesis_hash: H256::repeat_byte(10),
				commons_spec_version: 1,
				commons_transaction_version: 1,
				commons_metadata_hash: H256::repeat_byte(11),
				duty_id: H256::repeat_byte(id),
				bucket_id: H256::repeat_byte(bucket),
				primary: primary.clone(),
				replicas: vec![replica.clone()],
				authorities: vec![
					authority(
						primary.clone(),
						ProviderDutyRole::Primary,
						0,
						if role == CheckpointDutyRole::Primary { 2 } else { 3 },
						true,
					),
					authority(
						replica,
						ProviderDutyRole::Replica,
						1,
						if role == CheckpointDutyRole::Replica { 2 } else { 3 },
						true,
					),
				],
				initiator: None,
				phase: RuntimeCheckpointDutyPhase::NotDue,
				mode: RuntimeCheckpointDutyMode::Standard,
				snapshot_checkpoint: snapshot,
				snapshot_hash: H256::repeat_byte(12),
				due_at: snapshot + 10,
				grace_until: snapshot + 20,
				expected_nonce: snapshot,
				scheduled_at: snapshot,
				previous_commitment: None,
				previous_checkpoint: None,
				expected_next_start_seq: 0,
				required_primary_confirmations: 1,
				required_replica_confirmations: 2,
			},
			&provider,
			[2; 32],
			snapshot,
		)
		.unwrap()
	}

	fn cursor(snapshot: u32, bucket: u8) -> CheckpointDutyScanCursor {
		CheckpointDutyScanCursor {
			snapshot_checkpoint: snapshot,
			last_key: format!("0x{}", hex::encode([bucket; 32])),
		}
	}

	fn page(
		snapshot: u32,
		requested_cursor: Option<CheckpointDutyScanCursor>,
		next_cursor: Option<CheckpointDutyScanCursor>,
		duties: Vec<CheckpointDuty>,
	) -> CheckpointDutyBatch {
		CheckpointDutyBatch {
			finalized_hash: format!("0x{}", "10".repeat(32)),
			finalized_number: snapshot + 1,
			provider: format!("0x{}", "01".repeat(32)),
			snapshot_checkpoint: snapshot,
			requested_cursor,
			next_cursor,
			duties,
		}
	}

	fn checkpoint_duties(count: usize, snapshot: u32) -> Vec<CheckpointDuty> {
		(0..count)
			.map(|index| duty(index as u8, index as u8, snapshot))
			.collect()
	}

	fn deletion_duty(manifest: u8, snapshot: u32) -> DeletionDuty {
		crate::chain::validate_deletion_duty(
			DeletionDutyInfo {
				provider: AccountId32::new([1; 32]),
				manifest: [manifest; 32],
				bucket_id: H256::repeat_byte(manifest.saturating_add(1)),
				provider_commitment: [manifest.saturating_add(2); 32],
				tombstoned_at: snapshot.saturating_sub(1),
			},
			&AccountId32::new([1; 32]),
			snapshot,
		)
		.unwrap()
	}

	fn deletion_cursor(snapshot: u32, manifest: u8) -> DeletionDutyScanCursor {
		DeletionDutyScanCursor {
			snapshot_checkpoint: snapshot,
			last_manifest: format!("0x{}", hex::encode([manifest; 32])),
		}
	}

	fn deletion_page(
		snapshot: u32,
		requested_cursor: Option<DeletionDutyScanCursor>,
		next_cursor: Option<DeletionDutyScanCursor>,
		duties: Vec<DeletionDuty>,
	) -> DeletionDutyBatch {
		DeletionDutyBatch {
			finalized_hash: format!("0x{}", "20".repeat(32)),
			finalized_number: snapshot + 1,
			provider: format!("0x{}", "01".repeat(32)),
			snapshot_checkpoint: snapshot,
			requested_cursor,
			next_cursor,
			duties,
		}
	}

	#[test]
	fn manifest_deletion_pages_resume_after_restart_and_complete_idempotently() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		// Runtime double-map iteration follows hashed storage-key order, not manifest order.
		let first = deletion_duty(2, 70);
		let second = deletion_duty(1, 70);
		let cursor = deletion_cursor(70, 2);
		assert!(!store
			.stage_deletion_duty_page(deletion_page(
				70,
				None,
				Some(cursor.clone()),
				vec![first.clone()],
			))
			.unwrap());
		assert_eq!(store.pending_manifest_deletions(8).unwrap(), vec![first.clone()]);
		assert!(store.deletion_duty_resume_request().unwrap().is_none());
		assert!(store
			.stage_deletion_duty_page(deletion_page(
				70,
				Some(cursor.clone()),
				None,
				vec![second.clone()],
			))
			.is_err());
		drop(store);

		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert_eq!(reopened.pending_manifest_deletions(8).unwrap(), vec![first.clone()]);
		assert!(reopened.deletion_duty_resume_request().unwrap().is_none());
		reopened
			.complete_manifest_deletion(&first.manifest, &first.duty_fingerprint)
			.unwrap();
		drop(reopened);
		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let resume = reopened.deletion_duty_resume_request().unwrap().unwrap();
		assert_eq!(resume.cursor, cursor);
		assert!(reopened
			.stage_deletion_duty_page(deletion_page(70, Some(cursor), None, vec![second.clone()],))
			.unwrap());
		assert_eq!(reopened.pending_manifest_deletions(8).unwrap(), vec![second.clone()]);
		reopened
			.complete_manifest_deletion(&second.manifest, &second.duty_fingerprint)
			.unwrap();
		reopened
			.complete_manifest_deletion(&second.manifest, &second.duty_fingerprint)
			.unwrap();
		assert!(reopened.pending_manifest_deletions(8).unwrap().is_empty());
		assert!(reopened.deletion_duty_resume_request().unwrap().is_none());
	}

	#[test]
	fn manifest_deletion_intake_stays_one_page_bounded_across_restarts() {
		let temp = tempfile::tempdir().unwrap();
		let mut requested = None;
		let mut processed = std::collections::BTreeSet::new();
		let mut largest_journal = 0;
		for manifest in 1..=32u8 {
			let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
			let duty = deletion_duty(manifest, 70);
			let next = (manifest < 32).then(|| deletion_cursor(70, manifest));
			store
				.stage_deletion_duty_page(deletion_page(
					70,
					requested.clone(),
					next.clone(),
					vec![duty.clone()],
				))
				.unwrap();
			assert_eq!(store.pending_manifest_deletions(2).unwrap(), vec![duty.clone()]);
			largest_journal =
				largest_journal.max(std::fs::metadata(temp.path().join(INDEX_FILE)).unwrap().len());
			assert!(processed.insert(duty.manifest.clone()));
			store
				.complete_manifest_deletion(&duty.manifest, &duty.duty_fingerprint)
				.unwrap();
			requested = next;
			drop(store);
		}
		assert_eq!(processed.len(), 32);
		// A page contains one duty here; traversing 32 pages must not grow a snapshot-sized journal.
		assert!(largest_journal < 64 * 1024, "deletion intake journal grew to {largest_journal}");
		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert!(reopened.pending_manifest_deletions(2).unwrap().is_empty());
		assert!(reopened.deletion_duty_resume_request().unwrap().is_none());
	}

	#[test]
	fn manifest_deletion_restart_allows_runtime_tail_to_finish_before_other_page_duties() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let lexical_later = deletion_duty(2, 70);
		let runtime_tail = deletion_duty(1, 70);
		assert!(store
			.stage_deletion_duty_page(deletion_page(
				70,
				None,
				None,
				vec![lexical_later.clone(), runtime_tail.clone()],
			))
			.unwrap());
		store
			.complete_manifest_deletion(&runtime_tail.manifest, &runtime_tail.duty_fingerprint)
			.unwrap();
		drop(store);

		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert_eq!(reopened.pending_manifest_deletions(8).unwrap(), vec![lexical_later.clone()],);
		reopened
			.complete_manifest_deletion(&lexical_later.manifest, &lexical_later.duty_fingerprint)
			.unwrap();
		assert!(reopened.pending_manifest_deletions(8).unwrap().is_empty());
	}

	#[test]
	fn commit_read_proof_delete_and_reopen() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let bytes = b"native Orbis storage".to_vec();
		let commitment = DiskStore::content_commitment(&bytes);
		let input = CommitInput {
			commitment,
			authorization: authorization(commitment, bytes.len() as u64),
			bucket: Some("festival".into()),
			key: Some("ticket.json".into()),
			bytes: bytes.clone(),
		};
		let record = store.commit(input).unwrap();
		assert_eq!(store.read(&record.commitment).unwrap(), bytes);
		assert!(store.exists(&[record.commitment.clone()]).unwrap()[&record.commitment]);
		assert!(store.proof(&record.commitment).unwrap().is_empty());
		assert_eq!(store.peaks().unwrap().len(), 1);
		let authorization = authorization(commitment, bytes.len() as u64);
		let before_root = store.stats().unwrap().root;
		let (_, pending) = store.prepare_delete(&record.commitment, &authorization).unwrap();
		assert_ne!(store.stats().unwrap().root, before_root);
		assert_eq!(store.pending_deletions().unwrap(), vec![pending.clone()]);
		assert!(matches!(store.read(&record.commitment), Err(StoreError::NotFound)));
		assert!(temp.path().join(BLOBS_DIR).join(&record.commitment).exists());
		drop(store);
		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert_eq!(reopened.stats().unwrap().deleted_objects, 1);
		assert_eq!(reopened.pending_deletions().unwrap(), vec![pending]);
		reopened.complete_delete(&record.commitment).unwrap();
		assert!(reopened.pending_deletions().unwrap().is_empty());
		assert!(!temp.path().join(BLOBS_DIR).join(&record.commitment).exists());
	}

	#[test]
	fn pending_deletion_retains_capacity_across_reopen_until_bytes_are_removed() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 16).unwrap();
		let retained = b"twelve-bytes".to_vec();
		let retained_commitment = DiskStore::content_commitment(&retained);
		let retained_record = store
			.commit(CommitInput {
				commitment: retained_commitment,
				authorization: authorization(retained_commitment, retained.len() as u64),
				bucket: None,
				key: None,
				bytes: retained.clone(),
			})
			.unwrap();
		store
			.prepare_delete(
				&retained_record.commitment,
				&authorization(retained_commitment, retained.len() as u64),
			)
			.unwrap();
		let replacement = b"12345".to_vec();
		let replacement_commitment = DiskStore::content_commitment(&replacement);
		let replacement_input = || CommitInput {
			commitment: replacement_commitment,
			authorization: authorization(replacement_commitment, replacement.len() as u64),
			bucket: None,
			key: None,
			bytes: replacement.clone(),
		};
		assert!(matches!(store.commit(replacement_input()), Err(StoreError::Capacity)));
		drop(store);

		let reopened = DiskStore::open(temp.path(), profile(), 16).unwrap();
		assert!(matches!(reopened.commit(replacement_input()), Err(StoreError::Capacity)));
		reopened.complete_delete(&retained_record.commitment).unwrap();
		reopened.commit(replacement_input()).unwrap();
		drop(reopened);

		let invalid = tempfile::tempdir().unwrap();
		let store = DiskStore::open(invalid.path(), profile(), 16).unwrap();
		let commitment = DiskStore::content_commitment(&retained);
		let record = store
			.commit(CommitInput {
				commitment,
				authorization: authorization(commitment, retained.len() as u64),
				bucket: None,
				key: None,
				bytes: retained.clone(),
			})
			.unwrap();
		store
			.prepare_delete(
				&record.commitment,
				&authorization(commitment, retained.len() as u64),
			)
			.unwrap();
		drop(store);
		let index = invalid.path().join(INDEX_FILE);
		let mut state: PersistedState = serde_json::from_slice(&fs::read(&index).unwrap()).unwrap();
		state.capacity_bytes = retained.len() as u64 - 1;
		fs::write(&index, serde_json::to_vec(&state).unwrap()).unwrap();
		assert!(matches!(
			DiskStore::open(invalid.path(), profile(), retained.len() as u64 - 1),
			Err(StoreError::Capacity)
		));
	}

	#[test]
	fn canonical_deletion_vector_matches_runtime_scale_contract() {
		let record = ContentRecord {
			commitment: "02".repeat(32),
			agreement_id: format!("0x{}", "01".repeat(32)),
			container_ref: format!("0x{}", "04".repeat(32)),
			authorized_at: format!("0x{}", "05".repeat(32)),
			expires_at: 10,
			bucket: None,
			key: None,
			bytes: 1,
			leaf_index: 0,
			tombstone_leaf_index: Some(1),
			created_unix_ms: 0,
			deleted: true,
		};
		assert_eq!(
			hex::encode(deletion_leaf(&record, &"07".repeat(32)).unwrap()),
			"0ff8e8e8049774a562cf153e0361c2efa34a0f3c888ca4f9e4efaa7f510e11f4",
		);
	}

	#[test]
	fn checkpoint_duty_page_accepts_128_and_rejects_129_without_state_change() {
		let accepted = tempfile::tempdir().unwrap();
		let store = DiskStore::open(accepted.path(), profile(), 1024).unwrap();
		let duties = checkpoint_duties(MAX_CHECKPOINT_DUTY_PAGE_SIZE as usize, 40);
		assert!(store.stage_checkpoint_duty_page(page(40, None, None, duties)).unwrap());
		assert_eq!(
			store.checkpoint_duty_inventory().unwrap().unwrap().duties.len(),
			MAX_CHECKPOINT_DUTY_PAGE_SIZE as usize,
		);
		drop(store);
		assert!(DiskStore::open(accepted.path(), profile(), 1024).is_ok());

		let rejected = tempfile::tempdir().unwrap();
		let store = DiskStore::open(rejected.path(), profile(), 1024).unwrap();
		let index_before = fs::read(rejected.path().join(INDEX_FILE)).unwrap();
		let oversized = checkpoint_duties(MAX_CHECKPOINT_DUTY_PAGE_SIZE as usize + 1, 40);
		assert!(matches!(
			store.stage_checkpoint_duty_page(page(40, None, None, oversized)),
			Err(StoreError::Invalid(_)),
		));
		assert!(store.checkpoint_duty_watermark().unwrap().is_none());
		assert!(store.checkpoint_duty_inventory().unwrap().is_none());
		assert!(store.pending_checkpoint_duties().unwrap().is_empty());
		assert_eq!(fs::read(rejected.path().join(INDEX_FILE)).unwrap(), index_before);
	}

	#[test]
	fn oversized_persisted_checkpoint_intake_and_inventory_fail_on_reopen() {
		for intake in [true, false] {
			let temp = tempfile::tempdir().unwrap();
			let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
			let mut state = store.read_state().unwrap().clone();
			drop(store);
			let duties = vec![duty(1, 1, 70); MAX_PROVIDER_INDEX_RECORDS + 1];
			if intake {
				state.checkpoint_duty_intake = Some(CheckpointDutyIntake {
					finalized_hash: format!("0x{}", "10".repeat(32)),
					finalized_number: 71,
					provider: format!("0x{}", "01".repeat(32)),
					snapshot_checkpoint: 70,
					next_cursor: cursor(70, 1),
					duties,
				});
			} else {
				state.checkpoint_duty_inventory = Some(CheckpointDutyInventory {
					finalized_hash: format!("0x{}", "10".repeat(32)),
					finalized_number: 71,
					snapshot_checkpoint: 70,
					duties,
				});
			}
			let encoded = serde_json::to_vec(&state).unwrap();
			assert!(encoded.len() as u64 <= MAX_PROVIDER_INDEX_BYTES);
			fs::write(temp.path().join(INDEX_FILE), encoded).unwrap();
			assert!(matches!(
				DiskStore::open(temp.path(), profile(), 1024),
				Err(StoreError::Capacity),
			), "intake {intake}");
		}
	}

	#[test]
	fn duplicated_persisted_checkpoint_intake_fails_on_reopen() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert!(!store
			.stage_checkpoint_duty_page(page(
				70,
				None,
				Some(cursor(70, 1)),
				vec![duty(1, 1, 70)],
			))
			.unwrap());
		drop(store);
		let bytes = fs::read(temp.path().join(INDEX_FILE)).unwrap();
		let mut state: PersistedState = serde_json::from_slice(&bytes).unwrap();
		let intake = state.checkpoint_duty_intake.as_mut().unwrap();
		let duplicate = intake.duties[0].clone();
		intake.duties.push(duplicate);
		fs::write(temp.path().join(INDEX_FILE), serde_json::to_vec(&state).unwrap()).unwrap();
		assert!(DiskStore::open(temp.path(), profile(), 1024).is_err());
	}

	#[test]
	fn checkpoint_duty_pages_stage_resume_and_install_only_at_terminal_page() {
		let temp = tempfile::tempdir().unwrap();
		let snapshot = 40;
		let first_cursor = cursor(snapshot, 1);
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert!(!store
			.stage_checkpoint_duty_page(page(
				snapshot,
				None,
				Some(first_cursor.clone()),
				vec![duty(1, 1, snapshot)],
			))
			.unwrap());
		assert!(store.pending_checkpoint_duties().unwrap().is_empty());
		assert!(store.checkpoint_duty_watermark().unwrap().is_none());
		let resume = store.checkpoint_duty_resume_request().unwrap().unwrap();
		assert_eq!(resume.cursor, first_cursor);
		drop(store);

		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert_eq!(reopened.checkpoint_duty_resume_request().unwrap().unwrap(), resume);
		assert!(reopened
			.stage_checkpoint_duty_page(page(
				snapshot,
				Some(first_cursor),
				None,
				vec![duty(2, 2, snapshot)],
			))
			.unwrap());
		assert!(reopened.checkpoint_duty_resume_request().unwrap().is_none());
		assert_eq!(reopened.pending_checkpoint_duties().unwrap().len(), 2);
		assert_eq!(
			reopened.checkpoint_duty_watermark().unwrap().unwrap().cursor,
			Some(cursor(snapshot, 2)),
		);
	}

	#[test]
	fn cross_page_duplicate_duty_fails_without_changing_durable_intake() {
		let temp = tempfile::tempdir().unwrap();
		let snapshot = 41;
		let first_cursor = cursor(snapshot, 1);
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert!(!store
			.stage_checkpoint_duty_page(page(
				snapshot,
				None,
				Some(first_cursor.clone()),
				vec![duty(1, 1, snapshot)],
			))
			.unwrap());
		let resume = store.checkpoint_duty_resume_request().unwrap().unwrap();
		let index_before = fs::read(temp.path().join(INDEX_FILE)).unwrap();

		assert!(store
			.stage_checkpoint_duty_page(page(
				snapshot,
				Some(first_cursor),
				Some(cursor(snapshot, 2)),
				vec![duty(1, 2, snapshot)],
			))
			.is_err());
		assert_eq!(store.checkpoint_duty_resume_request().unwrap().unwrap(), resume);
		assert!(store.pending_checkpoint_duties().unwrap().is_empty());
		assert!(store.checkpoint_duty_inventory().unwrap().is_none());
		assert_eq!(fs::read(temp.path().join(INDEX_FILE)).unwrap(), index_before);
		drop(store);

		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert_eq!(reopened.checkpoint_duty_resume_request().unwrap().unwrap(), resume);
	}

	#[test]
	fn checkpoint_duty_intake_cannot_regress_on_admission_or_reopen() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert!(store
			.stage_checkpoint_duty_page(page(80, None, None, vec![duty(1, 1, 80)]))
			.unwrap());
		let index_before = fs::read(temp.path().join(INDEX_FILE)).unwrap();

		let mut snapshot_regression =
			page(79, None, Some(cursor(79, 2)), vec![duty(2, 2, 79)]);
		snapshot_regression.finalized_number = 82;
		let mut finalized_regression =
			page(81, None, Some(cursor(81, 2)), vec![duty(2, 2, 81)]);
		finalized_regression.finalized_number = 80;
		for batch in [snapshot_regression, finalized_regression] {
			assert!(store.stage_checkpoint_duty_page(batch).is_err());
			assert!(store.checkpoint_duty_resume_request().unwrap().is_none());
			assert_eq!(fs::read(temp.path().join(INDEX_FILE)).unwrap(), index_before);
		}
		drop(store);
		assert!(DiskStore::open(temp.path(), profile(), 1024).is_ok());

		for regress_snapshot in [true, false] {
			let temp = tempfile::tempdir().unwrap();
			let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
			assert!(store
				.stage_checkpoint_duty_page(page(80, None, None, vec![duty(1, 1, 80)]))
				.unwrap());
			let mut state = store.read_state().unwrap().clone();
			drop(store);
			let (snapshot_checkpoint, finalized_number) =
				if regress_snapshot { (79, 82) } else { (81, 80) };
			state.checkpoint_duty_intake = Some(CheckpointDutyIntake {
				finalized_hash: format!("0x{}", "20".repeat(32)),
				finalized_number,
				provider: format!("0x{}", "01".repeat(32)),
				snapshot_checkpoint,
				next_cursor: cursor(snapshot_checkpoint, 2),
				duties: vec![duty(2, 2, snapshot_checkpoint)],
			});
			fs::write(temp.path().join(INDEX_FILE), serde_json::to_vec(&state).unwrap()).unwrap();
			assert!(DiskStore::open(temp.path(), profile(), 1024).is_err());
		}
	}

	#[test]
	fn latest_duty_inventory_survives_partial_next_scan_then_replaces_exactly() {
		let temp = tempfile::tempdir().unwrap();
		let snapshot = 45;
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let first = vec![
			replica_duty(1, 1, snapshot),
			replica_duty(2, 2, snapshot),
			replica_duty(3, 3, snapshot),
		];
		assert!(store
			.stage_checkpoint_duty_page(page(snapshot, None, None, first.clone()))
			.unwrap());
		let installed_a = store.checkpoint_duty_inventory().unwrap().unwrap();
		assert_eq!(installed_a.duties, first);

		let mut partial_b =
			page(snapshot, None, Some(cursor(snapshot, 1)), vec![replica_duty(4, 1, snapshot)]);
		partial_b.finalized_hash = format!("0x{}", "20".repeat(32));
		partial_b.finalized_number += 1;
		assert!(!store.stage_checkpoint_duty_page(partial_b).unwrap());
		assert_eq!(store.checkpoint_duty_inventory().unwrap().unwrap(), installed_a);
		drop(store);

		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert_eq!(reopened.checkpoint_duty_inventory().unwrap().unwrap(), installed_a);
		let resume = reopened.checkpoint_duty_resume_request().unwrap().unwrap();
		let second_tail = replica_duty(5, 2, snapshot);
		let terminal_b = CheckpointDutyBatch {
			finalized_hash: resume.finalized_hash,
			finalized_number: resume.finalized_number,
			provider: resume.provider,
			snapshot_checkpoint: resume.snapshot_checkpoint,
			requested_cursor: Some(resume.cursor),
			next_cursor: None,
			duties: vec![second_tail.clone()],
		};
		assert!(reopened.stage_checkpoint_duty_page(terminal_b).unwrap());
		let installed_b = reopened.checkpoint_duty_inventory().unwrap().unwrap();
		assert_eq!(installed_b.finalized_hash, format!("0x{}", "20".repeat(32)));
		assert_eq!(installed_b.finalized_number, snapshot + 2);
		assert_eq!(installed_b.duties, vec![replica_duty(4, 1, snapshot), second_tail]);
		assert_eq!(reopened.pending_checkpoint_duties().unwrap().len(), 5);
	}

	#[test]
	fn fixed_view_inventory_replay_is_exact_but_a_different_finalized_hash_replaces() {
		let temp = tempfile::tempdir().unwrap();
		let snapshot = 47;
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let original = vec![replica_duty(1, 1, snapshot), replica_duty(2, 2, snapshot)];
		assert!(store
			.stage_checkpoint_duty_page(page(snapshot, None, None, original.clone()))
			.unwrap());
		let inventory = store.checkpoint_duty_inventory().unwrap().unwrap();
		assert_eq!(store.reserve_checkpoint_replica_duties(&inventory, 1).unwrap(), original[..1],);
		assert!(store
			.stage_checkpoint_duty_page(page(snapshot, None, None, original.clone()))
			.unwrap());
		assert_eq!(store.reserve_checkpoint_replica_duties(&inventory, 1).unwrap(), original[1..],);
		assert!(store
			.stage_checkpoint_duty_page(page(snapshot, None, None, Vec::new()))
			.is_err());

		let mut replacement = page(snapshot, None, None, Vec::new());
		replacement.finalized_hash = format!("0x{}", "30".repeat(32));
		assert!(store.stage_checkpoint_duty_page(replacement).unwrap());
		let inventory = store.checkpoint_duty_inventory().unwrap().unwrap();
		assert_eq!(inventory.finalized_hash, format!("0x{}", "30".repeat(32)));
		assert!(inventory.duties.is_empty());
	}

	#[test]
	fn reopen_rejects_inventory_coordinates_changed_outside_atomic_install() {
		let temp = tempfile::tempdir().unwrap();
		let snapshot = 48;
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		assert!(store
			.stage_checkpoint_duty_page(page(
				snapshot,
				None,
				None,
				vec![replica_duty(1, 1, snapshot)],
			))
			.unwrap());
		drop(store);
		let path = temp.path().join(INDEX_FILE);
		let mut persisted: serde_json::Value =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		persisted["checkpoint_duty_inventory"]["finalized_number"] =
			serde_json::json!(snapshot + 9);
		fs::write(path, serde_json::to_vec_pretty(&persisted).unwrap()).unwrap();
		assert!(DiskStore::open(temp.path(), profile(), 1024).is_err());
	}

	#[test]
	fn replica_duty_reservations_round_robin_durably_past_first_page() {
		let temp = tempfile::tempdir().unwrap();
		let snapshot = 46;
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let duties =
			(1..=200).map(|value| replica_duty(value, value, snapshot)).collect::<Vec<_>>();
		let first_cursor = cursor(snapshot, 128);
		assert!(!store
			.stage_checkpoint_duty_page(page(
				snapshot,
				None,
				Some(first_cursor.clone()),
				duties[..128].to_vec(),
			))
			.unwrap());
		assert!(store
			.stage_checkpoint_duty_page(page(
				snapshot,
				Some(first_cursor),
				None,
				duties[128..].to_vec(),
			))
			.unwrap());
		let inventory = store.checkpoint_duty_inventory().unwrap().unwrap();
		let first = store.reserve_checkpoint_replica_duties(&inventory, 64).unwrap();
		assert_eq!(first, duties[..64]);
		drop(store);

		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let inventory = reopened.checkpoint_duty_inventory().unwrap().unwrap();
		let second = reopened.reserve_checkpoint_replica_duties(&inventory, 64).unwrap();
		let third = reopened.reserve_checkpoint_replica_duties(&inventory, 64).unwrap();
		let fourth = reopened.reserve_checkpoint_replica_duties(&inventory, 64).unwrap();
		assert_eq!(second, duties[64..128]);
		assert_eq!(third, duties[128..192]);
		assert_eq!(fourth[..8], duties[192..]);
		assert_eq!(fourth[8..], duties[..56]);
	}

	#[test]
	fn checkpoint_duty_replay_is_idempotent_but_changed_payload_and_regression_fail() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let original = duty(1, 1, 50);
		assert!(store
			.stage_checkpoint_duty_page(page(50, None, None, vec![original.clone()]))
			.unwrap());
		assert!(store
			.stage_checkpoint_duty_page(page(50, None, None, vec![original.clone()]))
			.unwrap());
		assert_eq!(store.pending_checkpoint_duties().unwrap(), vec![original.clone()]);

		let mut changed = original;
		changed.due_at += 1;
		assert!(store.stage_checkpoint_duty_page(page(50, None, None, vec![changed])).is_err());
		assert!(store
			.stage_checkpoint_duty_page(page(49, None, None, vec![duty(2, 2, 49)]))
			.is_err());
		assert_eq!(store.pending_checkpoint_duties().unwrap().len(), 1);
		assert_eq!(store.checkpoint_duty_watermark().unwrap().unwrap().snapshot_checkpoint, 50);
	}

	#[test]
	fn invalid_or_unwritable_checkpoint_duty_page_cannot_advance_durable_state() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let mut invalid = duty(1, 1, 60);
		invalid.duty_fingerprint = format!("0x{}", "ff".repeat(32));
		assert!(store
			.stage_checkpoint_duty_page(page(60, None, Some(cursor(60, 1)), vec![invalid],))
			.is_err());
		assert!(store.checkpoint_duty_resume_request().unwrap().is_none());

		let moved = temp.path().with_extension("moved");
		std::fs::rename(temp.path(), &moved).unwrap();
		std::fs::write(temp.path(), b"not-a-directory").unwrap();
		assert!(store
			.stage_checkpoint_duty_page(page(60, None, Some(cursor(60, 1)), vec![duty(1, 1, 60)],))
			.is_err());
		assert!(store.checkpoint_duty_resume_request().unwrap().is_none());
		std::fs::remove_file(temp.path()).unwrap();
		std::fs::rename(moved, temp.path()).unwrap();
	}

	#[test]
	fn checkpoint_duty_version_cursor_audience_and_service_key_mismatches_fail_closed() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let base = duty(1, 1, 70);

		let mut wrong_page_provider = page(70, None, None, vec![base.clone()]);
		wrong_page_provider.provider = format!("0x{}", "09".repeat(32));
		assert!(store.stage_checkpoint_duty_page(wrong_page_provider).is_err());

		let mut wrong_key = base.clone();
		wrong_key.service_key = format!("0x{}", "09".repeat(32));
		assert!(store.stage_checkpoint_duty_page(page(70, None, None, vec![wrong_key])).is_err());

		let mut wrong_cursor = page(
			70,
			None,
			Some(CheckpointDutyScanCursor {
				snapshot_checkpoint: 70,
				last_key: format!("0x{}", "09".repeat(32)),
			}),
			vec![base.clone()],
		);
		assert!(store.stage_checkpoint_duty_page(wrong_cursor.clone()).is_err());
		wrong_cursor.next_cursor.as_mut().unwrap().snapshot_checkpoint = 71;
		assert!(store.stage_checkpoint_duty_page(wrong_cursor).is_err());

		let mut wrong_version = base;
		let mut encoded = hex::decode(wrong_version.encoded_duty.trim_start_matches("0x")).unwrap();
		encoded[..2].copy_from_slice(&(RESPONSE_VERSION - 1).to_le_bytes());
		wrong_version.encoded_duty = format!("0x{}", hex::encode(&encoded));
		wrong_version.duty_fingerprint =
			format!("0x{}", hex::encode(sp_crypto_hashing::blake2_256(&encoded)));
		assert!(store
			.stage_checkpoint_duty_page(page(70, None, None, vec![wrong_version]))
			.is_err());
		assert!(store.checkpoint_duty_resume_request().unwrap().is_none());
		assert!(store.checkpoint_duty_watermark().unwrap().is_none());
	}

	#[test]
	fn provider_index_recovery_is_metadata_and_count_bounded() {
		let oversized = tempfile::tempdir().unwrap();
		let index = fs::File::create(oversized.path().join(INDEX_FILE)).unwrap();
		index.set_len(MAX_PROVIDER_INDEX_BYTES + 1).unwrap();
		assert!(DiskStore::open(oversized.path(), profile(), 1024).is_err());

		let counted = tempfile::tempdir().unwrap();
		let store = DiskStore::open(counted.path(), profile(), 1024).unwrap();
		let mut state = store.read_state().unwrap().clone();
		state.leaf_hashes = vec!["00".repeat(32); MAX_PROVIDER_INDEX_LEAVES];
		validate_persisted_state_bounds(&state).unwrap();
		state.leaf_hashes.push("00".repeat(32));
		assert!(matches!(validate_persisted_state_bounds(&state), Err(StoreError::Capacity)));
	}

	#[test]
	fn provider_index_recovery_cleans_one_temp_and_rejects_a_temp_flood() {
		let recovered = tempfile::tempdir().unwrap();
		let temp_path = recovered.path().join(format!("{INDEX_TEMP_PREFIX}11"));
		fs::write(&temp_path, b"partial").unwrap();
		DiskStore::open(recovered.path(), profile(), 1024).unwrap();
		assert!(!temp_path.exists());

		let flooded = tempfile::tempdir().unwrap();
		fs::write(flooded.path().join(format!("{INDEX_TEMP_PREFIX}11")), b"partial").unwrap();
		fs::write(flooded.path().join(format!("{INDEX_TEMP_PREFIX}12")), b"partial").unwrap();
		assert!(DiskStore::open(flooded.path(), profile(), 1024).is_err());
		assert_eq!(
			fs::read_dir(flooded.path())
				.unwrap()
				.filter(|item| {
					item.as_ref()
						.unwrap()
						.file_name()
						.to_string_lossy()
						.starts_with(INDEX_TEMP_PREFIX)
				})
				.count(),
			2
		);
	}

	#[test]
	fn invalid_provider_index_preserves_legal_temp_byte_for_byte() {
		for case in 0..3 {
			let temp = tempfile::tempdir().unwrap();
			DiskStore::open(temp.path(), profile(), 1024).unwrap();
			let crash_temp = temp.path().join(format!("{INDEX_TEMP_PREFIX}77"));
			let crash_bytes = b"exact-index-crash-artifact";
			fs::write(&crash_temp, crash_bytes).unwrap();
			let index = temp.path().join(INDEX_FILE);
			let result = match case {
				0 => {
					fs::write(&index, b"not-json").unwrap();
					DiskStore::open(temp.path(), profile(), 1024)
				},
				1 => {
					fs::OpenOptions::new()
						.write(true)
						.open(&index)
						.unwrap()
						.set_len(MAX_PROVIDER_INDEX_BYTES + 1)
						.unwrap();
					DiskStore::open(temp.path(), profile(), 1024)
				},
				_ => {
					let mut wrong = profile();
					wrong.provider = "03".repeat(32);
					DiskStore::open(temp.path(), wrong, 1024)
				},
			};
			assert!(result.is_err(), "case {case}");
			assert_eq!(fs::read(&crash_temp).unwrap(), crash_bytes, "case {case}");
		}
	}

	#[test]
	fn blob_temp_recovery_is_retryable_bounded_and_validation_gated() {
		let retry = tempfile::tempdir().unwrap();
		let store = DiskStore::open(retry.path(), profile(), 1024).unwrap();
		let bytes = b"same-process-blob-retry".to_vec();
		let commitment = DiskStore::content_commitment(&bytes);
		let commitment_hex = hex::encode(commitment);
		let blob = retry.path().join(BLOBS_DIR).join(&commitment_hex);
		let same_process_temp = atomic_temp_path(&blob);
		fs::write(&same_process_temp, b"partial").unwrap();
		store
			.commit(CommitInput {
				commitment,
				authorization: authorization(commitment, bytes.len() as u64),
				bucket: None,
				key: None,
				bytes: bytes.clone(),
			})
			.unwrap();
		assert!(!same_process_temp.exists());
		assert_eq!(store.read(&commitment_hex).unwrap(), bytes);
		drop(store);

		let flooded = tempfile::tempdir().unwrap();
		drop(DiskStore::open(flooded.path(), profile(), 1024).unwrap());
		let blobs = flooded.path().join(BLOBS_DIR);
		let first = blobs.join(format!("{}.tmp-77", "11".repeat(32)));
		let second = blobs.join(format!("{}.tmp-78", "22".repeat(32)));
		fs::write(&first, b"first-exact-temp").unwrap();
		fs::write(&second, b"second-exact-temp").unwrap();
		assert!(DiskStore::open(flooded.path(), profile(), 1024).is_err());
		assert_eq!(fs::read(first).unwrap(), b"first-exact-temp");
		assert_eq!(fs::read(second).unwrap(), b"second-exact-temp");

		let collision = tempfile::tempdir().unwrap();
		let target = collision.path().join("collision");
		let collision_temp = atomic_temp_path(&target);
		fs::write(&collision_temp, b"unowned-collision-evidence").unwrap();
		let directory = fs::File::open(collision.path()).unwrap();
		assert!(crate::bounded_io::write_atomic_at(
			&directory,
			target.file_name().unwrap(),
			collision_temp.file_name().unwrap(),
			b"new-bytes",
		)
		.is_err());
		assert_eq!(fs::read(collision_temp).unwrap(), b"unowned-collision-evidence");

		let invalid = tempfile::tempdir().unwrap();
		let store = DiskStore::open(invalid.path(), profile(), 1024).unwrap();
		let bytes = b"validation-before-cleanup".to_vec();
		let commitment = DiskStore::content_commitment(&bytes);
		let record = store
			.commit(CommitInput {
				commitment,
				authorization: authorization(commitment, bytes.len() as u64),
				bucket: None,
				key: None,
				bytes,
			})
			.unwrap();
		drop(store);
		fs::write(
			invalid.path().join(BLOBS_DIR).join(&record.commitment),
			b"same-length-corruption",
		)
		.unwrap();
		let preserved = invalid.path().join(BLOBS_DIR).join(format!("{}.tmp-79", "33".repeat(32)));
		fs::write(&preserved, b"preserve-until-valid").unwrap();
		assert!(DiskStore::open(invalid.path(), profile(), 1024).is_err());
		assert_eq!(fs::read(preserved).unwrap(), b"preserve-until-valid");
	}

	#[test]
	fn prepared_disk_open_is_read_only_until_apply() {
		let temp = tempfile::tempdir().unwrap();
		let index = temp.path().join(INDEX_FILE);
		let blobs = temp.path().join(BLOBS_DIR);

		let prepared = DiskStore::prepare_open(temp.path(), profile(), 1024).unwrap();

		assert!(!index.exists());
		assert!(!blobs.exists());
		let _store = prepared.arm().unwrap().apply().unwrap();
		assert!(index.is_file());
		assert!(blobs.is_dir());
	}

	#[test]
	fn provider_root_lock_rolls_from_prepared_plan_into_live_store() {
		let temp = tempfile::tempdir().unwrap();
		drop(DiskStore::open(temp.path(), profile(), 1024).unwrap());

		let prepared = DiskStore::prepare_open(temp.path(), profile(), 1024).unwrap();
		assert!(DiskStore::prepare_open(temp.path(), profile(), 1024).is_err());
		let store = prepared.arm().unwrap().apply().unwrap();
		assert!(DiskStore::prepare_open(temp.path(), profile(), 1024).is_err());
		drop(store);

		let retry = DiskStore::prepare_open(temp.path(), profile(), 1024).unwrap();
		drop(retry);
	}

	#[test]
	fn dropping_armed_missing_root_plan_removes_only_its_empty_root() {
		let parent = tempfile::tempdir().unwrap();
		let root = parent.path().join("provider");
		let prepared = DiskStore::prepare_open(&root, profile(), 1024).unwrap();
		let armed = prepared.arm().unwrap();
		assert!(root.is_dir());
		drop(armed);
		assert!(!root.exists());
	}

	#[cfg(unix)]
	#[test]
	fn missing_provider_root_collision_is_rejected_without_following_symlink() {
		use std::os::unix::fs::symlink;

		let parent = tempfile::tempdir().unwrap();
		let root = parent.path().join("provider");
		let prepared = DiskStore::prepare_open(&root, profile(), 1024).unwrap();
		let external = tempfile::tempdir().unwrap();
		let marker = external.path().join("marker");
		fs::write(&marker, b"external-must-remain-exact").unwrap();
		symlink(external.path(), &root).unwrap();

		assert!(prepared.arm().is_err());
		assert_eq!(fs::read(marker).unwrap(), b"external-must-remain-exact");
		assert!(fs::symlink_metadata(root).unwrap().file_type().is_symlink());
		assert!(fs::read_dir(parent.path()).unwrap().all(|entry| {
			!entry.unwrap().file_name().to_string_lossy().starts_with(".provider-root.create-")
		}));
	}

	#[cfg(unix)]
	#[test]
	fn provider_root_ancestor_symlink_is_rejected_without_external_creation() {
		use std::os::unix::fs::symlink;

		let parent = tempfile::tempdir().unwrap();
		let external = tempfile::tempdir().unwrap();
		let alias = parent.path().join("alias");
		symlink(external.path(), &alias).unwrap();
		let root = alias.join("provider");

		assert!(DiskStore::prepare_open(&root, profile(), 1024).is_err());
		assert!(!external.path().join("provider").exists());
	}

	#[test]
	fn existing_provider_root_substitution_is_rejected_without_touching_the_replacement() {
		let parent = tempfile::tempdir().unwrap();
		let root = parent.path().join("provider");
		drop(DiskStore::open(&root, profile(), 1024).unwrap());
		let prepared = DiskStore::prepare_open(&root, profile(), 1024).unwrap();
		let displaced = parent.path().join("displaced");
		fs::rename(&root, &displaced).unwrap();
		fs::create_dir(&root).unwrap();
		let marker = root.join("replacement-marker");
		fs::write(&marker, b"replacement-must-remain-exact").unwrap();

		assert!(prepared.arm().is_err());
		assert_eq!(fs::read(marker).unwrap(), b"replacement-must-remain-exact");
		assert!(displaced.join(INDEX_FILE).is_file());
	}

	#[test]
	fn live_store_io_remains_bound_to_retained_root_and_blob_capabilities() {
		let parent = tempfile::tempdir().unwrap();
		let root = parent.path().join("provider");
		let store = DiskStore::open(&root, profile(), 4096).unwrap();
		let first = b"capability-bound-first-blob".to_vec();
		let first_commitment = DiskStore::content_commitment(&first);
		let first_record = store
			.commit(CommitInput {
				commitment: first_commitment,
				authorization: authorization(first_commitment, first.len() as u64),
				bucket: None,
				key: None,
				bytes: first.clone(),
			})
			.unwrap();
		let displaced = parent.path().join("provider-displaced");
		fs::rename(&root, &displaced).unwrap();
		fs::create_dir(&root).unwrap();
		fs::create_dir(root.join(BLOBS_DIR)).unwrap();
		let replacement_marker = root.join("replacement-marker");
		fs::write(&replacement_marker, b"replacement-must-remain-exact").unwrap();

		assert_eq!(store.read(&first_record.commitment).unwrap(), first);
		let mut updated = profile();
		updated.endpoint = "http://127.0.0.1:9090".into();
		store.update_profile(updated, 4096).unwrap();
		let second = b"capability-bound-second-blob".to_vec();
		let second_commitment = DiskStore::content_commitment(&second);
		let second_record = store
			.commit(CommitInput {
				commitment: second_commitment,
				authorization: authorization(second_commitment, second.len() as u64),
				bucket: None,
				key: None,
				bytes: second,
			})
			.unwrap();

		assert!(displaced.join(INDEX_FILE).is_file());
		assert!(displaced.join(BLOBS_DIR).join(second_record.commitment).is_file());
		assert!(!root.join(INDEX_FILE).exists());
		assert_eq!(fs::read(replacement_marker).unwrap(), b"replacement-must-remain-exact");
		assert_eq!(fs::read_dir(root.join(BLOBS_DIR)).unwrap().count(), 0);
	}

	#[test]
	fn canonical_index_replacement_after_arm_preserves_replacement_and_recovery_temp() {
		let temp = tempfile::tempdir().unwrap();
		drop(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let canonical = temp.path().join(INDEX_FILE);
		let mut replacement: serde_json::Value =
			serde_json::from_slice(&fs::read(&canonical).unwrap()).unwrap();
		replacement["profile"]["endpoint"] = "http://replacement.invalid".into();
		let replacement_bytes = serde_json::to_vec(&replacement).unwrap();
		let crash_temp = temp.path().join(format!("{INDEX_TEMP_PREFIX}904"));
		let crash_bytes = b"preserve-planned-index-recovery-temp";
		fs::write(&crash_temp, crash_bytes).unwrap();
		let armed = DiskStore::prepare_open(temp.path(), profile(), 1024)
			.unwrap()
			.arm()
			.unwrap();
		let replacement_path = temp.path().join("replacement-index");
		fs::write(&replacement_path, &replacement_bytes).unwrap();
		fs::rename(replacement_path, &canonical).unwrap();

		assert!(armed.apply().is_err());
		assert_eq!(fs::read(canonical).unwrap(), replacement_bytes);
		assert_eq!(fs::read(crash_temp).unwrap(), crash_bytes);
	}

	#[test]
	fn missing_canonical_index_appearance_after_arm_is_preserved() {
		let temp = tempfile::tempdir().unwrap();
		drop(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let canonical = temp.path().join(INDEX_FILE);
		let unexpected = fs::read(&canonical).unwrap();
		fs::remove_file(&canonical).unwrap();
		let crash_temp = temp.path().join(format!("{INDEX_TEMP_PREFIX}905"));
		let crash_bytes = b"preserve-missing-index-recovery-temp";
		fs::write(&crash_temp, crash_bytes).unwrap();
		let armed = DiskStore::prepare_open(temp.path(), profile(), 1024)
			.unwrap()
			.arm()
			.unwrap();
		fs::write(&canonical, &unexpected).unwrap();

		assert!(armed.apply().is_err());
		assert_eq!(fs::read(canonical).unwrap(), unexpected);
		assert_eq!(fs::read(crash_temp).unwrap(), crash_bytes);
	}

	#[test]
	fn blob_directory_replacement_after_arm_is_preserved() {
		let temp = tempfile::tempdir().unwrap();
		drop(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let armed = DiskStore::prepare_open(temp.path(), profile(), 1024)
			.unwrap()
			.arm()
			.unwrap();
		let blobs = temp.path().join(BLOBS_DIR);
		let displaced = temp.path().join("displaced-blobs");
		fs::rename(&blobs, &displaced).unwrap();
		fs::create_dir(&blobs).unwrap();
		let marker = blobs.join("replacement-marker");
		fs::write(&marker, b"replacement-must-remain-exact").unwrap();

		assert!(armed.apply().is_err());
		assert_eq!(fs::read(marker).unwrap(), b"replacement-must-remain-exact");
	}

	#[test]
	fn prepared_index_temp_replacement_is_preserved() {
		let temp = tempfile::tempdir().unwrap();
		drop(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let artifact = temp.path().join(format!("{INDEX_TEMP_PREFIX}906"));
		fs::write(&artifact, b"prepared-index-temp").unwrap();
		let armed = DiskStore::prepare_open(temp.path(), profile(), 1024)
			.unwrap()
			.arm()
			.unwrap();
		let replacement = temp.path().join("replacement-index-temp");
		fs::write(&replacement, b"replacement-temp--").unwrap();
		fs::rename(&replacement, &artifact).unwrap();

		assert!(armed.apply().is_err());
		assert_eq!(fs::read(artifact).unwrap(), b"replacement-temp--");
	}

	#[test]
	fn prepared_blob_temp_replacement_is_preserved() {
		let temp = tempfile::tempdir().unwrap();
		drop(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let artifact = temp
			.path()
			.join(BLOBS_DIR)
			.join(format!("{}.tmp-906", "11".repeat(32)));
		fs::write(&artifact, b"prepared-blob-temp").unwrap();
		let armed = DiskStore::prepare_open(temp.path(), profile(), 1024)
			.unwrap()
			.arm()
			.unwrap();
		let replacement = temp.path().join(BLOBS_DIR).join("replacement-blob-temp");
		fs::write(&replacement, b"replacement-temp-").unwrap();
		fs::rename(&replacement, &artifact).unwrap();

		assert!(armed.apply().is_err());
		assert_eq!(fs::read(artifact).unwrap(), b"replacement-temp-");
	}

	#[test]
	fn same_inode_blob_mutation_fails_before_prepared_temp_cleanup() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let bytes = b"prepared-blob-exact".to_vec();
		let commitment = DiskStore::content_commitment(&bytes);
		let record = store
			.commit(CommitInput {
				commitment,
				authorization: authorization(commitment, bytes.len() as u64),
				bucket: None,
				key: None,
				bytes: bytes.clone(),
			})
			.unwrap();
		drop(store);
		let preserved = temp.path().join(format!("{INDEX_TEMP_PREFIX}907"));
		fs::write(&preserved, b"preserve-before-mutation-reject").unwrap();
		let armed = DiskStore::prepare_open(temp.path(), profile(), 1024)
			.unwrap()
			.arm()
			.unwrap();
		let blob = temp.path().join(BLOBS_DIR).join(record.commitment);
		let metadata_before = fs::metadata(&blob).unwrap();
		let mut mutated = bytes;
		mutated[0] ^= 0x20;
		fs::write(&blob, &mutated).unwrap();
		let metadata_after = fs::metadata(&blob).unwrap();
		assert_eq!(
			crate::bounded_io::file_identity(&metadata_before),
			crate::bounded_io::file_identity(&metadata_after),
		);

		assert!(armed.apply().is_err());
		assert_eq!(fs::read(preserved).unwrap(), b"preserve-before-mutation-reject");
	}

	#[cfg(unix)]
	#[test]
	fn blob_namespace_symlink_is_rejected_without_external_writes() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		drop(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let blobs = temp.path().join(BLOBS_DIR);
		fs::remove_dir(&blobs).unwrap();
		let external = tempfile::tempdir().unwrap();
		symlink(external.path(), &blobs).unwrap();

		assert!(DiskStore::open(temp.path(), profile(), 1024).is_err());
		assert_eq!(fs::read_dir(external.path()).unwrap().count(), 0);
	}

	#[cfg(unix)]
	#[test]
	fn provider_root_rejects_live_and_dangling_symlinks_without_external_mutation() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let external = tempfile::tempdir().unwrap();
		let marker = external.path().join("external-marker");
		let marker_bytes = b"external-tree-must-not-change";
		fs::write(&marker, marker_bytes).unwrap();
		let live = temp.path().join("live-root");
		symlink(external.path(), &live).unwrap();
		let missing = temp.path().join("missing-root");
		let dangling = temp.path().join("dangling-root");
		symlink(&missing, &dangling).unwrap();

		assert!(DiskStore::prepare_open(&live, profile(), 1024).is_err());
		assert!(DiskStore::prepare_open(&dangling, profile(), 1024).is_err());
		assert_eq!(fs::read(marker).unwrap(), marker_bytes);
		assert_eq!(fs::read_dir(external.path()).unwrap().count(), 1);
		assert!(!missing.exists());
	}

	#[cfg(unix)]
	#[test]
	fn canonical_index_rejects_live_and_dangling_symlinks_without_external_mutation() {
		use std::os::unix::fs::symlink;

		let live = tempfile::tempdir().unwrap();
		drop(DiskStore::open(live.path(), profile(), 1024).unwrap());
		let canonical = live.path().join(INDEX_FILE);
		let index_bytes = fs::read(&canonical).unwrap();
		fs::remove_file(&canonical).unwrap();
		let external = tempfile::tempdir().unwrap();
		let external_index = external.path().join("external-index.json");
		fs::write(&external_index, &index_bytes).unwrap();
		let external_marker = external.path().join("marker");
		fs::write(&external_marker, b"external-tree-marker").unwrap();
		symlink(&external_index, &canonical).unwrap();
		let local_temp = live.path().join(format!("{INDEX_TEMP_PREFIX}779"));
		let local_temp_bytes = b"local-recovery-evidence";
		fs::write(&local_temp, local_temp_bytes).unwrap();

		assert!(DiskStore::prepare_open(live.path(), profile(), 1024).is_err());
		assert_eq!(fs::read(&external_index).unwrap(), index_bytes);
		assert_eq!(fs::read(&external_marker).unwrap(), b"external-tree-marker");
		assert_eq!(fs::read(&local_temp).unwrap(), local_temp_bytes);
		assert_eq!(fs::read_dir(external.path()).unwrap().count(), 2);

		let dangling = tempfile::tempdir().unwrap();
		drop(DiskStore::open(dangling.path(), profile(), 1024).unwrap());
		let canonical = dangling.path().join(INDEX_FILE);
		fs::remove_file(&canonical).unwrap();
		let missing = dangling.path().join("missing-index.json");
		symlink(&missing, &canonical).unwrap();
		assert!(DiskStore::prepare_open(dangling.path(), profile(), 1024).is_err());
		assert!(!missing.exists());
	}

	#[test]
	fn missing_provider_index_rejects_existing_blob_namespace_without_mutation() {
		for temporary in [false, true] {
			let temp = tempfile::tempdir().unwrap();
			let blobs = temp.path().join(BLOBS_DIR);
			fs::create_dir(&blobs).unwrap();
			let name =
				if temporary { format!("{}.tmp-77", "44".repeat(32)) } else { "44".repeat(32) };
			let artifact = blobs.join(name);
			let bytes = b"orphaned-without-index";
			fs::write(&artifact, bytes).unwrap();

			assert!(DiskStore::open(temp.path(), profile(), 1024).is_err());

			assert_eq!(fs::read(&artifact).unwrap(), bytes);
			assert!(!temp.path().join(INDEX_FILE).exists());
		}
	}

	#[test]
	fn invalid_provider_index_does_not_create_blob_directory() {
		let temp = tempfile::tempdir().unwrap();
		fs::write(temp.path().join(INDEX_FILE), b"not-json").unwrap();

		assert!(DiskStore::open(temp.path(), profile(), 1024).is_err());

		assert!(!temp.path().join(BLOBS_DIR).exists());
	}

	#[test]
	fn missing_provider_index_recovers_around_same_process_temp() {
		let temp = tempfile::tempdir().unwrap();
		DiskStore::open(temp.path(), profile(), 1024).unwrap();
		fs::remove_file(temp.path().join(INDEX_FILE)).unwrap();
		let crash_temp =
			temp.path().join(format!("{INDEX_TEMP_PREFIX}{}", std::process::id()));
		fs::write(&crash_temp, b"same-process-crash-artifact").unwrap();

		DiskStore::open(temp.path(), profile(), 1024).unwrap();

		assert!(temp.path().join(INDEX_FILE).is_file());
		assert!(!crash_temp.exists());
	}
}
