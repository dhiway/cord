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

//! Private crash-safe replication intent state.

use std::{
	collections::BTreeMap,
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
};

use serde::{Deserialize, Serialize};
use sp_crypto_hashing::blake2_256;

use crate::{CanonicalCid, ContentError, CHUNK_BYTES, MAX_CHUNKS, MAX_STORED_BYTES};

const ROOT: &str = "replication-v1";
const VERSION: u16 = 1;
const MAX_RECORDS: usize = 4_096;
const MAX_TEMP_ARTIFACTS: usize = 128;
const MAX_RECORD_BYTES: usize = 32 * 1024;
const MAX_TICK_WORK: usize = 128;
const KEY_DOMAIN: &[u8] = b"origin/replication-intent-key/v1";
const RECORD_DOMAIN: &[u8] = b"origin/replication-intent-record/v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicationIntentInputV1 {
	pub(crate) bucket_id: [u8; 32],
	pub(crate) candidate_mmr_root: [u8; 32],
	pub(crate) candidate_start: u64,
	pub(crate) candidate_count: u64,
	pub(crate) source_provider: [u8; 32],
	pub(crate) target_provider: [u8; 32],
	pub(crate) source_service_key: [u8; 32],
	pub(crate) source_service_key_version: u64,
	pub(crate) target_service_key: [u8; 32],
	pub(crate) target_service_key_version: u64,
	pub(crate) topology_snapshot_hash: [u8; 32],
	pub(crate) topology_finalized_hash: [u8; 32],
	pub(crate) topology_finalized_number: u32,
	pub(crate) topology_version: u64,
	pub(crate) topology_governed_checkpoint: u32,
	pub(crate) peer_operation_id: [u8; 16],
	pub(crate) peer_context_hash: [u8; 32],
	pub(crate) peer_commitment_hash: [u8; 32],
	pub(crate) streaming_operation_id: [u8; 16],
	pub(crate) streaming_repair_id: [u8; 16],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReplicationPhase {
	Planned,
	Receiving,
	Installed,
	MmrCommitted,
	Confirmed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicationPageCursorRefV1 {
	pub(crate) last_sequence: u64,
	pub(crate) cumulative_total: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicationCursorRefV1 {
	pub(crate) object_sequence: u64,
	pub(crate) object_cid: String,
	pub(crate) page_cursor: Option<ReplicationPageCursorRefV1>,
}

/// Transient progress observed from the authoritative streaming journal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StreamingProgressObservationV1 {
	pub(crate) streaming_operation_id: [u8; 16],
	pub(crate) streaming_repair_id: [u8; 16],
	pub(crate) next_chunk: u16,
	pub(crate) staged_bytes: u64,
	pub(crate) ready_to_finalize: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicationIntentV1 {
	version: u16,
	pub(crate) intent_key: String,
	pub(crate) identity: ReplicationIntentInputV1,
	pub(crate) phase: ReplicationPhase,
	pub(crate) current: Option<ReplicationCursorRefV1>,
	pub(crate) attempts: u32,
	pub(crate) last_authenticated_request_hash: Option<[u8; 32]>,
	pub(crate) last_authenticated_response_hash: Option<[u8; 32]>,
	record_hash: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplicationFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
}

pub(crate) struct ReplicationIntentStore {
	root: PathBuf,
	records: RwLock<BTreeMap<String, ReplicationIntentV1>>,
	poisoned: RwLock<bool>,
	fault: RwLock<Option<ReplicationFault>>,
	limit: usize,
}

impl ReplicationIntentStore {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		Self::open_with_limit(root, MAX_RECORDS)
	}

	fn open_with_limit(root: impl AsRef<Path>, limit: usize) -> Result<Self, ContentError> {
		if limit == 0 || limit > MAX_RECORDS {
			return Err(ContentError::SchemaInvalid);
		}
		let root = root.as_ref().join(ROOT);
		fs::create_dir_all(&root).map_err(io_error)?;
		let mut records = BTreeMap::new();
		for file in read_records(&root, limit)? {
			let record: ReplicationIntentV1 =
				serde_json::from_slice(&file.bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_record(&record)?;
			if file.name != format!("{}.json", record.intent_key)
				|| records.insert(record.intent_key.clone(), record).is_some()
			{
				return Err(ContentError::IntegrityFailed);
			}
		}
		sync_dir(&root)?;
		Ok(Self {
			root,
			records: RwLock::new(records),
			poisoned: RwLock::new(false),
			fault: RwLock::new(None),
			limit,
		})
	}

	pub(crate) fn plan(
		&self,
		input: &ReplicationIntentInputV1,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.ensure_healthy()?;
		let candidate = new_record(input)?;
		let mut records = self.records.write().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		if let Some(existing) = records.get(&candidate.intent_key) {
			return if same_identity(existing, &candidate) {
				Ok(existing.clone())
			} else {
				Err(ContentError::IdempotencyConflict)
			};
		}
		if records.len() >= self.limit {
			return Err(ContentError::ProviderRecoveryTableFull);
		}
		self.persist_or_poison(&candidate)?;
		records.insert(candidate.intent_key.clone(), candidate.clone());
		Ok(candidate)
	}

	pub(crate) fn advance(
		&self,
		intent_key: &str,
		expected_phase: ReplicationPhase,
		next_phase: ReplicationPhase,
		current: Option<ReplicationCursorRefV1>,
		progress: Option<StreamingProgressObservationV1>,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.ensure_healthy()?;
		let mut records = self.records.write().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		let existing = records.get(intent_key).cloned().ok_or(ContentError::NotFound)?;
		validate_record(&existing)?;
		validate_observed_progress(&existing, next_phase, progress)?;
		if existing.phase == next_phase && existing.current == current {
			return if exact_replay_predecessor(expected_phase, next_phase) {
				Ok(existing)
			} else {
				Err(ContentError::IdempotencyConflict)
			};
		}
		if existing.phase != expected_phase
			|| !legal_transition(existing.phase, next_phase, &existing.current, &current)
		{
			return Err(ContentError::IdempotencyConflict);
		}
		let mut next = existing.clone();
		next.phase = next_phase;
		next.current = current;
		next.record_hash = record_hash(&next)?;
		validate_record(&next)?;
		self.persist_or_poison(&next)?;
		records.insert(intent_key.into(), next.clone());
		Ok(next)
	}

	pub(crate) fn record_authenticated_attempt(
		&self,
		intent_key: &str,
		request_hash: [u8; 32],
		response_hash: Option<[u8; 32]>,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.ensure_healthy()?;
		if request_hash == [0; 32] || response_hash == Some([0; 32]) {
			return Err(ContentError::SchemaInvalid);
		}
		let mut records = self.records.write().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		let existing = records.get(intent_key).cloned().ok_or(ContentError::NotFound)?;
		if existing.last_authenticated_request_hash == Some(request_hash) {
			if existing.last_authenticated_response_hash == response_hash {
				return Ok(existing);
			}
			if existing.last_authenticated_response_hash.is_some() || response_hash.is_none() {
				return Err(ContentError::IdempotencyConflict);
			}
			let mut completed = existing;
			completed.last_authenticated_response_hash = response_hash;
			completed.record_hash = record_hash(&completed)?;
			validate_record(&completed)?;
			self.persist_or_poison(&completed)?;
			records.insert(intent_key.into(), completed.clone());
			return Ok(completed);
		}
		let mut next = existing;
		next.attempts = next.attempts.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		next.last_authenticated_request_hash = Some(request_hash);
		next.last_authenticated_response_hash = response_hash;
		next.record_hash = record_hash(&next)?;
		validate_record(&next)?;
		self.persist_or_poison(&next)?;
		records.insert(intent_key.into(), next.clone());
		Ok(next)
	}

	pub(crate) fn select_tick(&self) -> Result<Vec<ReplicationIntentV1>, ContentError> {
		self.ensure_healthy()?;
		Ok(self
			.records
			.read()
			.map_err(|_| lock_error())?
			.values()
			.filter(|record| record.phase != ReplicationPhase::Confirmed)
			.take(MAX_TICK_WORK)
			.cloned()
			.collect())
	}

	#[cfg(test)]
	fn inject_fault_once(&self, fault: ReplicationFault) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	fn ensure_healthy(&self) -> Result<(), ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			Err(ContentError::IntegrityFailed)
		} else {
			Ok(())
		}
	}

	fn persist_or_poison(&self, record: &ReplicationIntentV1) -> Result<(), ContentError> {
		if let Err(error) = self.persist(record) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error);
		}
		Ok(())
	}

	fn persist(&self, record: &ReplicationIntentV1) -> Result<(), ContentError> {
		validate_record(record)?;
		let bytes = serde_json::to_vec(record).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed);
		}
		let temp = self.root.join(format!("{}.json.tmp-{}", record.intent_key, std::process::id()));
		let path = self.root.join(format!("{}.json", record.intent_key));
		let mut file = File::create(&temp).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		self.trip(ReplicationFault::BeforeTempFsync)?;
		file.sync_all().map_err(io_error)?;
		self.trip(ReplicationFault::AfterTempFsync)?;
		fs::rename(&temp, path).map_err(io_error)?;
		self.trip(ReplicationFault::AfterRename)?;
		sync_dir(&self.root)?;
		self.trip(ReplicationFault::AfterDirectoryFsync)
	}

	fn trip(&self, point: ReplicationFault) -> Result<(), ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			Err(ContentError::Io(format!("injected replication fault: {point:?}")))
		} else {
			Ok(())
		}
	}
}

