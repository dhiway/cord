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

//! Private crash-safe replay store for exact authenticated peer replies.
//!
//! A response must be durable before a caller can return or send its bytes. Records are immutable
//! until an authenticated acknowledgement protocol exists; there is deliberately no pruning API.

use std::{
	collections::BTreeMap,
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
};

use codec::{Decode, Encode};

use crate::{
	peer::{
		PeerChunkRequestV1, PeerChunkResponseV1, PeerReplayIdentityV1, PeerSyncPageRequestV1,
		PeerSyncPageResponseV1, MAX_CHUNK_RESPONSE_ENCODED, MAX_PAGE_RESPONSE_ENCODED,
		MAX_REQUEST_ENCODED,
	},
	ContentError,
};

const ROOT: &str = "peer-replies-v1";
const VERSION: u8 = 1;
const EXTENSION: &str = ".reply";
const RECORD_DOMAIN: &[u8] = b"cord/provider/peer-reply-record/v1";
const MAX_RECORDS: usize = 4_096;
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TEMP_ARTIFACTS: usize = 128;
const MAX_RECORD_OVERHEAD: usize = 512;
const MAX_RECORD_BYTES: usize =
	MAX_REQUEST_ENCODED + MAX_PAGE_RESPONSE_ENCODED + MAX_RECORD_OVERHEAD;

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, Ord, PartialEq, PartialOrd)]
enum PeerReplyKindV1 {
	Page,
	Chunk,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PeerReplyKeyV1 {
	operation_id: [u8; 16],
	request_nonce: [u8; 16],
}

impl PeerReplyKeyV1 {
	fn from_identity(identity: PeerReplayIdentityV1) -> Self {
		Self { operation_id: identity.operation_id, request_nonce: identity.request_nonce }
	}

	fn filename(self) -> String {
		format!(
			"{}-{}{}",
			hex::encode(self.operation_id),
			hex::encode(self.request_nonce),
			EXTENSION,
		)
	}
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct PeerReplyRecordV1 {
	version: u8,
	kind: PeerReplyKindV1,
	operation_id: [u8; 16],
	request_nonce: [u8; 16],
	request_hash: [u8; 32],
	request_bytes: Vec<u8>,
	response_bytes: Vec<u8>,
	record_hash: [u8; 32],
}

impl PeerReplyRecordV1 {
	fn key(&self) -> PeerReplyKeyV1 {
		PeerReplyKeyV1 { operation_id: self.operation_id, request_nonce: self.request_nonce }
	}
}

#[derive(Default)]
struct PeerReplyState {
	records: BTreeMap<PeerReplyKeyV1, PeerReplyRecordV1>,
	total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PeerReplyFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
}

/// Immutable replay table for exact source-authenticated responses.
pub(crate) struct PeerReplyStore {
	root: PathBuf,
	state: RwLock<PeerReplyState>,
	poisoned: RwLock<bool>,
	fault: RwLock<Option<PeerReplyFault>>,
	record_limit: usize,
	byte_limit: u64,
}

pub(crate) struct PreparedPeerReplyStore {
	root: PathBuf,
	root_missing: bool,
	state: PeerReplyState,
	temp_artifacts: Vec<PathBuf>,
	record_limit: usize,
	byte_limit: u64,
}

impl PreparedPeerReplyStore {
	pub(crate) fn apply(self) -> Result<PeerReplyStore, ContentError> {
		crate::bounded_io::create_prepared_directory(&self.root, self.root_missing)?;
		crate::bounded_io::remove_validated_temp_artifacts(&self.root, &self.temp_artifacts)?;
		Ok(PeerReplyStore {
			root: self.root,
			state: RwLock::new(self.state),
			poisoned: RwLock::new(false),
			fault: RwLock::new(None),
			record_limit: self.record_limit,
			byte_limit: self.byte_limit,
		})
	}
}

impl PeerReplyStore {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		Self::open_with_limits(root, MAX_RECORDS, MAX_TOTAL_BYTES)
	}

