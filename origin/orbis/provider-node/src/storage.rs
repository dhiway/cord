// This file is part of CORD - https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Crash-safe content and provider metadata storage.

use std::{
	collections::BTreeMap,
	fs,
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
	time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{merkle, AgreementAuthorization, PROTOCOL_VERSION};

const INDEX_FILE: &str = "provider-index-v1.json";
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
	pending_deletions: BTreeMap<String, PendingDeletion>,
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
		let state = if path.exists() {
			let data = fs::read(&path).map_err(io_error)?;
			let existing: PersistedState = serde_json::from_slice(&data).map_err(io_error)?;
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
			existing
		} else {
			PersistedState {
				version: PROTOCOL_VERSION,
				profile,
				capacity_bytes,
				records: BTreeMap::new(),
				leaf_hashes: Vec::new(),
				pending_deletions: BTreeMap::new(),
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
		if profile.provider != state.profile.provider
			|| profile.service_key != state.profile.service_key
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
		next.leaf_hashes.push(hex::encode(leaf));
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
		let tombstone = record_leaf(&result, true)?;
		next.leaf_hashes.push(hex::encode(tombstone));
		let tombstone_root = hex::encode(merkle::root(&decode_leaves(&next)?));
		let pending = PendingDeletion {
			commitment: normalized.clone(),
			agreement_id: authorization.agreement_id.clone(),
			authorized_at: authorization.finalized_hash.clone(),
			tombstone_root,
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
		let leaves = decode_leaves(&state)?;
		let stored_bytes = stored_bytes(&state);
		Ok(ProviderStats {
			live_objects: state.records.values().filter(|record| !record.deleted).count() as u64,
			deleted_objects: state.records.values().filter(|record| record.deleted).count() as u64,
			stored_bytes,
			capacity_bytes: state.capacity_bytes,
			available_bytes: state.capacity_bytes.saturating_sub(stored_bytes),
			root: hex::encode(merkle::root(&leaves)),
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

	/// Persist a signed checkpoint if it advances the covered leaf count or root.
	pub fn append_checkpoint(&self, checkpoint: SignedCheckpoint) -> Result<(), StoreError> {
		let mut state = self.write_state()?;
		if checkpoint.leaves != state.leaf_hashes.len() as u64 {
			return Err(StoreError::Invalid("checkpoint leaf count is not current".into()));
		}
		if checkpoint.root != hex::encode(merkle::root(&decode_leaves(&state)?)) {
			return Err(StoreError::Invalid("checkpoint root is not current".into()));
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
				if hex::encode(record_leaf(record, true)?) != *actual {
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
	state
		.leaf_hashes
		.iter()
		.map(|value| {
			let raw = hex::decode(value).map_err(io_error)?;
			raw.try_into().map_err(|_| StoreError::Io("proof leaf is not 32 bytes".into()))
		})
		.collect()
}

fn stored_bytes(state: &PersistedState) -> u64 {
	state
		.records
		.values()
		.filter(|record| !record.deleted)
		.fold(0u64, |total, record| total.saturating_add(record.bytes))
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
}