struct RecordFile {
	name: String,
	bytes: Vec<u8>,
}

fn read_records(root: &Path, limit: usize) -> Result<Vec<RecordFile>, ContentError> {
	let mut records = Vec::new();
	let mut visited = 0usize;
	let mut temps = 0usize;
	for item in fs::read_dir(root).map_err(io_error)? {
		visited = visited.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		if visited > limit + MAX_TEMP_ARTIFACTS {
			return Err(ContentError::IntegrityFailed);
		}
		let item = item.map_err(io_error)?;
		let name = item.file_name().into_string().map_err(|_| ContentError::IntegrityFailed)?;
		if name.contains(".tmp-") {
			if !item.file_type().map_err(io_error)?.is_file() {
				return Err(ContentError::IntegrityFailed);
			}
			temps = temps.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			if temps > MAX_TEMP_ARTIFACTS {
				return Err(ContentError::IntegrityFailed);
			}
			fs::remove_file(item.path()).map_err(io_error)?;
			continue;
		}
		if records.len() >= limit
			|| !name.ends_with(".json")
			|| !item.file_type().map_err(io_error)?.is_file()
		{
			return Err(ContentError::IntegrityFailed);
		}
		let bytes = fs::read(item.path()).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed);
		}
		records.push(RecordFile { name, bytes });
	}
	Ok(records)
}

