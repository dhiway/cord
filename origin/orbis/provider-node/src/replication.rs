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

use crate::{CanonicalCid, ContentError, MAX_STORED_BYTES};

const ROOT: &str = "replication-v1";
const VERSION: u16 = 2;
const SCHEDULER: &str = "scheduler.json";
const MAX_RECORDS: usize = 4_096;
const MAX_TEMP_ARTIFACTS: usize = 128;
const MAX_RECORD_BYTES: usize = 96 * 1024;
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_TICK_WORK: usize = 128;
const KEY_DOMAIN: &[u8] = b"origin/replication-intent-key/v2";
const RECORD_DOMAIN: &[u8] = b"origin/replication-intent-record/v2";
const READY_DOMAIN: &[u8] = b"origin/replication-full-cid-ready/v1";
const OPERATION_DOMAIN: &[u8] = b"origin/replication-stream-operation/v1";
const REPAIR_DOMAIN: &[u8] = b"origin/replication-stream-repair/v1";
const CONFIRMATION_DOMAIN: &[u8] = b"origin/replication-confirmation-binding/v1";
const SCHEDULER_DOMAIN: &[u8] = b"origin/replication-scheduler/v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicationIntentInputV1 {
	pub(crate) genesis_hash: [u8; 32],
	pub(crate) topology_snapshot_hash: [u8; 32],
	pub(crate) topology_finalized_hash: [u8; 32],
	pub(crate) topology_finalized_number: u32,
	pub(crate) topology_governed_checkpoint: u32,
	pub(crate) bucket_id: [u8; 32],
	pub(crate) bucket_version: u64,
	pub(crate) source_provider: [u8; 32],
	pub(crate) target_provider: [u8; 32],
	pub(crate) source_service_key: [u8; 32],
	pub(crate) source_service_key_version: u64,
	pub(crate) target_service_key: [u8; 32],
	pub(crate) target_service_key_version: u64,
	pub(crate) source_endpoint_hash: [u8; 32],
	pub(crate) target_endpoint_hash: [u8; 32],
	pub(crate) candidate_mmr_root: [u8; 32],
	pub(crate) candidate_start: u64,
	pub(crate) candidate_count: u64,
	pub(crate) candidate_predecessor_total: u64,
	pub(crate) peer_operation_id: [u8; 16],
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReplicationRequestKindV1 {
	Page,
	Chunk,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OutstandingReplicationRequestV1 {
	pub(crate) kind: ReplicationRequestKindV1,
	pub(crate) request_nonce: [u8; 16],
	pub(crate) signed_request_bytes: Vec<u8>,
	pub(crate) request_hash: [u8; 32],
	pub(crate) verified_response_hash: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicatedObjectCompletionV1 {
	pub(crate) sequence: u64,
	pub(crate) cid: String,
	pub(crate) length: u64,
	pub(crate) cumulative_total: u64,
	pub(crate) streaming_operation_id: [u8; 16],
	pub(crate) streaming_repair_id: [u8; 16],
	pub(crate) streaming_receipt_fingerprint: [u8; 32],
	pub(crate) full_cid_ready_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalCommitmentEvidenceV1 {
	pub(crate) mmr_root: [u8; 32],
	pub(crate) start: u64,
	pub(crate) count: u64,
	pub(crate) predecessor_total: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicationConfirmationEvidenceV1 {
	pub(crate) intent_binding: [u8; 32],
	pub(crate) confirmation_hash: [u8; 32],
	pub(crate) finalized_hash: [u8; 32],
	pub(crate) finalized_number: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicationIntentV1 {
	version: u16,
	pub(crate) intent_key: String,
	pub(crate) identity: ReplicationIntentInputV1,
	pub(crate) phase: ReplicationPhase,
	pub(crate) next_sequence: u64,
	pub(crate) cumulative_total: u64,
	pub(crate) last_completed: Option<ReplicatedObjectCompletionV1>,
	pub(crate) outstanding_request: Option<OutstandingReplicationRequestV1>,
	pub(crate) attempts: u32,
	pub(crate) local_commitment: Option<LocalCommitmentEvidenceV1>,
	pub(crate) confirmation: Option<ReplicationConfirmationEvidenceV1>,
	record_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchedulerStateV1 {
	version: u16,
	after_key: Option<String>,
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
	scheduler: RwLock<SchedulerStateV1>,
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
		let loaded = read_state(&root, limit)?;
		let mut records = BTreeMap::new();
		for file in loaded.records {
			let record: ReplicationIntentV1 =
				serde_json::from_slice(&file.bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_record(&record)?;
			if file.name != format!("{}.json", record.intent_key)
				|| records.insert(record.intent_key.clone(), record).is_some()
			{
				return Err(ContentError::IntegrityFailed);
			}
		}
		let scheduler = match loaded.scheduler {
			Some(bytes) => {
				let scheduler: SchedulerStateV1 =
					serde_json::from_slice(&bytes).map_err(|_| ContentError::IntegrityFailed)?;
				validate_scheduler(&scheduler)?;
				scheduler
			},
			None => new_scheduler()?,
		};
		let store = Self {
			root,
			records: RwLock::new(records),
			scheduler: RwLock::new(scheduler),
			poisoned: RwLock::new(false),
			fault: RwLock::new(None),
			limit,
		};
		if !store.root.join(SCHEDULER).exists() {
			let scheduler = store.scheduler.read().map_err(|_| lock_error())?.clone();
			store.persist_scheduler(&scheduler)?;
		}
		Ok(store)
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
			return if existing.identity == candidate.identity {
				Ok(existing.clone())
			} else {
				Err(ContentError::IdempotencyConflict)
			};
		}
		if records.len() >= self.limit {
			return Err(ContentError::ProviderRecoveryTableFull);
		}
		self.persist_record_or_poison(&candidate)?;
		records.insert(candidate.intent_key.clone(), candidate.clone());
		Ok(candidate)
	}

	pub(crate) fn stage_request_before_send(
		&self,
		intent_key: &str,
		request: &OutstandingReplicationRequestV1,
	) -> Result<ReplicationIntentV1, ContentError> {
		validate_request(request, false)?;
		self.mutate(intent_key, |existing| {
			if existing.phase >= ReplicationPhase::Installed {
				return Err(ContentError::IdempotencyConflict);
			}
			if let Some(outstanding) = &existing.outstanding_request {
				if same_request_identity(outstanding, request) {
					return Ok(existing.clone());
				}
				if outstanding.verified_response_hash.is_none()
					|| outstanding.request_nonce == request.request_nonce
				{
					return Err(ContentError::IdempotencyConflict);
				}
			}
			let mut next = existing.clone();
			next.attempts = next.attempts.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			next.outstanding_request = Some(request.clone());
			Ok(next)
		})
	}

	pub(crate) fn attach_verified_response(
		&self,
		intent_key: &str,
		request_hash: [u8; 32],
		response_hash: [u8; 32],
	) -> Result<ReplicationIntentV1, ContentError> {
		if request_hash == [0; 32] || response_hash == [0; 32] {
			return Err(ContentError::SchemaInvalid);
		}
		self.mutate(intent_key, |existing| {
			let outstanding =
				existing.outstanding_request.as_ref().ok_or(ContentError::IdempotencyConflict)?;
			if outstanding.request_hash != request_hash {
				return Err(ContentError::IdempotencyConflict);
			}
			if outstanding.verified_response_hash == Some(response_hash) {
				return Ok(existing.clone());
			}
			if outstanding.verified_response_hash.is_some() {
				return Err(ContentError::IdempotencyConflict);
			}
			let mut next = existing.clone();
			next.outstanding_request.as_mut().expect("checked above").verified_response_hash =
				Some(response_hash);
			Ok(next)
		})
	}

	pub(crate) fn complete_object(
		&self,
		intent_key: &str,
		completion: &ReplicatedObjectCompletionV1,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.mutate(intent_key, |existing| {
			validate_completion(existing, completion)?;
			if existing.last_completed.as_ref() == Some(completion)
				&& existing.next_sequence == completion.sequence.saturating_add(1)
				&& existing.cumulative_total == completion.cumulative_total
			{
				return Ok(existing.clone());
			}
			if !matches!(existing.phase, ReplicationPhase::Planned | ReplicationPhase::Receiving)
				|| completion.sequence != existing.next_sequence
				|| existing.cumulative_total.checked_add(completion.length)
					!= Some(completion.cumulative_total)
			{
				return Err(ContentError::IdempotencyConflict);
			}
			if existing
				.outstanding_request
				.as_ref()
				.and_then(|request| request.verified_response_hash)
				.is_none()
			{
				return Err(ContentError::IdempotencyConflict);
			}
			let mut next = existing.clone();
			next.next_sequence =
				next.next_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			next.cumulative_total = completion.cumulative_total;
			next.last_completed = Some(completion.clone());
			next.phase = ReplicationPhase::Receiving;
			Ok(next)
		})
	}

	pub(crate) fn mark_installed(
		&self,
		intent_key: &str,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.mutate(intent_key, |existing| {
			if existing.phase == ReplicationPhase::Installed {
				return Ok(existing.clone());
			}
			let end = candidate_end(&existing.identity)?;
			if existing.phase != ReplicationPhase::Receiving || existing.next_sequence != end {
				return Err(ContentError::IdempotencyConflict);
			}
			if existing
				.outstanding_request
				.as_ref()
				.is_some_and(|request| request.verified_response_hash.is_none())
			{
				return Err(ContentError::IdempotencyConflict);
			}
			let mut next = existing.clone();
			next.phase = ReplicationPhase::Installed;
			Ok(next)
		})
	}

	pub(crate) fn commit_local_mmr(
		&self,
		intent_key: &str,
		evidence: &LocalCommitmentEvidenceV1,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.mutate(intent_key, |existing| {
			if existing.phase == ReplicationPhase::MmrCommitted
				&& existing.local_commitment.as_ref() == Some(evidence)
			{
				return Ok(existing.clone());
			}
			if existing.phase != ReplicationPhase::Installed
				|| evidence != &expected_commitment(&existing.identity)
			{
				return Err(ContentError::IdempotencyConflict);
			}
			let mut next = existing.clone();
			next.local_commitment = Some(evidence.clone());
			next.phase = ReplicationPhase::MmrCommitted;
			Ok(next)
		})
	}

	pub(crate) fn confirm(
		&self,
		intent_key: &str,
		evidence: &ReplicationConfirmationEvidenceV1,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.mutate(intent_key, |existing| {
			if existing.phase == ReplicationPhase::Confirmed
				&& existing.confirmation.as_ref() == Some(evidence)
			{
				return Ok(existing.clone());
			}
			if existing.phase != ReplicationPhase::MmrCommitted
				|| evidence.confirmation_hash == [0; 32]
				|| evidence.finalized_hash == [0; 32]
				|| evidence.finalized_number < existing.identity.topology_finalized_number
				|| evidence.intent_binding != confirmation_binding(existing)?
			{
				return Err(ContentError::IdempotencyConflict);
			}
			let mut next = existing.clone();
			next.confirmation = Some(evidence.clone());
			next.phase = ReplicationPhase::Confirmed;
			Ok(next)
		})
	}

	pub(crate) fn select_tick(&self) -> Result<Vec<ReplicationIntentV1>, ContentError> {
		self.ensure_healthy()?;
		let records = self.records.read().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		let active: Vec<_> = records
			.values()
			.filter(|record| record.phase != ReplicationPhase::Confirmed)
			.collect();
		if active.is_empty() {
			return Ok(Vec::new());
		}
		let mut scheduler = self.scheduler.write().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		let start = scheduler
			.after_key
			.as_ref()
			.and_then(|after| active.iter().position(|record| record.intent_key > *after))
			.unwrap_or(0);
		let selected: Vec<_> = active
			.iter()
			.cycle()
			.skip(start)
			.take(active.len().min(MAX_TICK_WORK))
			.map(|record| (*record).clone())
			.collect();
		let mut next = scheduler.clone();
		next.after_key = selected.last().map(|record| record.intent_key.clone());
		next.record_hash = scheduler_hash(&next)?;
		self.persist_scheduler_or_poison(&next)?;
		*scheduler = next;
		Ok(selected)
	}

	#[cfg(test)]
	fn inject_fault_once(&self, fault: ReplicationFault) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	fn mutate(
		&self,
		intent_key: &str,
		change: impl FnOnce(&ReplicationIntentV1) -> Result<ReplicationIntentV1, ContentError>,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.ensure_healthy()?;
		let mut records = self.records.write().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		let existing = records.get(intent_key).ok_or(ContentError::NotFound)?;
		let mut next = change(existing)?;
		if &next == existing {
			return Ok(next);
		}
		next.record_hash = record_hash(&next)?;
		validate_record(&next)?;
		self.persist_record_or_poison(&next)?;
		records.insert(intent_key.into(), next.clone());
		Ok(next)
	}

	fn ensure_healthy(&self) -> Result<(), ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			Err(ContentError::IntegrityFailed)
		} else {
			Ok(())
		}
	}

	fn persist_record_or_poison(&self, record: &ReplicationIntentV1) -> Result<(), ContentError> {
		if let Err(error) = self.persist_record(record) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error);
		}
		Ok(())
	}

	fn persist_scheduler_or_poison(&self, state: &SchedulerStateV1) -> Result<(), ContentError> {
		if let Err(error) = self.persist_scheduler(state) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error);
		}
		Ok(())
	}

	fn persist_record(&self, record: &ReplicationIntentV1) -> Result<(), ContentError> {
		validate_record(record)?;
		self.persist_json(&format!("{}.json", record.intent_key), record)
	}

	fn persist_scheduler(&self, state: &SchedulerStateV1) -> Result<(), ContentError> {
		validate_scheduler(state)?;
		self.persist_json(SCHEDULER, state)
	}

	fn persist_json<T: Serialize>(&self, name: &str, value: &T) -> Result<(), ContentError> {
		let bytes = serde_json::to_vec(value).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed);
		}
		let temp = self.root.join(format!("{name}.tmp-{}", std::process::id()));
		let path = self.root.join(name);
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