	pub(crate) fn prepare_open(
		root: impl AsRef<Path>,
	) -> Result<PreparedPeerReplyStore, ContentError> {
		Self::prepare_open_with_limits(root, MAX_RECORDS, MAX_TOTAL_BYTES)
	}

	fn open_with_limits(
		root: impl AsRef<Path>,
		record_limit: usize,
		byte_limit: u64,
	) -> Result<Self, ContentError> {
		Self::prepare_open_with_limits(root, record_limit, byte_limit)?.apply()
	}

	fn prepare_open_with_limits(
		root: impl AsRef<Path>,
		record_limit: usize,
		byte_limit: u64,
	) -> Result<PreparedPeerReplyStore, ContentError> {
		if record_limit == 0
			|| record_limit > MAX_RECORDS
			|| byte_limit == 0
			|| byte_limit > MAX_TOTAL_BYTES
		{
			return Err(ContentError::SchemaInvalid);
		}
		let root = root.as_ref().join(ROOT);
		let root_missing = !crate::bounded_io::optional_directory_exists(&root)?;
		let loaded = if root_missing {
			LoadedPeerReplies { state: PeerReplyState::default(), temp_artifacts: Vec::new() }
		} else {
			load_records(&root, record_limit, byte_limit)?
		};
		Ok(PreparedPeerReplyStore {
			root,
			root_missing,
			state: loaded.state,
			temp_artifacts: loaded.temp_artifacts,
			record_limit,
			byte_limit,
		})
	}

	/// Persist exact page response bytes after authenticating them against the exact request.
	pub(crate) fn record_page(
		&self,
		request: &PeerSyncPageRequestV1,
		response_bytes: &[u8],
	) -> Result<Vec<u8>, ContentError> {
		let record = page_record(request, response_bytes)?;
		self.record(record)
	}

	/// Return only the durable source-signed bytes for this exact page request.
	pub(crate) fn replay_page(
		&self,
		request: &PeerSyncPageRequestV1,
	) -> Result<Vec<u8>, ContentError> {
		let identity = request.authenticated_replay_identity()?;
		let key = PeerReplyKeyV1::from_identity(identity);
		let record = self.replay(key)?;
		validate_page_request_binding(&record, request, identity)?;
		Ok(record.response_bytes)
	}

	/// Persist exact chunk response bytes after authenticating them against the exact request.
	pub(crate) fn record_chunk(
		&self,
		request: &PeerChunkRequestV1,
		response_bytes: &[u8],
	) -> Result<Vec<u8>, ContentError> {
		let record = chunk_record(request, response_bytes)?;
		self.record(record)
	}

	/// Return only the durable source-signed bytes for this exact chunk request.
	pub(crate) fn replay_chunk(
		&self,
		request: &PeerChunkRequestV1,
	) -> Result<Vec<u8>, ContentError> {
		let identity = request.authenticated_replay_identity()?;
		let key = PeerReplyKeyV1::from_identity(identity);
		let record = self.replay(key)?;
		validate_chunk_request_binding(&record, request, identity)?;
		Ok(record.response_bytes)
	}

