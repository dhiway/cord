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

//! Crash-safe staged streaming content storage.

use std::{
	collections::{BTreeMap, BTreeSet},
	fs::{self, File, OpenOptions},
	io::{Read, Seek, SeekFrom, Write},
	path::{Path, PathBuf},
	sync::{Arc, Condvar, Mutex, RwLock},
};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use blake2::{digest::consts::U32, Blake2b, Digest};
use serde::{Deserialize, Serialize};

use crate::{
	BucketId, CanonicalCid, ContentError, OperationId, CHUNK_BYTES, INGRESS_WINDOW_BYTES,
	INGRESS_WINDOW_CHUNKS, MAX_CHUNKS, MAX_RANGE_BYTES, MAX_STORED_BYTES, MAX_STREAMING_OPERATIONS,
};

#[allow(dead_code)]
pub(crate) mod recovery;
use recovery::{CapabilityReplayRecord, RecoveryRecord};

const STREAM_VERSION: u16 = 9;
const STREAM_ROOT: &str = "streaming-v1";
const JOURNAL: &str = "journal.json";
const STAGING: &str = "staging";
const OBJECTS: &str = "objects";

/// Immutable descriptor for one stored-byte operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamingDescriptor {
	/// Exact 128-bit caller idempotency operation id.
	pub operation_id: OperationId,
	/// Exact 256-bit canonical bucket id.
	pub bucket_id: BucketId,
	/// Expected canonical stored-byte CID.
	pub expected_cid: String,
	/// Exact stored-byte length.
	pub object_len: u64,
}

/// Unsigned local installation receipt. It makes no durable-read or publishability claim.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamingReceipt {
	/// Idempotency operation id.
	pub operation_id: OperationId,
	/// Bucket id.
	pub bucket_id: BucketId,
	/// Canonical stored-byte CID.
	pub cid: String,
	/// Exact stored-byte length.
	pub stored_bytes: u64,
	/// Number of stored chunks.
	pub chunks: u16,
	/// Exact fixed-binary descriptor-and-content replay fingerprint.
	pub fingerprint: String,
	/// Bytes completed the local atomic installation transition.
	pub locally_installed: bool,
}

/// Cumulative durable acknowledgement and current sender window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressAck {
	/// Exact next contiguous chunk index.
	pub next_chunk: u16,
	/// Number of chunks durably journaled.
	pub persisted_chunks: u16,
	/// Number of bytes durably journaled.
	pub persisted_bytes: u64,
	/// Maximum additional chunks that may currently be admitted.
	pub available_window_chunks: usize,
	/// Maximum additional bytes that may currently be admitted.
	pub available_window_bytes: usize,
}

/// Durable progress for one quarantined-object repair operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairProgress {
	/// Exact next contiguous chunk index required by the repair.
	pub next_chunk: u16,
	/// Number of chunks durably journaled for the repair.
	pub persisted_chunks: u16,
	/// Number of repaired bytes durably journaled.
	pub persisted_bytes: u64,
	/// True when every expected chunk is durable and finalization may be retried.
	pub ready_to_finalize: bool,
}

/// Result of opening an idempotent operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BeginStreaming {
	/// Operation is accepting a full replay from index zero or an explicit tail.
	Receiving(ProgressAck),
	/// Exact operation was already installed.
	Installed(StreamingReceipt),
}

/// Non-mutating classification for authenticated replication ingress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplicationIngressState {
	/// The exact derived operation is already installed and fully verified.
	ExactReady,
	/// The exact operation is absent/receiving, including a healthy duplicate CID logical entry.
	Fresh,
	/// The matching installed object is quarantined and requires the exact repair operation.
	Repair,
}