struct ReadState {
	records: Vec<RecordFile>,
	scheduler: Option<Vec<u8>>,
}

fn read_state(root: &Path, limit: usize) -> Result<ReadState, ContentError> {
	let mut records = Vec::new();
	let mut scheduler = None;
	let mut visited = 0usize;
	let mut temps = 0usize;
	for item in fs::read_dir(root).map_err(io_error)? {
		visited = visited.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		if visited > limit + MAX_TEMP_ARTIFACTS + 1 {
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
		if !item.file_type().map_err(io_error)?.is_file() {
			return Err(ContentError::IntegrityFailed);
		}
		let bytes = fs::read(item.path()).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed);
		}
		if name == SCHEDULER {
			if scheduler.replace(bytes).is_some() {
				return Err(ContentError::IntegrityFailed);
			}
		} else {
			if records.len() >= limit || !name.ends_with(".json") {
				return Err(ContentError::IntegrityFailed);
			}
			records.push(RecordFile { name, bytes });
		}
	}
	sync_dir(root)?;
	Ok(ReadState { records, scheduler })
}

fn new_record(input: &ReplicationIntentInputV1) -> Result<ReplicationIntentV1, ContentError> {
	validate_identity(input)?;
	let mut record = ReplicationIntentV1 {
		version: VERSION,
		intent_key: intent_key(input),
		identity: input.clone(),
		phase: ReplicationPhase::Planned,
		next_sequence: input.candidate_start,
		cumulative_total: input.candidate_predecessor_total,
		last_completed: None,
		outstanding_request: None,
		attempts: 0,
		local_commitment: None,
		confirmation: None,
		record_hash: String::new(),
	};
	record.record_hash = record_hash(&record)?;
	validate_record(&record)?;
	Ok(record)
}

