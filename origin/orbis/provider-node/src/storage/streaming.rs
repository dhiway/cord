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
	sync::RwLock,
};

use blake2::{digest::consts::U32, Blake2b, Digest};
use serde::{Deserialize, Serialize};

use crate::{
	CanonicalCid, ContentError, CHUNK_BYTES, MAX_CHUNKS, MAX_RANGE_BYTES, MAX_STORED_BYTES,
};

const STREAM_VERSION: u16 = 1;
const STREAM_ROOT: &str = "streaming-v1";
const JOURNAL: &str = "journal.json";
const STAGING: &str = "staging";
const OBJECTS: &str = "objects";

/// Immutable descriptor for one stored-byte operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamingDescriptor {
	/// Caller idempotency operation id.
	pub operation_id: String,
	/// Canonical bucket id or host bucket reference.
	pub bucket_id: String,
	/// Expected canonical stored-byte CID.
	pub expected_cid: String,
	/// Exact stored-byte length.
	pub object_len: u64,
}

/// Byte-plane receipt. Durable readability is not checkpoint publishability.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamingReceipt {
	/// Idempotency operation id.
	pub operation_id: String,
	/// Bucket id.
	pub bucket_id: String,
	/// Canonical stored-byte CID.
	pub cid: String,
	/// Exact stored-byte length.
	pub stored_bytes: u64,
	/// Number of stored chunks.
	pub chunks: u16,
	/// Exact descriptor-and-content replay fingerprint.
	pub fingerprint: String,
	/// Bytes are installed and fully verified locally, but not necessarily publishable.
	pub durably_readable: bool,
}

/// Result of opening an idempotent operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BeginStreaming {
	/// Operation is accepting contiguous chunks from `next_chunk`.
	Receiving {
		/// Exact next contiguous chunk index.
		next_chunk: u16,
		/// Bytes already durably acknowledged.
		received_bytes: u64,
	},
	/// Exact operation was already installed.
	Installed(StreamingReceipt),
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
struct OperationRecord {
	descriptor: StreamingDescriptor,
	phase: Phase,
	next_chunk: u16,
	received_bytes: u64,
	receipt: Option<StreamingReceipt>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalState {
	version: u16,
	operations: BTreeMap<String, OperationRecord>,
}

/// Internal provider streaming store. No public HTTP route is attached to this type.
pub struct StreamingStore {
	root: PathBuf,
	state: RwLock<JournalState>,
}

impl StreamingStore {
	/// Open a staged store, recover finalizing operations and remove unreferenced staging files.
	pub fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let root = root.as_ref().join(STREAM_ROOT);
		fs::create_dir_all(root.join(STAGING)).map_err(io_error)?;
		fs::create_dir_all(root.join(OBJECTS)).map_err(io_error)?;
		let journal = root.join(JOURNAL);
		let state = if journal.exists() {
			let bytes = fs::read(&journal).map_err(io_error)?;
			let state: JournalState = serde_json::from_slice(&bytes).map_err(io_error)?;
			if state.version != STREAM_VERSION {
				return Err(ContentError::IntegrityFailed)
			}
			state
		} else {
			JournalState { version: STREAM_VERSION, operations: BTreeMap::new() }
		};
		let store = Self { root, state: RwLock::new(state) };
		store.recover()?;
		if !journal.exists() {
			store.persist()?;
		}
		Ok(store)
	}