/// Deterministic one-shot crash boundary used only by focused local recovery tests.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamingFault {
	/// Staging file is durable but the operation journal is absent.
	AfterStagingSync,
	/// Appended chunk is durable but its cumulative acknowledgement is absent.
	AfterChunkSync,
	/// Finalizing journal is durable but installation has not started.
	AfterFinalizingJournal,
	/// Object rename is durable but the installed journal is absent.
	AfterObjectRename,
	/// Verified repair rename is durable but sticky quarantine is not cleared.
	AfterRepairRename,
	/// A verified repair chunk is durable but its progress journal is absent.
	AfterRepairChunkSync,
	/// A verified repair prefix is durable but its repair journal is absent.
	AfterRepairPrefixSync,
	/// Repair progress is durable but its acknowledgement was not delivered.
	AfterRepairProgressCommit,
	/// The finalizing repair journal is durable but installation has not started.
	AfterRepairFinalizingJournal,
	/// Quarantine clearance is durable but the successful response was not delivered.
	AfterRepairQuarantineClear,
	/// Recovery effect is staged but its combined journal transition is absent.
	BeforeRecoveryCommit,
	/// Combined recovery transition is durable but its response was not delivered.
	AfterRecoveryCommit,
	/// Cancellation is validated but its terminal journal transition is absent.
	BeforeTerminalCommit,
	/// Terminal cancellation is durable but staged-byte cleanup or response delivery is absent.
	AfterTerminalCommit,
	/// Recovered bytes are installed but the combined Installed terminal journal is absent.
	BeforeRecoveryInstallCommit,
	/// The combined Installed operation and terminal response are durable but were not delivered.
	AfterRecoveryInstallCommit,
	/// Expired recovery records have been selected but the bounded GC transition is absent.
	BeforeRecoveryGcCommit,
	/// The bounded GC transition is durable but staged-byte cleanup or completion is absent.
	AfterRecoveryGcCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
	Receiving,
	Finalizing,
	Installed,
	Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkRecord {
	length: u32,
	hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationRecord {
	descriptor: StreamingDescriptor,
	phase: Phase,
	install_sequence: Option<u64>,
	next_chunk: u16,
	received_bytes: u64,
	chunks: Vec<ChunkRecord>,
	receipt: Option<StreamingReceipt>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalState {
	version: u16,
	operations: BTreeMap<String, OperationRecord>,
	next_install_sequence: u64,
	quarantine: BTreeMap<String, QuarantineRecord>,
	repairs: BTreeMap<String, RepairRecord>,
	detection_sequence: u64,
	recovery: BTreeMap<String, RecoveryRecord>,
	capability_replay: BTreeMap<String, CapabilityReplayRecord>,
}

/// One immutable, fully reverified installed operation used by the private commitment store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct VerifiedInstallation {
	pub(super) install_sequence: u64,
	pub(super) operation_id: OperationId,
	pub(super) bucket_id: BucketId,
	pub(super) cid: CanonicalCid,
	pub(super) stored_bytes: u64,
}

/// Fully reverified local readiness bound to one replication object identity.
pub(crate) struct VerifiedReplicationReady {
	pub(crate) receipt_fingerprint: [u8; 32],
}

/// Fully reverified descriptor and bounded chunk manifest for private peer replication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedReplicationObject {
	pub(crate) operation_id: OperationId,
	pub(crate) bucket_id: BucketId,
	pub(crate) cid: CanonicalCid,
	pub(crate) stored_bytes: u64,
	pub(crate) chunk_hashes: Vec<[u8; 32]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum QuarantineReason {
	Missing,
	Unreadable,
	LengthMismatch,
	ChunkMismatch,
	FullCidMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuarantineRecord {
	reason: QuarantineReason,
	expected_bytes: u64,
	observed_bytes: Option<u64>,
	detection_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RepairPhase {
	Receiving,
	Finalizing,
	Installed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepairRecord {
	operation_id: OperationId,
	cid: String,
	descriptor_hash: String,
	manifest_hash: String,
	next_chunk: u16,
	staged_bytes: u64,
	phase: RepairPhase,
}

/// Redacted local integrity state without object identifiers or failure details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegritySummary {
	/// Number of distinct installed canonical objects.
	pub installed_objects: u64,
	/// Number of installed objects admitted for verified reads.
	pub ready_objects: u64,
	/// Number of distinct installed objects in sticky quarantine.
	pub quarantined_objects: u64,
	/// True only when every installed object is outside quarantine.
	pub ready: bool,
	/// Last monotonically assigned integrity detection sequence.
	pub last_detection_sequence: u64,
}

#[derive(Default)]
struct IngressWindow {
	chunks: usize,
	bytes: usize,
}

#[derive(Default)]
struct IngressShared {
	window: Mutex<IngressWindow>,
	available: Condvar,
}

/// A reserved bounded ingress slot. Dropping it releases the slot without consuming bytes.
pub struct IngressPermit {
	shared: Arc<IngressShared>,
	operation_key: String,
	index: u16,
	bytes: usize,
}

impl Drop for IngressPermit {
	fn drop(&mut self) {
		let mut window = self.shared.window.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
		window.chunks = window.chunks.saturating_sub(1);
		window.bytes = window.bytes.saturating_sub(self.bytes);
		self.shared.available.notify_one();
	}
}

/// Internal provider streaming store. No public HTTP route is attached to this type.
///
/// Operation records are retained for exact replay and never evicted implicitly. New operation
/// admission stops at [`MAX_STREAMING_OPERATIONS`]; exact replay remains available at the bound.
pub struct StreamingStore {
	root: PathBuf,
	state: RwLock<JournalState>,
	window: Arc<IngressShared>,
	fault: RwLock<Option<StreamingFault>>,
	operation_limit: usize,
	#[cfg(test)]
	exact_record_probes: AtomicUsize,
	#[cfg(test)]
	exact_quarantine_probes: AtomicUsize,
}

impl StreamingStore {
	/// Open a staged store, recover durable transitions and remove unowned files.
	pub fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		Self::open_with_limit(root, MAX_STREAMING_OPERATIONS)
	}

	/// Test seam for the durable operation-retention admission bound.
	#[doc(hidden)]
	pub fn open_with_operation_limit(
		root: impl AsRef<Path>,
		operation_limit: usize,
	) -> Result<Self, ContentError> {
		if operation_limit == 0 || operation_limit > MAX_STREAMING_OPERATIONS {
			return Err(ContentError::SchemaInvalid)
		}
		Self::open_with_limit(root, operation_limit)
	}

	fn open_with_limit(
		root: impl AsRef<Path>,
		operation_limit: usize,
	) -> Result<Self, ContentError> {
		let root = root.as_ref().join(STREAM_ROOT);
		fs::create_dir_all(root.join(STAGING)).map_err(io_error)?;
		fs::create_dir_all(root.join(OBJECTS)).map_err(io_error)?;
		let journal = root.join(JOURNAL);
		let state = if journal.exists() {
			let bytes = fs::read(&journal).map_err(io_error)?;
			let state: JournalState = serde_json::from_slice(&bytes).map_err(io_error)?;
			if state.version != STREAM_VERSION ||
				state.operations.len() > operation_limit ||
				state.recovery.len() > MAX_STREAMING_OPERATIONS ||
				state.repairs.len() > operation_limit
			{
				return Err(ContentError::IntegrityFailed)
			}
			recovery::validate_recovery_state(&state)?;
			state
		} else {
			JournalState {
				version: STREAM_VERSION,
				operations: BTreeMap::new(),
				next_install_sequence: 0,
				quarantine: BTreeMap::new(),
				repairs: BTreeMap::new(),
				detection_sequence: 0,
				recovery: BTreeMap::new(),
				capability_replay: BTreeMap::new(),
			}
		};
		let store = Self {
			root,
			state: RwLock::new(state),
			window: Arc::new(IngressShared::default()),
			fault: RwLock::new(None),
			operation_limit,
			#[cfg(test)]
			exact_record_probes: AtomicUsize::new(0),
			#[cfg(test)]
			exact_quarantine_probes: AtomicUsize::new(0),
		};
		store.recover()?;
		if !journal.exists() {
			store.persist()?;
		}
		Ok(store)
	}

	/// Arm one deterministic crash boundary for the next matching transition.
	#[doc(hidden)]
	pub fn inject_fault_once(&self, fault: StreamingFault) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	/// Start or resume one operation. Installed replay requires an exact fixed descriptor.
	pub fn begin(&self, descriptor: StreamingDescriptor) -> Result<BeginStreaming, ContentError> {
		validate_descriptor(&descriptor)?;
		let key = operation_key(&descriptor);
		let mut state = self.write_state()?;
		if state.recovery.values().any(|item| item.descriptor == descriptor) {
			return Err(ContentError::IdempotencyConflict)
		}
		if let Some(existing) = state.operations.get(&key) {
			if existing.descriptor != descriptor {
				return Err(ContentError::IdempotencyConflict)
			}
			if existing.phase == Phase::Installed &&
				state.quarantine.contains_key(&existing.descriptor.expected_cid)
			{
				return Err(ContentError::IntegrityFailed)
			}
			return match (&existing.phase, &existing.receipt) {
				(Phase::Installed, Some(receipt)) => Ok(BeginStreaming::Installed(receipt.clone())),
				(Phase::Receiving, _) => Ok(BeginStreaming::Receiving(self.progress(existing)?)),
				_ => Err(ContentError::IntegrityFailed),
			}
		}
		if state.operations.len() >= self.operation_limit {
			return Err(ContentError::ProviderRecoveryTableFull)
		}
		let path = self.part_path(&key);
		let file = OpenOptions::new().create_new(true).write(true).open(&path).map_err(io_error)?;
		file.sync_all().map_err(io_error)?;
		sync_dir(path.parent().expect("staging path has parent"))?;
		self.trip_fault(StreamingFault::AfterStagingSync)?;
		let mut next = state.clone();
		next.operations.insert(
			key.clone(),
			OperationRecord {
				descriptor,
				phase: Phase::Receiving,
				install_sequence: None,
				next_chunk: 0,
				received_bytes: 0,
				chunks: Vec::new(),
				receipt: None,
			},
		);
		persist_state(&self.root, &next)?;
		let ack = self.progress(next.operations.get(&key).expect("inserted operation"))?;
		*state = next;
		Ok(BeginStreaming::Receiving(ack))
	}

	/// Reserve one enforceable sender-window slot before accepting chunk bytes.
	pub fn try_acquire_ingress(
		&self,
		bucket_id: BucketId,
		operation_id: OperationId,
		index: u16,
		bytes: usize,
	) -> Result<Option<IngressPermit>, ContentError> {
		if bytes > CHUNK_BYTES {
			return Err(ContentError::ChunkTooLarge)
		}
		let key = operation_key_parts(bucket_id, operation_id);
		let state = self.read_state()?;
		let record = state.operations.get(&key).ok_or(ContentError::NotFound)?;
		if state.recovery.values().any(|item| item.descriptor == record.descriptor) {
			return Err(ContentError::IdempotencyConflict)
		}
		if record.phase != Phase::Receiving {
			return Err(ContentError::ChunkOutOfOrder)
		}
		if index as usize >= record.next_chunk as usize + INGRESS_WINDOW_CHUNKS {
			return Err(ContentError::ChunkOutOfOrder)
		}
		validate_chunk_len(record.descriptor.object_len, index, bytes)?;
		drop(state);
		let mut window = self.window.window.lock().map_err(|_| lock_error())?;
		if window.chunks >= INGRESS_WINDOW_CHUNKS ||
			window.bytes.saturating_add(bytes) > INGRESS_WINDOW_BYTES
		{
			return Ok(None)
		}
		window.chunks += 1;
		window.bytes += bytes;
		Ok(Some(IngressPermit {
			shared: Arc::clone(&self.window),
			operation_key: key,
			index,
			bytes,
		}))
	}

	/// Append and durably acknowledge exactly one contiguous fixed-size chunk.
	pub fn push_chunk(
		&self,
		permit: IngressPermit,
		bytes: &[u8],
	) -> Result<ProgressAck, ContentError> {
		if permit.bytes != bytes.len() {
			return Err(ContentError::LengthMismatch)
		}
		let key = permit.operation_key.clone();
		let index = permit.index;
		let mut state = self.write_state()?;
		let record = state.operations.get(&key).cloned().ok_or(ContentError::NotFound)?;
		if state.recovery.values().any(|item| item.descriptor == record.descriptor) {
			return Err(ContentError::IdempotencyConflict)
		}
		if record.phase != Phase::Receiving {
			return Err(ContentError::ChunkOutOfOrder)
		}
		validate_chunk_len(record.descriptor.object_len, index, bytes.len())?;
		let path = self.part_path(&key);
		if index < record.next_chunk {
			let expected =
				record.chunks.get(index as usize).ok_or(ContentError::IntegrityFailed)?;
			if expected.length as usize != bytes.len() || expected.hash != chunk_hash(bytes) {
				return Err(ContentError::IdempotencyConflict)
			}
			let mut file = File::open(&path).map_err(io_error)?;
			let installed = read_chunk_bytes(&mut file, index, expected.length as usize)?;
			return if installed == bytes {
				drop(permit);
				self.progress(&record)
			} else {
				Err(ContentError::IdempotencyConflict)
			}
		}
		if index != record.next_chunk {
			return Err(ContentError::ChunkOutOfOrder)
		}
		let mut file = OpenOptions::new().append(true).open(&path).map_err(io_error)?;
		if file.metadata().map_err(io_error)?.len() != record.received_bytes {
			return Err(ContentError::IntegrityFailed)
		}
		file.write_all(bytes).map_err(io_error)?;
		file.sync_all().map_err(io_error)?;
		self.trip_fault(StreamingFault::AfterChunkSync)?;
		let mut next = state.clone();
		let next_record = next.operations.get_mut(&key).expect("record exists");
		next_record.received_bytes = next_record
			.received_bytes
			.checked_add(bytes.len() as u64)
			.ok_or(ContentError::ObjectTooLarge)?;
		next_record.next_chunk =
			next_record.next_chunk.checked_add(1).ok_or(ContentError::ObjectTooLarge)?;
		next_record
			.chunks
			.push(ChunkRecord { length: bytes.len() as u32, hash: chunk_hash(bytes) });
		persist_state(&self.root, &next)?;
		drop(permit);
		let ack = self.progress(next.operations.get(&key).expect("record exists"))?;
		*state = next;
		Ok(ack)
	}

	/// Synchronous convenience path that still passes through the enforceable ingress window.
	fn push_chunk_sync(
		&self,
		bucket_id: BucketId,
		operation_id: OperationId,
		index: u16,
		bytes: &[u8],
	) -> Result<ProgressAck, ContentError> {
		if bytes.len() > CHUNK_BYTES {
			return Err(ContentError::ChunkTooLarge)
		}
		let key = operation_key_parts(bucket_id, operation_id);
		let state = self.read_state()?;
		let record = state.operations.get(&key).ok_or(ContentError::NotFound)?;
		if state.recovery.values().any(|item| item.descriptor == record.descriptor) {
			return Err(ContentError::IdempotencyConflict)
		}
		if record.phase != Phase::Receiving {
			return Err(ContentError::ChunkOutOfOrder)
		}
		validate_chunk_len(record.descriptor.object_len, index, bytes.len())?;
		drop(state);
		let mut window = self.window.window.lock().map_err(|_| lock_error())?;
		while window.chunks >= INGRESS_WINDOW_CHUNKS ||
			window.bytes.saturating_add(bytes.len()) > INGRESS_WINDOW_BYTES
		{
			window = self.window.available.wait(window).map_err(|_| lock_error())?;
		}
		window.chunks += 1;
		window.bytes += bytes.len();
		drop(window);
		let permit = IngressPermit {
			shared: Arc::clone(&self.window),
			operation_key: key,
			index,
			bytes: bytes.len(),
		};
		self.push_chunk(permit, bytes)
	}

	/// Verify and atomically install a complete staged operation.
	pub fn finalize(
		&self,
		bucket_id: BucketId,
		operation_id: OperationId,
	) -> Result<StreamingReceipt, ContentError> {
		let key = operation_key_parts(bucket_id, operation_id);
		let mut state = self.write_state()?;
		validate_install_sequences(&state)?;
		let record = state.operations.get(&key).cloned().ok_or(ContentError::NotFound)?;
		if state.recovery.values().any(|item| item.descriptor == record.descriptor) {
			return Err(ContentError::IdempotencyConflict)
		}
		if record.phase == Phase::Installed {
			if state.quarantine.contains_key(&record.descriptor.expected_cid) {
				return Err(ContentError::IntegrityFailed)
			}
			return record.receipt.ok_or(ContentError::IntegrityFailed)
		}
		if record.phase != Phase::Receiving {
			return Err(ContentError::IntegrityFailed)
		}
		if record.received_bytes != record.descriptor.object_len ||
			record.next_chunk as usize != chunk_count(record.descriptor.object_len)? ||
			record.chunks.len() != record.next_chunk as usize
		{
			return Err(ContentError::ChunkMissing)
		}
		let part = self.part_path(&key);
		let (cid, fingerprint, length) = verify_file(&part, &record.descriptor, &record.chunks)?;
		if length != record.descriptor.object_len {
			return Err(ContentError::LengthMismatch)
		}
		let receipt = receipt(&record.descriptor, cid.as_str(), fingerprint);
		let mut finalizing = state.clone();
		finalizing.operations.get_mut(&key).expect("record exists").phase = Phase::Finalizing;
		persist_state(&self.root, &finalizing)?;
		*state = finalizing;
		self.trip_fault(StreamingFault::AfterFinalizingJournal)?;
		install_file(&part, &self.object_path(cid.as_str()), &record.descriptor, &record.chunks)?;
		self.trip_fault(StreamingFault::AfterObjectRename)?;
		let mut installed = state.clone();
		let install_sequence = installed.next_install_sequence;
		installed.next_install_sequence =
			install_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		let installed_record = installed.operations.get_mut(&key).expect("record exists");
		installed_record.phase = Phase::Installed;
		installed_record.install_sequence = Some(install_sequence);
		installed_record.receipt = Some(receipt.clone());
		validate_install_sequences(&installed)?;
		persist_state(&self.root, &installed)?;
		*state = installed;
		Ok(receipt)
	}

	/// Consume an exact full-stream replay from index zero. Previously acknowledged chunks are
	/// compared and re-acknowledged; callers that possess only a tail must explicitly reserve and
	/// push from the durable `next_chunk` returned by `begin`.
	pub fn put_chunks<I>(
		&self,
		descriptor: StreamingDescriptor,
		chunks: I,
	) -> Result<StreamingReceipt, ContentError>
	where
		I: IntoIterator<Item = Vec<u8>>,
	{
		match self.begin(descriptor.clone())? {
			BeginStreaming::Installed(receipt) => {
				let (cid, fingerprint, length, records) = verify_chunk_stream(&descriptor, chunks)
					.map_err(|_| ContentError::IdempotencyConflict)?;
				if cid.as_str() != receipt.cid ||
					fingerprint != receipt.fingerprint ||
					length != receipt.stored_bytes ||
					records.len() != receipt.chunks as usize
				{
					return Err(ContentError::IdempotencyConflict)
				}
				Ok(receipt)
			},
			BeginStreaming::Receiving(_) => {
				for (index, chunk) in chunks.into_iter().enumerate() {
					let index: u16 = index.try_into().map_err(|_| ContentError::ObjectTooLarge)?;
					self.push_chunk_sync(
						descriptor.bucket_id,
						descriptor.operation_id,
						index,
						&chunk,
					)?;
				}
				self.finalize(descriptor.bucket_id, descriptor.operation_id)
			},
		}
	}

	/// Verify an installed object completely without exposing a filesystem handle or bytes.
	pub fn verify_installed(&self, cid: &str) -> Result<(), ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let record = self.installed_record(canonical.as_str())?;
		self.reject_quarantined(canonical.as_str())?;
		let path = self.object_path(canonical.as_str());
		match verify_file(&path, &record.descriptor, &record.chunks) {
			Ok(_) => Ok(()),
			Err(error) => {
				self.quarantine_failure(canonical.as_str(), &record, &path, &error)?;
				Err(ContentError::IntegrityFailed)
			},
		}
	}

	/// Return a bounded half-open range only after a full preflight and per-touched-chunk re-hash.
	pub fn read_range_verified(
		&self,
		cid: &str,
		start: u64,
		end: u64,
	) -> Result<Vec<u8>, ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let record = self.installed_record(canonical.as_str())?;
		self.reject_quarantined(canonical.as_str())?;
		if start > end ||
			end > record.descriptor.object_len ||
			end.saturating_sub(start) > MAX_RANGE_BYTES
		{
			return Err(ContentError::RangeInvalid)
		}
		let path = self.object_path(canonical.as_str());
		let (mut file, _, _, _) = match verify_open_file(&path, &record.descriptor, &record.chunks)
		{
			Ok(verified) => verified,
			Err(error) => {
				self.quarantine_failure(canonical.as_str(), &record, &path, &error)?;
				return Err(ContentError::IntegrityFailed)
			},
		};
		if start == end {
			return Ok(Vec::new())
		}
		let first = (start / CHUNK_BYTES as u64) as usize;
		let last = ((end - 1) / CHUNK_BYTES as u64) as usize;
		let mut output = Vec::with_capacity((end - start) as usize);
		for index in first..=last {
			let chunk = match read_verified_chunk(&mut file, index as u16, &record.chunks[index]) {
				Ok(chunk) => chunk,
				Err(error) => {
					self.quarantine_failure(canonical.as_str(), &record, &path, &error)?;
					return Err(ContentError::IntegrityFailed)
				},
			};
			let chunk_start = index as u64 * CHUNK_BYTES as u64;
			let from = start.saturating_sub(chunk_start) as usize;
			let to = (end - chunk_start).min(chunk.len() as u64) as usize;
			output.extend_from_slice(&chunk[from..to]);
		}
		Ok(output)
	}

	/// Return one chunk only after a full preflight and complete emitted-chunk re-hash.
	pub fn read_chunk_verified(&self, cid: &str, index: u16) -> Result<Vec<u8>, ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let record = self.installed_record(canonical.as_str())?;
		self.reject_quarantined(canonical.as_str())?;
		let expected = record.chunks.get(index as usize).ok_or(ContentError::ChunkOutOfOrder)?;
		let path = self.object_path(canonical.as_str());
		let (mut file, _, _, _) = match verify_open_file(&path, &record.descriptor, &record.chunks)
		{
			Ok(verified) => verified,
			Err(error) => {
				self.quarantine_failure(canonical.as_str(), &record, &path, &error)?;
				return Err(ContentError::IntegrityFailed)
			},
		};
		match read_verified_chunk(&mut file, index, expected) {
			Ok(bytes) => Ok(bytes),
			Err(error) => {
				self.quarantine_failure(canonical.as_str(), &record, &path, &error)?;
				Err(ContentError::IntegrityFailed)
			},
		}
	}

	/// Audit every non-quarantined object and return redacted durable readiness counts.
	pub fn integrity_summary(&self) -> Result<IntegritySummary, ContentError> {
		self.audit_integrity()
	}

	fn audit_integrity(&self) -> Result<IntegritySummary, ContentError> {
		for (cid, record) in self.installed_records()? {
			if self.is_quarantined(&cid)? {
				continue
			}
			let path = self.object_path(&cid);
			if let Err(error) = verify_file(&path, &record.descriptor, &record.chunks) {
				self.quarantine_failure(&cid, &record, &path, &error)?;
			}
		}
		self.unchecked_integrity_summary()
	}

	fn unchecked_integrity_summary(&self) -> Result<IntegritySummary, ContentError> {
		let state = self.read_state()?;
		Ok(integrity_summary(&state))
	}

	/// Install exact verified bytes for an existing quarantine and clear it only after durability.
	pub fn install_verified_repair(&self, cid: &str, bytes: &[u8]) -> Result<(), ContentError> {
		self.install_verified_repair_reader(cid, bytes)
	}

	/// Stream an exact verified repair for an existing quarantine through the resumable journal.
	pub fn install_verified_repair_reader<R: Read>(
		&self,
		cid: &str,
		mut reader: R,
	) -> Result<(), ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let record = self.installed_record(canonical.as_str())?;
		let operation_id = full_repair_operation(canonical.as_str());
		let progress = match self.begin_repair(canonical.as_str(), operation_id) {
			Ok(progress) => progress,
			Err(ContentError::IdempotencyConflict) => {
				self.completed_repair_progress(canonical.as_str(), operation_id)?
			},
			Err(error) => return Err(error),
		};
		for (index, expected) in record.chunks.iter().enumerate() {
			let mut bytes = vec![0; expected.length as usize];
			reader.read_exact(&mut bytes).map_err(|_| ContentError::LengthMismatch)?;
			if chunk_hash(&bytes) != expected.hash {
				return Err(ContentError::CidMismatch);
			}
			let index: u16 = index.try_into().map_err(|_| ContentError::ObjectTooLarge)?;
			if index >= progress.next_chunk {
				self.push_repair_chunk(canonical.as_str(), operation_id, index, &bytes)?;
			}
		}
		let mut trailing = [0u8; 1];
		if reader.read(&mut trailing).map_err(|_| ContentError::IntegrityFailed)? != 0 {
			return Err(ContentError::LengthMismatch);
		}
		self.finalize_repair(canonical.as_str(), operation_id)
	}

	/// Start or resume one stable repair and return its first absent or invalid chunk.
	pub fn begin_repair(
		&self,
		cid: &str,
		operation_id: OperationId,
	) -> Result<RepairProgress, ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let mut state = self.write_state()?;
		let record = installed_record_in(&state, canonical.as_str())?;
		validate_installed_record(&record)?;
		let key = repair_key(canonical.as_str(), operation_id)?;
		if !state.quarantine.contains_key(canonical.as_str()) {
			return Err(ContentError::IdempotencyConflict);
		}
		if let Some(existing) = state.repairs.get(&key) {
			validate_repair_record(&key, existing, &record)?;
			return if existing.phase == RepairPhase::Installed {
				Err(ContentError::IntegrityFailed)
			} else {
				repair_progress(existing, &record)
			};
		}
		if state.repairs.values().any(|repair| repair.cid == canonical.as_str()) {
			return Err(ContentError::IdempotencyConflict);
		}

		let repair = RepairRecord {
			operation_id,
			cid: canonical.as_str().into(),
			descriptor_hash: repair_descriptor_hash(&record.descriptor),
			manifest_hash: repair_manifest_hash(&record.chunks),
			next_chunk: 0,
			staged_bytes: 0,
			phase: RepairPhase::Receiving,
		};
		let path = self.repair_path(&key);
		remove_repair_if_present(&path)?;
		let mut staged =
			OpenOptions::new().create_new(true).write(true).open(&path).map_err(io_error)?;
		let mut source = File::open(self.object_path(canonical.as_str())).ok();
		let mut repair = repair;
		for expected in &record.chunks {
			let mut bytes = vec![0; expected.length as usize];
			let Some(source) = source.as_mut() else { break };
			if source.read_exact(&mut bytes).is_err() || chunk_hash(&bytes) != expected.hash {
				break;
			}
			staged.write_all(&bytes).map_err(io_error)?;
			repair.next_chunk =
				repair.next_chunk.checked_add(1).ok_or(ContentError::ObjectTooLarge)?;
			repair.staged_bytes = repair
				.staged_bytes
				.checked_add(bytes.len() as u64)
				.ok_or(ContentError::ObjectTooLarge)?;
		}
		staged.sync_all().map_err(io_error)?;
		sync_dir(path.parent().expect("repair path has parent"))?;
		self.trip_fault(StreamingFault::AfterRepairPrefixSync)?;
		let mut next = state.clone();
		next.repairs.insert(key, repair.clone());
		persist_state(&self.root, &next)?;
		*state = next;
		repair_progress(&repair, &record)
	}

	/// Append and durably journal one exact contiguous verified repair chunk.
	pub fn push_repair_chunk(
		&self,
		cid: &str,
		operation_id: OperationId,
		index: u16,
		bytes: &[u8],
	) -> Result<RepairProgress, ContentError> {
		if bytes.len() > CHUNK_BYTES {
			return Err(ContentError::ChunkTooLarge);
		}
		let canonical = CanonicalCid::parse(cid)?;
		let key = repair_key(canonical.as_str(), operation_id)?;
		let mut state = self.write_state()?;
		let installed = installed_record_in(&state, canonical.as_str())?;
		validate_installed_record(&installed)?;
		let repair = state.repairs.get(&key).cloned().ok_or(ContentError::NotFound)?;
		validate_repair_record(&key, &repair, &installed)?;
		if !state.quarantine.contains_key(canonical.as_str()) {
			return Err(ContentError::IdempotencyConflict);
		}
		if repair.phase != RepairPhase::Receiving {
			return Err(ContentError::ChunkOutOfOrder);
		}
		if index < repair.next_chunk {
			let expected =
				installed.chunks.get(index as usize).ok_or(ContentError::IntegrityFailed)?;
			if expected.length as usize != bytes.len() || expected.hash != chunk_hash(bytes) {
				return Err(ContentError::IdempotencyConflict);
			}
			let mut file = File::open(self.repair_path(&key)).map_err(io_error)?;
			let durable = read_chunk_bytes(&mut file, index, expected.length as usize)?;
			if durable != bytes || chunk_hash(&durable) != expected.hash {
				return Err(ContentError::IntegrityFailed);
			}
			return repair_progress(&repair, &installed);
		}
		if index != repair.next_chunk {
			return Err(ContentError::ChunkOutOfOrder);
		}
		validate_chunk_len(installed.descriptor.object_len, index, bytes.len())?;
		let expected = installed.chunks.get(index as usize).ok_or(ContentError::ChunkOutOfOrder)?;
		if chunk_hash(bytes) != expected.hash {
			return Err(ContentError::CidMismatch);
		}
		let path = self.repair_path(&key);
		let mut file = OpenOptions::new().append(true).open(&path).map_err(io_error)?;
		if file.metadata().map_err(io_error)?.len() != repair.staged_bytes {
			return Err(ContentError::IntegrityFailed);
		}
		file.write_all(bytes).map_err(io_error)?;
		file.sync_all().map_err(io_error)?;
		self.trip_fault(StreamingFault::AfterRepairChunkSync)?;
		let mut next = state.clone();
		let next_repair = next.repairs.get_mut(&key).expect("repair exists");
		next_repair.next_chunk =
			next_repair.next_chunk.checked_add(1).ok_or(ContentError::ObjectTooLarge)?;
		next_repair.staged_bytes = next_repair
			.staged_bytes
			.checked_add(bytes.len() as u64)
			.ok_or(ContentError::ObjectTooLarge)?;
		persist_state(&self.root, &next)?;
		let progress = repair_progress(next.repairs.get(&key).expect("repair exists"), &installed)?;
		*state = next;
		self.trip_fault(StreamingFault::AfterRepairProgressCommit)?;
		Ok(progress)
	}

	/// Verify and atomically install one complete durable repair.
	pub fn finalize_repair(
		&self,
		cid: &str,
		operation_id: OperationId,
	) -> Result<(), ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let key = repair_key(canonical.as_str(), operation_id)?;
		let mut state = self.write_state()?;
		let installed = installed_record_in(&state, canonical.as_str())?;
		validate_installed_record(&installed)?;
		let repair = state.repairs.get(&key).cloned().ok_or(ContentError::NotFound)?;
		validate_repair_record(&key, &repair, &installed)?;
		let object = self.object_path(canonical.as_str());
		if repair.phase == RepairPhase::Installed
			&& !state.quarantine.contains_key(canonical.as_str())
		{
			return verify_file(&object, &installed.descriptor, &installed.chunks).map(|_| ());
		}
		if !state.quarantine.contains_key(canonical.as_str()) {
			return Err(ContentError::IdempotencyConflict);
		}
		if repair.next_chunk as usize != installed.chunks.len()
			|| repair.staged_bytes != installed.descriptor.object_len
		{
			return Err(ContentError::ChunkMissing);
		}
		let path = self.repair_path(&key);
		if repair.phase == RepairPhase::Receiving {
			verify_file(&path, &installed.descriptor, &installed.chunks)?;
			let mut next = state.clone();
			next.repairs.get_mut(&key).expect("repair exists").phase = RepairPhase::Finalizing;
			persist_state(&self.root, &next)?;
			*state = next;
			self.trip_fault(StreamingFault::AfterRepairFinalizingJournal)?;
		}
		if path.exists() {
			fs::rename(&path, &object).map_err(io_error)?;
		}
		verify_file(&object, &installed.descriptor, &installed.chunks)?;
		File::open(&object).and_then(|file| file.sync_all()).map_err(io_error)?;
		sync_dir(path.parent().expect("repair path has parent"))?;
		sync_dir(object.parent().expect("object path has parent"))?;
		self.trip_fault(StreamingFault::AfterRepairRename)?;
		let mut next = state.clone();
		next.quarantine.remove(canonical.as_str());
		next.repairs.get_mut(&key).expect("repair exists").phase = RepairPhase::Installed;
		persist_state(&self.root, &next)?;
		*state = next;
		self.trip_fault(StreamingFault::AfterRepairQuarantineClear)
	}

	fn completed_repair_progress(
		&self,
		cid: &str,
		operation_id: OperationId,
	) -> Result<RepairProgress, ContentError> {
		let state = self.read_state()?;
		if state.quarantine.contains_key(cid) {
			return Err(ContentError::IdempotencyConflict);
		}
		let installed = installed_record_in(&state, cid)?;
		validate_installed_record(&installed)?;
		let key = repair_key(cid, operation_id)?;
		let repair = state.repairs.get(&key).ok_or(ContentError::IdempotencyConflict)?;
		validate_repair_record(&key, repair, &installed)?;
		if repair.phase != RepairPhase::Installed {
			return Err(ContentError::IdempotencyConflict);
		}
		verify_file(&self.object_path(cid), &installed.descriptor, &installed.chunks)?;
		repair_progress(repair, &installed)
	}

	fn recover(&self) -> Result<(), ContentError> {
		let mut state = self.write_state()?;
		if state
			.operations
			.iter()
			.any(|(key, record)| key != &operation_key(&record.descriptor))
		{
			return Err(ContentError::IntegrityFailed)
		}
		validate_install_sequences(&state)?;
		let mut next = state.clone();
		let mut changed = false;
		let installed_cids: BTreeSet<_> = next
			.operations
			.values()
			.filter(|record| record.phase == Phase::Installed)
			.map(|record| record.descriptor.expected_cid.clone())
			.collect();
		let previous_quarantines = next.quarantine.len();
		next.quarantine.retain(|cid, _| installed_cids.contains(cid));
		changed |= previous_quarantines != next.quarantine.len();
		let mut quarantine_sequences = BTreeSet::new();
		for (cid, quarantine) in &next.quarantine {
			let descriptor = next
				.operations
				.values()
				.find(|record| {
					record.phase == Phase::Installed && record.descriptor.expected_cid == *cid
				})
				.map(|record| &record.descriptor)
				.ok_or(ContentError::IntegrityFailed)?;
			if quarantine.expected_bytes != descriptor.object_len ||
				quarantine.detection_sequence == 0 ||
				quarantine.detection_sequence > next.detection_sequence ||
				!quarantine_sequences.insert(quarantine.detection_sequence)
			{
				return Err(ContentError::IntegrityFailed)
			}
		}
		let mut repaired_cids = BTreeSet::new();
		for (key, repair) in &next.repairs {
			if !repaired_cids.insert(repair.cid.clone()) {
				return Err(ContentError::IntegrityFailed);
			}
			let installed = installed_record_for_repair(&next, repair)?;
			validate_installed_record(&installed)?;
			validate_repair_record(key, repair, &installed)?;
			let path = self.repair_path(key);
			match repair.phase {
				RepairPhase::Receiving => {
					if !next.quarantine.contains_key(&repair.cid) {
						return Err(ContentError::IntegrityFailed);
					}
					let file = OpenOptions::new()
						.read(true)
						.write(true)
						.open(&path)
						.map_err(|_| ContentError::IntegrityFailed)?;
					let length = file.metadata().map_err(io_error)?.len();
					if length < repair.staged_bytes {
						return Err(ContentError::IntegrityFailed);
					}
					if length > repair.staged_bytes {
						file.set_len(repair.staged_bytes).map_err(io_error)?;
						file.sync_all().map_err(io_error)?;
					}
					verify_repair_prefix(&path, repair, &installed)?;
				},
				RepairPhase::Finalizing => {
					if !next.quarantine.contains_key(&repair.cid) {
						return Err(ContentError::IntegrityFailed);
					}
					if path.exists() {
						verify_file(&path, &installed.descriptor, &installed.chunks)?;
					} else {
						verify_file(
							&self.object_path(&repair.cid),
							&installed.descriptor,
							&installed.chunks,
						)?;
					}
				},
				RepairPhase::Installed => {
					if next.quarantine.contains_key(&repair.cid) || path.exists() {
						return Err(ContentError::IntegrityFailed);
					}
					verify_file(
						&self.object_path(&repair.cid),
						&installed.descriptor,
						&installed.chunks,
					)?;
				},
			}
		}
		let keys: Vec<_> = next.operations.keys().cloned().collect();
		for key in keys {
			let record = next.operations.get(&key).cloned().expect("key exists");
			match record.phase {
				Phase::Receiving => {
					if record.install_sequence.is_some() {
						return Err(ContentError::IntegrityFailed)
					}
					let path = self.part_path(&key);
					let file = OpenOptions::new()
						.read(true)
						.write(true)
						.open(&path)
						.map_err(|_| ContentError::IntegrityFailed)?;
					let length = file.metadata().map_err(io_error)?.len();
					if length < record.received_bytes {
						return Err(ContentError::IntegrityFailed)
					}
					if length > record.received_bytes {
						file.set_len(record.received_bytes).map_err(io_error)?;
						file.sync_all().map_err(io_error)?;
					}
					verify_persisted_chunks(&path, &record.chunks, record.received_bytes)?;
				},
				Phase::Finalizing => {
					if record.install_sequence.is_some() {
						return Err(ContentError::IntegrityFailed)
					}
					let part = self.part_path(&key);
					let object = self.object_path(&record.descriptor.expected_cid);
					if !object.exists() {
						verify_file(&part, &record.descriptor, &record.chunks)?;
						install_file(&part, &object, &record.descriptor, &record.chunks)?;
					} else {
						verify_file(&object, &record.descriptor, &record.chunks)?;
						if part.exists() {
							fs::remove_file(&part).map_err(io_error)?;
							sync_dir(part.parent().expect("staging path has parent"))?;
						}
					}
					let (_, fingerprint, _) =
						verify_file(&object, &record.descriptor, &record.chunks)?;
					let install_sequence = next.next_install_sequence;
					next.next_install_sequence =
						install_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
					let recovered_receipt =
						receipt(&record.descriptor, &record.descriptor.expected_cid, fingerprint);
					let recovered = next.operations.get_mut(&key).expect("key exists");
					recovered.phase = Phase::Installed;
					recovered.install_sequence = Some(install_sequence);
					recovered.receipt = Some(recovered_receipt.clone());
					if next.recovery.values().any(|item| item.descriptor == record.descriptor) {
						recovery::append_installed_terminal(
							&mut next,
							&key,
							recovered_receipt,
							install_sequence,
						)?;
					}
					changed = true;
				},
				Phase::Installed => {
					validate_installed_record(&record)?;
					let expected_receipt =
						record.receipt.as_ref().expect("validated installed record has a receipt");
					if next.quarantine.contains_key(&record.descriptor.expected_cid) {
						continue
					}
					let path = self.object_path(&record.descriptor.expected_cid);
					match verify_file(&path, &record.descriptor, &record.chunks) {
						Ok((_, fingerprint, length)) => {
							if fingerprint != expected_receipt.fingerprint ||
								expected_receipt.stored_bytes != length
							{
								return Err(ContentError::IntegrityFailed)
							}
						},
						Err(error) => {
							let (reason, observed_bytes) = diagnose_failure(&path, &record, &error);
							changed |= insert_quarantine(
								&mut next,
								&record.descriptor,
								reason,
								observed_bytes,
							)?;
						},
					}
				},
				Phase::Cancelled => {
					if record.install_sequence.is_some() || record.receipt.is_some() {
						return Err(ContentError::IntegrityFailed)
					}
					let path = self.part_path(&key);
					if path.exists() {
						fs::remove_file(&path).map_err(io_error)?;
						sync_dir(path.parent().expect("staging path has parent"))?;
						changed = true;
					}
				},
			}
		}
		let referenced_staging: BTreeSet<_> = next
			.operations
			.iter()
			.filter(|(_, record)| matches!(record.phase, Phase::Receiving | Phase::Finalizing))
			.map(|(key, _)| format!("{key}.part"))
			.chain(next.repairs.iter().filter_map(|(key, repair)| {
				(repair.phase != RepairPhase::Installed).then(|| format!("repair-{key}.part"))
			}))
			.collect();
		changed |= remove_unowned(&self.root.join(STAGING), &referenced_staging)?;
		let referenced_objects: BTreeSet<_> = next
			.operations
			.values()
			.filter(|record| matches!(record.phase, Phase::Finalizing | Phase::Installed))
			.map(|record| record.descriptor.expected_cid.clone())
			.collect();
		changed |= remove_unowned(&self.root.join(OBJECTS), &referenced_objects)?;
		if changed {
			validate_install_sequences(&next)?;
			recovery::validate_recovery_state(&next)?;
			persist_state(&self.root, &next)?;
			*state = next;
		}
		Ok(())
	}

	/// Return immutable installed descriptors in sequence order without admitting their bytes.
	pub(super) fn installation_records(&self) -> Result<Vec<VerifiedInstallation>, ContentError> {
		let state = self.read_state()?;
		validate_install_sequences(&state)?;
		let mut installations = state
			.operations
			.values()
			.filter(|record| record.phase == Phase::Installed)
			.map(|record| {
				validate_installed_record(record)?;
				Ok(VerifiedInstallation {
					install_sequence: record
						.install_sequence
						.ok_or(ContentError::IntegrityFailed)?,
					operation_id: record.descriptor.operation_id,
					bucket_id: record.descriptor.bucket_id,
					cid: CanonicalCid::parse(&record.descriptor.expected_cid)?,
					stored_bytes: record.descriptor.object_len,
				})
			})
			.collect::<Result<Vec<_>, ContentError>>()?;
		installations.sort_by_key(|installation| installation.install_sequence);
		Ok(installations)
	}

	/// Reverify one exact installed operation without accepting a CID-only alias.
	pub(super) fn verified_installation(
		&self,
		bucket_id: BucketId,
		operation_id: OperationId,
	) -> Result<VerifiedInstallation, ContentError> {
		let key = operation_key_parts(bucket_id, operation_id);
		let state = self.read_state()?;
		#[cfg(test)]
		self.exact_record_probes.fetch_add(1, Ordering::Relaxed);
		let record = state.operations.get(&key).cloned().ok_or(ContentError::NotFound)?;
		#[cfg(test)]
		self.exact_quarantine_probes.fetch_add(1, Ordering::Relaxed);
		let quarantined = state.quarantine.contains_key(&record.descriptor.expected_cid);
		drop(state);
		self.verify_installation_record(record, quarantined)
	}

	/// Reverify one replication object and its exact derived install or repair operation.
	pub(crate) fn verified_replication_ready(
		&self,
		bucket_id: BucketId,
		cid: &str,
		object_len: u64,
		install_operation_id: OperationId,
		repair_operation_id: OperationId,
	) -> Result<VerifiedReplicationReady, ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let state = self.read_state()?;
		let install_key = operation_key_parts(bucket_id, install_operation_id);
		let exact_install = state.operations.get(&install_key).cloned().filter(|record| {
			record.phase == Phase::Installed
				&& record.descriptor.bucket_id == bucket_id
				&& record.descriptor.expected_cid == canonical.as_str()
				&& record.descriptor.object_len == object_len
		});
		let (record, repaired) = if let Some(record) = exact_install {
			(record, false)
		} else {
			let record = state
				.operations
				.values()
				.find(|record| {
					record.phase == Phase::Installed
						&& record.descriptor.bucket_id == bucket_id
						&& record.descriptor.expected_cid == canonical.as_str()
						&& record.descriptor.object_len == object_len
				})
				.cloned()
				.ok_or(ContentError::NotFound)?;
			let key = repair_key(canonical.as_str(), repair_operation_id)?;
			let repair = state.repairs.get(&key).ok_or(ContentError::IntegrityFailed)?;
			validate_repair_record(&key, repair, &record)?;
			if repair.phase != RepairPhase::Installed {
				return Err(ContentError::IntegrityFailed);
			}
			(record, true)
		};
		if state.quarantine.contains_key(canonical.as_str()) {
			return Err(ContentError::IntegrityFailed);
		}
		let receipt = record.receipt.clone().ok_or(ContentError::IntegrityFailed)?;
		drop(state);
		let verified = self.verify_installation_record(record, false)?;
		if verified.bucket_id != bucket_id
			|| verified.cid.as_str() != canonical.as_str()
			|| verified.stored_bytes != object_len
			|| (!repaired && verified.operation_id != install_operation_id)
		{
			return Err(ContentError::IntegrityFailed);
		}
		Ok(VerifiedReplicationReady {
			receipt_fingerprint: decode_chunk_hash(&receipt.fingerprint)?,
		})
	}

	/// Classify exact replication ingress without creating an install or repair journal entry.
	pub(crate) fn classify_replication_ingress(
		&self,
		bucket_id: BucketId,
		cid: &str,
		object_len: u64,
		install_operation_id: OperationId,
		repair_operation_id: OperationId,
	) -> Result<ReplicationIngressState, ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let state = self.read_state()?;
		let install_key = operation_key_parts(bucket_id, install_operation_id);
		if let Some(exact) = state.operations.get(&install_key).cloned() {
			if exact.descriptor.bucket_id != bucket_id
				|| exact.descriptor.expected_cid != canonical.as_str()
				|| exact.descriptor.object_len != object_len
			{
				return Err(ContentError::IdempotencyConflict);
			}
			return match exact.phase {
				Phase::Receiving => Ok(ReplicationIngressState::Fresh),
				Phase::Installed if state.quarantine.contains_key(canonical.as_str()) => {
					validate_replication_repair_identity(
						&state,
						canonical.as_str(),
						repair_operation_id,
					)?;
					Ok(ReplicationIngressState::Repair)
				},
				Phase::Installed => {
					drop(state);
					self.verify_installation_record(exact, false)?;
					Ok(ReplicationIngressState::ExactReady)
				},
				_ => Err(ContentError::IntegrityFailed),
			};
		}
		let matching = state
			.operations
			.values()
			.filter(|record| {
				record.phase == Phase::Installed
					&& record.descriptor.bucket_id == bucket_id
					&& record.descriptor.expected_cid == canonical.as_str()
			})
			.cloned()
			.collect::<Vec<_>>();
		if matching.iter().any(|record| record.descriptor.object_len != object_len)
			|| matching
				.first()
				.is_some_and(|first| matching.iter().any(|record| record.chunks != first.chunks))
		{
			return Err(ContentError::IdempotencyConflict);
		}
		let Some(_) = matching.first() else {
			return Ok(ReplicationIngressState::Fresh);
		};
		let exact_repair_key = repair_key(canonical.as_str(), repair_operation_id)?;
		if let Some(repair) = state.repairs.get(&exact_repair_key) {
			validate_repair_record(
				&exact_repair_key,
				repair,
				matching.first().expect("matching record exists"),
			)?;
			if repair.phase == RepairPhase::Installed {
				if state.quarantine.contains_key(canonical.as_str()) {
					return Err(ContentError::IntegrityFailed);
				}
				drop(state);
				for record in matching {
					self.verify_installation_record(record, false)?;
				}
				return Ok(ReplicationIngressState::ExactReady);
			}
		}
		if state.quarantine.contains_key(canonical.as_str()) {
			validate_replication_repair_identity(&state, canonical.as_str(), repair_operation_id)?;
			return Ok(ReplicationIngressState::Repair);
		}
		drop(state);
		for record in matching {
			self.verify_installation_record(record, false)?;
		}
		Ok(ReplicationIngressState::Fresh)
	}

	/// Retire a completed shared-CID repair after the exact derived logical install is durable.
	pub(crate) fn retire_completed_replication_repair(
		&self,
		bucket_id: BucketId,
		cid: &str,
		repair_operation_id: OperationId,
		install_operation_id: OperationId,
	) -> Result<(), ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let mut state = self.write_state()?;
		let install = state
			.operations
			.get(&operation_key_parts(bucket_id, install_operation_id))
			.cloned()
			.ok_or(ContentError::NotFound)?;
		if install.phase != Phase::Installed
			|| install.descriptor.expected_cid != canonical.as_str()
			|| state.quarantine.contains_key(canonical.as_str())
		{
			return Err(ContentError::IdempotencyConflict);
		}
		let key = repair_key(canonical.as_str(), repair_operation_id)?;
		let Some(repair) = state.repairs.get(&key) else {
			return Ok(());
		};
		if repair.operation_id != repair_operation_id
			|| repair.cid != canonical.as_str()
			|| repair.phase != RepairPhase::Installed
		{
			return Err(ContentError::IdempotencyConflict);
		}
		let mut next = state.clone();
		next.repairs.remove(&key);
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(())
	}

	/// Return one exact replication descriptor and manifest only after full-file verification.
	pub(crate) fn verified_replication_object(
		&self,
		bucket_id: BucketId,
		operation_id: OperationId,
	) -> Result<VerifiedReplicationObject, ContentError> {
		let key = operation_key_parts(bucket_id, operation_id);
		let state = self.read_state()?;
		let record = state.operations.get(&key).cloned().ok_or(ContentError::NotFound)?;
		let quarantined = state.quarantine.contains_key(&record.descriptor.expected_cid);
		drop(state);
		let installation = self.verify_installation_record(record.clone(), quarantined)?;
		let chunk_hashes = record
			.chunks
			.iter()
			.map(|chunk| decode_chunk_hash(&chunk.hash))
			.collect::<Result<Vec<_>, _>>()?;
		if chunk_hashes.len() > MAX_CHUNKS {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(VerifiedReplicationObject {
			operation_id: installation.operation_id,
			bucket_id: installation.bucket_id,
			cid: installation.cid,
			stored_bytes: installation.stored_bytes,
			chunk_hashes,
		})
	}

	fn verify_installation_record(
		&self,
		record: OperationRecord,
		quarantined: bool,
	) -> Result<VerifiedInstallation, ContentError> {
		validate_installed_record(&record)?;
		if quarantined {
			return Err(ContentError::IntegrityFailed)
		}
		let receipt = record.receipt.as_ref().ok_or(ContentError::IntegrityFailed)?;
		let (cid, fingerprint, length) = verify_file(
			&self.object_path(&record.descriptor.expected_cid),
			&record.descriptor,
			&record.chunks,
		)
		.map_err(|_| ContentError::IntegrityFailed)?;
		if fingerprint != receipt.fingerprint || length != receipt.stored_bytes {
			return Err(ContentError::IntegrityFailed)
		}
		Ok(VerifiedInstallation {
			install_sequence: record.install_sequence.ok_or(ContentError::IntegrityFailed)?,
			operation_id: record.descriptor.operation_id,
			bucket_id: record.descriptor.bucket_id,
			cid,
			stored_bytes: length,
		})
	}

	fn installed_record(&self, cid: &str) -> Result<OperationRecord, ContentError> {
		self.read_state()?
			.operations
			.values()
			.find(|record| {
				record.phase == Phase::Installed && record.descriptor.expected_cid == cid
			})
			.cloned()
			.ok_or(ContentError::NotFound)
	}

	fn installed_records(&self) -> Result<BTreeMap<String, OperationRecord>, ContentError> {
		Ok(self
			.read_state()?
			.operations
			.values()
			.filter(|record| record.phase == Phase::Installed)
			.map(|record| (record.descriptor.expected_cid.clone(), record.clone()))
			.collect())
	}

	fn reject_quarantined(&self, cid: &str) -> Result<(), ContentError> {
		if self.is_quarantined(cid)? {
			Err(ContentError::IntegrityFailed)
		} else {
			Ok(())
		}
	}

	fn is_quarantined(&self, cid: &str) -> Result<bool, ContentError> {
		Ok(self.read_state()?.quarantine.contains_key(cid))
	}

	fn quarantine_failure(
		&self,
		cid: &str,
		record: &OperationRecord,
		path: &Path,
		error: &ContentError,
	) -> Result<(), ContentError> {
		let (reason, observed_bytes) = diagnose_failure(path, record, error);
		let mut state = self.write_state()?;
		if state.quarantine.contains_key(cid) {
			return Ok(())
		}
		if !state.operations.values().any(|candidate| {
			candidate.phase == Phase::Installed && candidate.descriptor.expected_cid == cid
		}) {
			return Err(ContentError::NotFound)
		}
		let mut next = state.clone();
		insert_quarantine(&mut next, &record.descriptor, reason, observed_bytes)?;
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(())
	}

	fn progress(&self, record: &OperationRecord) -> Result<ProgressAck, ContentError> {
		let window = self.window.window.lock().map_err(|_| lock_error())?;
		Ok(ProgressAck {
			next_chunk: record.next_chunk,
			persisted_chunks: record.next_chunk,
			persisted_bytes: record.received_bytes,
			available_window_chunks: INGRESS_WINDOW_CHUNKS.saturating_sub(window.chunks),
			available_window_bytes: INGRESS_WINDOW_BYTES.saturating_sub(window.bytes),
		})
	}

	fn trip_fault(&self, point: StreamingFault) -> Result<(), ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			return Err(ContentError::Io(format!("injected streaming fault: {point:?}")))
		}
		Ok(())
	}

	fn part_path(&self, key: &str) -> PathBuf {
		self.root.join(STAGING).join(format!("{key}.part"))
	}

	fn object_path(&self, cid: &str) -> PathBuf {
		self.root.join(OBJECTS).join(cid)
	}

	fn repair_path(&self, cid: &str) -> PathBuf {
		self.root.join(STAGING).join(format!("repair-{cid}.part"))
	}

	fn persist(&self) -> Result<(), ContentError> {
		let state = self.read_state()?;
		persist_state(&self.root, &state)
	}

	fn read_state(&self) -> Result<std::sync::RwLockReadGuard<'_, JournalState>, ContentError> {
		self.state.read().map_err(|_| lock_error())
	}

	fn write_state(&self) -> Result<std::sync::RwLockWriteGuard<'_, JournalState>, ContentError> {
		self.state.write().map_err(|_| lock_error())
	}
}