	#[doc(hidden)]
	pub(crate) fn inject_fault_once(&self, fault: PeerReplyFault) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	fn record(&self, record: PeerReplyRecordV1) -> Result<Vec<u8>, ContentError> {
		self.ensure_healthy()?;
		validate_record(&record)?;
		let encoded = record.encode();
		if encoded.len() > MAX_RECORD_BYTES {
			return Err(ContentError::ObjectTooLarge);
		}
		let encoded_len: u64 =
			encoded.len().try_into().map_err(|_| ContentError::ObjectTooLarge)?;
		let key = record.key();
		let mut state = self.state.write().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		if let Some(existing) = state.records.get(&key) {
			return if existing == &record {
				Ok(existing.response_bytes.clone())
			} else {
				Err(ContentError::IdempotencyConflict)
			};
		}
		if state.records.len() >= self.record_limit
			|| state.total_bytes.checked_add(encoded_len).is_none()
			|| state.total_bytes + encoded_len > self.byte_limit
		{
			return Err(ContentError::ProviderRecoveryTableFull);
		}
		if let Err(error) = self.persist(&key.filename(), &encoded) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error);
		}
		state.total_bytes += encoded_len;
		state.records.insert(key, record.clone());
		Ok(record.response_bytes)
	}

	fn replay(&self, key: PeerReplyKeyV1) -> Result<PeerReplyRecordV1, ContentError> {
		self.ensure_healthy()?;
		let state = self.state.read().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		let record = state.records.get(&key).cloned().ok_or(ContentError::NotFound)?;
		validate_record(&record)?;
		Ok(record)
	}

	fn ensure_healthy(&self) -> Result<(), ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	fn persist(&self, name: &str, bytes: &[u8]) -> Result<(), ContentError> {
		let destination = self.root.join(name);
		let temporary = self.root.join(format!("{name}.tmp-{}", std::process::id()));
		let mut file = File::create(&temporary).map_err(io_error)?;
		file.write_all(bytes).map_err(io_error)?;
		self.trip(PeerReplyFault::BeforeTempFsync)?;
		file.sync_all().map_err(io_error)?;
		self.trip(PeerReplyFault::AfterTempFsync)?;
		fs::rename(&temporary, destination).map_err(io_error)?;
		self.trip(PeerReplyFault::AfterRename)?;
		sync_dir(&self.root)?;
		self.trip(PeerReplyFault::AfterDirectoryFsync)
	}

	fn trip(&self, point: PeerReplyFault) -> Result<(), ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			return Err(ContentError::Io(format!("injected peer reply fault: {point:?}")));
		}
		Ok(())
	}
}

fn page_record(
	request: &PeerSyncPageRequestV1,
	response_bytes: &[u8],
) -> Result<PeerReplyRecordV1, ContentError> {
	if response_bytes.len() > MAX_PAGE_RESPONSE_ENCODED {
		return Err(ContentError::ObjectTooLarge);
	}
	let identity = request.authenticated_replay_identity()?;
	PeerSyncPageResponseV1::decode_canonical(response_bytes, request)?;
	new_record(PeerReplyKindV1::Page, request.encode_wire(), response_bytes.to_vec(), identity)
}

fn chunk_record(
	request: &PeerChunkRequestV1,
	response_bytes: &[u8],
) -> Result<PeerReplyRecordV1, ContentError> {
	if response_bytes.len() > MAX_CHUNK_RESPONSE_ENCODED {
		return Err(ContentError::ObjectTooLarge);
	}
	let identity = request.authenticated_replay_identity()?;
	PeerChunkResponseV1::decode_canonical(response_bytes, request)?;
	new_record(PeerReplyKindV1::Chunk, request.encode_wire(), response_bytes.to_vec(), identity)
}

fn new_record(
	kind: PeerReplyKindV1,
	request_bytes: Vec<u8>,
	response_bytes: Vec<u8>,
	identity: PeerReplayIdentityV1,
) -> Result<PeerReplyRecordV1, ContentError> {
	if request_bytes.len() > MAX_REQUEST_ENCODED {
		return Err(ContentError::ObjectTooLarge);
	}
	let mut record = PeerReplyRecordV1 {
		version: VERSION,
		kind,
		operation_id: identity.operation_id,
		request_nonce: identity.request_nonce,
		request_hash: identity.request_hash,
		request_bytes,
		response_bytes,
		record_hash: [0; 32],
	};
	record.record_hash = record_hash(&record);
	validate_record(&record)?;
	Ok(record)
}

