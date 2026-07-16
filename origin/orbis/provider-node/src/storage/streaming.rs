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

use blake2::{digest::consts::U32, Blake2b, Digest};
use serde::{Deserialize, Serialize};

use crate::{
	BucketId, CanonicalCid, ContentError, OperationId, CHUNK_BYTES, INGRESS_WINDOW_BYTES,
	INGRESS_WINDOW_CHUNKS, MAX_CHUNKS, MAX_RANGE_BYTES, MAX_STORED_BYTES, MAX_STREAMING_OPERATIONS,
};

const STREAM_VERSION: u16 = 3;
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

/// Result of opening an idempotent operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BeginStreaming {
	/// Operation is accepting a full replay from index zero or an explicit tail.
	Receiving(ProgressAck),
	/// Exact operation was already installed.
	Installed(StreamingReceipt),
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
	Receiving,
	Finalizing,
	Installed,
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
	#[serde(default)]
	quarantine: BTreeMap<String, QuarantineRecord>,
	#[serde(default)]
	detection_sequence: u64,
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
			if state.version != STREAM_VERSION || state.operations.len() > operation_limit {
				return Err(ContentError::IntegrityFailed)
			}
			state
		} else {
			JournalState {
				version: STREAM_VERSION,
				operations: BTreeMap::new(),
				quarantine: BTreeMap::new(),
				detection_sequence: 0,
			}
		};
		let store = Self {
			root,
			state: RwLock::new(state),
			window: Arc::new(IngressShared::default()),
			fault: RwLock::new(None),
			operation_limit,
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
		let record = state.operations.get(&key).cloned().ok_or(ContentError::NotFound)?;
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
		let installed_record = installed.operations.get_mut(&key).expect("record exists");
		installed_record.phase = Phase::Installed;
		installed_record.receipt = Some(receipt.clone());
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

	/// Stream an exact verified repair for an existing quarantine.
	pub fn install_verified_repair_reader<R: Read>(
		&self,
		cid: &str,
		mut reader: R,
	) -> Result<(), ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let record = self.installed_record(canonical.as_str())?;
		let mut state = self.write_state()?;
		if !state.quarantine.contains_key(canonical.as_str()) {
			return Err(ContentError::IdempotencyConflict)
		}
		let repair = self.repair_path(canonical.as_str());
		remove_repair_if_present(&repair)?;
		if let Err(error) = stage_verified_repair(&repair, &record, &mut reader) {
			remove_repair_if_present(&repair)?;
			return Err(error)
		}
		let object = self.object_path(canonical.as_str());
		fs::rename(&repair, &object).map_err(io_error)?;
		File::open(&object).and_then(|file| file.sync_all()).map_err(io_error)?;
		sync_dir(repair.parent().expect("repair path has parent"))?;
		sync_dir(object.parent().expect("object path has parent"))?;
		self.trip_fault(StreamingFault::AfterRepairRename)?;
		let mut next = state.clone();
		next.quarantine.remove(canonical.as_str());
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(())
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
		let keys: Vec<_> = next.operations.keys().cloned().collect();
		for key in keys {
			let record = next.operations.get(&key).cloned().expect("key exists");
			match record.phase {
				Phase::Receiving => {
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
					let recovered = next.operations.get_mut(&key).expect("key exists");
					recovered.phase = Phase::Installed;
					recovered.receipt = Some(receipt(
						&record.descriptor,
						&record.descriptor.expected_cid,
						fingerprint,
					));
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
			}
		}
		let referenced_staging: BTreeSet<_> = next
			.operations
			.iter()
			.filter(|(_, record)| record.phase != Phase::Installed)
			.map(|(key, _)| format!("{key}.part"))
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
			persist_state(&self.root, &next)?;
			*state = next;
		}
		Ok(())
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
	Ok(true)
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

fn stage_verified_repair<R: Read>(
	path: &Path,
	record: &OperationRecord,
	reader: &mut R,
) -> Result<(), ContentError> {
	validate_installed_record(record)?;
	let mut file = OpenOptions::new().create_new(true).write(true).open(path).map_err(io_error)?;
	let mut content = Blake2b::<U32>::new();
	let mut length = 0u64;
	for expected in &record.chunks {
		let mut bytes = vec![0; expected.length as usize];
		reader.read_exact(&mut bytes).map_err(|_| ContentError::LengthMismatch)?;
		if chunk_hash(&bytes) != expected.hash {
			return Err(ContentError::CidMismatch)
		}
		length = length.checked_add(bytes.len() as u64).ok_or(ContentError::ObjectTooLarge)?;
		content.update(&bytes);
		file.write_all(&bytes).map_err(io_error)?;
	}
	let mut trailing = [0u8; 1];
	if reader.read(&mut trailing).map_err(|_| ContentError::IntegrityFailed)? != 0 ||
		length != record.descriptor.object_len ||
		record.chunks.len() != chunk_count(length)?
	{
		return Err(ContentError::LengthMismatch)
	}
	let cid = CanonicalCid::from_digest(content.finalize().into());
	if cid.as_str() != record.descriptor.expected_cid {
		return Err(ContentError::CidMismatch)
	}
	file.sync_all().map_err(io_error)?;
	sync_dir(path.parent().expect("repair path has parent"))
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