fn validate_identity(input: &ReplicationIntentInputV1) -> Result<(), ContentError> {
	if input.genesis_hash == [0; 32]
		|| input.topology_snapshot_hash == [0; 32]
		|| input.topology_finalized_hash == [0; 32]
		|| input.topology_governed_checkpoint == 0
		|| input.bucket_id == [0; 32]
		|| input.bucket_version == 0
		|| input.source_provider == [0; 32]
		|| input.target_provider == [0; 32]
		|| input.source_provider == input.target_provider
		|| input.source_service_key == [0; 32]
		|| input.target_service_key == [0; 32]
		|| input.source_service_key == input.target_service_key
		|| input.source_service_key_version == 0
		|| input.target_service_key_version == 0
		|| input.source_endpoint_hash == [0; 32]
		|| input.target_endpoint_hash == [0; 32]
		|| input.source_endpoint_hash == input.target_endpoint_hash
		|| input.candidate_mmr_root == [0; 32]
		|| input.candidate_count == 0
		|| input.candidate_start.checked_add(input.candidate_count).is_none()
		|| (input.candidate_start == 0 && input.candidate_predecessor_total != 0)
		|| input.peer_operation_id == [0; 16]
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
		|| record.next_sequence < record.identity.candidate_start
		|| record.next_sequence > candidate_end(&record.identity)?
		|| record.cumulative_total < record.identity.candidate_predecessor_total
		|| (record.attempts == 0 && record.outstanding_request.is_some())
		|| (record.attempts > 0 && record.outstanding_request.is_none())
	{
		return Err(ContentError::IntegrityFailed);
	}
	if let Some(request) = &record.outstanding_request {
		validate_request(request, true)?;
	}
	if let Some(completed) = &record.last_completed {
		validate_completion(record, completed)?;
		if completed.sequence.checked_add(1) != Some(record.next_sequence) {
			return Err(ContentError::IntegrityFailed);
		}
		if completed.cumulative_total != record.cumulative_total {
			return Err(ContentError::IntegrityFailed);
		}
	} else if record.next_sequence != record.identity.candidate_start
		|| record.cumulative_total != record.identity.candidate_predecessor_total
	{
		return Err(ContentError::IntegrityFailed);
	}
	match record.phase {
		ReplicationPhase::Planned
			if record.next_sequence == record.identity.candidate_start
				&& record.local_commitment.is_none()
				&& record.confirmation.is_none() => {},
		ReplicationPhase::Receiving
			if record.next_sequence > record.identity.candidate_start
				&& record.local_commitment.is_none()
				&& record.confirmation.is_none() => {},
		ReplicationPhase::Installed
			if record.next_sequence == candidate_end(&record.identity)?
				&& record.local_commitment.is_none()
				&& record.confirmation.is_none() => {},
		ReplicationPhase::MmrCommitted
			if record.next_sequence == candidate_end(&record.identity)?
				&& record.local_commitment.as_ref()
					== Some(&expected_commitment(&record.identity))
				&& record.confirmation.is_none() => {},
		ReplicationPhase::Confirmed
			if record.next_sequence == candidate_end(&record.identity)?
				&& record.local_commitment.as_ref()
					== Some(&expected_commitment(&record.identity))
				&& record.confirmation.as_ref().is_some_and(|evidence| {
					evidence.confirmation_hash != [0; 32]
						&& evidence.finalized_hash != [0; 32]
						&& evidence.finalized_number >= record.identity.topology_finalized_number
						&& confirmation_binding(record)
							.is_ok_and(|binding| evidence.intent_binding == binding)
				}) => {},
		_ => return Err(ContentError::IntegrityFailed),
	}
	Ok(())
}