fn integrity_summary(state: &JournalState) -> IntegritySummary {
	let installed: BTreeSet<_> = state
		.operations
		.values()
		.filter(|record| record.phase == Phase::Installed)
		.map(|record| record.descriptor.expected_cid.as_str())
		.collect();
	let installed_objects = installed.len() as u64;
	let quarantined_objects =
		state.quarantine.keys().filter(|cid| installed.contains(cid.as_str())).count() as u64;
	let ready_objects = installed_objects.saturating_sub(quarantined_objects);
	IntegritySummary {
		installed_objects,
		ready_objects,
		quarantined_objects,
		ready: quarantined_objects == 0,
		last_detection_sequence: state.detection_sequence,
	}
}

fn insert_quarantine(
	state: &mut JournalState,
	descriptor: &StreamingDescriptor,
	reason: QuarantineReason,
	observed_bytes: Option<u64>,
) -> Result<bool, ContentError> {
	if state.quarantine.contains_key(&descriptor.expected_cid) {
		return Ok(false)
	}
	state.detection_sequence =
		state.detection_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
	state.quarantine.insert(
		descriptor.expected_cid.clone(),
		QuarantineRecord {
			reason,
			expected_bytes: descriptor.object_len,
			observed_bytes,
			detection_sequence: state.detection_sequence,
		},
	);
	state.repairs.retain(|_, repair| {
		repair.cid != descriptor.expected_cid || repair.phase != RepairPhase::Installed
	});
	Ok(true)
}