fn validate_record(record: &PeerReplyRecordV1) -> Result<(), ContentError> {
	if record.version != VERSION
		|| record.operation_id == [0; 16]
		|| record.request_nonce == [0; 16]
		|| record.request_hash == [0; 32]
		|| record.request_bytes.is_empty()
		|| record.request_bytes.len() > MAX_REQUEST_ENCODED
		|| record.response_bytes.is_empty()
		|| record.record_hash != record_hash(record)
	{
		return Err(ContentError::IntegrityFailed);
	}
	match record.kind {
		PeerReplyKindV1::Page => {
			if record.response_bytes.len() > MAX_PAGE_RESPONSE_ENCODED {
				return Err(ContentError::IntegrityFailed);
			}
			let request = PeerSyncPageRequestV1::decode_authenticated(&record.request_bytes)?;
			let identity = request.authenticated_replay_identity()?;
			validate_identity(record, identity)?;
			PeerSyncPageResponseV1::decode_canonical(&record.response_bytes, &request)?;
		},
		PeerReplyKindV1::Chunk => {
			if record.response_bytes.len() > MAX_CHUNK_RESPONSE_ENCODED {
				return Err(ContentError::IntegrityFailed);
			}
			let request = PeerChunkRequestV1::decode_authenticated(&record.request_bytes)?;
			let identity = request.authenticated_replay_identity()?;
			validate_identity(record, identity)?;
			PeerChunkResponseV1::decode_canonical(&record.response_bytes, &request)?;
		},
	}
	Ok(())
}