fn validate_request(
	request: &OutstandingReplicationRequestV1,
	allow_response: bool,
) -> Result<(), ContentError> {
	if request.request_nonce == [0; 16]
		|| request.signed_request_bytes.is_empty()
		|| request.signed_request_bytes.len() > MAX_REQUEST_BYTES
		|| request.request_hash != blake2_256(&request.signed_request_bytes)
		|| (!allow_response && request.verified_response_hash.is_some())
		|| request.verified_response_hash == Some([0; 32])
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn same_request_identity(
	left: &OutstandingReplicationRequestV1,
	right: &OutstandingReplicationRequestV1,
) -> bool {
	left.kind == right.kind
		&& left.request_nonce == right.request_nonce
		&& left.signed_request_bytes == right.signed_request_bytes
		&& left.request_hash == right.request_hash
}

fn validate_completion(
	record: &ReplicationIntentV1,
	completion: &ReplicatedObjectCompletionV1,
) -> Result<(), ContentError> {
	let cid = CanonicalCid::parse(&completion.cid)?;
	let end = candidate_end(&record.identity)?;
	if completion.sequence < record.identity.candidate_start
		|| completion.sequence >= end
		|| completion.length > MAX_STORED_BYTES
		|| completion.streaming_operation_id
			!= derived_stream_id(&record.intent_key, completion.sequence, OPERATION_DOMAIN)
		|| completion.streaming_repair_id
			!= derived_stream_id(&record.intent_key, completion.sequence, REPAIR_DOMAIN)
		|| completion.streaming_operation_id == completion.streaming_repair_id
		|| completion.streaming_receipt_fingerprint == [0; 32]
		|| completion.full_cid_ready_hash
			!= full_cid_ready_hash(
				&record.intent_key,
				completion.sequence,
				cid.digest(),
				completion.length,
				completion.cumulative_total,
				completion.streaming_operation_id,
				completion.streaming_repair_id,
				completion.streaming_receipt_fingerprint,
			) {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn expected_commitment(identity: &ReplicationIntentInputV1) -> LocalCommitmentEvidenceV1 {
	LocalCommitmentEvidenceV1 {
		mmr_root: identity.candidate_mmr_root,
		start: identity.candidate_start,
		count: identity.candidate_count,
		predecessor_total: identity.candidate_predecessor_total,
	}
}

fn candidate_end(identity: &ReplicationIntentInputV1) -> Result<u64, ContentError> {
	identity
		.candidate_start
		.checked_add(identity.candidate_count)
		.ok_or(ContentError::IntegrityFailed)
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

fn derived_stream_id(intent_key: &str, sequence: u64, domain: &[u8]) -> [u8; 16] {
	let mut bytes = domain.to_vec();
	bytes.extend_from_slice(intent_key.as_bytes());
	bytes.extend_from_slice(&sequence.to_le_bytes());
	let digest = blake2_256(&bytes);
	let mut id = [0; 16];
	id.copy_from_slice(&digest[..16]);
	id
}

fn full_cid_ready_hash(
	intent_key: &str,
	sequence: u64,
	cid_digest: [u8; 32],
	length: u64,
	cumulative_total: u64,
	operation_id: [u8; 16],
	repair_id: [u8; 16],
	receipt_fingerprint: [u8; 32],
) -> [u8; 32] {
	let mut bytes = READY_DOMAIN.to_vec();
	bytes.extend_from_slice(intent_key.as_bytes());
	bytes.extend_from_slice(&sequence.to_le_bytes());
	bytes.extend_from_slice(&cid_digest);
	bytes.extend_from_slice(&length.to_le_bytes());
	bytes.extend_from_slice(&cumulative_total.to_le_bytes());
	bytes.extend_from_slice(&operation_id);
	bytes.extend_from_slice(&repair_id);
	bytes.extend_from_slice(&receipt_fingerprint);
	blake2_256(&bytes)
}

fn confirmation_binding(record: &ReplicationIntentV1) -> Result<[u8; 32], ContentError> {
	let commitment = record.local_commitment.as_ref().ok_or(ContentError::IntegrityFailed)?;
	let mut bytes = CONFIRMATION_DOMAIN.to_vec();
	bytes.extend_from_slice(record.intent_key.as_bytes());
	bytes.extend_from_slice(&commitment.mmr_root);
	bytes.extend_from_slice(&commitment.start.to_le_bytes());
	bytes.extend_from_slice(&commitment.count.to_le_bytes());
	bytes.extend_from_slice(&commitment.predecessor_total.to_le_bytes());
	bytes.extend_from_slice(&serde_json::to_vec(&record.identity).map_err(io_error)?);
	Ok(blake2_256(&bytes))
}

fn record_hash(record: &ReplicationIntentV1) -> Result<String, ContentError> {
	let mut canonical = record.clone();
	canonical.record_hash.clear();
	let mut bytes = RECORD_DOMAIN.to_vec();
	bytes.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&bytes)))
}

fn new_scheduler() -> Result<SchedulerStateV1, ContentError> {
	let mut scheduler =
		SchedulerStateV1 { version: VERSION, after_key: None, record_hash: String::new() };
	scheduler.record_hash = scheduler_hash(&scheduler)?;
	Ok(scheduler)
}

fn scheduler_hash(state: &SchedulerStateV1) -> Result<String, ContentError> {
	let mut canonical = state.clone();
	canonical.record_hash.clear();
	let mut bytes = SCHEDULER_DOMAIN.to_vec();
	bytes.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&bytes)))
}