fn installed_record_in(state: &JournalState, cid: &str) -> Result<OperationRecord, ContentError> {
	state
		.operations
		.values()
		.find(|record| record.phase == Phase::Installed && record.descriptor.expected_cid == cid)
		.cloned()
		.ok_or(ContentError::NotFound)
}

fn repair_key(cid: &str, operation_id: OperationId) -> Result<String, ContentError> {
	let canonical = CanonicalCid::parse(cid)?;
	let mut hash = Blake2b::<U32>::new();
	hash.update(b"origin/quarantined-object-repair/v1");
	hash.update(canonical.digest());
	hash.update(operation_id.as_bytes());
	Ok(hex::encode(hash.finalize()))
}

fn validate_replication_repair_identity(
	state: &JournalState,
	cid: &str,
	operation_id: OperationId,
) -> Result<(), ContentError> {
	if state
		.repairs
		.values()
		.any(|repair| repair.cid == cid && repair.operation_id != operation_id)
	{
		Err(ContentError::IdempotencyConflict)
	} else {
		Ok(())
	}
}

fn installed_record_for_repair(
	state: &JournalState,
	repair: &RepairRecord,
) -> Result<OperationRecord, ContentError> {
	state
		.operations
		.values()
		.find(|record| {
			record.phase == Phase::Installed
				&& record.descriptor.expected_cid == repair.cid
				&& repair_descriptor_hash(&record.descriptor) == repair.descriptor_hash
		})
		.cloned()
		.ok_or(ContentError::IntegrityFailed)
}