fn new_record(input: &ReplicationIntentInputV1) -> Result<ReplicationIntentV1, ContentError> {
	validate_identity(input)?;
	let mut record = ReplicationIntentV1 {
		version: VERSION,
		intent_key: intent_key(input),
		identity: input.clone(),
		phase: ReplicationPhase::Planned,
		current: None,
		attempts: 0,
		last_authenticated_request_hash: None,
		last_authenticated_response_hash: None,
		record_hash: String::new(),
	};
	record.record_hash = record_hash(&record)?;
	validate_record(&record)?;
	Ok(record)
}

fn validate_identity(input: &ReplicationIntentInputV1) -> Result<(), ContentError> {
	if input.bucket_id == [0; 32]
		|| input.candidate_mmr_root == [0; 32]
		|| input.candidate_count == 0
		|| input.candidate_start.checked_add(input.candidate_count).is_none()
		|| input.source_provider == [0; 32]
		|| input.target_provider == [0; 32]
		|| input.source_provider == input.target_provider
		|| input.topology_version == 0
		|| input.source_service_key == [0; 32]
		|| input.source_service_key_version == 0
		|| input.target_service_key == [0; 32]
		|| input.target_service_key_version == 0
		|| input.topology_snapshot_hash == [0; 32]
		|| input.topology_finalized_hash == [0; 32]
		|| input.topology_governed_checkpoint == 0
		|| input.peer_operation_id == [0; 16]
		|| input.peer_context_hash == [0; 32]
		|| input.peer_commitment_hash == [0; 32]
		|| input.streaming_operation_id == [0; 16]
		|| input.streaming_repair_id == [0; 16]
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn validate_record(record: &ReplicationIntentV1) -> Result<(), ContentError> {
	validate_identity(&record.identity)?;
	if record.version != VERSION
		|| record.intent_key != intent_key(&record.identity)
		|| record.record_hash != record_hash(record)?
		|| (record.attempts == 0 && record.last_authenticated_request_hash.is_some())
		|| (record.attempts > 0 && record.last_authenticated_request_hash.is_none())
		|| record.last_authenticated_request_hash == Some([0; 32])
		|| record.last_authenticated_response_hash == Some([0; 32])
		|| (record.last_authenticated_response_hash.is_some()
			&& record.last_authenticated_request_hash.is_none())
	{
		return Err(ContentError::IntegrityFailed);
	}
	match (&record.phase, &record.current) {
		(ReplicationPhase::Planned, None) => {},
		(ReplicationPhase::Planned, Some(_)) => return Err(ContentError::IntegrityFailed),
		(_, Some(current)) => validate_cursor(current, &record.identity)?,
		(_, None) => return Err(ContentError::IntegrityFailed),
	}
	Ok(())
}

fn validate_cursor(
	current: &ReplicationCursorRefV1,
	identity: &ReplicationIntentInputV1,
) -> Result<(), ContentError> {
	CanonicalCid::parse(&current.object_cid)?;
	let end = identity
		.candidate_start
		.checked_add(identity.candidate_count)
		.ok_or(ContentError::IntegrityFailed)?;
	if current.object_sequence < identity.candidate_start || current.object_sequence >= end {
		return Err(ContentError::IntegrityFailed);
	}
	if let Some(cursor) = &current.page_cursor {
		if cursor.last_sequence < current.object_sequence || cursor.last_sequence >= end {
			return Err(ContentError::IntegrityFailed);
		}
	}
	Ok(())
}

fn validate_observed_progress(
	record: &ReplicationIntentV1,
	next_phase: ReplicationPhase,
	progress: Option<StreamingProgressObservationV1>,
) -> Result<(), ContentError> {
	if matches!(next_phase, ReplicationPhase::Receiving | ReplicationPhase::Installed) {
		let progress = progress.ok_or(ContentError::IntegrityFailed)?;
		if progress.streaming_operation_id != record.identity.streaming_operation_id
			|| progress.streaming_repair_id != record.identity.streaming_repair_id
			|| progress.next_chunk as usize > MAX_CHUNKS
			|| progress.staged_bytes > MAX_STORED_BYTES
			|| (progress.next_chunk == 0 && progress.staged_bytes != 0)
			|| (progress.next_chunk > 0 && progress.staged_bytes == 0)
			|| (progress.next_chunk > 0
				&& progress.staged_bytes > progress.next_chunk as u64 * CHUNK_BYTES as u64)
			|| (next_phase == ReplicationPhase::Installed && !progress.ready_to_finalize)
		{
			return Err(ContentError::IntegrityFailed);
		}
	} else if progress.is_some() {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn legal_transition(
	current_phase: ReplicationPhase,
	next_phase: ReplicationPhase,
	current: &Option<ReplicationCursorRefV1>,
	next: &Option<ReplicationCursorRefV1>,
) -> bool {
	match (current_phase, next_phase) {
		(ReplicationPhase::Planned, ReplicationPhase::Receiving) => next.is_some(),
		(ReplicationPhase::Receiving, ReplicationPhase::Receiving) => {
			cursor_advances(current.as_ref(), next.as_ref())
		},
		(ReplicationPhase::Receiving, ReplicationPhase::Installed)
		| (ReplicationPhase::Installed, ReplicationPhase::MmrCommitted)
		| (ReplicationPhase::MmrCommitted, ReplicationPhase::Confirmed) => current == next,
		_ => false,
	}
}

fn exact_replay_predecessor(expected: ReplicationPhase, installed: ReplicationPhase) -> bool {
	matches!(
		(expected, installed),
		(ReplicationPhase::Planned, ReplicationPhase::Receiving)
			| (ReplicationPhase::Receiving, ReplicationPhase::Receiving)
			| (ReplicationPhase::Receiving, ReplicationPhase::Installed)
			| (ReplicationPhase::Installed, ReplicationPhase::MmrCommitted)
			| (ReplicationPhase::MmrCommitted, ReplicationPhase::Confirmed)
	)
}

fn cursor_advances(
	current: Option<&ReplicationCursorRefV1>,
	next: Option<&ReplicationCursorRefV1>,
) -> bool {
	match (current, next) {
		(Some(current), Some(next)) if next.object_sequence > current.object_sequence => true,
		(Some(current), Some(next)) if next.object_sequence == current.object_sequence => {
			next.object_cid == current.object_cid && next.page_cursor != current.page_cursor
		},
		_ => false,
	}
}

fn same_identity(left: &ReplicationIntentV1, right: &ReplicationIntentV1) -> bool {
	left.intent_key == right.intent_key && left.identity == right.identity
}

fn intent_key(input: &ReplicationIntentInputV1) -> String {
	let mut bytes = KEY_DOMAIN.to_vec();
	bytes.extend_from_slice(&input.bucket_id);
	bytes.extend_from_slice(&input.candidate_mmr_root);
	bytes.extend_from_slice(&input.candidate_start.to_le_bytes());
	bytes.extend_from_slice(&input.candidate_count.to_le_bytes());
	bytes.extend_from_slice(&input.target_provider);
	hex::encode(blake2_256(&bytes))
}

fn record_hash(record: &ReplicationIntentV1) -> Result<String, ContentError> {
	let mut canonical = record.clone();
	canonical.record_hash.clear();
	let mut bytes = RECORD_DOMAIN.to_vec();
	bytes.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&bytes)))
}