fn validate_scheduler(state: &SchedulerStateV1) -> Result<(), ContentError> {
	if state.version != VERSION || state.record_hash != scheduler_hash(state)? {
		return Err(ContentError::IntegrityFailed);
	}
	if let Some(key) = &state.after_key {
		if key.len() != 64
			|| key.bytes().any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
		{
			return Err(ContentError::IntegrityFailed);
		}
	}
	Ok(())
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

	fn bytes32(value: u16, salt: u8) -> [u8; 32] {
		let mut bytes = [salt; 32];
		bytes[..2].copy_from_slice(&value.to_le_bytes());
		bytes
	}

	fn input(value: u16) -> ReplicationIntentInputV1 {
		ReplicationIntentInputV1 {
			genesis_hash: [1; 32],
			topology_snapshot_hash: bytes32(value, 2),
			topology_finalized_hash: [3; 32],
			topology_finalized_number: 100,
			topology_governed_checkpoint: 90,
			bucket_id: bytes32(value, 4),
			bucket_version: 5,
			source_provider: [6; 32],
			target_provider: bytes32(value, 7),
			source_service_key: [8; 32],
			source_service_key_version: 1,
			target_service_key: bytes32(value, 9),
			target_service_key_version: 2,
			source_endpoint_hash: [10; 32],
			target_endpoint_hash: bytes32(value, 11),
			candidate_mmr_root: bytes32(value, 12),
			candidate_start: value as u64 + 1,
			candidate_count: 3,
			candidate_predecessor_total: 1000,
			peer_operation_id: {
				let mut id = [13; 16];
				id[..2].copy_from_slice(&value.to_le_bytes());
				id
			},
		}
	}

	fn request(value: u8, kind: ReplicationRequestKindV1) -> OutstandingReplicationRequestV1 {
		let bytes = vec![value; 48];
		OutstandingReplicationRequestV1 {
			kind,
			request_nonce: [value; 16],
			request_hash: blake2_256(&bytes),
			signed_request_bytes: bytes,
			verified_response_hash: None,
		}
	}

	fn completion(
		record: &ReplicationIntentV1,
		sequence: u64,
		value: u8,
	) -> ReplicatedObjectCompletionV1 {
		let cid = CanonicalCid::from_digest([value; 32]);
		let operation = derived_stream_id(&record.intent_key, sequence, OPERATION_DOMAIN);
		let repair = derived_stream_id(&record.intent_key, sequence, REPAIR_DOMAIN);
		let receipt_fingerprint = [value.wrapping_add(1); 32];
		ReplicatedObjectCompletionV1 {
			sequence,
			cid: cid.to_string(),
			length: value as u64 + 10,
			cumulative_total: record.cumulative_total + value as u64 + 10,
			streaming_operation_id: operation,
			streaming_repair_id: repair,
			streaming_receipt_fingerprint: receipt_fingerprint,
			full_cid_ready_hash: full_cid_ready_hash(
				&record.intent_key,
				sequence,
				cid.digest(),
				value as u64 + 10,
				record.cumulative_total + value as u64 + 10,
				operation,
				repair,
				receipt_fingerprint,
			),
		}
	}

	fn finish_objects(
		store: &ReplicationIntentStore,
		record: ReplicationIntentV1,
	) -> ReplicationIntentV1 {
		let mut record = with_verified_page(store, record);
		let end = candidate_end(&record.identity).unwrap();
		while record.next_sequence < end {
			let item = completion(&record, record.next_sequence, record.next_sequence as u8);
			record = store.complete_object(&record.intent_key, &item).unwrap();
		}
		store.mark_installed(&record.intent_key).unwrap()
	}

	fn with_verified_page(
		store: &ReplicationIntentStore,
		record: ReplicationIntentV1,
	) -> ReplicationIntentV1 {
		if record
			.outstanding_request
			.as_ref()
			.and_then(|request| request.verified_response_hash)
			.is_some()
		{
			return record;
		}
		let page = request(77, ReplicationRequestKindV1::Page);
		let staged = store.stage_request_before_send(&record.intent_key, &page).unwrap();
		store
			.attach_verified_response(&staged.intent_key, page.request_hash, [78; 32])
			.unwrap()
	}

	#[test]
	fn request_envelope_is_canonical_durable_and_cannot_be_overwritten() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let planned = store.plan(&input(20)).unwrap();
		let mut changed_session = input(20);
		changed_session.target_service_key_version += 1;
		assert_eq!(store.plan(&changed_session), Err(ContentError::IdempotencyConflict));
		assert_eq!(
			store.attach_verified_response(&planned.intent_key, [1; 32], [2; 32]),
			Err(ContentError::IdempotencyConflict)
		);
		let page = request(21, ReplicationRequestKindV1::Page);
		let mut noncanonical = page.clone();
		noncanonical.request_hash[0] ^= 1;
		assert!(store.stage_request_before_send(&planned.intent_key, &noncanonical).is_err());
		let staged = store.stage_request_before_send(&planned.intent_key, &page).unwrap();
		assert_eq!(staged.attempts, 1);
		assert_eq!(store.stage_request_before_send(&planned.intent_key, &page).unwrap(), staged);
		assert_eq!(
			store.stage_request_before_send(
				&planned.intent_key,
				&request(22, ReplicationRequestKindV1::Chunk)
			),
			Err(ContentError::IdempotencyConflict)
		);
		let answered = store
			.attach_verified_response(&planned.intent_key, page.request_hash, [23; 32])
			.unwrap();
		assert_eq!(
			store
				.attach_verified_response(&planned.intent_key, page.request_hash, [23; 32])
				.unwrap(),
			answered
		);
		drop(store);
		let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
		assert_eq!(reopened.plan(&input(20)).unwrap(), answered);
		assert_eq!(
			reopened.stage_request_before_send(&planned.intent_key, &page).unwrap(),
			answered
		);
		let record_bytes =
			fs::read_to_string(temp.path().join(ROOT).join(format!("{}.json", planned.intent_key)))
				.unwrap();
		assert!(!record_bytes.contains(&"23".repeat(1024)));
	}

	#[test]
	fn objects_are_contiguous_full_cid_ready_and_use_distinct_derived_ids() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let planned = with_verified_page(&store, store.plan(&input(30)).unwrap());
		let first = completion(&planned, planned.next_sequence, 31);
		let skipped = completion(&planned, planned.next_sequence + 1, 31);
		assert_eq!(
			store.complete_object(&planned.intent_key, &skipped),
			Err(ContentError::IdempotencyConflict)
		);
		let receiving = store.complete_object(&planned.intent_key, &first).unwrap();
		assert_eq!(store.complete_object(&planned.intent_key, &first).unwrap(), receiving);
		let oscillated = completion(&receiving, first.sequence, 99);
		assert!(store.complete_object(&planned.intent_key, &oscillated).is_err());
		let second = completion(&receiving, receiving.next_sequence, 32);
		assert_ne!(first.streaming_operation_id, second.streaming_operation_id);
		assert_ne!(first.streaming_repair_id, second.streaming_repair_id);
		let receiving = store.complete_object(&planned.intent_key, &second).unwrap();
		assert_eq!(
			store.complete_object(&planned.intent_key, &first),
			Err(ContentError::IdempotencyConflict)
		);
		assert_eq!(
			store.mark_installed(&planned.intent_key),
			Err(ContentError::IdempotencyConflict)
		);
		let installed = finish_objects(&store, receiving);
		assert_eq!(installed.phase, ReplicationPhase::Installed);
		assert_eq!(installed.next_sequence, candidate_end(&installed.identity).unwrap());
		let json = serde_json::to_string(&installed).unwrap();
		assert!(!json.contains("next_chunk"));
		assert!(!json.contains("staged_bytes"));
	}

	#[test]
	fn commitment_and_confirmation_require_exact_bound_evidence() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let planned = store.plan(&input(40)).unwrap();
		let expected = expected_commitment(&planned.identity);
		let early_confirmation = ReplicationConfirmationEvidenceV1 {
			intent_binding: [1; 32],
			confirmation_hash: [2; 32],
			finalized_hash: [3; 32],
			finalized_number: 120,
		};
		assert_eq!(
			store.confirm(&planned.intent_key, &early_confirmation),
			Err(ContentError::IdempotencyConflict)
		);
		assert_eq!(
			store.commit_local_mmr(&planned.intent_key, &expected),
			Err(ContentError::IdempotencyConflict)
		);
		let installed = finish_objects(&store, planned);
		let mut wrong = expected.clone();
		wrong.mmr_root[0] ^= 1;
		assert_eq!(
			store.commit_local_mmr(&installed.intent_key, &wrong),
			Err(ContentError::IdempotencyConflict)
		);
		let committed = store.commit_local_mmr(&installed.intent_key, &expected).unwrap();
		let mut confirmation = ReplicationConfirmationEvidenceV1 {
			intent_binding: confirmation_binding(&committed).unwrap(),
			confirmation_hash: [41; 32],
			finalized_hash: [42; 32],
			finalized_number: 120,
		};
		let mut mismatch = confirmation.clone();
		mismatch.intent_binding[0] ^= 1;
		assert_eq!(
			store.confirm(&committed.intent_key, &mismatch),
			Err(ContentError::IdempotencyConflict)
		);
		let confirmed = store.confirm(&committed.intent_key, &confirmation).unwrap();
		assert_eq!(store.confirm(&committed.intent_key, &confirmation).unwrap(), confirmed);
		confirmation.confirmation_hash[0] ^= 1;
		assert_eq!(
			store.confirm(&committed.intent_key, &confirmation),
			Err(ContentError::IdempotencyConflict)
		);
	}

	#[test]
	fn scheduler_is_fair_across_multiple_and_reopened_ticks() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		for value in 1..=260 {
			store.plan(&input(value)).unwrap();
		}
		let first = store.select_tick().unwrap();
		let second = store.select_tick().unwrap();
		assert_eq!(first.len(), 128);
		assert_eq!(second.len(), 128);
		assert!(first
			.iter()
			.all(|item| !second.iter().any(|other| item.intent_key == other.intent_key)));
		drop(store);
		let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
		let third = reopened.select_tick().unwrap();
		assert!(third.iter().any(|item| {
			!first.iter().any(|other| item.intent_key == other.intent_key)
				&& !second.iter().any(|other| item.intent_key == other.intent_key)
		}));
	}

	#[test]
	fn faults_tamper_bounds_and_poison_fail_closed() {
		for (fault, installed) in [
			(ReplicationFault::BeforeTempFsync, false),
			(ReplicationFault::AfterTempFsync, false),
			(ReplicationFault::AfterRename, true),
			(ReplicationFault::AfterDirectoryFsync, true),
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = ReplicationIntentStore::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(store.plan(&input(300)), Err(ContentError::Io(_))));
			assert_eq!(store.select_tick(), Err(ContentError::IntegrityFailed));
			drop(store);
			let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
			assert_eq!(!reopened.select_tick().unwrap().is_empty(), installed);
		}

		let tamper = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(tamper.path()).unwrap();
		let record = store.plan(&input(400)).unwrap();
		drop(store);
		let path = tamper.path().join(ROOT).join(format!("{}.json", record.intent_key));
		let mut invalid: ReplicationIntentV1 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		invalid.next_sequence += 1;
		invalid.record_hash = record_hash(&invalid).unwrap();
		fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
		assert!(matches!(
			ReplicationIntentStore::open(tamper.path()),
			Err(ContentError::IntegrityFailed)
		));

		let capacity = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open_with_limit(capacity.path(), 1).unwrap();
		store.plan(&input(500)).unwrap();
		assert_eq!(store.plan(&input(501)), Err(ContentError::ProviderRecoveryTableFull));
		let max_bytes = vec![51; MAX_REQUEST_BYTES];
		let max_request = OutstandingReplicationRequestV1 {
			kind: ReplicationRequestKindV1::Chunk,
			request_nonce: [51; 16],
			request_hash: blake2_256(&max_bytes),
			signed_request_bytes: max_bytes,
			verified_response_hash: None,
		};
		store.stage_request_before_send(&intent_key(&input(500)), &max_request).unwrap();
		let oversized = OutstandingReplicationRequestV1 {
			signed_request_bytes: vec![1; MAX_REQUEST_BYTES + 1],
			..request(50, ReplicationRequestKindV1::Chunk)
		};
		assert!(store.stage_request_before_send(&intent_key(&input(500)), &oversized).is_err());
	}
}