fn full_repair_operation(cid: &str) -> OperationId {
	let canonical = CanonicalCid::parse(cid).expect("repair operation requires canonical CID");
	let mut hash = Blake2b::<U32>::new();
	hash.update(b"origin/full-reader-repair/v1");
	hash.update(canonical.digest());
	let digest: [u8; 32] = hash.finalize().into();
	let mut operation = [0u8; 16];
	operation.copy_from_slice(&digest[..16]);
	OperationId::from_bytes(operation)
}

fn repair_descriptor_hash(descriptor: &StreamingDescriptor) -> String {
	let canonical =
		CanonicalCid::parse(&descriptor.expected_cid).expect("installed descriptor is canonical");
	let mut hash = Blake2b::<U32>::new();
	hash.update(b"origin/repair-descriptor/v1");
	hash.update(descriptor.operation_id.as_bytes());
	hash.update(descriptor.bucket_id.as_bytes());
	hash.update(canonical.digest());
	hash.update(descriptor.object_len.to_le_bytes());
	format!("0x{}", hex::encode(hash.finalize()))
}

fn repair_manifest_hash(chunks: &[ChunkRecord]) -> String {
	let mut hash = Blake2b::<U32>::new();
	hash.update(b"origin/repair-manifest/v1");
	hash.update((chunks.len() as u64).to_le_bytes());
	for (index, chunk) in chunks.iter().enumerate() {
		hash.update((index as u16).to_le_bytes());
		hash.update(chunk.length.to_le_bytes());
		hash.update(decode_chunk_hash(&chunk.hash).expect("installed manifest is valid"));
	}
	format!("0x{}", hex::encode(hash.finalize()))
}