fn sync_dir(path: &Path) -> Result<(), ContentError> {
	File::open(path).and_then(|directory| directory.sync_all()).map_err(io_error)
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

fn lock_error() -> ContentError {
	ContentError::Io("replication intent lock poisoned".into())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn input(value: u8) -> ReplicationIntentInputV1 {
		ReplicationIntentInputV1 {
			bucket_id: [value; 32],
			candidate_mmr_root: [value + 1; 32],
			candidate_start: value as u64,
			candidate_count: 3,
			source_provider: [1; 32],
			target_provider: [value + 2; 32],
			source_service_key: [3; 32],
			source_service_key_version: 1,
			target_service_key: [value + 4; 32],
			target_service_key_version: 2,
			topology_snapshot_hash: [5; 32],
			topology_finalized_hash: [6; 32],
			topology_finalized_number: 10,
			topology_version: 2,
			topology_governed_checkpoint: 7,
			peer_operation_id: [value + 8; 16],
			peer_context_hash: [9; 32],
			peer_commitment_hash: [10; 32],
			streaming_operation_id: [value + 11; 16],
			streaming_repair_id: [value + 12; 16],
		}
	}

	fn cursor(sequence: u64, value: u8) -> ReplicationCursorRefV1 {
		ReplicationCursorRefV1 {
			object_sequence: sequence,
			object_cid: CanonicalCid::from_digest([value; 32]).to_string(),
			page_cursor: Some(ReplicationPageCursorRefV1 {
				last_sequence: sequence,
				cumulative_total: value as u64,
			}),
		}
	}

	fn progress(input: &ReplicationIntentInputV1, ready: bool) -> StreamingProgressObservationV1 {
		StreamingProgressObservationV1 {
			streaming_operation_id: input.streaming_operation_id,
			streaming_repair_id: input.streaming_repair_id,
			next_chunk: if ready { 2 } else { 1 },
			staged_bytes: if ready { CHUNK_BYTES as u64 + 3 } else { CHUNK_BYTES as u64 },
			ready_to_finalize: ready,
		}
	}

	#[test]
	fn exact_plan_attempt_transition_and_tick_are_bounded_and_idempotent() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let first_input = input(20);
		let planned = store.plan(&first_input).unwrap();
		assert_eq!(store.plan(&first_input).unwrap(), planned);
		let mut changed = first_input.clone();
		changed.peer_context_hash = [99; 32];
		assert_eq!(store.plan(&changed), Err(ContentError::IdempotencyConflict));
		let mut rotated = first_input.clone();
		rotated.target_service_key_version += 1;
		assert_eq!(store.plan(&rotated), Err(ContentError::IdempotencyConflict));

		let attempted = store
			.record_authenticated_attempt(&planned.intent_key, [30; 32], Some([31; 32]))
			.unwrap();
		assert_eq!(attempted.attempts, 1);
		assert_eq!(
			store
				.record_authenticated_attempt(&planned.intent_key, [30; 32], Some([31; 32]))
				.unwrap(),
			attempted
		);
		assert_eq!(
			store.record_authenticated_attempt(&planned.intent_key, [30; 32], Some([32; 32])),
			Err(ContentError::IdempotencyConflict)
		);
		let pending_response =
			store.record_authenticated_attempt(&planned.intent_key, [33; 32], None).unwrap();
		assert_eq!(pending_response.attempts, 2);
		let completed_response = store
			.record_authenticated_attempt(&planned.intent_key, [33; 32], Some([34; 32]))
			.unwrap();
		assert_eq!(completed_response.attempts, 2);
		assert_eq!(completed_response.last_authenticated_response_hash, Some([34; 32]));

		let current = cursor(first_input.candidate_start, 40);
		let receiving = store
			.advance(
				&planned.intent_key,
				ReplicationPhase::Planned,
				ReplicationPhase::Receiving,
				Some(current.clone()),
				Some(progress(&first_input, false)),
			)
			.unwrap();
		assert_eq!(
			store
				.advance(
					&planned.intent_key,
					ReplicationPhase::Planned,
					ReplicationPhase::Receiving,
					Some(current.clone()),
					Some(progress(&first_input, false)),
				)
				.unwrap(),
			receiving
		);
		assert_eq!(store.select_tick().unwrap().len(), 1);

		for value in 21u8..=149 {
			store.plan(&input(value)).unwrap();
		}
		let tick = store.select_tick().unwrap();
		assert_eq!(tick.len(), 128);
		assert!(tick.windows(2).all(|items| items[0].intent_key < items[1].intent_key));
	}

	#[test]
	fn progress_is_observed_not_persisted_and_phase_graph_is_strict() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let input = input(50);
		let mut ungoverned = input.clone();
		ungoverned.topology_governed_checkpoint = 0;
		assert_eq!(store.plan(&ungoverned), Err(ContentError::IntegrityFailed));
		let planned = store.plan(&input).unwrap();
		let current = cursor(input.candidate_start, 51);
		let outside = cursor(input.candidate_start + input.candidate_count, 52);
		assert_eq!(
			store.advance(
				&planned.intent_key,
				ReplicationPhase::Planned,
				ReplicationPhase::Receiving,
				Some(outside),
				Some(progress(&input, false)),
			),
			Err(ContentError::IntegrityFailed)
		);
		assert_eq!(
			store.advance(
				&planned.intent_key,
				ReplicationPhase::Planned,
				ReplicationPhase::Installed,
				Some(current.clone()),
				Some(progress(&input, true)),
			),
			Err(ContentError::IdempotencyConflict)
		);
		store
			.advance(
				&planned.intent_key,
				ReplicationPhase::Planned,
				ReplicationPhase::Receiving,
				Some(current.clone()),
				Some(progress(&input, false)),
			)
			.unwrap();
		let installed = store
			.advance(
				&planned.intent_key,
				ReplicationPhase::Receiving,
				ReplicationPhase::Installed,
				Some(current.clone()),
				Some(progress(&input, true)),
			)
			.unwrap();
		let json = fs::read_to_string(
			temp.path().join(ROOT).join(format!("{}.json", installed.intent_key)),
		)
		.unwrap();
		assert!(!json.contains("next_chunk"));
		assert!(!json.contains("staged_bytes"));
		let committed = store
			.advance(
				&planned.intent_key,
				ReplicationPhase::Installed,
				ReplicationPhase::MmrCommitted,
				Some(current.clone()),
				None,
			)
			.unwrap();
		let confirmed = store
			.advance(
				&planned.intent_key,
				ReplicationPhase::MmrCommitted,
				ReplicationPhase::Confirmed,
				Some(current),
				None,
			)
			.unwrap();
		assert_eq!(committed.phase, ReplicationPhase::MmrCommitted);
		assert_eq!(confirmed.phase, ReplicationPhase::Confirmed);
		assert!(store.select_tick().unwrap().is_empty());
		drop(store);
		let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
		assert!(reopened.select_tick().unwrap().is_empty());
		assert_eq!(reopened.plan(&input).unwrap(), confirmed);
	}

	#[test]
	fn faults_recover_exact_old_or_new_and_poison_until_reopen() {
		for (fault, installed) in [
			(ReplicationFault::BeforeTempFsync, false),
			(ReplicationFault::AfterTempFsync, false),
			(ReplicationFault::AfterRename, true),
			(ReplicationFault::AfterDirectoryFsync, true),
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = ReplicationIntentStore::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(store.plan(&input(60)), Err(ContentError::Io(_))));
			assert_eq!(store.plan(&input(61)), Err(ContentError::IntegrityFailed));
			drop(store);
			let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
			assert_eq!(!reopened.select_tick().unwrap().is_empty(), installed);
		}
	}

	#[test]
	fn tamper_capacity_and_temp_scans_fail_closed() {
		let tamper = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(tamper.path()).unwrap();
		let record = store.plan(&input(70)).unwrap();
		drop(store);
		let path = tamper.path().join(ROOT).join(format!("{}.json", record.intent_key));
		let mut value: serde_json::Value =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		value["attempts"] = 99.into();
		fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
		assert!(matches!(
			ReplicationIntentStore::open(tamper.path()),
			Err(ContentError::IntegrityFailed)
		));

		let semantic_tamper = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(semantic_tamper.path()).unwrap();
		let identity = input(73);
		let planned = store.plan(&identity).unwrap();
		let receiving = store
			.advance(
				&planned.intent_key,
				ReplicationPhase::Planned,
				ReplicationPhase::Receiving,
				Some(cursor(identity.candidate_start, 74)),
				Some(progress(&identity, false)),
			)
			.unwrap();
		drop(store);
		let path = semantic_tamper.path().join(ROOT).join(format!("{}.json", receiving.intent_key));
		let mut invalid: ReplicationIntentV1 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		invalid.current.as_mut().unwrap().object_sequence =
			identity.candidate_start + identity.candidate_count;
		invalid.record_hash = record_hash(&invalid).unwrap();
		fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
		assert!(matches!(
			ReplicationIntentStore::open(semantic_tamper.path()),
			Err(ContentError::IntegrityFailed)
		));

		let capacity = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open_with_limit(capacity.path(), 1).unwrap();
		store.plan(&input(71)).unwrap();
		assert_eq!(store.plan(&input(72)), Err(ContentError::ProviderRecoveryTableFull));

		let temps = tempfile::tempdir().unwrap();
		fs::create_dir_all(temps.path().join(ROOT)).unwrap();
		for index in 0..=MAX_TEMP_ARTIFACTS {
			fs::write(temps.path().join(ROOT).join(format!("orphan.tmp-{index}")), b"x").unwrap();
		}
		assert!(matches!(
			ReplicationIntentStore::open(temps.path()),
			Err(ContentError::IntegrityFailed)
		));
	}
}