	/// Start or resume one operation. Installed replay is returned only for an exact descriptor.
	pub fn begin(&self, descriptor: StreamingDescriptor) -> Result<BeginStreaming, ContentError> {
		validate_descriptor(&descriptor)?;
		let key = operation_key(&descriptor);
		let mut state = self.write_state()?;
		if let Some(existing) = state.operations.get(&key) {
			if existing.descriptor != descriptor {
				return Err(ContentError::IdempotencyConflict)
			}
			return match (&existing.phase, &existing.receipt) {
				(Phase::Installed, Some(receipt)) => Ok(BeginStreaming::Installed(receipt.clone())),
				(Phase::Receiving, _) => Ok(BeginStreaming::Receiving {
					next_chunk: existing.next_chunk,
					received_bytes: existing.received_bytes,
				}),
				_ => Err(ContentError::IntegrityFailed),
			}
		}
		let path = self.part_path(&key);
		let file = OpenOptions::new().create_new(true).write(true).open(&path).map_err(io_error)?;
		file.sync_all().map_err(io_error)?;
		sync_dir(path.parent().expect("staging path has parent"))?;
		let mut next = state.clone();
		next.operations.insert(
			key,
			OperationRecord {
				descriptor,
				phase: Phase::Receiving,
				next_chunk: 0,
				received_bytes: 0,
				receipt: None,
			},
		);
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(BeginStreaming::Receiving { next_chunk: 0, received_bytes: 0 })
	}

