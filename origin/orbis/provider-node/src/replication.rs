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
use sp_core::{ed25519, Pair as _};
use sp_crypto_hashing::blake2_256;

use crate::{
	peer::{
		PeerChunkRequestV1, PeerChunkResponseV1, PeerContextV1, PeerMmrCommitmentV1,
		PeerReplayIdentityV1, PeerSyncPageRequestV1, PeerSyncPageResponseV1, MAX_REQUEST_ENCODED,
	},
	storage::bucket_mmr::BucketMmrStore,
	BucketId, CanonicalCid, ContentError, OperationId, StreamingStore, MAX_STORED_BYTES,
};

const ROOT: &str = "replication-v1";
const VERSION: u16 = 2;
const SCHEDULER: &str = "scheduler.json";
const MAX_RECORDS: usize = 4_096;
const MAX_TEMP_ARTIFACTS: usize = 128;
const MAX_RECORD_BYTES: usize = 96 * 1024;
const MAX_TICK_WORK: usize = 128;
const KEY_DOMAIN: &[u8] = b"origin/replication-intent-key/v2";
const RECORD_DOMAIN: &[u8] = b"origin/replication-intent-record/v2";
const OPERATION_DOMAIN: &[u8] = b"origin/replication-stream-operation/v1";
const REPAIR_DOMAIN: &[u8] = b"origin/replication-stream-repair/v1";
const CONFIRMATION_DOMAIN: &[u8] = b"origin/replication-confirmation-binding/v1";
const CONFIRMATION_SIGNATURE_DOMAIN: &[u8] = b"origin/replication-target-confirmation/v1";
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
	pub(crate) target_may_confirm: bool,
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
	pub(crate) target_service_key_version: u64,
	pub(crate) target_signature: Vec<u8>,
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

	pub(crate) fn stage_page_request_before_send(
		&self,
		intent_key: &str,
		signed_request_bytes: &[u8],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.stage_request_before_send(
			intent_key,
			ReplicationRequestKindV1::Page,
			signed_request_bytes,
		)
	}

	pub(crate) fn stage_chunk_request_before_send(
		&self,
		intent_key: &str,
		signed_request_bytes: &[u8],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.stage_request_before_send(
			intent_key,
			ReplicationRequestKindV1::Chunk,
			signed_request_bytes,
		)
	}

	fn stage_request_before_send(
		&self,
		intent_key: &str,
		kind: ReplicationRequestKindV1,
		signed_request_bytes: &[u8],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.mutate(intent_key, |existing| {
			if existing.phase >= ReplicationPhase::Installed {
				return Err(ContentError::IdempotencyConflict);
			}
			let replay = authenticate_request(&existing.identity, kind, signed_request_bytes)?;
			let request = OutstandingReplicationRequestV1 {
				kind,
				request_nonce: replay.request_nonce,
				signed_request_bytes: signed_request_bytes.to_vec(),
				request_hash: replay.request_hash,
				verified_response_hash: None,
			};
			if let Some(outstanding) = &existing.outstanding_request {
				if same_request_identity(outstanding, &request) {
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
			next.outstanding_request = Some(request);
			Ok(next)
		})
	}

	pub(crate) fn attach_page_response(
		&self,
		intent_key: &str,
		response_bytes: &[u8],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.attach_response(intent_key, ReplicationRequestKindV1::Page, response_bytes)
	}

	pub(crate) fn attach_chunk_response(
		&self,
		intent_key: &str,
		response_bytes: &[u8],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.attach_response(intent_key, ReplicationRequestKindV1::Chunk, response_bytes)
	}

	fn attach_response(
		&self,
		intent_key: &str,
		kind: ReplicationRequestKindV1,
		response_bytes: &[u8],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.mutate(intent_key, |existing| {
			let outstanding =
				existing.outstanding_request.as_ref().ok_or(ContentError::IdempotencyConflict)?;
			if outstanding.kind != kind {
				return Err(ContentError::IdempotencyConflict);
			}
			validate_request(&existing.identity, outstanding, true)?;
			let response_hash = match kind {
				ReplicationRequestKindV1::Page => {
					let request = PeerSyncPageRequestV1::decode_authenticated(
						&outstanding.signed_request_bytes,
					)?;
					let response =
						PeerSyncPageResponseV1::decode_canonical(response_bytes, &request)?;
					blake2_256(&response.encode_wire())
				},
				ReplicationRequestKindV1::Chunk => {
					let request = PeerChunkRequestV1::decode_authenticated(
						&outstanding.signed_request_bytes,
					)?;
					let response = PeerChunkResponseV1::decode_canonical(response_bytes, &request)?;
					blake2_256(&response.encode_wire())
				},
			};
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
		streaming: &StreamingStore,
		intent_key: &str,
		sequence: u64,
		cid: &str,
		length: u64,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.ensure_healthy()?;
		let identity = self
			.records
			.read()
			.map_err(|_| lock_error())?
			.get(intent_key)
			.map(|record| record.identity.clone())
			.ok_or(ContentError::NotFound)?;
		let operation_id = derived_stream_id(intent_key, sequence, OPERATION_DOMAIN);
		let repair_id = derived_stream_id(intent_key, sequence, REPAIR_DOMAIN);
		let ready = streaming.verified_replication_ready(
			BucketId::from_bytes(identity.bucket_id),
			cid,
			length,
			OperationId::from_bytes(operation_id),
			OperationId::from_bytes(repair_id),
		)?;
		let completion = ReplicatedObjectCompletionV1 {
			sequence,
			cid: CanonicalCid::parse(cid)?.to_string(),
			length,
			cumulative_total: 0,
			streaming_operation_id: operation_id,
			streaming_repair_id: repair_id,
			streaming_receipt_fingerprint: ready.receipt_fingerprint,
		};
		self.mutate(intent_key, |existing| {
			let mut completion = completion.clone();
			completion.cumulative_total = existing
				.cumulative_total
				.checked_add(completion.length)
				.ok_or(ContentError::IntegrityFailed)?;
			if let Some(last) = &existing.last_completed {
				if last.sequence == completion.sequence {
					completion.cumulative_total = last.cumulative_total;
				}
			}
			validate_completion(existing, &completion)?;
			if existing.last_completed.as_ref() == Some(&completion)
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
			next.last_completed = Some(completion);
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
		mmr: &BucketMmrStore,
		streaming: &StreamingStore,
		intent_key: &str,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.ensure_healthy()?;
		let identity = self
			.records
			.read()
			.map_err(|_| lock_error())?
			.get(intent_key)
			.map(|record| record.identity.clone())
			.ok_or(ContentError::NotFound)?;
		let bucket = BucketId::from_bytes(identity.bucket_id);
		let candidate = mmr.commitment_candidate(streaming, bucket, identity.candidate_start)?;
		let evidence = LocalCommitmentEvidenceV1 {
			mmr_root: candidate.mmr_root.0,
			start: candidate.start_seq,
			count: candidate.leaf_count,
			predecessor_total: mmr
				.commitment_predecessor_total(bucket, identity.candidate_start)?,
		};
		self.mutate(intent_key, |existing| {
			if existing.phase == ReplicationPhase::MmrCommitted
				&& existing.local_commitment.as_ref() == Some(&evidence)
			{
				return Ok(existing.clone());
			}
			if existing.phase != ReplicationPhase::Installed
				|| evidence != expected_commitment(&existing.identity)
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
				|| !verify_confirmation(existing, evidence)?
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
		validate_request(&record.identity, request, true)?;
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
					verify_confirmation(record, evidence).unwrap_or(false)
				}) => {},
		_ => return Err(ContentError::IntegrityFailed),
	}
	Ok(())
}

fn validate_request(
	identity: &ReplicationIntentInputV1,
	request: &OutstandingReplicationRequestV1,
	allow_response: bool,
) -> Result<(), ContentError> {
	let replay = authenticate_request(identity, request.kind, &request.signed_request_bytes)?;
	if request.request_nonce == [0; 16]
		|| request.signed_request_bytes.is_empty()
		|| request.signed_request_bytes.len() > MAX_REQUEST_ENCODED
		|| request.request_nonce != replay.request_nonce
		|| request.request_hash != replay.request_hash
		|| (!allow_response && request.verified_response_hash.is_some())
		|| request.verified_response_hash == Some([0; 32])
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn expected_peer_context(
	identity: &ReplicationIntentInputV1,
) -> Result<PeerContextV1, ContentError> {
	PeerContextV1::new(
		identity.genesis_hash,
		identity.topology_finalized_hash,
		identity.topology_finalized_number,
		identity.bucket_id,
		identity.source_provider,
		identity.target_provider,
		identity.source_service_key_version,
		identity.source_service_key,
		identity.target_service_key_version,
		identity.target_service_key,
		identity.source_endpoint_hash,
		identity.target_endpoint_hash,
		PeerMmrCommitmentV1::new(
			identity.candidate_mmr_root,
			identity.candidate_start,
			identity.candidate_count,
			identity.candidate_predecessor_total,
		)?,
	)
}

fn authenticate_request(
	identity: &ReplicationIntentInputV1,
	kind: ReplicationRequestKindV1,
	bytes: &[u8],
) -> Result<PeerReplayIdentityV1, ContentError> {
	let expected = expected_peer_context(identity)?;
	let (context, canonical, replay) = match kind {
		ReplicationRequestKindV1::Page => {
			let request = PeerSyncPageRequestV1::decode_authenticated(bytes)?;
			let replay = request.authenticated_replay_identity()?;
			(request.context().clone(), request.encode_wire(), replay)
		},
		ReplicationRequestKindV1::Chunk => {
			let request = PeerChunkRequestV1::decode_authenticated(bytes)?;
			let replay = request.authenticated_replay_identity()?;
			(request.context().clone(), request.encode_wire(), replay)
		},
	};
	if context != expected
		|| canonical != bytes
		|| replay.operation_id != identity.peer_operation_id
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(replay)
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
	CanonicalCid::parse(&completion.cid)?;
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
	{
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

fn confirmation_signature_digest(
	record: &ReplicationIntentV1,
	binding: [u8; 32],
) -> Result<[u8; 32], ContentError> {
	let commitment = record.local_commitment.as_ref().ok_or(ContentError::IntegrityFailed)?;
	let mut bytes = CONFIRMATION_SIGNATURE_DOMAIN.to_vec();
	bytes.extend_from_slice(&binding);
	bytes.extend_from_slice(&record.identity.target_provider);
	bytes.extend_from_slice(&record.identity.target_service_key);
	bytes.extend_from_slice(&record.identity.target_service_key_version.to_le_bytes());
	bytes.extend_from_slice(&record.identity.topology_snapshot_hash);
	bytes.extend_from_slice(&record.identity.topology_finalized_hash);
	bytes.extend_from_slice(&record.identity.topology_finalized_number.to_le_bytes());
	bytes.extend_from_slice(&record.identity.topology_governed_checkpoint.to_le_bytes());
	bytes.extend_from_slice(&commitment.mmr_root);
	bytes.extend_from_slice(&commitment.start.to_le_bytes());
	bytes.extend_from_slice(&commitment.count.to_le_bytes());
	bytes.extend_from_slice(&commitment.predecessor_total.to_le_bytes());
	Ok(blake2_256(&bytes))
}

fn verify_confirmation(
	record: &ReplicationIntentV1,
	evidence: &ReplicationConfirmationEvidenceV1,
) -> Result<bool, ContentError> {
	let binding = confirmation_binding(record)?;
	let signature: [u8; 64] = evidence
		.target_signature
		.as_slice()
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)?;
	Ok(record.identity.target_may_confirm
		&& evidence.intent_binding == binding
		&& evidence.target_service_key_version == record.identity.target_service_key_version
		&& ed25519::Pair::verify(
			&ed25519::Signature::from_raw(signature),
			&confirmation_signature_digest(record, binding)?,
			&ed25519::Public::from_raw(record.identity.target_service_key),
		))
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
	use crate::{
		peer::{
			PeerChunkExpectationV1, PeerObjectV1, PeerPageCursorV1, PeerPageExpectationV1,
			PeerRequestIdentityV1,
		},
		StreamingDescriptor,
	};

	fn bytes32(value: u16, salt: u8) -> [u8; 32] {
		let mut bytes = [salt; 32];
		bytes[..2].copy_from_slice(&value.to_le_bytes());
		bytes
	}

	fn target_pair(value: u16) -> ed25519::Pair {
		ed25519::Pair::from_seed(&bytes32(value, 90))
	}

	fn source_pair() -> ed25519::Pair {
		ed25519::Pair::from_seed(&[8; 32])
	}

	fn input(value: u16) -> ReplicationIntentInputV1 {
		let source = ed25519::Pair::from_seed(&[8; 32]);
		let target = target_pair(value);
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
			source_service_key: source.public().0,
			source_service_key_version: 1,
			target_service_key: target.public().0,
			target_service_key_version: 2,
			source_endpoint_hash: [10; 32],
			target_endpoint_hash: bytes32(value, 11),
			target_may_confirm: true,
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

	fn page_request(
		record: &ReplicationIntentV1,
		nonce: [u8; 16],
		operation: [u8; 16],
	) -> PeerSyncPageRequestV1 {
		let expectation = PeerPageExpectationV1::new(
			expected_peer_context(&record.identity).unwrap(),
			PeerRequestIdentityV1::new(operation, nonce).unwrap(),
			None,
			32,
		)
		.unwrap();
		PeerSyncPageRequestV1::new_signed(
			&expectation,
			&target_pair(u16::from_le_bytes([
				record.identity.topology_snapshot_hash[0],
				record.identity.topology_snapshot_hash[1],
			])),
		)
		.unwrap()
	}

	fn page_response(
		record: &ReplicationIntentV1,
		page: &PeerSyncPageRequestV1,
	) -> PeerSyncPageResponseV1 {
		let bytes = b"authenticated page object".to_vec();
		let total = record.identity.candidate_predecessor_total + bytes.len() as u64;
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest(blake2_256(&bytes)),
			bytes.len() as u64,
			record.identity.candidate_start,
			total,
			vec![blake2_256(&bytes)],
		)
		.unwrap();
		let next = (record.identity.candidate_count > 1)
			.then(|| PeerPageCursorV1::new(record.identity.candidate_start, total));
		PeerSyncPageResponseV1::new_signed(page, vec![object], next, &source_pair()).unwrap()
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
		let page = page_request(&record, [77; 16], record.identity.peer_operation_id);
		let staged = store
			.stage_page_request_before_send(&record.intent_key, &page.encode_wire())
			.unwrap();
		let response = page_response(&record, &page);
		store.attach_page_response(&staged.intent_key, &response.encode_wire()).unwrap()
	}

	fn install_ready(
		streaming: &StreamingStore,
		record: &ReplicationIntentV1,
		sequence: u64,
		bytes: Vec<u8>,
		operation: [u8; 16],
	) -> (String, u64) {
		let cid = CanonicalCid::from_digest(blake2_256(&bytes));
		let length = bytes.len() as u64;
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: OperationId::from_bytes(operation),
					bucket_id: BucketId::from_bytes(record.identity.bucket_id),
					expected_cid: cid.to_string(),
					object_len: length,
				},
				[bytes],
			)
			.unwrap();
		let _ = sequence;
		(cid.to_string(), length)
	}

	fn complete_ready(
		store: &ReplicationIntentStore,
		streaming: &StreamingStore,
		record: &ReplicationIntentV1,
		sequence: u64,
		bytes: Vec<u8>,
	) -> ReplicationIntentV1 {
		let operation = derived_stream_id(&record.intent_key, sequence, OPERATION_DOMAIN);
		let (cid, length) = install_ready(streaming, record, sequence, bytes, operation);
		store
			.complete_object(streaming, &record.intent_key, sequence, &cid, length)
			.unwrap()
	}

	fn finish_objects(
		store: &ReplicationIntentStore,
		streaming: &StreamingStore,
		record: ReplicationIntentV1,
	) -> ReplicationIntentV1 {
		let mut record = with_verified_page(store, record);
		let end = candidate_end(&record.identity).unwrap();
		while record.next_sequence < end {
			let sequence = record.next_sequence;
			record = complete_ready(
				store,
				streaming,
				&record,
				sequence,
				vec![(sequence as u8).wrapping_add(1); 17],
			);
		}
		store.mark_installed(&record.intent_key).unwrap()
	}

	fn mmr_input(value: u16, objects: &[Vec<u8>]) -> ReplicationIntentInputV1 {
		let scratch = tempfile::tempdir().unwrap();
		let streaming = StreamingStore::open(scratch.path()).unwrap();
		let mut candidate = input(value);
		candidate.candidate_start = 0;
		candidate.candidate_count = objects.len() as u64;
		candidate.candidate_predecessor_total = 0;
		for (index, bytes) in objects.iter().enumerate() {
			let cid = CanonicalCid::from_digest(blake2_256(bytes));
			streaming
				.put_chunks(
					StreamingDescriptor {
						operation_id: OperationId::from_bytes([(index + 1) as u8; 16]),
						bucket_id: BucketId::from_bytes(candidate.bucket_id),
						expected_cid: cid.to_string(),
						object_len: bytes.len() as u64,
					},
					[bytes.clone()],
				)
				.unwrap();
		}
		let mmr = BucketMmrStore::open(scratch.path(), &streaming).unwrap();
		let commitment = mmr
			.commitment_candidate(&streaming, BucketId::from_bytes(candidate.bucket_id), 0)
			.unwrap();
		candidate.candidate_mmr_root = commitment.mmr_root.0;
		candidate
	}

	fn run_to_mmr(
		root: &Path,
		input: &ReplicationIntentInputV1,
		objects: &[Vec<u8>],
	) -> (ReplicationIntentStore, StreamingStore, BucketMmrStore, ReplicationIntentV1) {
		let store = ReplicationIntentStore::open(root).unwrap();
		let streaming = StreamingStore::open(root).unwrap();
		let mut record = with_verified_page(&store, store.plan(input).unwrap());
		for bytes in objects {
			let sequence = record.next_sequence;
			record = complete_ready(&store, &streaming, &record, sequence, bytes.clone());
		}
		let installed = store.mark_installed(&record.intent_key).unwrap();
		let mmr = BucketMmrStore::open(root, &streaming).unwrap();
		let committed = store.commit_local_mmr(&mmr, &streaming, &installed.intent_key).unwrap();
		(store, streaming, mmr, committed)
	}

	fn confirmation(
		record: &ReplicationIntentV1,
		pair: &ed25519::Pair,
	) -> ReplicationConfirmationEvidenceV1 {
		let binding = confirmation_binding(record).unwrap();
		ReplicationConfirmationEvidenceV1 {
			intent_binding: binding,
			target_service_key_version: record.identity.target_service_key_version,
			target_signature: pair
				.sign(&confirmation_signature_digest(record, binding).unwrap())
				.0
				.to_vec(),
		}
	}

	#[test]
	fn requests_are_typed_authenticated_context_bound_and_revalidated_on_open() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let mut shared_ingress = input(20);
		shared_ingress.target_endpoint_hash = shared_ingress.source_endpoint_hash;
		let planned = store.plan(&shared_ingress).unwrap();

		assert!(store
			.stage_page_request_before_send(&planned.intent_key, b"caller bytes")
			.is_err());
		let wrong_operation = page_request(&planned, [1; 16], [99; 16]);
		assert!(store
			.stage_page_request_before_send(&planned.intent_key, &wrong_operation.encode_wire())
			.is_err());
		let other = store.plan(&input(21)).unwrap();
		let wrong_context = page_request(&other, [2; 16], other.identity.peer_operation_id);
		assert!(store
			.stage_page_request_before_send(&planned.intent_key, &wrong_context.encode_wire())
			.is_err());
		let page = page_request(&planned, [3; 16], planned.identity.peer_operation_id);
		let mut wrong_signature = page.encode_wire();
		*wrong_signature.last_mut().unwrap() ^= 1;
		assert!(store
			.stage_page_request_before_send(&planned.intent_key, &wrong_signature)
			.is_err());
		assert!(store
			.stage_chunk_request_before_send(&planned.intent_key, &page.encode_wire())
			.is_err());

		let replay = page.authenticated_replay_identity().unwrap();
		let staged = store
			.stage_page_request_before_send(&planned.intent_key, &page.encode_wire())
			.unwrap();
		assert_eq!(staged.outstanding_request.as_ref().unwrap().request_hash, replay.request_hash);
		assert!(store.attach_page_response(&planned.intent_key, b"caller hash").is_err());
		assert!(store
			.attach_chunk_response(
				&planned.intent_key,
				&page_response(&planned, &page).encode_wire()
			)
			.is_err());
		let other_page = page_request(&other, [6; 16], other.identity.peer_operation_id);
		assert!(store
			.attach_page_response(
				&planned.intent_key,
				&page_response(&other, &other_page).encode_wire()
			)
			.is_err());
		let changed_request = page_request(&planned, [7; 16], planned.identity.peer_operation_id);
		assert!(store
			.attach_page_response(
				&planned.intent_key,
				&page_response(&planned, &changed_request).encode_wire()
			)
			.is_err());
		let response = page_response(&planned, &page);
		let mut bad_response_signature = response.encode_wire();
		*bad_response_signature.last_mut().unwrap() ^= 1;
		assert!(store
			.attach_page_response(&planned.intent_key, &bad_response_signature)
			.is_err());
		let answered = store
			.attach_page_response(&planned.intent_key, &response.encode_wire())
			.unwrap();
		assert_eq!(
			store
				.attach_page_response(&planned.intent_key, &response.encode_wire())
				.unwrap(),
			answered
		);

		let bytes = b"authenticated chunk".to_vec();
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest(blake2_256(&bytes)),
			bytes.len() as u64,
			planned.identity.candidate_start,
			planned.identity.candidate_predecessor_total + bytes.len() as u64,
			vec![blake2_256(&bytes)],
		)
		.unwrap();
		let chunk_expected = PeerChunkExpectationV1::new(
			expected_peer_context(&planned.identity).unwrap(),
			PeerRequestIdentityV1::new(planned.identity.peer_operation_id, [5; 16]).unwrap(),
			object,
			0,
		)
		.unwrap();
		let chunk = PeerChunkRequestV1::new_signed(&chunk_expected, &target_pair(20)).unwrap();
		let chunk_replay = chunk.authenticated_replay_identity().unwrap();
		let chunk_staged = store
			.stage_chunk_request_before_send(&planned.intent_key, &chunk.encode_wire())
			.unwrap();
		assert_eq!(chunk_staged.attempts, answered.attempts + 1);
		assert_eq!(
			chunk_staged.outstanding_request.as_ref().unwrap().request_hash,
			chunk_replay.request_hash
		);
		assert!(store
			.attach_page_response(&planned.intent_key, &response.encode_wire())
			.is_err());
		assert!(store.attach_chunk_response(&planned.intent_key, b"caller hash").is_err());
		let chunk_response =
			PeerChunkResponseV1::new_signed(&chunk, bytes, &source_pair()).unwrap();
		let mut bad_chunk_signature = chunk_response.encode_wire();
		*bad_chunk_signature.last_mut().unwrap() ^= 1;
		assert!(store.attach_chunk_response(&planned.intent_key, &bad_chunk_signature).is_err());
		let chunk_answered = store
			.attach_chunk_response(&planned.intent_key, &chunk_response.encode_wire())
			.unwrap();
		assert_eq!(
			store
				.attach_chunk_response(&planned.intent_key, &chunk_response.encode_wire())
				.unwrap(),
			chunk_answered
		);
		drop(store);
		let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
		assert_eq!(reopened.plan(&shared_ingress).unwrap(), chunk_answered);
	}

	#[test]
	fn completion_requires_authoritative_full_file_and_exact_derived_operation() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let planned = with_verified_page(&store, store.plan(&input(30)).unwrap());
		let fabricated = CanonicalCid::from_digest(blake2_256(b"not installed"));
		assert!(store
			.complete_object(
				&streaming,
				&planned.intent_key,
				planned.next_sequence,
				fabricated.as_str(),
				13
			)
			.is_err());

		let skipped_sequence = planned.next_sequence + 1;
		let (skipped_cid, skipped_len) = install_ready(
			&streaming,
			&planned,
			skipped_sequence,
			b"skipped".to_vec(),
			derived_stream_id(&planned.intent_key, skipped_sequence, OPERATION_DOMAIN),
		);
		assert_eq!(
			store.complete_object(
				&streaming,
				&planned.intent_key,
				skipped_sequence,
				&skipped_cid,
				skipped_len,
			),
			Err(ContentError::IdempotencyConflict)
		);

		let (wrong_cid, wrong_len) = install_ready(
			&streaming,
			&planned,
			planned.next_sequence,
			b"wrong operation".to_vec(),
			[200; 16],
		);
		assert!(store
			.complete_object(
				&streaming,
				&planned.intent_key,
				planned.next_sequence,
				&wrong_cid,
				wrong_len,
			)
			.is_err());

		let first_sequence = planned.next_sequence;
		let first = complete_ready(&store, &streaming, &planned, first_sequence, b"first".to_vec());
		let replay = store
			.complete_object(
				&streaming,
				&planned.intent_key,
				first_sequence,
				&first.last_completed.as_ref().unwrap().cid,
				first.last_completed.as_ref().unwrap().length,
			)
			.unwrap();
		assert_eq!(replay, first);
		let first_ids = first.last_completed.as_ref().unwrap().clone();
		let second_sequence = first.next_sequence;
		let second = store
			.complete_object(
				&streaming,
				&first.intent_key,
				second_sequence,
				&skipped_cid,
				skipped_len,
			)
			.unwrap();
		let second_ids = second.last_completed.as_ref().unwrap();
		assert_ne!(first_ids.streaming_operation_id, second_ids.streaming_operation_id);
		assert_ne!(first_ids.streaming_repair_id, second_ids.streaming_repair_id);
		assert!(store
			.complete_object(
				&streaming,
				&planned.intent_key,
				first_sequence,
				&first_ids.cid,
				first_ids.length,
			)
			.is_err());
		let installed = finish_objects(&store, &streaming, second);
		assert_eq!(installed.phase, ReplicationPhase::Installed);
	}

	#[test]
	fn mmr_commitment_is_derived_from_reverified_local_state_and_confirmation_is_target_signed() {
		let objects = vec![b"one".to_vec(), b"two-two".to_vec(), b"three-three".to_vec()];
		let identity = mmr_input(40, &objects);
		let temp = tempfile::tempdir().unwrap();
		let (store, _streaming, _mmr, committed) = run_to_mmr(temp.path(), &identity, &objects);
		assert_eq!(committed.local_commitment, Some(expected_commitment(&identity)));

		let valid = confirmation(&committed, &target_pair(40));
		let mut wrong_binding = valid.clone();
		wrong_binding.intent_binding[0] ^= 1;
		assert_eq!(
			store.confirm(&committed.intent_key, &wrong_binding),
			Err(ContentError::IdempotencyConflict)
		);
		let wrong_key = confirmation(&committed, &ed25519::Pair::from_seed(&[99; 32]));
		assert_eq!(
			store.confirm(&committed.intent_key, &wrong_key),
			Err(ContentError::IdempotencyConflict)
		);
		let confirmed = store.confirm(&committed.intent_key, &valid).unwrap();
		assert_eq!(confirmed.phase, ReplicationPhase::Confirmed);
		assert_eq!(store.confirm(&committed.intent_key, &valid).unwrap(), confirmed);

		let denied_temp = tempfile::tempdir().unwrap();
		let mut denied_identity = identity.clone();
		denied_identity.target_may_confirm = false;
		let (denied_store, _, _, denied) =
			run_to_mmr(denied_temp.path(), &denied_identity, &objects);
		let denied_evidence = confirmation(&denied, &target_pair(40));
		assert_eq!(
			denied_store.confirm(&denied.intent_key, &denied_evidence),
			Err(ContentError::IdempotencyConflict)
		);
	}

	#[test]
	fn fabricated_mmr_cannot_be_committed() {
		let objects = vec![b"alpha".to_vec(), b"bravo".to_vec()];
		let identity = mmr_input(50, &objects);
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let installed = finish_objects(&store, &streaming, store.plan(&identity).unwrap());
		let unrelated = tempfile::tempdir().unwrap();
		let unrelated_streaming = StreamingStore::open(unrelated.path()).unwrap();
		let unrelated_mmr = BucketMmrStore::open(unrelated.path(), &unrelated_streaming).unwrap();
		assert!(store
			.commit_local_mmr(&unrelated_mmr, &unrelated_streaming, &installed.intent_key)
			.is_err());
		assert_eq!(store.plan(&identity).unwrap().phase, ReplicationPhase::Installed);
	}

	#[test]
	fn scheduler_is_fair_across_reopened_ticks() {
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
}
