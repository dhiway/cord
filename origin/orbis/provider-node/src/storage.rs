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
	path::{Path, PathBuf},
	sync::RwLock,
};

use orbis_storage_runtime_api::{
	CheckpointDutyInfo, DeletionDutyInfo, MAX_CHECKPOINT_DUTY_PAGE_SIZE,
};
use serde::{Deserialize, Serialize};
use codec::Decode;
use sp_core::{crypto::AccountId32, H256};

use crate::{
	CheckpointDuty, CheckpointDutyBatch, CheckpointDutyPageRequest,
	CheckpointDutyRole, CheckpointDutyScanCursor, DeletionDuty, DeletionDutyBatch,
	DeletionDutyPageRequest, DeletionDutyScanCursor, MAX_STREAMING_OPERATIONS, PROTOCOL_VERSION,
};

const INDEX_FILE: &str = "provider-index-v6.json";
const INDEX_TEMP_PREFIX: &str = "provider-index-v6.tmp-";
const MAX_PROVIDER_INDEX_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PROVIDER_INDEX_RECORDS: usize = MAX_STREAMING_OPERATIONS;
const MAX_PROVIDER_ROOT_ARTIFACTS: usize = 64;
const MAX_PROVIDER_INDEX_TEMP_ARTIFACTS: usize = 1;

/// Public provider registration profile.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeProfile {
	/// Provider account encoded as lowercase hex.
	pub provider: String,
	/// Runtime-advertised endpoint.
	pub endpoint: String,
	/// Finalized provider service key.
	pub service_key: String,
	/// Optional operator region label.
	pub region: Option<String>,
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

/// The clean-break provider control state. Object bytes and provider-root/proof journals are not
/// a provider-node authority: Host-v2 streaming and canonical runtime duty outboxes own them.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedState {
	version: u16,
	profile: NodeProfile,
	checkpoint_duty_watermark: Option<CheckpointDutyWatermark>,
	checkpoint_duty_intake: Option<CheckpointDutyIntake>,
	pending_checkpoint_duties: BTreeMap<String, CheckpointDuty>,
	checkpoint_duty_inventory: Option<CheckpointDutyInventory>,
	checkpoint_duty_discovery_cursor: Option<CheckpointDutyDiscoveryCursor>,
	deletion_duty_intake: Option<DeletionDutyIntake>,
	deletion_duty_watermark: Option<DeletionDutyWatermark>,
	pending_manifest_deletions: BTreeMap<String, DeletionDuty>,
}

/// Provider control-journal failures. Callers map these to stable HTTP status codes.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
	/// Invalid input or attempted invariant violation.
	#[error("invalid provider control request: {0}")]
	Invalid(String),
	/// A bounded provider control journal would be exceeded.
	#[error("provider control journal capacity exceeded")]
	Capacity,
	/// Local filesystem or serialization failure.
	#[error("provider store I/O failed: {0}")]
	Io(String),
}

/// Crash-safe provider control journal.
///
/// The JSON control index is written through a same-directory temporary followed by an atomic
/// rename. Object bytes and provider proof state remain owned by the private streaming stack.
pub struct DiskStore {
	root: PathBuf,
	state: RwLock<PersistedState>,
	_root_guard: Option<ProviderRootGuard>,
}

pub(crate) struct PreparedDiskStore {
	root: PathBuf,
	state: PersistedState,
	initial_index: Option<Vec<u8>>,
	index_temps: Vec<crate::bounded_io::PreparedRegularFile>,
	root_guard: PreparedProviderRootGuard,
	index_guard: PreparedProviderIndexGuard,
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
		crate::bounded_io::remove_validated_temp_artifacts_at(
			root_guard.directory.file(),
			&prepared.index_temps,
		)
		.map_err(io_error)?;
		let mut store = DiskStore {
			root: prepared.root,
			state: RwLock::new(prepared.state),
			_root_guard: Some(root_guard),
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

	fn persist_state(&self, state: &PersistedState) -> Result<(), StoreError> {
		persist_state_at(self.root_file()?, state)
	}

	/// Reject non-directory and symlink provider roots before any child startup validation.
	pub(crate) fn validate_root(root: &Path) -> Result<(), StoreError> {
		optional_owned_directory_exists(root).map(|_| ())
	}

	/// Open or create a provider control journal. Existing protocol and provider identity must match.
	#[cfg(any(test, feature = "evidence", feature = "test-seams"))]
	pub fn open(
		root: impl AsRef<Path>,
		profile: NodeProfile,
	) -> Result<Self, StoreError> {
		Self::prepare_open(root, profile)?.arm()?.apply()
	}

	pub(crate) fn prepare_open(
		root: impl AsRef<Path>,
		profile: NodeProfile,
	) -> Result<PreparedDiskStore, StoreError> {
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
			};
		store.verify_index()?;
		let state = store
			.state
			.into_inner()
			.map_err(|_| StoreError::Io("provider index validation lock was poisoned".into()))?;
		let recovered_artifacts = root_artifacts
			.checked_sub(index_temps.len())
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
			root_guard,
			index_guard,
		})
	}

	/// Return the provider data root for co-located private durable kernels.
	pub(crate) fn root(&self) -> &Path {
		&self.root
	}

	/// Return the finalized provider profile retained for canonical duty validation.
	pub fn profile(&self) -> Result<NodeProfile, StoreError> {
		Ok(self.read_state()?.profile.clone())
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

	fn verify_index(&self) -> Result<(), StoreError> {
		let state = self.read_state()?;
		verify_checkpoint_duty_state(&state)?;
		verify_deletion_duty_state(&state)
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

fn normalize_hash(value: &str) -> Result<String, StoreError> {
	let value = value.strip_prefix("0x").unwrap_or(value).to_ascii_lowercase();
	let raw = hex::decode(&value).map_err(|_| StoreError::Invalid("hash is not hex".into()))?;
	if raw.len() != 32 {
		return Err(StoreError::Invalid("hash must be exactly 32 bytes".into()));
	}
	Ok(value)
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
	if state.pending_checkpoint_duties.len() > MAX_PROVIDER_INDEX_RECORDS ||
		state.checkpoint_duty_intake.as_ref().is_some_and(|intake| intake.duties.len() > MAX_PROVIDER_INDEX_RECORDS) ||
		state.checkpoint_duty_inventory.as_ref().is_some_and(|inventory| inventory.duties.len() > MAX_PROVIDER_INDEX_RECORDS) ||
		state.pending_manifest_deletions.len() > MAX_PROVIDER_INDEX_RECORDS {
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