	/// Append and durably acknowledge exactly one contiguous fixed-size chunk.
	pub fn push_chunk(
		&self,
		bucket_id: &str,
		operation_id: &str,
		index: u16,
		bytes: &[u8],
	) -> Result<(), ContentError> {
		let key = operation_key_parts(bucket_id, operation_id);
		let mut state = self.write_state()?;
		let record = state.operations.get(&key).cloned().ok_or(ContentError::NotFound)?;
		if record.phase != Phase::Receiving {
			return Err(ContentError::ChunkOutOfOrder)
		}
		let expected = expected_chunk_len(record.descriptor.object_len, index)?;
		if bytes.len() != expected {
			return Err(ContentError::ChunkSizeInvalid)
		}
		let path = self.part_path(&key);
		if index < record.next_chunk {
			let mut file = File::open(&path).map_err(io_error)?;
			file.seek(SeekFrom::Start(index as u64 * CHUNK_BYTES as u64))
				.map_err(io_error)?;
			let mut installed = vec![0; expected];
			file.read_exact(&mut installed).map_err(io_error)?;
			return if installed == bytes { Ok(()) } else { Err(ContentError::IdempotencyConflict) }
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
		let mut next = state.clone();
		let next_record = next.operations.get_mut(&key).expect("record was cloned from state");
		next_record.received_bytes = next_record
			.received_bytes
			.checked_add(bytes.len() as u64)
			.ok_or(ContentError::ObjectTooLarge)?;
		next_record.next_chunk =
			next_record.next_chunk.checked_add(1).ok_or(ContentError::ObjectTooLarge)?;
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(())
	}

	/// Verify and atomically install a complete staged operation.
	pub fn finalize(
		&self,
		bucket_id: &str,
		operation_id: &str,
	) -> Result<StreamingReceipt, ContentError> {
		let key = operation_key_parts(bucket_id, operation_id);
		let mut state = self.write_state()?;
		let record = state.operations.get(&key).cloned().ok_or(ContentError::NotFound)?;
		if record.phase == Phase::Installed {
			return record.receipt.ok_or(ContentError::IntegrityFailed)
		}
		if record.phase != Phase::Receiving {
			return Err(ContentError::IntegrityFailed)
		}
		if record.received_bytes != record.descriptor.object_len ||
			record.next_chunk as usize != chunk_count(record.descriptor.object_len)?
		{
			return Err(ContentError::ChunkMissing)
		}
		let part = self.part_path(&key);
		let (cid, fingerprint, length) = verify_file(&part, &record.descriptor)?;
		if length != record.descriptor.object_len {
			return Err(ContentError::LengthMismatch)
		}
		let receipt = receipt(&record.descriptor, cid.as_str(), fingerprint);
		let mut finalizing = state.clone();
		finalizing.operations.get_mut(&key).expect("record exists").phase = Phase::Finalizing;
		persist_state(&self.root, &finalizing)?;
		*state = finalizing;
		install_file(&part, &self.object_path(cid.as_str()), &record.descriptor)?;
		let mut installed = state.clone();
		let installed_record = installed.operations.get_mut(&key).expect("record exists");
		installed_record.phase = Phase::Installed;
		installed_record.receipt = Some(receipt.clone());
		persist_state(&self.root, &installed)?;
		*state = installed;
		Ok(receipt)
	}

	/// Convenience streaming path. Chunks are processed one at a time and never assembled in
	/// memory.
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
				let (cid, fingerprint, length, count) = verify_chunk_stream(&descriptor, chunks)
					.map_err(|_| ContentError::IdempotencyConflict)?;
				if cid.as_str() != receipt.cid ||
					fingerprint != receipt.fingerprint ||
					length != receipt.stored_bytes ||
					count != receipt.chunks
				{
					return Err(ContentError::IdempotencyConflict)
				}
				Ok(receipt)
			},
			BeginStreaming::Receiving { next_chunk, .. } => {
				for (offset, chunk) in chunks.into_iter().enumerate() {
					let index = next_chunk
						.checked_add(offset.try_into().map_err(|_| ContentError::ObjectTooLarge)?)
						.ok_or(ContentError::ObjectTooLarge)?;
					self.push_chunk(
						&descriptor.bucket_id,
						&descriptor.operation_id,
						index,
						&chunk,
					)?;
				}
				self.finalize(&descriptor.bucket_id, &descriptor.operation_id)
			},
		}
	}

	/// Open a fully verified object for streaming. Verification completes before the file is
	/// returned.
	pub fn open_verified(&self, cid: &str) -> Result<File, ContentError> {
		let canonical = CanonicalCid::parse(cid)?;
		let descriptor = self.installed_descriptor(canonical.as_str())?;
		let (mut file, _, _, _) =
			verify_open_file(&self.object_path(canonical.as_str()), &descriptor)?;
		file.seek(SeekFrom::Start(0)).map_err(io_error)?;
		Ok(file)
	}

	/// Return a fully verified bounded half-open range. No bytes are released before full-CID
	/// check.
	pub fn read_range_verified(
		&self,
		cid: &str,
		start: u64,
		end: u64,
	) -> Result<Vec<u8>, ContentError> {
		let mut file = self.open_verified(cid)?;
		let length = file.metadata().map_err(io_error)?.len();
		if start > end || end > length || end.saturating_sub(start) > MAX_RANGE_BYTES {
			return Err(ContentError::RangeInvalid)
		}
		file.seek(SeekFrom::Start(start)).map_err(io_error)?;
		let mut bytes = vec![0; (end - start) as usize];
		file.read_exact(&mut bytes).map_err(io_error)?;
		Ok(bytes)
	}

	/// Return one verified stored chunk after full-CID verification.
	pub fn read_chunk_verified(&self, cid: &str, index: u16) -> Result<Vec<u8>, ContentError> {
		let mut file = self.open_verified(cid)?;
		let length = file.metadata().map_err(io_error)?.len();
		let expected = expected_chunk_len(length, index)?;
		file.seek(SeekFrom::Start(index as u64 * CHUNK_BYTES as u64))
			.map_err(io_error)?;
		let mut bytes = vec![0; expected];
		file.read_exact(&mut bytes).map_err(io_error)?;
		Ok(bytes)
	}

	fn recover(&self) -> Result<(), ContentError> {
		let mut state = self.write_state()?;
		let mut next = state.clone();
		let mut changed = false;
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
				},
				Phase::Finalizing => {
					let part = self.part_path(&key);
					let object = self.object_path(&record.descriptor.expected_cid);
					if !object.exists() {
						verify_file(&part, &record.descriptor)?;
						install_file(&part, &object, &record.descriptor)?;
					} else {
						verify_file(&object, &record.descriptor)?;
						if part.exists() {
							fs::remove_file(&part).map_err(io_error)?;
							sync_dir(part.parent().expect("staging path has parent"))?;
						}
					}
					let (_, fingerprint, _) = verify_file(&object, &record.descriptor)?;
					let receipt =
						receipt(&record.descriptor, &record.descriptor.expected_cid, fingerprint);
					let recovered = next.operations.get_mut(&key).expect("key exists");
					recovered.phase = Phase::Installed;
					recovered.receipt = Some(receipt);
					changed = true;
				},
				Phase::Installed => {
					let receipt = record.receipt.as_ref().ok_or(ContentError::IntegrityFailed)?;
					let (_, fingerprint, length) = verify_file(
						&self.object_path(&record.descriptor.expected_cid),
						&record.descriptor,
					)?;
					if receipt.fingerprint != fingerprint || receipt.stored_bytes != length {
						return Err(ContentError::IntegrityFailed)
					}
				},
			}
		}
		let referenced: BTreeSet<_> = next
			.operations
			.iter()
			.filter(|(_, record)| record.phase != Phase::Installed)
			.map(|(key, _)| format!("{key}.part"))
			.collect();
		let mut removed_orphan = false;
		for entry in fs::read_dir(self.root.join(STAGING)).map_err(io_error)? {
			let entry = entry.map_err(io_error)?;
			let name = entry.file_name().to_string_lossy().into_owned();
			if !referenced.contains(&name) {
				fs::remove_file(entry.path()).map_err(io_error)?;
				changed = true;
				removed_orphan = true;
			}
		}
		if removed_orphan {
			sync_dir(&self.root.join(STAGING))?;
		}
		if changed {
			persist_state(&self.root, &next)?;
			*state = next;
		}
		Ok(())
	}

	fn installed_descriptor(&self, cid: &str) -> Result<StreamingDescriptor, ContentError> {
		self.read_state()?
			.operations
			.values()
			.find(|record| {
				record.phase == Phase::Installed && record.descriptor.expected_cid == cid
			})
			.map(|record| record.descriptor.clone())
			.ok_or(ContentError::NotFound)
	}

	fn part_path(&self, key: &str) -> PathBuf {
		self.root.join(STAGING).join(format!("{key}.part"))
	}

	fn object_path(&self, cid: &str) -> PathBuf {
		self.root.join(OBJECTS).join(cid)
	}

	fn persist(&self) -> Result<(), ContentError> {
		let state = self.read_state()?;
		persist_state(&self.root, &state)
	}

	fn read_state(&self) -> Result<std::sync::RwLockReadGuard<'_, JournalState>, ContentError> {
		self.state
			.read()
			.map_err(|_| ContentError::Io("stream state lock poisoned".into()))
	}

	fn write_state(&self) -> Result<std::sync::RwLockWriteGuard<'_, JournalState>, ContentError> {
		self.state
			.write()
			.map_err(|_| ContentError::Io("stream state lock poisoned".into()))
	}
}

