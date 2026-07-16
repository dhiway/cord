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

use std::{
	collections::BTreeMap,
	fs,
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
	time::{SystemTime, UNIX_EPOCH},
};

use codec::{Decode, Encode};
use orbis_storage_runtime_api::CheckpointDutyInfo;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, H256};

use crate::{
	merkle, AgreementAuthorization, CheckpointDuty, CheckpointDutyBatch, CheckpointDutyPageRequest,
	CheckpointDutyScanCursor, PROTOCOL_VERSION,
};

const INDEX_FILE: &str = "provider-index-v5.json";
const LEGACY_INDEX_FILE: &str = "provider-index-v4.json";
const BLOBS_DIR: &str = "blobs";
const MAX_BUCKET_BYTES: usize = 255;
const MAX_KEY_BYTES: usize = 1024;
const CHUNK_BYTES: usize = 256 * 1024;

/// A validated content write ready for persistence.
#[derive(Clone, Debug)]
pub struct CommitInput {
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
pub struct PendingDeletion {
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
pub struct PendingRootSubmission {
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

/// One authenticated root observation from the append log.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RootObservation {
	/// Canonical root hash.
	pub root: String,
	/// Exact append-log leaf count covered by the root.
	pub leaf_count: u64,
}

/// Signed append-only provider root used by checkpoint and replica surfaces.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedCheckpoint {
	/// Current append-only proof root.
	pub root: String,
	/// Number of proof leaves covered.
	pub leaves: u64,
	/// Creation time for operator ordering.
	pub created_unix_ms: u64,
	/// Service-key signature over the domain-separated checkpoint payload.
	pub signature: String,
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
	#[serde(default)]
	checkpoints: Vec<SignedCheckpoint>,
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
}

impl DiskStore {
	/// Open or create a provider store. Existing protocol/capacity/provider identity must match.
	pub fn open(
		root: impl AsRef<Path>,
		profile: NodeProfile,
		capacity_bytes: u64,
	) -> Result<Self, StoreError> {
		if capacity_bytes == 0 {
			return Err(StoreError::Invalid("capacity must be non-zero".into()));
		}
		validate_profile(&profile)?;
		let root = root.as_ref().to_path_buf();
		fs::create_dir_all(root.join(BLOBS_DIR)).map_err(io_error)?;
		let path = root.join(INDEX_FILE);
		if !path.exists() && root.join(LEGACY_INDEX_FILE).exists() {
			return Err(StoreError::Invalid(
				"provider protocol v4 state is unsupported; initialize a clean data path".into(),
			));
		}
		let state = if path.exists() {
			let data = fs::read(&path).map_err(io_error)?;
			let existing: PersistedState = serde_json::from_slice(&data).map_err(io_error)?;
			if existing.version != PROTOCOL_VERSION {
				return Err(StoreError::Invalid(format!(
					"unsupported persisted protocol version {}",
					existing.version
				)));
			}
			if existing.profile.provider != profile.provider ||
				existing.profile.service_key != profile.service_key
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
			existing
		} else {
			PersistedState {
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
				checkpoints: Vec::new(),
			}
		};
		let store = Self { root, state: RwLock::new(state) };
		if !path.exists() {
			store.persist()?;
		}
		store.verify_index()?;
		Ok(store)
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
		if profile.provider != state.profile.provider ||
			profile.service_key != state.profile.service_key
		{
			return Err(StoreError::Invalid(
				"provider and service_key are immutable for an initialized store".into(),
			));
		}
		let stored = stored_bytes(&state);
		if capacity_bytes < stored || capacity_bytes == 0 {
			return Err(StoreError::Invalid("capacity is below stored bytes".into()));
		}
		let mut next = state.clone();
		next.profile = profile;
		next.capacity_bytes = capacity_bytes;
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(())
	}