fn validate_repair_record(
	key: &str,
	repair: &RepairRecord,
	installed: &OperationRecord,
) -> Result<(), ContentError> {
	if repair_key(&repair.cid, repair.operation_id).map_err(|_| ContentError::IntegrityFailed)?
		!= key || repair.cid != installed.descriptor.expected_cid
		|| repair.descriptor_hash != repair_descriptor_hash(&installed.descriptor)
		|| repair.manifest_hash != repair_manifest_hash(&installed.chunks)
		|| repair.next_chunk as usize > installed.chunks.len()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let staged_bytes = installed
		.chunks
		.iter()
		.take(repair.next_chunk as usize)
		.try_fold(0u64, |total, chunk| total.checked_add(chunk.length as u64))
		.ok_or(ContentError::IntegrityFailed)?;
	if repair.staged_bytes != staged_bytes
		|| (matches!(repair.phase, RepairPhase::Finalizing | RepairPhase::Installed)
			&& repair.next_chunk as usize != installed.chunks.len())
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn verify_repair_prefix(
	path: &Path,
	repair: &RepairRecord,
	installed: &OperationRecord,
) -> Result<(), ContentError> {
	let mut file = File::open(path).map_err(|_| ContentError::IntegrityFailed)?;
	for expected in installed.chunks.iter().take(repair.next_chunk as usize) {
		let mut bytes = vec![0; expected.length as usize];
		file.read_exact(&mut bytes).map_err(|_| ContentError::IntegrityFailed)?;
		if chunk_hash(&bytes) != expected.hash {
			return Err(ContentError::IntegrityFailed);
		}
	}
	if file.metadata().map_err(io_error)?.len() != repair.staged_bytes {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn repair_progress(
	repair: &RepairRecord,
	installed: &OperationRecord,
) -> Result<RepairProgress, ContentError> {
	Ok(RepairProgress {
		next_chunk: repair.next_chunk,
		persisted_chunks: repair.next_chunk,
		persisted_bytes: repair.staged_bytes,
		ready_to_finalize: repair.next_chunk as usize == installed.chunks.len(),
	})
}

fn diagnose_failure(
	path: &Path,
	record: &OperationRecord,
	original: &ContentError,
) -> (QuarantineReason, Option<u64>) {
	let metadata = match fs::metadata(path) {
		Ok(metadata) => metadata,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound =>
			return (QuarantineReason::Missing, None),
		Err(_) => return (QuarantineReason::Unreadable, None),
	};
	let observed = Some(metadata.len());
	if metadata.len() != record.descriptor.object_len {
		return (QuarantineReason::LengthMismatch, observed)
	}
	let mut file = match File::open(path) {
		Ok(file) => file,
		Err(_) => return (QuarantineReason::Unreadable, observed),
	};
	let mut content = Blake2b::<U32>::new();
	for chunk in &record.chunks {
		let mut bytes = vec![0; chunk.length as usize];
		if file.read_exact(&mut bytes).is_err() {
			return (QuarantineReason::Unreadable, observed)
		}
		if chunk_hash(&bytes) != chunk.hash {
			return (QuarantineReason::ChunkMismatch, observed)
		}
		content.update(&bytes);
	}
	let mut trailing = [0u8; 1];
	if file.read(&mut trailing).ok() != Some(0) {
		return (QuarantineReason::LengthMismatch, observed)
	}
	let cid = CanonicalCid::from_digest(content.finalize().into());
	if cid.as_str() != record.descriptor.expected_cid {
		return (QuarantineReason::FullCidMismatch, observed)
	}
	match original {
		ContentError::LengthMismatch => (QuarantineReason::LengthMismatch, observed),
		ContentError::CidMismatch => (QuarantineReason::FullCidMismatch, observed),
		_ => (QuarantineReason::Unreadable, observed),
	}
}

fn remove_repair_if_present(path: &Path) -> Result<(), ContentError> {
	if path.exists() {
		fs::remove_file(path).map_err(io_error)?;
		sync_dir(path.parent().expect("repair path has parent"))?;
	}
	Ok(())
}

fn expected_receipt(record: &OperationRecord) -> Result<StreamingReceipt, ContentError> {
	Ok(receipt(
		&record.descriptor,
		&record.descriptor.expected_cid,
		fingerprint_from_chunks(&record.descriptor, &record.chunks)?,
	))
}

fn validate_installed_record(record: &OperationRecord) -> Result<(), ContentError> {
	if record.phase != Phase::Installed ||
		record.install_sequence.is_none() ||
		validate_descriptor(&record.descriptor).is_err() ||
		record.received_bytes != record.descriptor.object_len
	{
		return Err(ContentError::IntegrityFailed)
	}
	let expected_chunks = chunk_count(record.descriptor.object_len)?;
	if record.next_chunk as usize != expected_chunks || record.chunks.len() != expected_chunks {
		return Err(ContentError::IntegrityFailed)
	}
	for (index, chunk) in record.chunks.iter().enumerate() {
		let index: u16 = index.try_into().map_err(|_| ContentError::IntegrityFailed)?;
		if chunk.length as usize != expected_chunk_len(record.descriptor.object_len, index)? {
			return Err(ContentError::IntegrityFailed)
		}
		decode_chunk_hash(&chunk.hash)?;
	}
	let installed_receipt = record.receipt.as_ref().ok_or(ContentError::IntegrityFailed)?;
	if installed_receipt != &expected_receipt(record)? {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(())
}

fn validate_install_sequences(state: &JournalState) -> Result<(), ContentError> {
	let mut sequences = BTreeSet::new();
	for record in state.operations.values() {
		match (record.phase, record.install_sequence) {
			(Phase::Installed, Some(sequence)) if sequences.insert(sequence) => {},
			(Phase::Installed, _) => return Err(ContentError::IntegrityFailed),
			(_, None) => {},
			(_, Some(_)) => return Err(ContentError::IntegrityFailed),
		}
	}
	if state.next_install_sequence != sequences.len() as u64 ||
		sequences.iter().copied().ne(0..state.next_install_sequence)
	{
		return Err(ContentError::IntegrityFailed)
	}
	Ok(())
}

fn validate_descriptor(descriptor: &StreamingDescriptor) -> Result<(), ContentError> {
	CanonicalCid::parse(&descriptor.expected_cid)?;
	chunk_count(descriptor.object_len)?;
	Ok(())
}

fn chunk_count(length: u64) -> Result<usize, ContentError> {
	if length > MAX_STORED_BYTES {
		return Err(ContentError::ObjectTooLarge)
	}
	let chunks = if length == 0 { 0 } else { length.div_ceil(CHUNK_BYTES as u64) as usize };
	if chunks > MAX_CHUNKS {
		return Err(ContentError::ObjectTooLarge)
	}
	Ok(chunks)
}

fn expected_chunk_len(length: u64, index: u16) -> Result<usize, ContentError> {
	let chunks = chunk_count(length)?;
	let index = index as usize;
	if index >= chunks {
		return Err(ContentError::ChunkOutOfOrder)
	}
	let start = index as u64 * CHUNK_BYTES as u64;
	Ok((length - start).min(CHUNK_BYTES as u64) as usize)
}

fn validate_chunk_len(length: u64, index: u16, supplied: usize) -> Result<(), ContentError> {
	if supplied > CHUNK_BYTES {
		return Err(ContentError::ChunkTooLarge)
	}
	if supplied != expected_chunk_len(length, index)? {
		return Err(ContentError::LengthMismatch)
	}
	Ok(())
}

fn operation_key(descriptor: &StreamingDescriptor) -> String {
	operation_key_parts(descriptor.bucket_id, descriptor.operation_id)
}

fn operation_key_parts(bucket_id: BucketId, operation_id: OperationId) -> String {
	let mut hash = Blake2b::<U32>::new();
	hash.update(b"origin/streaming-operation/v1");
	hash.update(bucket_id.as_bytes());
	hash.update(operation_id.as_bytes());
	hex::encode(hash.finalize())
}

fn new_fingerprint(descriptor: &StreamingDescriptor) -> Blake2b<U32> {
	let mut hash = Blake2b::<U32>::new();
	hash.update(b"origin/streaming-content-receipt/v2");
	hash.update(descriptor.bucket_id.as_bytes());
	hash.update(descriptor.operation_id.as_bytes());
	hash.update(CanonicalCid::parse(&descriptor.expected_cid).expect("validated CID").digest());
	hash.update(descriptor.object_len.to_le_bytes());
	hash
}

fn fingerprint_from_chunks(
	descriptor: &StreamingDescriptor,
	chunks: &[ChunkRecord],
) -> Result<String, ContentError> {
	let mut fingerprint = new_fingerprint(descriptor);
	for (index, chunk) in chunks.iter().enumerate() {
		let index: u16 = index.try_into().map_err(|_| ContentError::IntegrityFailed)?;
		let hash = decode_chunk_hash(&chunk.hash)?;
		fingerprint.update(index.to_le_bytes());
		fingerprint.update(chunk.length.to_le_bytes());
		fingerprint.update(hash);
	}
	Ok(format!("0x{}", hex::encode(fingerprint.finalize())))
}

fn decode_chunk_hash(value: &str) -> Result<[u8; 32], ContentError> {
	let encoded = value.strip_prefix("0x").ok_or(ContentError::IntegrityFailed)?;
	if encoded.len() != 64 || encoded.bytes().any(|byte| byte.is_ascii_uppercase()) {
		return Err(ContentError::IntegrityFailed)
	}
	let hash: [u8; 32] = hex::decode(encoded)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)?;
	if value != format!("0x{}", hex::encode(hash)) {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(hash)
}

fn chunk_hash(bytes: &[u8]) -> String {
	format!("0x{}", hex::encode(Blake2b::<U32>::digest(bytes)))
}

fn verify_file(
	path: &Path,
	descriptor: &StreamingDescriptor,
	chunks: &[ChunkRecord],
) -> Result<(CanonicalCid, String, u64), ContentError> {
	let (_, cid, fingerprint, length) = verify_open_file(path, descriptor, chunks)?;
	Ok((cid, fingerprint, length))
}

fn verify_open_file(
	path: &Path,
	descriptor: &StreamingDescriptor,
	chunks: &[ChunkRecord],
) -> Result<(File, CanonicalCid, String, u64), ContentError> {
	if chunks.len() != chunk_count(descriptor.object_len)? {
		return Err(ContentError::IntegrityFailed)
	}
	let mut file = File::open(path).map_err(|_| ContentError::IntegrityFailed)?;
	let mut content = Blake2b::<U32>::new();
	let mut length = 0u64;
	for (index, expected) in chunks.iter().enumerate() {
		if expected.length as usize != expected_chunk_len(descriptor.object_len, index as u16)? {
			return Err(ContentError::IntegrityFailed)
		}
		let mut bytes = vec![0u8; expected.length as usize];
		file.read_exact(&mut bytes).map_err(|_| ContentError::IntegrityFailed)?;
		if chunk_hash(&bytes) != expected.hash {
			return Err(ContentError::IntegrityFailed)
		}
		content.update(&bytes);
		length += bytes.len() as u64;
	}
	let mut trailing = [0u8; 1];
	if file.read(&mut trailing).map_err(|_| ContentError::IntegrityFailed)? != 0 {
		return Err(ContentError::LengthMismatch)
	}
	if length != descriptor.object_len {
		return Err(ContentError::LengthMismatch)
	}
	let cid = CanonicalCid::from_digest(content.finalize().into());
	if cid.as_str() != descriptor.expected_cid {
		return Err(ContentError::CidMismatch)
	}
	Ok((file, cid, fingerprint_from_chunks(descriptor, chunks)?, length))
}

fn verify_persisted_chunks(
	path: &Path,
	chunks: &[ChunkRecord],
	received_bytes: u64,
) -> Result<(), ContentError> {
	let mut file = File::open(path).map_err(|_| ContentError::IntegrityFailed)?;
	let mut total = 0u64;
	for (index, expected) in chunks.iter().enumerate() {
		let bytes = read_chunk_bytes(&mut file, index as u16, expected.length as usize)?;
		if chunk_hash(&bytes) != expected.hash {
			return Err(ContentError::IntegrityFailed)
		}
		total += bytes.len() as u64;
	}
	if total != received_bytes {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(())
}

fn read_chunk_bytes(file: &mut File, index: u16, length: usize) -> Result<Vec<u8>, ContentError> {
	file.seek(SeekFrom::Start(index as u64 * CHUNK_BYTES as u64))
		.map_err(|_| ContentError::IntegrityFailed)?;
	let mut bytes = vec![0; length];
	file.read_exact(&mut bytes).map_err(|_| ContentError::IntegrityFailed)?;
	Ok(bytes)
}

fn read_verified_chunk(
	file: &mut File,
	index: u16,
	expected: &ChunkRecord,
) -> Result<Vec<u8>, ContentError> {
	let bytes = read_chunk_bytes(file, index, expected.length as usize)?;
	if chunk_hash(&bytes) != expected.hash {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(bytes)
}

fn verify_chunk_stream<I>(
	descriptor: &StreamingDescriptor,
	chunks: I,
) -> Result<(CanonicalCid, String, u64, Vec<ChunkRecord>), ContentError>
where
	I: IntoIterator<Item = Vec<u8>>,
{
	let mut content = Blake2b::<U32>::new();
	let mut length = 0u64;
	let mut records = Vec::new();
	for (index, chunk) in chunks.into_iter().enumerate() {
		let index: u16 = index.try_into().map_err(|_| ContentError::ObjectTooLarge)?;
		validate_chunk_len(descriptor.object_len, index, chunk.len())?;
		content.update(&chunk);
		length += chunk.len() as u64;
		records.push(ChunkRecord { length: chunk.len() as u32, hash: chunk_hash(&chunk) });
	}
	if length != descriptor.object_len || records.len() != chunk_count(descriptor.object_len)? {
		return Err(ContentError::ChunkMissing)
	}
	let cid = CanonicalCid::from_digest(content.finalize().into());
	if cid.as_str() != descriptor.expected_cid {
		return Err(ContentError::CidMismatch)
	}
	let fingerprint = fingerprint_from_chunks(descriptor, &records)?;
	Ok((cid, fingerprint, length, records))
}

fn receipt(descriptor: &StreamingDescriptor, cid: &str, fingerprint: String) -> StreamingReceipt {
	StreamingReceipt {
		operation_id: descriptor.operation_id,
		bucket_id: descriptor.bucket_id,
		cid: cid.into(),
		stored_bytes: descriptor.object_len,
		chunks: chunk_count(descriptor.object_len).expect("validated descriptor") as u16,
		fingerprint,
		locally_installed: true,
	}
}

fn install_file(
	part: &Path,
	object: &Path,
	descriptor: &StreamingDescriptor,
	chunks: &[ChunkRecord],
) -> Result<(), ContentError> {
	let staging = part.parent().expect("staging path has parent");
	let objects = object.parent().expect("object path has parent");
	if object.exists() {
		verify_file(object, descriptor, chunks)?;
		if part.exists() {
			fs::remove_file(part).map_err(io_error)?;
		}
	} else {
		fs::rename(part, object).map_err(io_error)?;
	}
	File::open(object).and_then(|file| file.sync_all()).map_err(io_error)?;
	sync_dir(staging)?;
	sync_dir(objects)
}

fn remove_unowned(directory: &Path, owned: &BTreeSet<String>) -> Result<bool, ContentError> {
	let mut changed = false;
	for entry in fs::read_dir(directory).map_err(io_error)? {
		let entry = entry.map_err(io_error)?;
		let name = entry.file_name().to_string_lossy().into_owned();
		if !owned.contains(&name) {
			fs::remove_file(entry.path()).map_err(io_error)?;
			changed = true;
		}
	}
	if changed {
		sync_dir(directory)?;
	}
	Ok(changed)
}

fn persist_state(root: &Path, state: &JournalState) -> Result<(), ContentError> {
	let bytes = serde_json::to_vec_pretty(state).map_err(io_error)?;
	let path = root.join(JOURNAL);
	let temporary = root.join(format!("{JOURNAL}.tmp-{}", std::process::id()));
	let mut file = File::create(&temporary).map_err(io_error)?;
	file.write_all(&bytes).map_err(io_error)?;
	file.sync_all().map_err(io_error)?;
	fs::rename(temporary, path).map_err(io_error)?;
	sync_dir(root)
}

fn sync_dir(path: &Path) -> Result<(), ContentError> {
	File::open(path).and_then(|directory| directory.sync_all()).map_err(io_error)
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

fn lock_error() -> ContentError {
	ContentError::Io("stream state lock poisoned".into())
}

#[cfg(test)]
mod exact_lookup_tests {
	use super::*;
	use crate::{capability::ProviderCapabilityV1, CapabilityAuthoritySnapshot};
	use orbis_storage_runtime_api::{
		AgreementInfo, AgreementStatus, BucketGrantInfo, BucketRole, ControlBucketInfo,
		HostDelegationInfo,
	};
	use recovery::ObjectPutRequestV2;
	use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};

	fn recovery_fixture(
	) -> (ObjectPutRequestV2, ProviderCapabilityV1, CapabilityAuthoritySnapshot, ed25519::Pair) {
		let host = ed25519::Pair::from_seed(&[9; 32]);
		let service = ed25519::Pair::from_seed(&[7; 32]);
		let cid = CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(b"first"));
		let request = ObjectPutRequestV2 {
			request_id: [1; 16],
			product_id: "festival".into(),
			grant_id: [3; 32],
			operation_id: [4; 16],
			trace_context: None,
			deadline: 120,
			bucket_id: [5; 32],
			cid: cid.clone(),
			object_len: 5,
			mode: 0,
		};
		let mut capability = ProviderCapabilityV1 {
			version: 1,
			registry_sha256: crate::capability::NORMATIVE_REGISTRY_SHA256,
			genesis_hash: [2; 32],
			grant_id: [3; 32],
			issuer_key_id: [6; 32],
			product_id: "festival".into(),
			bucket_id: [5; 32],
			agreement_id: Some([8; 32]),
			provider: [7; 32],
			methods: vec![1010],
			cid: Some(cid),
			max_bytes: 5,
			issued_at: 100,
			expires_at: 128,
			nonce: [9; 16],
			signature: [0; 64],
		};
		capability.signature = sp_core::Pair::sign(&host, &capability.signed_preimage()).0;
		let local = AccountId32::new([7; 32]);
		let snapshot = CapabilityAuthoritySnapshot {
			finalized_hash: format!("0x{}", "0a".repeat(32)),
			finalized_number: 110,
			genesis_hash: [2; 32],
			registry_sha256: crate::capability::NORMATIVE_REGISTRY_SHA256,
			local_provider: [7; 32],
			delegation: HostDelegationInfo {
				grant_id: H256([3; 32]),
				bucket_id: H256([5; 32]),
				owner: AccountId32::new([1; 32]),
				issuance_nonce: 0,
				issuer_key_id: H256([6; 32]),
				issuer_public_key: host.public().0,
				key_version: 1,
				state_version: 1,
				key_activated_at: 90,
				product_id: b"festival".to_vec(),
				methods: vec![1010],
				cid: Some(request.cid.as_str().as_bytes().to_vec()),
				max_bytes: 5,
				issued_at: 90,
				expires_at: 200,
				revoked_at: None,
			},
			bucket: ControlBucketInfo {
				bucket_id: H256([5; 32]),
				owner: AccountId32::new([1; 32]),
				version: 1,
				policy: H256([1; 32]),
				primary: local.clone(),
				replicas: vec![],
				grants: vec![BucketGrantInfo {
					account: AccountId32::new([1; 32]),
					role: BucketRole::Admin,
				}],
				created_at: 1,
			},
			agreement: Some(AgreementInfo {
				agreement_id: H256([8; 32]),
				owner: AccountId32::new([1; 32]),
				bucket_id: H256([5; 32]),
				primary: local,
				replicas: vec![],
				bytes: 5,
				created_at: 90,
				expires_at: 180,
				release_at: None,
				state_version: 1,
				status: AgreementStatus::Active,
			}),
		};
		(request, capability, snapshot, service)
	}

	#[test]
	fn exact_verified_lookup_does_not_scan_operations_or_quarantine() {
		let temp = tempfile::tempdir().unwrap();
		let store = StreamingStore::open(temp.path()).unwrap();
		let bucket_id = BucketId::from_bytes([1; 32]);
		let operation_id = OperationId::from_bytes([1; 16]);
		let bytes = b"exact";
		let cid = CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(bytes));
		store
			.put_chunks(
				StreamingDescriptor {
					operation_id,
					bucket_id,
					expected_cid: cid.as_str().into(),
					object_len: bytes.len() as u64,
				},
				[bytes.to_vec()],
			)
			.unwrap();
		{
			let mut state = store.write_state().unwrap();
			let template = state
				.operations
				.get(&operation_key_parts(bucket_id, operation_id))
				.unwrap()
				.clone();
			for value in 2u64..=1_025 {
				let mut operation = [0u8; 16];
				operation[..8].copy_from_slice(&value.to_le_bytes());
				let operation = OperationId::from_bytes(operation);
				let mut unrelated = template.clone();
				unrelated.descriptor.operation_id = operation;
				unrelated.phase = Phase::Receiving;
				unrelated.install_sequence = Some(value);
				unrelated.receipt = None;
				state.operations.insert(operation_key_parts(bucket_id, operation), unrelated);
				state.quarantine.insert(
					format!("unrelated-{value}"),
					QuarantineRecord {
						reason: QuarantineReason::Missing,
						expected_bytes: 1,
						observed_bytes: None,
						detection_sequence: value,
					},
				);
			}
		}
		let verified = store.verified_installation(bucket_id, operation_id).unwrap();
		assert_eq!(verified.install_sequence, 0);
		assert_eq!(store.exact_record_probes.load(Ordering::Relaxed), 1);
		assert_eq!(store.exact_quarantine_probes.load(Ordering::Relaxed), 1);
	}

	#[test]
	fn replication_descriptor_returns_max_decoded_manifest_and_exact_verified_chunk() {
		let temp = tempfile::tempdir().unwrap();
		let store = StreamingStore::open(temp.path()).unwrap();
		let bucket_id = BucketId::from_bytes([51; 32]);
		let operation_id = OperationId::from_bytes([52; 16]);
		let chunk = vec![53; CHUNK_BYTES];
		let chunk_digest: [u8; 32] = Blake2b::<U32>::digest(&chunk).into();
		let mut content = Blake2b::<U32>::new();
		for _ in 0..MAX_CHUNKS {
			content.update(&chunk);
		}
		let cid = CanonicalCid::from_digest(content.finalize().into());
		store
			.put_chunks(
				StreamingDescriptor {
					operation_id,
					bucket_id,
					expected_cid: cid.as_str().into(),
					object_len: MAX_STORED_BYTES,
				},
				(0..MAX_CHUNKS).map(|_| chunk.clone()),
			)
			.unwrap();

		let source = store.verified_replication_object(bucket_id, operation_id).unwrap();
		assert_eq!(source.bucket_id, bucket_id);
		assert_eq!(source.operation_id, operation_id);
		assert_eq!(source.cid, cid);
		assert_eq!(source.stored_bytes, MAX_STORED_BYTES);
		assert_eq!(source.chunk_hashes.len(), MAX_CHUNKS);
		assert!(source.chunk_hashes.iter().all(|hash| *hash == chunk_digest));
		assert_eq!(
			store.read_chunk_verified(cid.as_str(), (MAX_CHUNKS - 1) as u16).unwrap(),
			chunk
		);
		assert!(store
			.verified_replication_object(bucket_id, OperationId::from_bytes([54; 16]))
			.is_err());
	}

	#[test]
	fn recovery_owned_stream_rejects_standard_mutations_without_state_change() {
		let temp = tempfile::tempdir().unwrap();
		let (request, capability, snapshot, service) = recovery_fixture();
		let request_bytes = request.canonical_bytes();
		let store = StreamingStore::open(temp.path()).unwrap();
		let accepted = store
			.accept_object_put(
				&request_bytes,
				&capability.canonical_bytes(),
				&snapshot,
				service.public().0,
				&service,
				[10; 16],
			)
			.unwrap();
		let token = accepted.successor_token.unwrap();
		let descriptor = StreamingDescriptor {
			operation_id: OperationId::from_bytes(request.operation_id),
			bucket_id: BucketId::from_bytes(request.bucket_id),
			expected_cid: request.cid.as_str().into(),
			object_len: request.object_len,
		};
		let key = operation_key(&descriptor);
		let part = store.part_path(&key);
		let journal = store.root.join(JOURNAL);
		let (operation_before, recovery_before) = {
			let state = store.read_state().unwrap();
			(state.operations.get(&key).unwrap().clone(), state.recovery.clone())
		};
		let staged_before = fs::read(&part).unwrap();
		let journal_before = fs::read(&journal).unwrap();

		assert!(matches!(store.begin(descriptor.clone()), Err(ContentError::IdempotencyConflict)));
		assert!(matches!(
			store.try_acquire_ingress(descriptor.bucket_id, descriptor.operation_id, 0, 5,),
			Err(ContentError::IdempotencyConflict)
		));
		{
			let mut window = store.window.window.lock().unwrap();
			window.chunks += 1;
			window.bytes += 5;
		}
		let permit = IngressPermit {
			shared: Arc::clone(&store.window),
			operation_key: key.clone(),
			index: 0,
			bytes: 5,
		};
		assert!(matches!(
			store.push_chunk(permit, b"first"),
			Err(ContentError::IdempotencyConflict)
		));
		assert!(matches!(
			store.put_chunks(descriptor.clone(), [b"first".to_vec()]),
			Err(ContentError::IdempotencyConflict)
		));
		assert!(matches!(
			store.finalize(descriptor.bucket_id, descriptor.operation_id),
			Err(ContentError::IdempotencyConflict)
		));

		assert_eq!(fs::read(&part).unwrap(), staged_before);
		assert_eq!(fs::read(&journal).unwrap(), journal_before);
		{
			let state = store.read_state().unwrap();
			assert_eq!(state.operations.get(&key), Some(&operation_before));
			assert_eq!(state.recovery, recovery_before);
		}

		drop(store);
		let reopened = StreamingStore::open(temp.path()).unwrap();
		let progress = reopened
			.advance_object_put(
				&request_bytes,
				&token,
				&snapshot,
				service.public().0,
				1,
				b"first",
				&service,
				[11; 16],
			)
			.unwrap();
		let installed = reopened
			.finalize_object_put(
				&request_bytes,
				progress.successor_token.as_deref().unwrap(),
				&snapshot,
				service.public().0,
				&service,
			)
			.unwrap();
		assert!(installed.successor_token.is_none());
		reopened.verify_installed(request.cid.as_str()).unwrap();
	}

	fn repair_fixture(bytes: &[u8], operation: u8) -> (tempfile::TempDir, StreamingStore, String) {
		let temp = tempfile::tempdir().unwrap();
		let store = StreamingStore::open(temp.path()).unwrap();
		let descriptor = StreamingDescriptor {
			operation_id: OperationId::from_bytes([operation; 16]),
			bucket_id: BucketId::from_bytes([operation; 32]),
			expected_cid: CanonicalCid::from_digest(Blake2b::<U32>::digest(bytes).into())
				.to_string(),
			object_len: bytes.len() as u64,
		};
		let cid = store
			.put_chunks(descriptor, bytes.chunks(CHUNK_BYTES).map(ToOwned::to_owned))
			.unwrap()
			.cid;
		(temp, store, cid)
	}

	#[test]
	fn repair_starts_at_zero_middle_last_or_missing_chunk() {
		let bytes: Vec<_> = (0..CHUNK_BYTES * 3 - 17).map(|index| (index % 251) as u8).collect();
		for (case, damaged_chunk, expected_next) in
			[(1u8, Some(0usize), 0u16), (2, Some(1), 1), (3, Some(2), 2), (4, None, 0)]
		{
			let (temp, store, cid) = repair_fixture(&bytes, case);
			let object = store.object_path(&cid);
			if let Some(index) = damaged_chunk {
				let mut damaged = bytes.clone();
				damaged[index * CHUNK_BYTES] ^= 1;
				fs::write(&object, damaged).unwrap();
			} else {
				fs::remove_file(&object).unwrap();
			}
			assert_eq!(store.verify_installed(&cid), Err(ContentError::IntegrityFailed));
			assert_eq!(store.read_chunk_verified(&cid, 0), Err(ContentError::IntegrityFailed));
			let repair_operation = OperationId::from_bytes([case + 10; 16]);
			let progress = store.begin_repair(&cid, repair_operation).unwrap();
			assert_eq!(progress.next_chunk, expected_next);
			assert_eq!(progress.persisted_bytes, (expected_next as usize * CHUNK_BYTES) as u64);
			for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate().skip(expected_next as usize)
			{
				store.push_repair_chunk(&cid, repair_operation, index as u16, chunk).unwrap();
			}
			store.finalize_repair(&cid, repair_operation).unwrap();
			assert_eq!(store.read_range_verified(&cid, 0, 32).unwrap(), bytes[..32]);
			drop(store);
			StreamingStore::open(temp.path()).unwrap().verify_installed(&cid).unwrap();
		}
	}

	#[test]
	fn repair_preserves_255_256_and_257_kib_boundaries() {
		for (case, length) in
			[(20u8, 255usize * 1024), (21, 256usize * 1024), (22, 257usize * 1024)]
		{
			let bytes: Vec<_> = (0..length).map(|index| (index % 239) as u8).collect();
			let (_temp, store, cid) = repair_fixture(&bytes, case);
			assert_eq!(
				store.begin_repair(&cid, OperationId::from_bytes([case; 16])),
				Err(ContentError::IdempotencyConflict)
			);
			let mut damaged = bytes.clone();
			damaged[0] ^= 1;
			fs::write(store.object_path(&cid), damaged).unwrap();
			assert_eq!(store.verify_installed(&cid), Err(ContentError::IntegrityFailed));
			store.install_verified_repair(&cid, &bytes).unwrap();
			store.verify_installed(&cid).unwrap();
			assert_eq!(
				store.installed_record(&cid).unwrap().chunks.len(),
				length.div_ceil(CHUNK_BYTES)
			);
		}
	}

	#[test]
	fn repair_crashes_resume_at_chunk_rename_and_quarantine_clear() {
		let bytes: Vec<_> = (0..CHUNK_BYTES + 31).map(|index| (index % 223) as u8).collect();
		let (temp, store, cid) = repair_fixture(&bytes, 30);
		let mut damaged = bytes.clone();
		damaged[0] ^= 1;
		fs::write(store.object_path(&cid), damaged).unwrap();
		assert_eq!(store.verify_installed(&cid), Err(ContentError::IntegrityFailed));
		let operation = OperationId::from_bytes([31; 16]);
		assert_eq!(store.begin_repair(&cid, operation).unwrap().next_chunk, 0);
		assert_eq!(
			store.push_repair_chunk(&cid, operation, 1, &bytes[CHUNK_BYTES..]),
			Err(ContentError::ChunkOutOfOrder)
		);
		let mut wrong = bytes[..CHUNK_BYTES].to_vec();
		wrong[0] ^= 1;
		assert_eq!(
			store.push_repair_chunk(&cid, operation, 0, &wrong),
			Err(ContentError::CidMismatch)
		);
		assert_eq!(
			store.push_repair_chunk(&cid, operation, 0, &vec![0; CHUNK_BYTES + 1]),
			Err(ContentError::ChunkTooLarge)
		);
		store.inject_fault_once(StreamingFault::AfterRepairChunkSync).unwrap();
		assert!(matches!(
			store.push_repair_chunk(&cid, operation, 0, &bytes[..CHUNK_BYTES]),
			Err(ContentError::Io(_))
		));
		drop(store);

		let store = StreamingStore::open(temp.path()).unwrap();
		assert_eq!(store.begin_repair(&cid, operation).unwrap().next_chunk, 0);
		store.inject_fault_once(StreamingFault::AfterRepairProgressCommit).unwrap();
		assert!(matches!(
			store.push_repair_chunk(&cid, operation, 0, &bytes[..CHUNK_BYTES]),
			Err(ContentError::Io(_))
		));
		let committed = store.begin_repair(&cid, operation).unwrap();
		assert_eq!(committed.next_chunk, 1);
		assert_eq!(committed.persisted_bytes, CHUNK_BYTES as u64);
		assert_eq!(
			store.push_repair_chunk(&cid, operation, 0, &bytes[..CHUNK_BYTES]).unwrap(),
			committed
		);
		assert_eq!(
			fs::metadata(store.repair_path(&repair_key(&cid, operation).unwrap()))
				.unwrap()
				.len(),
			CHUNK_BYTES as u64
		);
		let mut changed_replay = bytes[..CHUNK_BYTES].to_vec();
		changed_replay[1] ^= 1;
		assert_eq!(
			store.push_repair_chunk(&cid, operation, 0, &changed_replay),
			Err(ContentError::IdempotencyConflict)
		);
		store.push_repair_chunk(&cid, operation, 1, &bytes[CHUNK_BYTES..]).unwrap();
		store.inject_fault_once(StreamingFault::AfterRepairRename).unwrap();
		assert!(matches!(store.finalize_repair(&cid, operation), Err(ContentError::Io(_))));
		drop(store);

		let store = StreamingStore::open(temp.path()).unwrap();
		assert!(store.begin_repair(&cid, operation).unwrap().ready_to_finalize);
		store.inject_fault_once(StreamingFault::AfterRepairQuarantineClear).unwrap();
		assert!(matches!(store.finalize_repair(&cid, operation), Err(ContentError::Io(_))));
		assert_eq!(store.read_chunk_verified(&cid, 0).unwrap(), bytes[..CHUNK_BYTES]);
		drop(store);

		let store = StreamingStore::open(temp.path()).unwrap();
		store.finalize_repair(&cid, operation).unwrap();
		store.verify_installed(&cid).unwrap();
	}

	#[test]
	fn repair_prefix_and_finalizing_journals_resume_without_duplicate_effects() {
		let bytes: Vec<_> = (0..CHUNK_BYTES * 3 - 9).map(|index| (index % 211) as u8).collect();
		let (temp, store, cid) = repair_fixture(&bytes, 40);
		let object = store.object_path(&cid);
		let mut damaged = bytes.clone();
		damaged[CHUNK_BYTES] ^= 1;
		fs::write(&object, &damaged).unwrap();
		assert_eq!(store.verify_installed(&cid), Err(ContentError::IntegrityFailed));
		let operation = OperationId::from_bytes([41; 16]);
		store.inject_fault_once(StreamingFault::AfterRepairPrefixSync).unwrap();
		assert!(matches!(store.begin_repair(&cid, operation), Err(ContentError::Io(_))));
		assert_eq!(fs::read_dir(store.root.join(STAGING)).unwrap().count(), 1);
		drop(store);

		let store = StreamingStore::open(temp.path()).unwrap();
		let progress = store.begin_repair(&cid, operation).unwrap();
		assert_eq!(progress.next_chunk, 1);
		assert_eq!(progress.persisted_bytes, CHUNK_BYTES as u64);
		for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate().skip(1) {
			store.push_repair_chunk(&cid, operation, index as u16, chunk).unwrap();
		}
		store.inject_fault_once(StreamingFault::AfterRepairFinalizingJournal).unwrap();
		assert!(matches!(store.finalize_repair(&cid, operation), Err(ContentError::Io(_))));
		assert_eq!(fs::read(&object).unwrap(), damaged);
		assert_eq!(store.read_chunk_verified(&cid, 0), Err(ContentError::IntegrityFailed));
		drop(store);

		let store = StreamingStore::open(temp.path()).unwrap();
		assert!(store.begin_repair(&cid, operation).unwrap().ready_to_finalize);
		store.finalize_repair(&cid, operation).unwrap();
		assert_eq!(
			store.read_chunk_verified(&cid, 1).unwrap(),
			bytes[CHUNK_BYTES..CHUNK_BYTES * 2]
		);
		store.finalize_repair(&cid, operation).unwrap();
	}

	#[test]
	fn replication_ingress_classifies_duplicate_cid_as_fresh_without_mutation() {
		let bytes = vec![9; CHUNK_BYTES + 3];
		let (_temp, store, cid) = repair_fixture(&bytes, 61);
		let install = OperationId::from_bytes([62; 16]);
		let repair = OperationId::from_bytes([63; 16]);
		assert_eq!(
			store
				.classify_replication_ingress(
					BucketId::from_bytes([61; 32]),
					&cid,
					bytes.len() as u64,
					install,
					repair,
				)
				.unwrap(),
			ReplicationIngressState::Fresh
		);
		store
			.put_chunks(
				StreamingDescriptor {
					operation_id: install,
					bucket_id: BucketId::from_bytes([61; 32]),
					expected_cid: cid.clone(),
					object_len: bytes.len() as u64,
				},
				bytes.chunks(CHUNK_BYTES).map(ToOwned::to_owned),
			)
			.unwrap();
		assert_eq!(
			store
				.classify_replication_ingress(
					BucketId::from_bytes([61; 32]),
					&cid,
					bytes.len() as u64,
					OperationId::from_bytes([64; 16]),
					OperationId::from_bytes([65; 16]),
				)
				.unwrap(),
			ReplicationIngressState::Fresh
		);
	}

	#[test]
	fn replication_ingress_classifies_quarantine_for_exact_repair() {
		let bytes = vec![7; CHUNK_BYTES + 5];
		let (_temp, store, cid) = repair_fixture(&bytes, 71);
		let mut damaged = bytes.clone();
		damaged[0] ^= 1;
		fs::write(store.object_path(&cid), damaged).unwrap();
		assert_eq!(store.verify_installed(&cid), Err(ContentError::IntegrityFailed));
		let install = OperationId::from_bytes([72; 16]);
		let repair = OperationId::from_bytes([73; 16]);
		assert_eq!(
			store
				.classify_replication_ingress(
					BucketId::from_bytes([71; 32]),
					&cid,
					(CHUNK_BYTES + 5) as u64,
					install,
					repair,
				)
				.unwrap(),
			ReplicationIngressState::Repair
		);
		store.begin_repair(&cid, repair).unwrap();
		for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
			store.push_repair_chunk(&cid, repair, index as u16, chunk).unwrap();
		}
		store.finalize_repair(&cid, repair).unwrap();
		assert_eq!(
			store
				.classify_replication_ingress(
					BucketId::from_bytes([71; 32]),
					&cid,
					bytes.len() as u64,
					install,
					repair,
				)
				.unwrap(),
			ReplicationIngressState::ExactReady
		);
	}
}