fn validate_descriptor(descriptor: &StreamingDescriptor) -> Result<(), ContentError> {
	if descriptor.operation_id.is_empty() ||
		descriptor.operation_id.len() > 128 ||
		descriptor.bucket_id.is_empty() ||
		descriptor.bucket_id.len() > 128
	{
		return Err(ContentError::IdempotencyConflict)
	}
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

fn operation_key(descriptor: &StreamingDescriptor) -> String {
	operation_key_parts(&descriptor.bucket_id, &descriptor.operation_id)
}

fn operation_key_parts(bucket_id: &str, operation_id: &str) -> String {
	let mut hash = Blake2b::<U32>::new();
	feed(&mut hash, bucket_id.as_bytes());
	feed(&mut hash, operation_id.as_bytes());
	hex::encode(hash.finalize())
}

fn new_fingerprint(descriptor: &StreamingDescriptor) -> Blake2b<U32> {
	let mut hash = Blake2b::<U32>::new();
	feed(&mut hash, b"origin/streaming-content/v1");
	feed(&mut hash, descriptor.bucket_id.as_bytes());
	feed(&mut hash, descriptor.operation_id.as_bytes());
	feed(&mut hash, descriptor.expected_cid.as_bytes());
	hash.update(descriptor.object_len.to_le_bytes());
	hash
}

fn feed(hash: &mut Blake2b<U32>, bytes: &[u8]) {
	hash.update((bytes.len() as u64).to_le_bytes());
	hash.update(bytes);
}

fn verify_file(
	path: &Path,
	descriptor: &StreamingDescriptor,
) -> Result<(CanonicalCid, String, u64), ContentError> {
	let (_, cid, fingerprint, length) = verify_open_file(path, descriptor)?;
	Ok((cid, fingerprint, length))
}

fn verify_open_file(
	path: &Path,
	descriptor: &StreamingDescriptor,
) -> Result<(File, CanonicalCid, String, u64), ContentError> {
	let mut file = File::open(path).map_err(|_| ContentError::IntegrityFailed)?;
	let mut content = Blake2b::<U32>::new();
	let mut fingerprint = new_fingerprint(descriptor);
	let mut length = 0u64;
	let mut buffer = vec![0u8; CHUNK_BYTES];
	loop {
		let read = file.read(&mut buffer).map_err(|_| ContentError::IntegrityFailed)?;
		if read == 0 {
			break
		}
		content.update(&buffer[..read]);
		fingerprint.update(&buffer[..read]);
		length = length.checked_add(read as u64).ok_or(ContentError::ObjectTooLarge)?;
	}
	if length != descriptor.object_len {
		return Err(ContentError::LengthMismatch)
	}
	let digest: [u8; 32] = content.finalize().into();
	let cid = CanonicalCid::from_digest(digest);
	if cid.as_str() != descriptor.expected_cid {
		return Err(ContentError::CidMismatch)
	}
	Ok((file, cid, format!("0x{}", hex::encode(fingerprint.finalize())), length))
}

fn verify_chunk_stream<I>(
	descriptor: &StreamingDescriptor,
	chunks: I,
) -> Result<(CanonicalCid, String, u64, u16), ContentError>
where
	I: IntoIterator<Item = Vec<u8>>,
{
	let mut content = Blake2b::<U32>::new();
	let mut fingerprint = new_fingerprint(descriptor);
	let mut length = 0u64;
	let mut count = 0u16;
	for (index, chunk) in chunks.into_iter().enumerate() {
		if chunk.len() != expected_chunk_len(descriptor.object_len, index as u16)? {
			return Err(ContentError::ChunkSizeInvalid)
		}
		content.update(&chunk);
		fingerprint.update(&chunk);
		length += chunk.len() as u64;
		count = count.checked_add(1).ok_or(ContentError::ObjectTooLarge)?;
	}
	if length != descriptor.object_len || count as usize != chunk_count(descriptor.object_len)? {
		return Err(ContentError::ChunkMissing)
	}
	let cid = CanonicalCid::from_digest(content.finalize().into());
	if cid.as_str() != descriptor.expected_cid {
		return Err(ContentError::CidMismatch)
	}
	Ok((cid, format!("0x{}", hex::encode(fingerprint.finalize())), length, count))
}

fn receipt(descriptor: &StreamingDescriptor, cid: &str, fingerprint: String) -> StreamingReceipt {
	StreamingReceipt {
		operation_id: descriptor.operation_id.clone(),
		bucket_id: descriptor.bucket_id.clone(),
		cid: cid.into(),
		stored_bytes: descriptor.object_len,
		chunks: chunk_count(descriptor.object_len).expect("validated descriptor") as u16,
		fingerprint,
		durably_readable: true,
	}
}

fn install_file(
	part: &Path,
	object: &Path,
	descriptor: &StreamingDescriptor,
) -> Result<(), ContentError> {
	if object.exists() {
		verify_file(object, descriptor)?;
		if part.exists() {
			fs::remove_file(part).map_err(io_error)?;
			sync_dir(part.parent().expect("staging path has parent"))?;
		}
	} else {
		fs::rename(part, object).map_err(io_error)?;
	}
	File::open(object).and_then(|file| file.sync_all()).map_err(io_error)?;
	sync_dir(object.parent().expect("object path has parent"))
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