	/// Atomically commit a finalized-authorized object.
	pub fn commit(&self, input: CommitInput) -> Result<ContentRecord, StoreError> {
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
		if stored_bytes(&state).saturating_add(input.bytes.len() as u64) > state.capacity_bytes {
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
		write_atomic(&self.root.join(BLOBS_DIR).join(&commitment), &input.bytes)?;
		let encoded_leaf = hex::encode(leaf);
		next.leaf_hashes.push(encoded_leaf.clone());
		journal_root_append(&mut next, encoded_leaf)?;
		next.records.insert(commitment, record.clone());
		if let Err(error) = persist_state(&self.root, &next) {
			let _ = fs::remove_file(self.root.join(BLOBS_DIR).join(&record.commitment));
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
		let bytes = fs::read(self.root.join(BLOBS_DIR).join(&normalized)).map_err(io_error)?;
		if bytes.len() as u64 != record.bytes ||
			hex::encode(Self::content_commitment(&bytes)) != normalized
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
	pub fn prepare_delete(
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
		persist_state(&self.root, &next)?;
		*state = next;
		Ok((result, pending))
	}

	/// Remove bytes only after the deletion submission has been durably appended to the outbox.
	pub fn complete_delete(&self, commitment: &str) -> Result<(), StoreError> {
		let normalized = normalize_hash(commitment)?;
		let mut state = self.write_state()?;
		if !state.pending_deletions.contains_key(&normalized) {
			return Ok(());
		}
		match fs::remove_file(self.root.join(BLOBS_DIR).join(&normalized)) {
			Ok(()) => {},
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
			Err(error) => return Err(io_error(error)),
		}
		let mut next = state.clone();
		next.pending_deletions.remove(&normalized);
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(())
	}

	/// Return deletion journal entries which must be re-enqueued after a crash.
	pub fn pending_deletions(&self) -> Result<Vec<PendingDeletion>, StoreError> {
		Ok(self.read_state()?.pending_deletions.values().cloned().collect())
	}

	/// Return root append journal entries in exact sequence order.
	pub fn pending_root_submissions(&self) -> Result<Vec<PendingRootSubmission>, StoreError> {
		Ok(self.read_state()?.pending_roots.values().cloned().collect())
	}

	/// Atomically stage one page or install a terminal finalized checkpoint-duty snapshot.
	///
	/// Non-terminal pages retain the fixed finalized hash, exact next cursor, and accumulated
	/// duties for restart. A terminal page atomically merges the complete snapshot into pending
	/// work, advances the watermark and clears staging. Existing pending duties are never removed
	/// by intake.
	pub fn stage_checkpoint_duty_page(
		&self,
		batch: CheckpointDutyBatch,
	) -> Result<bool, StoreError> {
		let mut state = self.write_state()?;
		validate_checkpoint_duty_page(&batch, &state.profile)?;
		if let Some(previous) = &state.checkpoint_duty_watermark {
			if batch.snapshot_checkpoint < previous.snapshot_checkpoint ||
				batch.finalized_number < previous.finalized_number
			{
				return Err(StoreError::Invalid(
					"checkpoint duty snapshot or finalized height regressed".into(),
				));
			}
		}
		let mut next = state.clone();
		let mut accumulated = match next.checkpoint_duty_intake.take() {
			Some(intake) => {
				if batch.finalized_hash != intake.finalized_hash ||
					batch.finalized_number != intake.finalized_number ||
					batch.provider != intake.provider ||
					batch.snapshot_checkpoint != intake.snapshot_checkpoint ||
					batch.requested_cursor.as_ref() != Some(&intake.next_cursor)
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
		if let Some(next_cursor) = batch.next_cursor {
			next.checkpoint_duty_intake = Some(CheckpointDutyIntake {
				finalized_hash: batch.finalized_hash,
				finalized_number: batch.finalized_number,
				provider: batch.provider,
				snapshot_checkpoint: batch.snapshot_checkpoint,
				next_cursor,
				duties: accumulated,
			});
			persist_state(&self.root, &next)?;
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
		if let Some(previous) = &state.checkpoint_duty_watermark {
			if batch.snapshot_checkpoint == previous.snapshot_checkpoint &&
				terminal_cursor_key(&cursor)? < terminal_cursor_key(&previous.cursor)?
			{
				return Err(StoreError::Invalid("checkpoint duty cursor regressed".into()));
			}
		}
		next.checkpoint_duty_watermark = Some(CheckpointDutyWatermark {
			finalized_hash: batch.finalized_hash,
			finalized_number: batch.finalized_number,
			snapshot_checkpoint: batch.snapshot_checkpoint,
			cursor,
		});
		next.checkpoint_duty_intake = None;
		persist_state(&self.root, &next)?;
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

	/// Return all durable checkpoint duties awaiting later signing/quorum processing.
	pub fn pending_checkpoint_duties(&self) -> Result<Vec<CheckpointDuty>, StoreError> {
		Ok(self.read_state()?.pending_checkpoint_duties.values().cloned().collect())
	}

	/// Remove a root journal entry only after it has been durably appended to the outbox.
	pub fn complete_root_submission(&self, sequence: u64) -> Result<(), StoreError> {
		let mut state = self.write_state()?;
		if !state.pending_roots.contains_key(&sequence) {
			return Ok(());
		}
		let mut next = state.clone();
		next.pending_roots.remove(&sequence);
		persist_state(&self.root, &next)?;
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
				!record.deleted &&
					bucket.map_or(true, |name| record.bucket.as_deref() == Some(name))
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

	/// Resolve an exact historical append-log root without requiring equality with the current
	/// root.
	pub fn root_observation(&self, root: &str) -> Result<RootObservation, StoreError> {
		let normalized = normalize_hash(root)?;
		let state = self.read_state()?;
		root_observation(&state, &normalized)
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

	/// Persist a signed checkpoint if it advances the covered leaf count or root.
	pub fn append_checkpoint(&self, checkpoint: SignedCheckpoint) -> Result<(), StoreError> {
		let mut state = self.write_state()?;
		let observation = root_observation(&state, &normalize_hash(&checkpoint.root)?)?;
		if checkpoint.leaves != observation.leaf_count {
			return Err(StoreError::Invalid(
				"checkpoint leaf count does not match root history".into(),
			));
		}
		if state
			.checkpoints
			.last()
			.is_some_and(|last| last.root == checkpoint.root && last.leaves == checkpoint.leaves)
		{
			return Ok(());
		}
		let mut next = state.clone();
		next.checkpoints.push(checkpoint);
		if next.checkpoints.len() > 1024 {
			next.checkpoints.remove(0);
		}
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(())
	}

	/// Return bounded checkpoint history for replica catch-up.
	pub fn checkpoints(&self, limit: usize) -> Result<Vec<SignedCheckpoint>, StoreError> {
		if limit == 0 || limit > 100 {
			return Err(StoreError::Invalid("limit must be in 1..=100".into()));
		}
		let state = self.read_state()?;
		Ok(state.checkpoints.iter().rev().take(limit).cloned().collect())
	}

	/// Return the latest signed checkpoint without creating a new one.
	pub fn latest_checkpoint(&self) -> Result<SignedCheckpoint, StoreError> {
		self.read_state()?.checkpoints.last().cloned().ok_or(StoreError::NotFound)
	}

	fn verify_index(&self) -> Result<(), StoreError> {
		let state = self.read_state()?;
		if state.root_sequence != state.leaf_hashes.len() as u64 {
			return Err(StoreError::Io("root sequence and proof-leaf counts differ".into()));
		}
		let leaves = decode_leaves(&state)?;
		let (frontier, history) = merkle::accumulate(&leaves)
			.ok_or_else(|| StoreError::Io("proof frontier reconstruction failed".into()))?;
		if encode_frontier(&frontier) != state.root_frontier ||
			history.iter().map(hex::encode).collect::<Vec<_>>() != state.root_history
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
			if *sequence != pending.sequence ||
				pending.appended_leaves.len() != 1 ||
				pending.expected_leaf_count == 0 ||
				pending.expected_leaf_count > state.leaf_hashes.len() as u64
			{
				return Err(StoreError::Io("pending provider root journal is invalid".into()));
			}
			let index = pending.expected_leaf_count as usize - 1;
			if state.leaf_hashes[index] != pending.appended_leaves[0] ||
				state.root_history.get(index) != Some(&pending.expected_root)
			{
				return Err(StoreError::Io(
					"pending provider root journal does not match proof log".into(),
				));
			}
		}
		Ok(())
	}

	fn persist(&self) -> Result<(), StoreError> {
		let state = self.read_state()?;
		persist_state(&self.root, &state)
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
	if profile.endpoint.len() > 256 ||
		profile.service_key.len() > 256 ||
		profile.provider.len() > 128
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

fn root_observation(state: &PersistedState, root: &str) -> Result<RootObservation, StoreError> {
	if state.leaf_hashes.is_empty() && root == hex::encode(merkle::root(&[])) {
		return Ok(RootObservation { root: root.to_owned(), leaf_count: 0 });
	}
	let leaf_count = state
		.root_index
		.get(root)
		.copied()
		.ok_or_else(|| StoreError::Invalid("root is not present in append history".into()))?;
	Ok(RootObservation { root: root.to_owned(), leaf_count })
}

fn stored_bytes(state: &PersistedState) -> u64 {
	state
		.records
		.values()
		.filter(|record| !record.deleted)
		.fold(0u64, |total, record| total.saturating_add(record.bytes))
}

fn validate_checkpoint_duty_page(
	batch: &CheckpointDutyBatch,
	profile: &NodeProfile,
) -> Result<(), StoreError> {
	normalize_hash(&batch.finalized_hash)?;
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
			if cursor.snapshot_checkpoint != batch.snapshot_checkpoint ||
				normalize_hash(&cursor.last_key)? != normalize_hash(&last.bucket_id)?
			{
				return Err(StoreError::Invalid(
					"checkpoint duty cursor does not bind the installed snapshot tail".into(),
				));
			}
		},
		(Some(_), None) =>
			return Err(StoreError::Invalid("checkpoint duty page advanced an empty cursor".into())),
	}
	let expected_provider = normalize_hash(&profile.provider)?;
	let expected_key = normalize_hash(&profile.service_key)?;
	if normalize_hash(&batch.provider)? != expected_provider {
		return Err(StoreError::Invalid("checkpoint duty page belongs to another provider".into()));
	}
	let mut previous_bucket = None;
	let mut ids = BTreeMap::new();
	for duty in &batch.duties {
		let duty_id = normalize_hash(&duty.duty_id)?;
		let bucket = normalize_hash(&duty.bucket_id)?;
		if duty.snapshot_checkpoint != batch.snapshot_checkpoint {
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
		if normalize_hash(&duty.duty_fingerprint)? !=
			hex::encode(sp_crypto_hashing::blake2_256(&encoded))
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
			batch.snapshot_checkpoint,
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

fn terminal_cursor_key(
	cursor: &Option<CheckpointDutyScanCursor>,
) -> Result<Option<String>, StoreError> {
	cursor.as_ref().map(|cursor| normalize_hash(&cursor.last_key)).transpose()
}

fn now_ms() -> Result<u64, StoreError> {
	Ok(SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_err(|error| StoreError::Io(error.to_string()))?
		.as_millis() as u64)
}

fn persist_state(root: &Path, state: &PersistedState) -> Result<(), StoreError> {
	let bytes = serde_json::to_vec_pretty(state).map_err(io_error)?;
	write_atomic(&root.join(INDEX_FILE), &bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
	let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
	let mut file = fs::File::create(&temporary).map_err(io_error)?;
	file.write_all(bytes).map_err(io_error)?;
	file.sync_all().map_err(io_error)?;
	fs::rename(&temporary, path).map_err(io_error).and_then(|()| {
		if let Some(parent) = path.parent() {
			fs::File::open(parent)
				.and_then(|directory| directory.sync_all())
				.map_err(io_error)?;
		}
		Ok(())
	})
}

fn io_error(error: impl std::fmt::Display) -> StoreError {
	StoreError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;
	use orbis_storage_runtime_api::{
		CheckpointDutyMode as RuntimeCheckpointDutyMode,
		CheckpointDutyPhase as RuntimeCheckpointDutyPhase, ProviderDutyAuthority, ProviderDutyRole,
		RESPONSE_VERSION,
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
		let provider = AccountId32::new([1; 32]);
		let replica = AccountId32::new([3; 32]);
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
				primary: provider.clone(),
				replicas: vec![replica.clone()],
				authorities: vec![
					authority(provider.clone(), ProviderDutyRole::Primary, 0, 2, false),
					authority(replica, ProviderDutyRole::Replica, 1, 3, true),
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
}