fn validate_identity(
	record: &PeerReplyRecordV1,
	identity: PeerReplayIdentityV1,
) -> Result<(), ContentError> {
	if record.operation_id != identity.operation_id
		|| record.request_nonce != identity.request_nonce
		|| record.request_hash != identity.request_hash
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn validate_page_request_binding(
	record: &PeerReplyRecordV1,
	request: &PeerSyncPageRequestV1,
	identity: PeerReplayIdentityV1,
) -> Result<(), ContentError> {
	if record.kind != PeerReplyKindV1::Page || record.request_bytes != request.encode_wire() {
		return Err(ContentError::IdempotencyConflict);
	}
	validate_identity(record, identity).map_err(|_| ContentError::IdempotencyConflict)?;
	PeerSyncPageResponseV1::decode_canonical(&record.response_bytes, request)?;
	Ok(())
}

fn validate_chunk_request_binding(
	record: &PeerReplyRecordV1,
	request: &PeerChunkRequestV1,
	identity: PeerReplayIdentityV1,
) -> Result<(), ContentError> {
	if record.kind != PeerReplyKindV1::Chunk || record.request_bytes != request.encode_wire() {
		return Err(ContentError::IdempotencyConflict);
	}
	validate_identity(record, identity).map_err(|_| ContentError::IdempotencyConflict)?;
	PeerChunkResponseV1::decode_canonical(&record.response_bytes, request)?;
	Ok(())
}

fn record_hash(record: &PeerReplyRecordV1) -> [u8; 32] {
	let mut input = Vec::with_capacity(
		RECORD_DOMAIN.len() + record.request_bytes.len() + record.response_bytes.len() + 128,
	);
	input.extend_from_slice(RECORD_DOMAIN);
	(
		record.version,
		record.kind,
		record.operation_id,
		record.request_nonce,
		record.request_hash,
		&record.request_bytes,
		&record.response_bytes,
	)
		.encode_to(&mut input);
	sp_crypto_hashing::blake2_256(&input)
}

struct LoadedPeerReplies {
	state: PeerReplyState,
	temp_artifacts: Vec<PathBuf>,
}

fn load_records(
	root: &Path,
	record_limit: usize,
	byte_limit: u64,
) -> Result<LoadedPeerReplies, ContentError> {
	let mut state = PeerReplyState::default();
	let mut visited = 0usize;
	let mut temps = Vec::new();
	for item in fs::read_dir(root).map_err(io_error)? {
		visited = visited.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		if visited > record_limit + MAX_TEMP_ARTIFACTS {
			return Err(ContentError::IntegrityFailed);
		}
		let item = item.map_err(io_error)?;
		let name = item.file_name().into_string().map_err(|_| ContentError::IntegrityFailed)?;
		if is_temp_name(&name) {
			if !item.file_type().map_err(io_error)?.is_file() {
				return Err(ContentError::IntegrityFailed);
			}
			temps.push(item.path());
			if temps.len() > MAX_TEMP_ARTIFACTS {
				return Err(ContentError::IntegrityFailed);
			}
			continue;
		}
		if !item.file_type().map_err(io_error)?.is_file() || !name.ends_with(EXTENSION) {
			return Err(ContentError::IntegrityFailed);
		}
		let bytes = crate::bounded_io::read_regular_file(item.path(), MAX_RECORD_BYTES as u64)?;
		if bytes.is_empty() {
			return Err(ContentError::IntegrityFailed);
		}
		let record: PeerReplyRecordV1 = decode_canonical(&bytes)?;
		validate_record(&record)?;
		let key = record.key();
		let length: u64 = bytes.len().try_into().map_err(|_| ContentError::IntegrityFailed)?;
		state.total_bytes =
			state.total_bytes.checked_add(length).ok_or(ContentError::IntegrityFailed)?;
		if name != key.filename()
			|| state.total_bytes > byte_limit
			|| state.records.len() >= record_limit
			|| state.records.insert(key, record).is_some()
		{
			return Err(ContentError::IntegrityFailed);
		}
	}
	Ok(LoadedPeerReplies { state, temp_artifacts: temps })
}

fn is_temp_name(name: &str) -> bool {
	let Some((record, process)) = name.rsplit_once(".tmp-") else { return false };
	record.ends_with(EXTENSION)
		&& !process.is_empty()
		&& process.bytes().all(|byte| byte.is_ascii_digit())
}

fn decode_canonical<T: Decode + Encode>(bytes: &[u8]) -> Result<T, ContentError> {
	let mut input = bytes;
	let value = T::decode(&mut input).map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || value.encode() != bytes {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(value)
}

fn sync_dir(path: &Path) -> Result<(), ContentError> {
	File::open(path).and_then(|directory| directory.sync_all()).map_err(io_error)
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

fn lock_error() -> ContentError {
	ContentError::Io("peer reply store lock poisoned".into())
}

#[cfg(test)]
mod tests {
	use std::fs::OpenOptions;

	use sp_core::{ed25519, Pair as _};
	use tempfile::TempDir;

	use super::*;
	use crate::{
		peer::{
			PeerChunkExpectationV1, PeerContextV1, PeerMmrCommitmentV1, PeerObjectV1,
			PeerPageCursorV1, PeerPageExpectationV1, PeerRequestIdentityV1,
		},
		CanonicalCid, CHUNK_BYTES, MAX_CHUNKS, MAX_STORED_BYTES,
	};

	fn pair(seed: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[seed; 32])
	}

	fn context(leaf_count: u64) -> PeerContextV1 {
		PeerContextV1::new(
			[1; 32],
			[2; 32],
			10,
			[3; 32],
			[4; 32],
			[5; 32],
			7,
			pair(11).public().0,
			9,
			pair(12).public().0,
			[13; 32],
			[14; 32],
			PeerMmrCommitmentV1::new([17; 32], 0, leaf_count, 0).unwrap(),
		)
		.unwrap()
	}

	fn identity(nonce: u8) -> PeerRequestIdentityV1 {
		PeerRequestIdentityV1::new([15; 16], [nonce; 16]).unwrap()
	}

	fn object(sequence: u64, length: u64, total: u64, seed: u8) -> PeerObjectV1 {
		let hashes = if length == 0 {
			Vec::new()
		} else {
			vec![[seed.max(1); 32]; length.div_ceil(CHUNK_BYTES as u64) as usize]
		};
		PeerObjectV1::new(&CanonicalCid::from_digest([seed; 32]), length, sequence, total, hashes)
			.unwrap()
	}

	fn page_request(nonce: u8, limit: u16, leaf_count: u64) -> PeerSyncPageRequestV1 {
		let expected =
			PeerPageExpectationV1::new(context(leaf_count), identity(nonce), None, limit).unwrap();
		PeerSyncPageRequestV1::new_signed(&expected, &pair(12)).unwrap()
	}

	fn one_item_page(request: &PeerSyncPageRequestV1, length: u64, seed: u8) -> Vec<u8> {
		let (_, end) = request.context().candidate_commitment().sequence_range();
		let next = if end == 1 { None } else { Some(PeerPageCursorV1::new(0, length)) };
		PeerSyncPageResponseV1::new_signed(
			request,
			vec![object(0, length, length, seed)],
			next,
			&pair(11),
		)
		.unwrap()
		.encode_wire()
	}

	fn two_item_page(request: &PeerSyncPageRequestV1) -> Vec<u8> {
		PeerSyncPageResponseV1::new_signed(
			request,
			vec![object(0, 4, 4, 21), object(1, 4, 8, 22)],
			None,
			&pair(11),
		)
		.unwrap()
		.encode_wire()
	}

	fn chunk_exchange(nonce: u8, bytes: &[u8]) -> (PeerChunkRequestV1, Vec<u8>) {
		let digest = sp_crypto_hashing::blake2_256(bytes);
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest(digest),
			bytes.len() as u64,
			0,
			bytes.len() as u64,
			vec![digest],
		)
		.unwrap();
		let expected = PeerChunkExpectationV1::new(context(1), identity(nonce), object, 0).unwrap();
		let request = PeerChunkRequestV1::new_signed(&expected, &pair(12)).unwrap();
		let response =
			PeerChunkResponseV1::new_signed(&request, bytes.to_vec(), &pair(11)).unwrap();
		(request, response.encode_wire())
	}

	fn only_record(root: &Path) -> PathBuf {
		fs::read_dir(root.join(ROOT))
			.unwrap()
			.map(|item| item.unwrap().path())
			.find(|path| path.extension().is_some_and(|extension| extension == "reply"))
			.unwrap()
	}

	#[test]
	fn page_and_chunk_replay_exact_source_bytes_after_restart() {
		let temp = TempDir::new().unwrap();
		let request = page_request(16, 1, 2);
		let mut response = one_item_page(&request, 4, 21);
		let expected_page = response.clone();
		let chunk = vec![31; CHUNK_BYTES];
		let (chunk_request, chunk_response) = chunk_exchange(17, &chunk);
		let store = PeerReplyStore::open(temp.path()).unwrap();
		assert_eq!(store.record_page(&request, &response).unwrap(), expected_page);
		assert_eq!(store.record_chunk(&chunk_request, &chunk_response).unwrap(), chunk_response);
		response.fill(0);
		assert_eq!(store.replay_page(&request).unwrap(), expected_page);
		drop(store);

		let reopened = PeerReplyStore::open(temp.path()).unwrap();
		assert_eq!(reopened.replay_page(&request).unwrap(), expected_page);
		assert_eq!(reopened.replay_chunk(&chunk_request).unwrap(), chunk_response);
		assert_eq!(reopened.record_page(&request, &expected_page).unwrap(), expected_page);
	}

	#[test]
	fn nonce_request_and_response_changes_never_alias_one_reply_key() {
		let temp = TempDir::new().unwrap();
		let request = page_request(20, 1, 2);
		let response = one_item_page(&request, 4, 21);
		let store = PeerReplyStore::open(temp.path()).unwrap();
		store.record_page(&request, &response).unwrap();

		let changed_nonce = page_request(21, 1, 2);
		assert_eq!(store.replay_page(&changed_nonce), Err(ContentError::NotFound));
		let changed_request = page_request(20, 2, 2);
		let changed_request_response = two_item_page(&changed_request);
		assert_eq!(
			store.record_page(&changed_request, &changed_request_response),
			Err(ContentError::IdempotencyConflict)
		);
		let changed_response = one_item_page(&request, 5, 22);
		assert_eq!(
			store.record_page(&request, &changed_response),
			Err(ContentError::IdempotencyConflict)
		);
		let mut trailing = response;
		trailing.push(0);
		assert!(store.record_page(&request, &trailing).is_err());
	}

	#[test]
	fn page_and_chunk_share_one_operation_nonce_replay_identity() {
		let page = page_request(22, 1, 1);
		let page_response = one_item_page(&page, 4, 21);
		let chunk_bytes = vec![31; CHUNK_BYTES];
		let (chunk, chunk_response) = chunk_exchange(22, &chunk_bytes);

		let page_first = TempDir::new().unwrap();
		let store = PeerReplyStore::open(page_first.path()).unwrap();
		store.record_page(&page, &page_response).unwrap();
		assert_eq!(
			store.record_chunk(&chunk, &chunk_response),
			Err(ContentError::IdempotencyConflict)
		);
		assert_eq!(store.replay_chunk(&chunk), Err(ContentError::IdempotencyConflict));

		let chunk_first = TempDir::new().unwrap();
		let store = PeerReplyStore::open(chunk_first.path()).unwrap();
		store.record_chunk(&chunk, &chunk_response).unwrap();
		assert_eq!(
			store.record_page(&page, &page_response),
			Err(ContentError::IdempotencyConflict)
		);
		assert_eq!(store.replay_page(&page), Err(ContentError::IdempotencyConflict));
	}

	#[test]
	fn maximum_page_and_chunk_records_round_trip_with_exact_peer_bounds() {
		let temp = TempDir::new().unwrap();
		let request = page_request(30, 128, 128);
		let items = (0..128u64)
			.map(|sequence| {
				object(
					sequence,
					MAX_STORED_BYTES,
					(sequence + 1) * MAX_STORED_BYTES,
					(sequence as u8).wrapping_add(1),
				)
			})
			.collect();
		let page = PeerSyncPageResponseV1::new_signed(&request, items, None, &pair(11))
			.unwrap()
			.encode_wire();
		assert!(page.len() <= MAX_PAGE_RESPONSE_ENCODED);

		let chunk = vec![41; CHUNK_BYTES];
		let chunk_hash = sp_crypto_hashing::blake2_256(&chunk);
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest([42; 32]),
			MAX_STORED_BYTES,
			0,
			MAX_STORED_BYTES,
			vec![chunk_hash; MAX_CHUNKS],
		)
		.unwrap();
		let expected =
			PeerChunkExpectationV1::new(context(1), identity(31), object, (MAX_CHUNKS - 1) as u16)
				.unwrap();
		let chunk_request = PeerChunkRequestV1::new_signed(&expected, &pair(12)).unwrap();
		let chunk_response = PeerChunkResponseV1::new_signed(&chunk_request, chunk, &pair(11))
			.unwrap()
			.encode_wire();
		assert!(chunk_response.len() <= MAX_CHUNK_RESPONSE_ENCODED);

		let store = PeerReplyStore::open(temp.path()).unwrap();
		assert_eq!(store.record_page(&request, &page).unwrap(), page);
		assert_eq!(store.record_chunk(&chunk_request, &chunk_response).unwrap(), chunk_response);
		assert_eq!(
			store.record_page(&request, &vec![0; MAX_PAGE_RESPONSE_ENCODED + 1]),
			Err(ContentError::ObjectTooLarge)
		);
		assert_eq!(
			store.record_chunk(&chunk_request, &vec![0; MAX_CHUNK_RESPONSE_ENCODED + 1]),
			Err(ContentError::ObjectTooLarge)
		);
	}

	#[test]
	fn count_and_total_byte_capacity_fail_closed_without_losing_exact_replay() {
		let count_root = TempDir::new().unwrap();
		let request = page_request(40, 1, 1);
		let response = one_item_page(&request, 4, 21);
		let second = page_request(41, 1, 1);
		let second_response = one_item_page(&second, 4, 21);
		let store =
			PeerReplyStore::open_with_limits(count_root.path(), 1, MAX_TOTAL_BYTES).unwrap();
		store.record_page(&request, &response).unwrap();
		assert_eq!(
			store.record_page(&second, &second_response),
			Err(ContentError::ProviderRecoveryTableFull)
		);
		assert_eq!(store.replay_page(&request).unwrap(), response);

		let byte_root = TempDir::new().unwrap();
		let exact_bytes = page_record(&request, &response).unwrap().encode().len() as u64;
		let store = PeerReplyStore::open_with_limits(byte_root.path(), 2, exact_bytes).unwrap();
		store.record_page(&request, &response).unwrap();
		assert_eq!(
			store.record_page(&second, &second_response),
			Err(ContentError::ProviderRecoveryTableFull)
		);
		assert_eq!(store.replay_page(&request).unwrap(), response);
	}

	#[test]
	fn tamper_trailing_nonfile_and_temp_artifacts_are_bounded() {
		let request = page_request(50, 1, 1);
		let response = one_item_page(&request, 4, 21);

		let tamper = TempDir::new().unwrap();
		let store = PeerReplyStore::open(tamper.path()).unwrap();
		store.record_page(&request, &response).unwrap();
		drop(store);
		let path = only_record(tamper.path());
		let mut bytes = fs::read(&path).unwrap();
		bytes[8] ^= 1;
		fs::write(&path, bytes).unwrap();
		assert!(PeerReplyStore::open(tamper.path()).is_err());

		let trailing = TempDir::new().unwrap();
		let store = PeerReplyStore::open(trailing.path()).unwrap();
		store.record_page(&request, &response).unwrap();
		drop(store);
		OpenOptions::new()
			.append(true)
			.open(only_record(trailing.path()))
			.unwrap()
			.write_all(&[0])
			.unwrap();
		assert!(PeerReplyStore::open(trailing.path()).is_err());

		let nonfile = TempDir::new().unwrap();
		fs::create_dir_all(nonfile.path().join(ROOT).join("unexpected")).unwrap();
		assert!(PeerReplyStore::open(nonfile.path()).is_err());

		let temps = TempDir::new().unwrap();
		let root = temps.path().join(ROOT);
		fs::create_dir_all(&root).unwrap();
		for index in 0..MAX_TEMP_ARTIFACTS {
			fs::write(root.join(format!("page-a{EXTENSION}.tmp-{index}")), b"partial").unwrap();
		}
		assert!(PeerReplyStore::open(temps.path()).is_ok());
		assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
		for index in 0..=MAX_TEMP_ARTIFACTS {
			fs::write(root.join(format!("page-b{EXTENSION}.tmp-{index}")), b"partial").unwrap();
		}
		assert!(PeerReplyStore::open(temps.path()).is_err());
		let malformed = TempDir::new().unwrap();
		fs::create_dir_all(malformed.path().join(ROOT)).unwrap();
		fs::write(malformed.path().join(ROOT).join("page.reply.tmp-invalid"), b"partial").unwrap();
		assert!(PeerReplyStore::open(malformed.path()).is_err());
	}

	#[test]
	fn every_fault_is_old_or_new_and_poison_blocks_replay_until_reopen() {
		let request = page_request(60, 1, 1);
		let response = one_item_page(&request, 4, 21);
		for fault in [
			PeerReplyFault::BeforeTempFsync,
			PeerReplyFault::AfterTempFsync,
			PeerReplyFault::AfterRename,
			PeerReplyFault::AfterDirectoryFsync,
		] {
			let temp = TempDir::new().unwrap();
			let store = PeerReplyStore::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(store.record_page(&request, &response), Err(ContentError::Io(_))));
			assert_eq!(store.replay_page(&request), Err(ContentError::IntegrityFailed));
			drop(store);
			let reopened = PeerReplyStore::open(temp.path()).unwrap();
			if matches!(fault, PeerReplyFault::AfterRename | PeerReplyFault::AfterDirectoryFsync) {
				assert_eq!(reopened.replay_page(&request).unwrap(), response);
			} else {
				assert_eq!(reopened.replay_page(&request), Err(ContentError::NotFound));
			}
		}
	}
}
