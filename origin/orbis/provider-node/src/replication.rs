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

use codec::Encode;
use serde::{Deserialize, Serialize};
use sp_core::{ed25519, Pair as _};
use sp_crypto_hashing::blake2_256;

use crate::{
	peer::{
		PeerChunkExpectationV1, PeerChunkRequestV1, PeerChunkResponseV1, PeerContextV1,
		PeerMmrCommitmentV1, PeerPageExpectationV1, PeerReplayIdentityV1, PeerRequestIdentityV1,
		PeerRequestProofV1, PeerResponseProofV1, PeerSyncPageRequestV1, PeerSyncPageResponseV1,
		MAX_REQUEST_ENCODED,
	},
	replication_session::ReplicationSessionV1,
	storage::bucket_mmr::BucketMmrStore,
	BucketId, CanonicalCid, ContentError, OperationId, StreamingStore, CHUNK_BYTES, MAX_CHUNKS,
	MAX_STORED_BYTES,
};

const ROOT: &str = "replication-v3";
const VERSION: u16 = 3;
const SCHEDULER: &str = "scheduler.json";
const MAX_RECORDS: usize = 4_096;
const MAX_TEMP_ARTIFACTS: usize = 128;
const MAX_RECORD_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_RECORD_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TICK_WORK: usize = 128;
const KEY_DOMAIN: &[u8] = b"origin/replication-intent-key/v3";
const RECORD_DOMAIN: &[u8] = b"origin/replication-intent-record/v3";
const OPERATION_DOMAIN: &[u8] = b"origin/replication-stream-operation/v1";
const REPAIR_DOMAIN: &[u8] = b"origin/replication-stream-repair/v1";
const CONFIRMATION_DOMAIN: &[u8] = b"origin/replication-confirmation-binding/v1";
const CONFIRMATION_SIGNATURE_DOMAIN: &[u8] = b"origin/replication-target-confirmation/v1";
const SCHEDULER_DOMAIN: &[u8] = b"origin/replication-scheduler/v3";
const REPLICATION_NONCE_VERSION: u8 = 1;
const PAGE_NONCE_KIND: u8 = 1;
const CHUNK_NONCE_KIND: u8 = 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicationIntentInputV1 {
	genesis_hash: [u8; 32],
	topology_snapshot_hash: [u8; 32],
	topology_finalized_hash: [u8; 32],
	topology_finalized_number: u32,
	topology_governed_checkpoint: u32,
	bucket_id: [u8; 32],
	bucket_version: u64,
	source_provider: [u8; 32],
	target_provider: [u8; 32],
	source_service_key: [u8; 32],
	source_service_key_version: u64,
	target_service_key: [u8; 32],
	target_service_key_version: u64,
	source_endpoint_hash: [u8; 32],
	target_endpoint_hash: [u8; 32],
	target_may_confirm: bool,
	candidate_mmr_root: [u8; 32],
	candidate_start: u64,
	candidate_count: u64,
	candidate_predecessor_total: u64,
	peer_operation_id: [u8; 16],
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PeerCursorEvidenceV1 {
	last_sequence: u64,
	cumulative_total: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifiedChunkEvidenceV1 {
	index: u16,
	chunk_hash: [u8; 32],
	request_nonce: [u8; 16],
	request_hash: [u8; 32],
	target_signature: Vec<u8>,
	response_wire_hash: [u8; 32],
	source_response_hash: [u8; 32],
	source_signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmittedObjectEvidenceV1 {
	sequence: u64,
	cid: String,
	length: u64,
	cumulative_total: u64,
	chunk_manifest_hash: [u8; 32],
	chunk_hashes: Vec<[u8; 32]>,
	verified_chunks: Vec<Option<VerifiedChunkEvidenceV1>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmittedPageEvidenceV1 {
	request_hash: [u8; 32],
	signed_request_bytes: Vec<u8>,
	response_wire_hash: [u8; 32],
	source_response_hash: [u8; 32],
	source_signature: Vec<u8>,
	requested_cursor: Option<PeerCursorEvidenceV1>,
	next_cursor: Option<PeerCursorEvidenceV1>,
	objects: Vec<AdmittedObjectEvidenceV1>,
	next_object: usize,
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
	admitted_page: Option<AdmittedPageEvidenceV1>,
	pub(crate) attempts: u32,
	pub(crate) local_commitment: Option<LocalCommitmentEvidenceV1>,
	pub(crate) confirmation: Option<ReplicationConfirmationEvidenceV1>,
	record_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReplicationActionV1 {
	SendPage { intent_key: String, request_bytes: Vec<u8> },
	SendChunk { intent_key: String, request_bytes: Vec<u8> },
	FinishObject { intent_key: String, sequence: u64, cid: String, length: u64 },
	MarkInstalled { intent_key: String },
	CommitMmr { intent_key: String },
	Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedIncomingChunkV1 {
	pub(crate) bucket_id: BucketId,
	pub(crate) install_operation_id: OperationId,
	pub(crate) repair_operation_id: OperationId,
	pub(crate) object: crate::peer::PeerObjectV1,
	pub(crate) index: u16,
	pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReplicationObjectInstallV1 {
	pub(crate) bucket_id: BucketId,
	pub(crate) operation_id: OperationId,
	pub(crate) repair_operation_id: OperationId,
	pub(crate) cumulative_total: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReplicationResumeV1 {
	pub(crate) intent_key: String,
	pub(crate) topology_snapshot_hash: [u8; 32],
	pub(crate) topology_finalized_hash: [u8; 32],
	pub(crate) topology_finalized_number: u32,
	pub(crate) bucket_id: [u8; 32],
	pub(crate) source_provider: [u8; 32],
	pub(crate) target_provider: [u8; 32],
	pub(crate) commitment: PeerMmrCommitmentV1,
	pub(crate) operation_id: [u8; 16],
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
	durable_bytes: RwLock<u64>,
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
			durable_bytes: RwLock::new(loaded.durable_bytes),
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

	pub(crate) fn plan_session(
		&self,
		session: &ReplicationSessionV1,
		peer_operation_id: [u8; 16],
	) -> Result<ReplicationIntentV1, ContentError> {
		let input = input_from_session(session, peer_operation_id)?;
		self.plan_input(&input)
	}

	/// Durably move an active transfer to another source at its current object boundary.
	///
	/// Completed objects retain their source-independent receipts and cursor. Any admitted page,
	/// partial current object proof, or outstanding request is discarded so responses from two
	/// source authorities can never contribute to the same admitted page. Streaming operation IDs
	/// are derived from the stable intent key, making already-fsynced chunks safe to reverify when
	/// the current object is requested again from the replacement source.
	pub(crate) fn replan_source(
		&self,
		existing_intent_key: &str,
		session: &ReplicationSessionV1,
		peer_operation_id: [u8; 16],
	) -> Result<ReplicationIntentV1, ContentError> {
		let replacement = input_from_session(session, peer_operation_id)?;
		self.mutate(existing_intent_key, |existing| {
			if existing.intent_key != existing_intent_key
				|| intent_key(&replacement) != existing_intent_key
				|| !same_replan_scope(&existing.identity, &replacement)
				|| existing.identity.source_provider == replacement.source_provider
				|| !matches!(
					existing.phase,
					ReplicationPhase::Planned | ReplicationPhase::Receiving
				) || existing.local_commitment.is_some()
				|| existing.confirmation.is_some()
			{
				return Err(ContentError::IdempotencyConflict);
			}
			let mut next = existing.clone();
			next.identity = replacement.clone();
			next.phase = if next.next_sequence == next.identity.candidate_start {
				ReplicationPhase::Planned
			} else {
				ReplicationPhase::Receiving
			};
			next.outstanding_request = None;
			next.admitted_page = None;
			next.attempts = 0;
			Ok(next)
		})
	}

	pub(crate) fn record(&self, intent_key: &str) -> Result<ReplicationIntentV1, ContentError> {
		self.ensure_healthy()?;
		self.records
			.read()
			.map_err(|_| lock_error())?
			.get(intent_key)
			.cloned()
			.ok_or(ContentError::NotFound)
	}

	pub(crate) fn next_action(
		&self,
		intent_key: &str,
		target: &ed25519::Pair,
	) -> Result<ReplicationActionV1, ContentError> {
		let record = self.record(intent_key)?;
		if target.public().0 != record.identity.target_service_key {
			return Err(ContentError::IntegrityFailed);
		}
		if let Some(outstanding) = record
			.outstanding_request
			.as_ref()
			.filter(|request| request.verified_response_hash.is_none())
		{
			validate_request(&record.identity, outstanding, false)?;
			return Ok(match outstanding.kind {
				ReplicationRequestKindV1::Page => ReplicationActionV1::SendPage {
					intent_key: record.intent_key,
					request_bytes: outstanding.signed_request_bytes.clone(),
				},
				ReplicationRequestKindV1::Chunk => ReplicationActionV1::SendChunk {
					intent_key: record.intent_key,
					request_bytes: outstanding.signed_request_bytes.clone(),
				},
			});
		}
		match record.phase {
			ReplicationPhase::Planned | ReplicationPhase::Receiving => {
				let end = candidate_end(&record.identity)?;
				if let Some(page) = &record.admitted_page {
					let object =
						page.objects.get(page.next_object).ok_or(ContentError::IntegrityFailed)?;
					if let Some(index) = object.verified_chunks.iter().position(Option::is_none) {
						let object = peer_object_from_evidence(object)?;
						let index: u16 =
							index.try_into().map_err(|_| ContentError::ObjectTooLarge)?;
						let expected = PeerChunkExpectationV1::new(
							expected_peer_context(&record.identity)?,
							PeerRequestIdentityV1::new(
								record.identity.peer_operation_id,
								replication_nonce(CHUNK_NONCE_KIND, object.position().1, index),
							)?,
							object,
							index,
						)?;
						let request = PeerChunkRequestV1::new_signed(&expected, target)?;
						let bytes = request.encode_wire();
						self.stage_chunk_request_before_send(intent_key, &bytes)?;
						return Ok(ReplicationActionV1::SendChunk {
							intent_key: intent_key.into(),
							request_bytes: bytes,
						});
					}
					return Ok(ReplicationActionV1::FinishObject {
						intent_key: intent_key.into(),
						sequence: object.sequence,
						cid: object.cid.clone(),
						length: object.length,
					});
				}
				if record.next_sequence == end {
					return Ok(ReplicationActionV1::MarkInstalled {
						intent_key: intent_key.into(),
					});
				}
				let cursor = expected_request_cursor(&record)?;
				let remaining =
					end.checked_sub(record.next_sequence).ok_or(ContentError::IntegrityFailed)?;
				let limit: u16 = remaining.min(128) as u16;
				let expected = PeerPageExpectationV1::new(
					expected_peer_context(&record.identity)?,
					PeerRequestIdentityV1::new(
						record.identity.peer_operation_id,
						replication_nonce(PAGE_NONCE_KIND, record.next_sequence, u16::MAX),
					)?,
					peer_cursor_from_evidence(cursor),
					limit,
				)?;
				let request = PeerSyncPageRequestV1::new_signed(&expected, target)?;
				let bytes = request.encode_wire();
				self.stage_page_request_before_send(intent_key, &bytes)?;
				Ok(ReplicationActionV1::SendPage {
					intent_key: intent_key.into(),
					request_bytes: bytes,
				})
			},
			ReplicationPhase::Installed => {
				Ok(ReplicationActionV1::CommitMmr { intent_key: intent_key.into() })
			},
			ReplicationPhase::MmrCommitted | ReplicationPhase::Confirmed => {
				Ok(ReplicationActionV1::Complete)
			},
		}
	}

	#[cfg(test)]
	fn plan(&self, input: &ReplicationIntentInputV1) -> Result<ReplicationIntentV1, ContentError> {
		self.plan_input(input)
	}

	fn plan_input(
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
			match kind {
				ReplicationRequestKindV1::Page => {
					if existing.admitted_page.is_some() {
						return Err(ContentError::IdempotencyConflict);
					}
					let page = PeerSyncPageRequestV1::decode_authenticated(signed_request_bytes)?;
					if cursor_evidence(page.page().0) != expected_request_cursor(existing)? {
						return Err(ContentError::IdempotencyConflict);
					}
				},
				ReplicationRequestKindV1::Chunk => {
					let page =
						existing.admitted_page.as_ref().ok_or(ContentError::IdempotencyConflict)?;
					let expected =
						page.objects.get(page.next_object).ok_or(ContentError::IntegrityFailed)?;
					let chunk = PeerChunkRequestV1::decode_authenticated(signed_request_bytes)?;
					let (object, index, hash) = chunk.chunk();
					let index_usize = usize::from(index);
					if !object_matches_evidence(object, expected)
						|| expected.chunk_hashes.get(index_usize) != Some(&hash)
						|| expected.verified_chunks.get(index_usize) != Some(&None)
					{
						return Err(ContentError::IdempotencyConflict);
					}
				},
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

	/// Authenticate one chunk response without advancing durable replication evidence.
	///
	/// The caller must durably install these exact bytes before calling
	/// [`Self::attach_chunk_response`]. This split makes a crash after the byte fsync replay the
	/// exact outstanding request rather than falsely acknowledging volatile data.
	pub(crate) fn inspect_chunk_response(
		&self,
		intent_key: &str,
		response_bytes: &[u8],
	) -> Result<VerifiedIncomingChunkV1, ContentError> {
		let record = self.record(intent_key)?;
		let outstanding =
			record.outstanding_request.as_ref().ok_or(ContentError::IdempotencyConflict)?;
		if outstanding.kind != ReplicationRequestKindV1::Chunk
			|| outstanding.verified_response_hash.is_some()
		{
			return Err(ContentError::IdempotencyConflict);
		}
		validate_request(&record.identity, outstanding, true)?;
		let request = PeerChunkRequestV1::decode_authenticated(&outstanding.signed_request_bytes)?;
		let response = PeerChunkResponseV1::decode_canonical(response_bytes, &request)?;
		let (object, index, bytes) = response.verified_chunk();
		let expected = record
			.admitted_page
			.as_ref()
			.and_then(|page| page.objects.get(page.next_object))
			.ok_or(ContentError::IdempotencyConflict)?;
		let slot = expected
			.verified_chunks
			.get(usize::from(index))
			.ok_or(ContentError::IdempotencyConflict)?;
		if !object_matches_evidence(object, expected)
			|| expected.chunk_hashes.get(usize::from(index))
				!= Some(&sp_crypto_hashing::blake2_256(bytes))
			|| slot.is_some()
		{
			return Err(ContentError::IdempotencyConflict);
		}
		Ok(VerifiedIncomingChunkV1 {
			bucket_id: BucketId::from_bytes(record.identity.bucket_id),
			install_operation_id: OperationId::from_bytes(derived_stream_id(
				intent_key,
				object.position().1,
				OPERATION_DOMAIN,
			)),
			repair_operation_id: OperationId::from_bytes(derived_stream_id(
				intent_key,
				object.position().1,
				REPAIR_DOMAIN,
			)),
			object: object.clone(),
			index,
			bytes: bytes.to_vec(),
		})
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
			let (response_hash, admitted_page, verified_chunk) = match kind {
				ReplicationRequestKindV1::Page => {
					let request = PeerSyncPageRequestV1::decode_authenticated(
						&outstanding.signed_request_bytes,
					)?;
					let response =
						PeerSyncPageResponseV1::decode_canonical(response_bytes, &request)?;
					let wire_hash = blake2_256(&response.encode_wire());
					let page =
						admitted_page_from_response(existing, &request, &response, wire_hash)?;
					(wire_hash, Some(page), None)
				},
				ReplicationRequestKindV1::Chunk => {
					let request = PeerChunkRequestV1::decode_authenticated(
						&outstanding.signed_request_bytes,
					)?;
					let response = PeerChunkResponseV1::decode_canonical(response_bytes, &request)?;
					let wire_hash = blake2_256(&response.encode_wire());
					let request_proof = request.compact_proof();
					let proof = response.compact_proof();
					let (object, index, _) = response.verified_chunk();
					let page =
						existing.admitted_page.as_ref().ok_or(ContentError::IdempotencyConflict)?;
					let expected =
						page.objects.get(page.next_object).ok_or(ContentError::IntegrityFailed)?;
					let index_usize = usize::from(index);
					let chunk_hash = *expected
						.chunk_hashes
						.get(index_usize)
						.ok_or(ContentError::IdempotencyConflict)?;
					if !object_matches_evidence(object, expected) {
						return Err(ContentError::IdempotencyConflict);
					}
					(
						wire_hash,
						None,
						Some(VerifiedChunkEvidenceV1 {
							index,
							chunk_hash,
							request_nonce: request_proof.request_nonce(),
							request_hash: request_proof.request_hash(),
							target_signature: request_proof.signature().to_vec(),
							response_wire_hash: wire_hash,
							source_response_hash: proof.response_hash(),
							source_signature: proof.signature().to_vec(),
						}),
					)
				},
			};
			if outstanding.verified_response_hash == Some(response_hash) {
				let exact = match (&admitted_page, &verified_chunk) {
					(Some(page), None) => existing.admitted_page.as_ref() == Some(page),
					(None, Some(chunk)) => {
						existing
							.admitted_page
							.as_ref()
							.and_then(|page| page.objects.get(page.next_object))
							.and_then(|object| object.verified_chunks.get(usize::from(chunk.index)))
							== Some(&Some(chunk.clone()))
					},
					_ => false,
				};
				return if exact {
					Ok(existing.clone())
				} else {
					Err(ContentError::IdempotencyConflict)
				};
			}
			if outstanding.verified_response_hash.is_some() {
				return Err(ContentError::IdempotencyConflict);
			}
			let mut next = existing.clone();
			next.outstanding_request.as_mut().expect("checked above").verified_response_hash =
				Some(response_hash);
			if let Some(page) = admitted_page {
				if next.admitted_page.is_some() {
					return Err(ContentError::IdempotencyConflict);
				}
				next.admitted_page = Some(page);
			}
			if let Some(chunk) = verified_chunk {
				let slot = next
					.admitted_page
					.as_mut()
					.and_then(|page| page.objects.get_mut(page.next_object))
					.and_then(|object| object.verified_chunks.get_mut(usize::from(chunk.index)))
					.ok_or(ContentError::IdempotencyConflict)?;
				if slot.is_some() {
					return Err(ContentError::IdempotencyConflict);
				}
				*slot = Some(chunk);
			}
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
		let record = self
			.records
			.read()
			.map_err(|_| lock_error())?
			.get(intent_key)
			.cloned()
			.ok_or(ContentError::NotFound)?;
		let identity = record.identity.clone();
		let canonical = CanonicalCid::parse(cid)?;
		validate_admitted_completion(&record, sequence, canonical.as_str(), length)?;
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
			cid: canonical.to_string(),
			length,
			cumulative_total: 0,
			streaming_operation_id: operation_id,
			streaming_repair_id: repair_id,
			streaming_receipt_fingerprint: ready.receipt_fingerprint,
		};
		self.mutate(intent_key, |existing| {
			validate_admitted_completion(
				existing,
				completion.sequence,
				&completion.cid,
				completion.length,
			)?;
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
			let mut next = existing.clone();
			next.next_sequence =
				next.next_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			next.cumulative_total = completion.cumulative_total;
			next.last_completed = Some(completion);
			let page = next.admitted_page.as_mut().ok_or(ContentError::IntegrityFailed)?;
			page.objects
				.get_mut(page.next_object)
				.ok_or(ContentError::IntegrityFailed)?
				.verified_chunks
				.clear();
			page.next_object =
				page.next_object.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			if page.next_object == page.objects.len() {
				next.admitted_page = None;
			}
			next.phase = ReplicationPhase::Receiving;
			Ok(next)
		})
	}

	pub(crate) fn object_installation(
		&self,
		intent_key: &str,
		sequence: u64,
		cid: &str,
		length: u64,
	) -> Result<ReplicationObjectInstallV1, ContentError> {
		let record = self.record(intent_key)?;
		validate_admitted_completion(&record, sequence, cid, length)?;
		let cumulative_total = record
			.last_completed
			.as_ref()
			.filter(|completed| {
				completed.sequence == sequence && completed.cid == cid && completed.length == length
			})
			.map(|completed| completed.cumulative_total)
			.or_else(|| {
				record
					.admitted_page
					.as_ref()
					.and_then(|page| page.objects.get(page.next_object))
					.map(|object| object.cumulative_total)
			})
			.ok_or(ContentError::IdempotencyConflict)?;
		Ok(ReplicationObjectInstallV1 {
			bucket_id: BucketId::from_bytes(record.identity.bucket_id),
			operation_id: OperationId::from_bytes(derived_stream_id(
				intent_key,
				sequence,
				OPERATION_DOMAIN,
			)),
			repair_operation_id: OperationId::from_bytes(derived_stream_id(
				intent_key,
				sequence,
				REPAIR_DOMAIN,
			)),
			cumulative_total,
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
			if existing.phase != ReplicationPhase::Receiving
				|| existing.next_sequence != end
				|| existing.admitted_page.is_some()
			{
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
		self.select_tick_limit(MAX_TICK_WORK)
	}

	fn select_tick_limit(&self, limit: usize) -> Result<Vec<ReplicationIntentV1>, ContentError> {
		if limit == 0 || limit > MAX_TICK_WORK {
			return Err(ContentError::SchemaInvalid);
		}
		self.ensure_healthy()?;
		let records = self.records.read().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		let active: Vec<_> = records
			.values()
			.filter(|record| record.phase < ReplicationPhase::MmrCommitted)
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
			.take(active.len().min(limit))
			.map(|record| (*record).clone())
			.collect();
		let mut next = scheduler.clone();
		next.after_key = selected.last().map(|record| record.intent_key.clone());
		next.record_hash = scheduler_hash(&next)?;
		self.persist_scheduler_or_poison(&next)?;
		*scheduler = next;
		Ok(selected)
	}

	pub(crate) fn select_resume_tick_limit(
		&self,
		limit: usize,
	) -> Result<Vec<ReplicationResumeV1>, ContentError> {
		self.select_tick_limit(limit)?
			.into_iter()
			.map(|record| {
				Ok(ReplicationResumeV1 {
					intent_key: record.intent_key,
					topology_snapshot_hash: record.identity.topology_snapshot_hash,
					topology_finalized_hash: record.identity.topology_finalized_hash,
					topology_finalized_number: record.identity.topology_finalized_number,
					bucket_id: record.identity.bucket_id,
					source_provider: record.identity.source_provider,
					target_provider: record.identity.target_provider,
					commitment: PeerMmrCommitmentV1::new(
						record.identity.candidate_mmr_root,
						record.identity.candidate_start,
						record.identity.candidate_count,
						record.identity.candidate_predecessor_total,
					)?,
					operation_id: record.identity.peer_operation_id,
				})
			})
			.collect()
	}

	#[cfg(test)]
	#[cfg(test)]
	pub(crate) fn inject_fault_once(&self, fault: ReplicationFault) -> Result<(), ContentError> {
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
		let mut durable_bytes = self.durable_bytes.write().map_err(|_| lock_error())?;
		let path = self.root.join(name);
		let existing_bytes = match fs::metadata(&path) {
			Ok(metadata) if metadata.is_file() => metadata.len(),
			Ok(_) => return Err(ContentError::IntegrityFailed),
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
			Err(error) => return Err(io_error(error)),
		};
		let next_total = durable_bytes
			.checked_sub(existing_bytes)
			.ok_or(ContentError::IntegrityFailed)?
			.checked_add(bytes.len() as u64)
			.ok_or(ContentError::ProviderRecoveryTableFull)?;
		if next_total > MAX_TOTAL_RECORD_BYTES {
			return Err(ContentError::ProviderRecoveryTableFull);
		}
		let temp = self.root.join(format!("{name}.tmp-{}", std::process::id()));
		let mut file = File::create(&temp).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		self.trip(ReplicationFault::BeforeTempFsync)?;
		file.sync_all().map_err(io_error)?;
		self.trip(ReplicationFault::AfterTempFsync)?;
		fs::rename(&temp, path).map_err(io_error)?;
		self.trip(ReplicationFault::AfterRename)?;
		sync_dir(&self.root)?;
		self.trip(ReplicationFault::AfterDirectoryFsync)?;
		*durable_bytes = next_total;
		Ok(())
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
	durable_bytes: u64,
}

fn read_state(root: &Path, limit: usize) -> Result<ReadState, ContentError> {
	let mut records = Vec::new();
	let mut scheduler = None;
	let mut visited = 0usize;
	let mut temps = 0usize;
	let mut durable_bytes = 0u64;
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
		let bytes = crate::bounded_io::read_regular_file(
			item.path(),
			MAX_RECORD_BYTES as u64,
		)?;
		durable_bytes = durable_bytes
			.checked_add(bytes.len() as u64)
			.ok_or(ContentError::IntegrityFailed)?;
		if durable_bytes > MAX_TOTAL_RECORD_BYTES {
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
	Ok(ReadState { records, scheduler, durable_bytes })
}

fn input_from_session(
	session: &ReplicationSessionV1,
	peer_operation_id: [u8; 16],
) -> Result<ReplicationIntentInputV1, ContentError> {
	let topology = session.topology();
	let commitment = session.context().candidate_commitment();
	let (candidate_start, candidate_end) = commitment.sequence_range();
	let candidate_count = candidate_end
		.checked_sub(candidate_start)
		.ok_or(ContentError::IntegrityFailed)?;
	Ok(ReplicationIntentInputV1 {
		genesis_hash: topology.genesis_hash,
		topology_snapshot_hash: topology.snapshot_hash,
		topology_finalized_hash: topology.finalized_hash,
		topology_finalized_number: topology.finalized_number,
		topology_governed_checkpoint: topology
			.governed_finalized_checkpoint
			.ok_or(ContentError::IntegrityFailed)?,
		bucket_id: topology.bucket_id,
		bucket_version: topology.bucket_version,
		source_provider: session.source().provider(),
		target_provider: session.target().provider(),
		source_service_key: session.source().service_key(),
		source_service_key_version: session.source().service_key_version(),
		target_service_key: session.target().service_key(),
		target_service_key_version: session.target().service_key_version(),
		source_endpoint_hash: session.source().endpoint_hash(),
		target_endpoint_hash: session.target().endpoint_hash(),
		target_may_confirm: session.target_may_confirm(),
		candidate_mmr_root: commitment.mmr_root(),
		candidate_start,
		candidate_count,
		candidate_predecessor_total: commitment.predecessor_total_size(),
		peer_operation_id,
	})
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
		admitted_page: None,
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

fn same_replan_scope(
	current: &ReplicationIntentInputV1,
	replacement: &ReplicationIntentInputV1,
) -> bool {
	current.genesis_hash == replacement.genesis_hash
		&& current.topology_snapshot_hash == replacement.topology_snapshot_hash
		&& current.topology_finalized_hash == replacement.topology_finalized_hash
		&& current.topology_finalized_number == replacement.topology_finalized_number
		&& current.topology_governed_checkpoint == replacement.topology_governed_checkpoint
		&& current.bucket_id == replacement.bucket_id
		&& current.bucket_version == replacement.bucket_version
		&& current.target_provider == replacement.target_provider
		&& current.target_service_key == replacement.target_service_key
		&& current.target_service_key_version == replacement.target_service_key_version
		&& current.target_endpoint_hash == replacement.target_endpoint_hash
		&& current.target_may_confirm == replacement.target_may_confirm
		&& current.candidate_mmr_root == replacement.candidate_mmr_root
		&& current.candidate_start == replacement.candidate_start
		&& current.candidate_count == replacement.candidate_count
		&& current.candidate_predecessor_total == replacement.candidate_predecessor_total
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
	if let Some(page) = &record.admitted_page {
		validate_admitted_page(record, page)?;
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
				&& record.admitted_page.is_none()
				&& record.local_commitment.is_none()
				&& record.confirmation.is_none() => {},
		ReplicationPhase::MmrCommitted
			if record.next_sequence == candidate_end(&record.identity)?
				&& record.admitted_page.is_none()
				&& record.local_commitment.as_ref()
					== Some(&expected_commitment(&record.identity))
				&& record.confirmation.is_none() => {},
		ReplicationPhase::Confirmed
			if record.next_sequence == candidate_end(&record.identity)?
				&& record.admitted_page.is_none()
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

fn cursor_evidence(cursor: Option<crate::peer::PeerPageCursorV1>) -> Option<PeerCursorEvidenceV1> {
	cursor.map(|cursor| {
		let (last_sequence, cumulative_total) = cursor.position();
		PeerCursorEvidenceV1 { last_sequence, cumulative_total }
	})
}

fn expected_request_cursor(
	record: &ReplicationIntentV1,
) -> Result<Option<PeerCursorEvidenceV1>, ContentError> {
	if record.next_sequence == record.identity.candidate_start {
		if record.cumulative_total != record.identity.candidate_predecessor_total {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(None)
	} else {
		Ok(Some(PeerCursorEvidenceV1 {
			last_sequence: record
				.next_sequence
				.checked_sub(1)
				.ok_or(ContentError::IntegrityFailed)?,
			cumulative_total: record.cumulative_total,
		}))
	}
}

fn object_matches_evidence(
	object: &crate::peer::PeerObjectV1,
	evidence: &AdmittedObjectEvidenceV1,
) -> bool {
	let (length, sequence, cumulative_total) = object.position();
	object.cid() == evidence.cid
		&& length == evidence.length
		&& sequence == evidence.sequence
		&& cumulative_total == evidence.cumulative_total
		&& object.chunk_manifest_hash() == evidence.chunk_manifest_hash
		&& object.chunk_hashes() == evidence.chunk_hashes
}

fn peer_object_from_evidence(
	evidence: &AdmittedObjectEvidenceV1,
) -> Result<crate::peer::PeerObjectV1, ContentError> {
	crate::peer::PeerObjectV1::new(
		&CanonicalCid::parse(&evidence.cid)?,
		evidence.length,
		evidence.sequence,
		evidence.cumulative_total,
		evidence.chunk_hashes.clone(),
	)
}

fn peer_cursor_from_evidence(
	cursor: Option<PeerCursorEvidenceV1>,
) -> Option<crate::peer::PeerPageCursorV1> {
	cursor.map(|cursor| {
		crate::peer::PeerPageCursorV1::new(cursor.last_sequence, cursor.cumulative_total)
	})
}

fn admitted_page_from_response(
	record: &ReplicationIntentV1,
	request: &PeerSyncPageRequestV1,
	response: &PeerSyncPageResponseV1,
	response_wire_hash: [u8; 32],
) -> Result<AdmittedPageEvidenceV1, ContentError> {
	let replay = request.authenticated_replay_identity()?;
	let requested_cursor = cursor_evidence(request.page().0);
	if response_wire_hash == [0; 32]
		|| requested_cursor != expected_request_cursor(record)?
		|| response.items().is_empty()
		|| response.items().len() > 128
	{
		return Err(ContentError::IntegrityFailed);
	}
	let mut objects = Vec::with_capacity(response.items().len());
	for object in response.items() {
		let (length, sequence, cumulative_total) = object.position();
		if object.chunk_hashes().len() > MAX_CHUNKS {
			return Err(ContentError::IntegrityFailed);
		}
		objects.push(AdmittedObjectEvidenceV1 {
			sequence,
			cid: object.cid().into(),
			length,
			cumulative_total,
			chunk_manifest_hash: object.chunk_manifest_hash(),
			chunk_hashes: object.chunk_hashes().to_vec(),
			verified_chunks: vec![None; object.chunk_hashes().len()],
		});
	}
	let page = AdmittedPageEvidenceV1 {
		request_hash: replay.request_hash,
		signed_request_bytes: request.encode_wire(),
		response_wire_hash,
		source_response_hash: response.compact_proof().response_hash(),
		source_signature: response.compact_proof().signature().to_vec(),
		requested_cursor,
		next_cursor: cursor_evidence(response.next_cursor()),
		objects,
		next_object: 0,
	};
	validate_admitted_page(record, &page)?;
	Ok(page)
}

fn validate_admitted_completion(
	record: &ReplicationIntentV1,
	sequence: u64,
	cid: &str,
	length: u64,
) -> Result<(), ContentError> {
	if record.last_completed.as_ref().is_some_and(|completed| {
		completed.sequence == sequence
			&& completed.cid == cid
			&& completed.length == length
			&& record.next_sequence == sequence.saturating_add(1)
	}) {
		return Ok(());
	}
	let object = record
		.admitted_page
		.as_ref()
		.and_then(|page| page.objects.get(page.next_object))
		.ok_or(ContentError::IdempotencyConflict)?;
	if object.sequence != sequence
		|| object.cid != cid
		|| object.length != length
		|| object.verified_chunks.iter().any(Option::is_none)
	{
		return Err(ContentError::IdempotencyConflict);
	}
	Ok(())
}

fn validate_admitted_page(
	record: &ReplicationIntentV1,
	page: &AdmittedPageEvidenceV1,
) -> Result<(), ContentError> {
	if page.request_hash == [0; 32]
		|| page.signed_request_bytes.is_empty()
		|| page.signed_request_bytes.len() > MAX_REQUEST_ENCODED
		|| page.response_wire_hash == [0; 32]
		|| page.objects.is_empty()
		|| page.objects.len() > 128
	{
		return Err(ContentError::IntegrityFailed);
	}
	let request = PeerSyncPageRequestV1::decode_authenticated(&page.signed_request_bytes)?;
	let replay = authenticate_request(
		&record.identity,
		ReplicationRequestKindV1::Page,
		&page.signed_request_bytes,
	)?;
	if replay.request_hash != page.request_hash
		|| cursor_evidence(request.page().0) != page.requested_cursor
	{
		return Err(ContentError::IntegrityFailed);
	}
	let page_signature: [u8; 64] = page
		.source_signature
		.as_slice()
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)?;
	let page_items = page
		.objects
		.iter()
		.map(peer_object_from_evidence)
		.collect::<Result<Vec<_>, _>>()?;
	PeerSyncPageResponseV1::verify_compact_proof(
		&request,
		page_items,
		peer_cursor_from_evidence(page.next_cursor),
		PeerResponseProofV1::from_parts(page.source_response_hash, page_signature),
	)?;
	if page.next_object >= page.objects.len() {
		return Err(ContentError::IntegrityFailed);
	}
	let (mut sequence, mut cumulative_total) = match page.requested_cursor {
		Some(cursor) => (
			cursor.last_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?,
			cursor.cumulative_total,
		),
		None => (record.identity.candidate_start, record.identity.candidate_predecessor_total),
	};
	for (object_index, object) in page.objects.iter().enumerate() {
		CanonicalCid::parse(&object.cid)?;
		let expected_chunks = if object.length == 0 {
			0
		} else {
			object.length.div_ceil(CHUNK_BYTES as u64) as usize
		};
		if object.sequence != sequence
			|| object.length > MAX_STORED_BYTES
			|| object.chunk_manifest_hash == [0; 32]
			|| object.chunk_manifest_hash != replication_chunk_manifest_hash(&object.chunk_hashes)
			|| object.chunk_hashes.len() != expected_chunks
			|| (object_index < page.next_object && !object.verified_chunks.is_empty())
			|| (object_index >= page.next_object && object.verified_chunks.len() != expected_chunks)
			|| expected_chunks > MAX_CHUNKS
		{
			return Err(ContentError::IntegrityFailed);
		}
		cumulative_total = cumulative_total
			.checked_add(object.length)
			.ok_or(ContentError::IntegrityFailed)?;
		if object.cumulative_total != cumulative_total {
			return Err(ContentError::IntegrityFailed);
		}
		for (index, evidence) in object.verified_chunks.iter().enumerate() {
			if let Some(evidence) = evidence {
				let target_signature: [u8; 64] = evidence
					.target_signature
					.as_slice()
					.try_into()
					.map_err(|_| ContentError::IntegrityFailed)?;
				let source_signature: [u8; 64] = evidence
					.source_signature
					.as_slice()
					.try_into()
					.map_err(|_| ContentError::IntegrityFailed)?;
				if usize::from(evidence.index) != index
					|| object.chunk_hashes.get(index) != Some(&evidence.chunk_hash)
					|| evidence.request_nonce == [0; 16]
					|| evidence.request_hash == [0; 32]
					|| evidence.response_wire_hash == [0; 32]
					|| evidence.source_response_hash == [0; 32]
				{
					return Err(ContentError::IntegrityFailed);
				}
				let request = PeerChunkRequestV1::verify_compact_proof(
					expected_peer_context(&record.identity)?,
					record.identity.peer_operation_id,
					peer_object_from_evidence(object)?,
					evidence.index,
					PeerRequestProofV1::from_parts(
						evidence.request_nonce,
						evidence.request_hash,
						target_signature,
					),
				)?;
				PeerChunkResponseV1::verify_compact_proof(
					&request,
					PeerResponseProofV1::from_parts(
						evidence.source_response_hash,
						source_signature,
					),
				)?;
			}
		}
		sequence = sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
	}
	if sequence > candidate_end(&record.identity)? {
		return Err(ContentError::IntegrityFailed);
	}
	let first = page.objects.get(page.next_object).ok_or(ContentError::IntegrityFailed)?;
	let expected_prior = if page.next_object == 0 {
		match page.requested_cursor {
			Some(cursor) => cursor.cumulative_total,
			None => record.identity.candidate_predecessor_total,
		}
	} else {
		page.objects[page.next_object - 1].cumulative_total
	};
	if first.sequence != record.next_sequence || expected_prior != record.cumulative_total {
		return Err(ContentError::IntegrityFailed);
	}
	match page.requested_cursor {
		None if first.sequence < record.identity.candidate_start => {
			return Err(ContentError::IntegrityFailed)
		},
		Some(cursor)
			if cursor.last_sequence >= first.sequence
				|| cursor.cumulative_total > record.cumulative_total =>
		{
			return Err(ContentError::IntegrityFailed)
		},
		_ => {},
	}
	let last = page.objects.last().ok_or(ContentError::IntegrityFailed)?;
	let expected_next = if last.sequence.checked_add(1) == Some(candidate_end(&record.identity)?) {
		None
	} else {
		Some(PeerCursorEvidenceV1 {
			last_sequence: last.sequence,
			cumulative_total: last.cumulative_total,
		})
	};
	if page.next_cursor != expected_next {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn replication_chunk_manifest_hash(chunk_hashes: &[[u8; 32]]) -> [u8; 32] {
	let mut bytes = b"cord/provider/peer-replication/chunk-manifest/v1".to_vec();
	bytes.extend_from_slice(&chunk_hashes.encode());
	blake2_256(&bytes)
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

fn replication_nonce(kind: u8, sequence: u64, chunk: u16) -> [u8; 16] {
	let mut nonce = [0; 16];
	nonce[0] = REPLICATION_NONCE_VERSION;
	nonce[1] = kind;
	nonce[2..10].copy_from_slice(&sequence.to_le_bytes());
	nonce[10..12].copy_from_slice(&chunk.to_le_bytes());
	nonce
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
		chain::{
			ReplicationProviderExclusion, ReplicationProviderSnapshot, ReplicationTopologySnapshot,
		},
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

	fn session_provider(
		provider: [u8; 32],
		order: u8,
		primary: bool,
		service_key: [u8; 32],
		confirmation_invalid: bool,
	) -> ReplicationProviderSnapshot {
		let endpoint = format!("https://replication-{order}.invalid").into_bytes();
		ReplicationProviderSnapshot {
			provider,
			order,
			primary,
			record_present: true,
			endpoint_hash: Some(blake2_256(&endpoint)),
			endpoint: Some(endpoint),
			active_service_key: Some(service_key),
			active_service_key_version: Some(u64::from(order) + 1),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(90),
			overdue_challenges: 0,
			eligible: true,
			usable: !confirmation_invalid,
			exclusions: confirmation_invalid
				.then_some(ReplicationProviderExclusion::ConfirmationInvalid)
				.into_iter()
				.collect(),
			confirmed_checkpoint: None,
		}
	}

	fn session_for_identity(
		identity: &ReplicationIntentInputV1,
		value: u16,
		confirmation_invalid: bool,
	) -> ReplicationSessionV1 {
		let source = identity.source_provider;
		let target = identity.target_provider;
		let third = [250; 32];
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: identity.genesis_hash,
			finalized_hash: identity.topology_finalized_hash,
			finalized_number: identity.topology_finalized_number,
			governed_finalized_checkpoint: Some(identity.topology_governed_checkpoint),
			bucket_id: identity.bucket_id,
			bucket_version: identity.bucket_version,
			primary: source,
			replicas: vec![target, third],
			providers: vec![
				session_provider(source, 0, true, source_pair().public().0, false),
				session_provider(
					target,
					1,
					false,
					target_pair(value).public().0,
					confirmation_invalid,
				),
				session_provider(
					third,
					2,
					false,
					ed25519::Pair::from_seed(&[77; 32]).public().0,
					false,
				),
			],
			current_checkpoint: None,
			snapshot_hash: [0; 32],
		};
		let mut encoded = b"cord/provider/replication-topology/v1".to_vec();
		topology.encode_to(&mut encoded);
		topology.snapshot_hash = blake2_256(&encoded);
		ReplicationSessionV1::from_topology(
			topology,
			target,
			target_pair(value).public().0,
			source,
			target,
			PeerMmrCommitmentV1::new(
				identity.candidate_mmr_root,
				identity.candidate_start,
				identity.candidate_count,
				identity.candidate_predecessor_total,
			)
			.unwrap(),
		)
		.unwrap()
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
			if record.next_sequence == record.identity.candidate_start {
				None
			} else {
				Some(PeerPageCursorV1::new(record.next_sequence - 1, record.cumulative_total))
			},
			32,
		)
		.unwrap();
		PeerSyncPageRequestV1::new_signed(
			&expectation,
			&target_pair(u16::from_le_bytes([
				record.identity.target_provider[0],
				record.identity.target_provider[1],
			])),
		)
		.unwrap()
	}

	fn page_response(
		record: &ReplicationIntentV1,
		page: &PeerSyncPageRequestV1,
		bytes: &[u8],
	) -> PeerSyncPageResponseV1 {
		let total = record.cumulative_total + bytes.len() as u64;
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest(blake2_256(&bytes)),
			bytes.len() as u64,
			record.next_sequence,
			total,
			vec![blake2_256(&bytes)],
		)
		.unwrap();
		let next = (record.next_sequence + 1 < candidate_end(&record.identity).unwrap())
			.then(|| PeerPageCursorV1::new(record.next_sequence, total));
		PeerSyncPageResponseV1::new_signed(page, vec![object], next, &source_pair()).unwrap()
	}

	fn with_verified_page(
		store: &ReplicationIntentStore,
		record: ReplicationIntentV1,
		bytes: &[u8],
	) -> ReplicationIntentV1 {
		if record.admitted_page.is_some() {
			return record;
		}
		let mut nonce = [77; 16];
		nonce[..8].copy_from_slice(&record.next_sequence.to_le_bytes());
		let page = page_request(&record, nonce, record.identity.peer_operation_id);
		let staged = store
			.stage_page_request_before_send(&record.intent_key, &page.encode_wire())
			.unwrap();
		let response = page_response(&record, &page, bytes);
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

	fn admit_object_chunks(
		store: &ReplicationIntentStore,
		record: ReplicationIntentV1,
		bytes: &[u8],
	) -> ReplicationIntentV1 {
		let evidence = record
			.admitted_page
			.as_ref()
			.and_then(|page| page.objects.get(page.next_object))
			.unwrap()
			.clone();
		let object = PeerObjectV1::new(
			&CanonicalCid::parse(&evidence.cid).unwrap(),
			evidence.length,
			evidence.sequence,
			evidence.cumulative_total,
			evidence.chunk_hashes.clone(),
		)
		.unwrap();
		let mut record = record;
		for index in 0..evidence.chunk_hashes.len() {
			let mut nonce = [91; 16];
			nonce[..8].copy_from_slice(&evidence.sequence.to_le_bytes());
			nonce[8..10].copy_from_slice(&(index as u16).to_le_bytes());
			let expectation = PeerChunkExpectationV1::new(
				expected_peer_context(&record.identity).unwrap(),
				PeerRequestIdentityV1::new(record.identity.peer_operation_id, nonce).unwrap(),
				object.clone(),
				index as u16,
			)
			.unwrap();
			let request = PeerChunkRequestV1::new_signed(
				&expectation,
				&target_pair(u16::from_le_bytes([
					record.identity.target_provider[0],
					record.identity.target_provider[1],
				])),
			)
			.unwrap();
			record = store
				.stage_chunk_request_before_send(&record.intent_key, &request.encode_wire())
				.unwrap();
			let start = index * CHUNK_BYTES;
			let end = (start + CHUNK_BYTES).min(bytes.len());
			let response = PeerChunkResponseV1::new_signed(
				&request,
				bytes[start..end].to_vec(),
				&source_pair(),
			)
			.unwrap();
			record = store
				.attach_chunk_response(&record.intent_key, &response.encode_wire())
				.unwrap();
		}
		record
	}

	fn complete_ready(
		store: &ReplicationIntentStore,
		streaming: &StreamingStore,
		record: &ReplicationIntentV1,
		sequence: u64,
		bytes: Vec<u8>,
	) -> ReplicationIntentV1 {
		let record = with_verified_page(store, record.clone(), &bytes);
		let record = admit_object_chunks(store, record, &bytes);
		let operation = derived_stream_id(&record.intent_key, sequence, OPERATION_DOMAIN);
		let (cid, length) = install_ready(streaming, &record, sequence, bytes, operation);
		store
			.complete_object(streaming, &record.intent_key, sequence, &cid, length)
			.unwrap()
	}

	fn finish_objects(
		store: &ReplicationIntentStore,
		streaming: &StreamingStore,
		record: ReplicationIntentV1,
	) -> ReplicationIntentV1 {
		let mut record = record;
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
		let mut record = store.plan(input).unwrap();
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
	fn source_replan_preserves_completed_cursor_and_discards_inflight_source_proof() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let candidate = input(19);
		let initial_session = session_for_identity(&candidate, 19, false);
		let initial_operation = [31; 16];
		let planned = store.plan_session(&initial_session, initial_operation).unwrap();
		let completed = complete_ready(
			&store,
			&streaming,
			&planned,
			planned.next_sequence,
			b"completed before source partition".to_vec(),
		);
		let completed_cursor = (
			completed.next_sequence,
			completed.cumulative_total,
			completed.last_completed.clone(),
		);
		let admitted = with_verified_page(&store, completed, b"inflight source proof");
		assert!(admitted.admitted_page.is_some());
		assert!(admitted.outstanding_request.is_some());

		let topology = initial_session.topology().clone();
		let replacement_source = [250; 32];
		let replacement_session = ReplicationSessionV1::from_topology(
			topology,
			candidate.target_provider,
			target_pair(19).public().0,
			replacement_source,
			candidate.target_provider,
			initial_session.context().candidate_commitment().clone(),
		)
		.unwrap();
		let replacement_operation = [42; 16];
		let replanned = store
			.replan_source(
				&admitted.intent_key,
				&replacement_session,
				replacement_operation,
			)
			.unwrap();

		assert_eq!(replanned.intent_key, admitted.intent_key);
		assert_eq!(replanned.identity.source_provider, replacement_source);
		assert_eq!(replanned.identity.peer_operation_id, replacement_operation);
		assert_eq!(replanned.next_sequence, completed_cursor.0);
		assert_eq!(replanned.cumulative_total, completed_cursor.1);
		assert_eq!(replanned.last_completed, completed_cursor.2);
		assert_eq!(replanned.phase, ReplicationPhase::Receiving);
		assert!(replanned.admitted_page.is_none());
		assert!(replanned.outstanding_request.is_none());
		assert_eq!(replanned.attempts, 0);
		assert!(store
			.attach_page_response(&replanned.intent_key, b"stale source response")
			.is_err());

		drop(store);
		let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
		assert_eq!(reopened.record(&replanned.intent_key).unwrap(), replanned);
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
		let wrong_cursor_expected = PeerPageExpectationV1::new(
			expected_peer_context(&planned.identity).unwrap(),
			PeerRequestIdentityV1::new(planned.identity.peer_operation_id, [8; 16]).unwrap(),
			Some(PeerPageCursorV1::new(
				planned.identity.candidate_start,
				planned.identity.candidate_predecessor_total + 1,
			)),
			32,
		)
		.unwrap();
		let wrong_cursor =
			PeerSyncPageRequestV1::new_signed(&wrong_cursor_expected, &target_pair(20)).unwrap();
		assert!(store
			.stage_page_request_before_send(&planned.intent_key, &wrong_cursor.encode_wire())
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
				&page_response(&planned, &page, b"authenticated page object").encode_wire()
			)
			.is_err());
		let other_page = page_request(&other, [6; 16], other.identity.peer_operation_id);
		assert!(store
			.attach_page_response(
				&planned.intent_key,
				&page_response(&other, &other_page, b"authenticated page object").encode_wire()
			)
			.is_err());
		let changed_request = page_request(&planned, [7; 16], planned.identity.peer_operation_id);
		assert!(store
			.attach_page_response(
				&planned.intent_key,
				&page_response(&planned, &changed_request, b"authenticated page object")
					.encode_wire()
			)
			.is_err());
		let response = page_response(&planned, &page, b"authenticated page object");
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

		let bytes = b"authenticated page object".to_vec();
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest(blake2_256(&bytes)),
			bytes.len() as u64,
			planned.identity.candidate_start,
			planned.identity.candidate_predecessor_total + bytes.len() as u64,
			vec![blake2_256(&bytes)],
		)
		.unwrap();
		let unrelated_bytes = b"unrelated signed object".to_vec();
		let unrelated = PeerObjectV1::new(
			&CanonicalCid::from_digest(blake2_256(&unrelated_bytes)),
			unrelated_bytes.len() as u64,
			planned.identity.candidate_start,
			planned.identity.candidate_predecessor_total + unrelated_bytes.len() as u64,
			vec![blake2_256(&unrelated_bytes)],
		)
		.unwrap();
		let unrelated_expected = PeerChunkExpectationV1::new(
			expected_peer_context(&planned.identity).unwrap(),
			PeerRequestIdentityV1::new(planned.identity.peer_operation_id, [44; 16]).unwrap(),
			unrelated,
			0,
		)
		.unwrap();
		let unrelated_request =
			PeerChunkRequestV1::new_signed(&unrelated_expected, &target_pair(20)).unwrap();
		assert!(store
			.stage_chunk_request_before_send(&planned.intent_key, &unrelated_request.encode_wire())
			.is_err());
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
		let planned = with_verified_page(&store, store.plan(&input(30)).unwrap(), b"first");
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
		let first = with_verified_page(&store, first, b"skipped");
		let first = admit_object_chunks(&store, first, b"skipped");
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
	fn session_factory_preserves_confirmation_invalid_as_non_confirming() {
		let objects = vec![b"session-bound".to_vec()];
		let raw = mmr_input(60, &objects);
		let session = session_for_identity(&raw, 60, true);
		assert!(!session.target_may_confirm());
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let planned = store.plan_session(&session, raw.peer_operation_id).unwrap();
		assert!(!planned.identity.target_may_confirm);
		assert_eq!(planned.identity.topology_snapshot_hash, session.topology().snapshot_hash);
		let sequence = planned.next_sequence;
		let completed = complete_ready(&store, &streaming, &planned, sequence, objects[0].clone());
		let installed = store.mark_installed(&completed.intent_key).unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let committed = store.commit_local_mmr(&mmr, &streaming, &installed.intent_key).unwrap();
		let evidence = confirmation(&committed, &target_pair(60));
		assert_eq!(
			store.confirm(&committed.intent_key, &evidence),
			Err(ContentError::IdempotencyConflict)
		);
	}

	#[test]
	fn schema_v3_faults_and_semantic_provenance_tamper_fail_closed() {
		for (fault, persisted) in [
			(ReplicationFault::BeforeTempFsync, false),
			(ReplicationFault::AfterTempFsync, false),
			(ReplicationFault::AfterRename, true),
			(ReplicationFault::AfterDirectoryFsync, true),
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = ReplicationIntentStore::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(store.plan(&input(70)), Err(ContentError::Io(_))));
			assert_eq!(store.select_tick(), Err(ContentError::IntegrityFailed));
			drop(store);
			let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
			assert_eq!(!reopened.select_tick().unwrap().is_empty(), persisted);
		}

		for fault in [
			ReplicationFault::BeforeTempFsync,
			ReplicationFault::AfterTempFsync,
			ReplicationFault::AfterRename,
			ReplicationFault::AfterDirectoryFsync,
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = ReplicationIntentStore::open(temp.path()).unwrap();
			store.plan(&input(71)).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(store.select_tick(), Err(ContentError::Io(_))));
			assert_eq!(store.select_tick(), Err(ContentError::IntegrityFailed));
			drop(store);
			assert!(ReplicationIntentStore::open(temp.path()).unwrap().select_tick().is_ok());
		}

		let aggregate = tempfile::tempdir().unwrap();
		let aggregate_store = ReplicationIntentStore::open(aggregate.path()).unwrap();
		*aggregate_store.durable_bytes.write().unwrap() = MAX_TOTAL_RECORD_BYTES;
		assert_eq!(aggregate_store.plan(&input(79)), Err(ContentError::ProviderRecoveryTableFull));
		let oversized = tempfile::tempdir().unwrap();
		drop(ReplicationIntentStore::open(oversized.path()).unwrap());
		fs::write(
			oversized.path().join(ROOT).join("oversized.json"),
			vec![0; MAX_RECORD_BYTES + 1],
		)
		.unwrap();
		assert!(matches!(
			ReplicationIntentStore::open(oversized.path()),
			Err(ContentError::IntegrityFailed)
		));

		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let planned = store.plan(&input(72)).unwrap();
		let admitted = with_verified_page(&store, planned, b"tamper-bound object");
		let admitted = admit_object_chunks(&store, admitted, b"tamper-bound object");
		let path = temp.path().join(ROOT).join(format!("{}.json", admitted.intent_key));
		drop(store);
		let original = fs::read(&path).unwrap();
		let mut tampered: ReplicationIntentV1 = serde_json::from_slice(&original).unwrap();
		tampered.admitted_page.as_mut().unwrap().source_signature[0] ^= 1;
		tampered.record_hash = record_hash(&tampered).unwrap();
		fs::write(&path, serde_json::to_vec(&tampered).unwrap()).unwrap();
		assert!(matches!(
			ReplicationIntentStore::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
		let mut tampered: ReplicationIntentV1 = serde_json::from_slice(&original).unwrap();
		tampered.admitted_page.as_mut().unwrap().objects[0].verified_chunks[0]
			.as_mut()
			.unwrap()
			.source_signature[0] ^= 1;
		tampered.record_hash = record_hash(&tampered).unwrap();
		fs::write(&path, serde_json::to_vec(&tampered).unwrap()).unwrap();
		assert!(matches!(
			ReplicationIntentStore::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
		let mut tampered: ReplicationIntentV1 = serde_json::from_slice(&original).unwrap();
		tampered.admitted_page.as_mut().unwrap().objects[0].chunk_hashes[0][0] ^= 1;
		tampered.record_hash = record_hash(&tampered).unwrap();
		fs::write(&path, serde_json::to_vec(&tampered).unwrap()).unwrap();
		assert!(matches!(
			ReplicationIntentStore::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn admitted_provenance_enforces_page_request_and_manifest_bounds() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let mut bounded = input(73);
		bounded.candidate_start = 0;
		bounded.candidate_count = 128;
		bounded.candidate_predecessor_total = 0;
		let planned = store.plan(&bounded).unwrap();
		let expectation = PeerPageExpectationV1::new(
			expected_peer_context(&planned.identity).unwrap(),
			PeerRequestIdentityV1::new(planned.identity.peer_operation_id, [73; 16]).unwrap(),
			None,
			128,
		)
		.unwrap();
		let request = PeerSyncPageRequestV1::new_signed(&expectation, &target_pair(73)).unwrap();
		let staged = store
			.stage_page_request_before_send(&planned.intent_key, &request.encode_wire())
			.unwrap();
		let max_cid = CanonicalCid::from_digest([76; 32]);
		let max_hashes = vec![[75; 32]; MAX_CHUNKS];
		let items = (0..128)
			.map(|sequence| {
				let cumulative = (sequence + 1) * MAX_STORED_BYTES;
				PeerObjectV1::new(
					&max_cid,
					MAX_STORED_BYTES,
					sequence,
					cumulative,
					max_hashes.clone(),
				)
				.unwrap()
			})
			.collect();
		let response =
			PeerSyncPageResponseV1::new_signed(&request, items, None, &source_pair()).unwrap();
		let admitted =
			store.attach_page_response(&staged.intent_key, &response.encode_wire()).unwrap();
		assert_eq!(admitted.admitted_page.as_ref().unwrap().objects.len(), 128);
		assert!(serde_json::to_vec(&admitted).unwrap().len() <= MAX_RECORD_BYTES);
		assert!(store.select_tick().is_ok());

		let manifest_temp = tempfile::tempdir().unwrap();
		let manifest_store = ReplicationIntentStore::open(manifest_temp.path()).unwrap();
		let mut manifest_identity = input(74);
		manifest_identity.candidate_start = 0;
		manifest_identity.candidate_count = 1;
		manifest_identity.candidate_predecessor_total = 0;
		let manifest_plan = manifest_store.plan(&manifest_identity).unwrap();
		let manifest_request =
			page_request(&manifest_plan, [74; 16], manifest_plan.identity.peer_operation_id);
		manifest_store
			.stage_page_request_before_send(
				&manifest_plan.intent_key,
				&manifest_request.encode_wire(),
			)
			.unwrap();
		let hashes = vec![[75; 32]; MAX_CHUNKS];
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest([76; 32]),
			MAX_STORED_BYTES,
			0,
			MAX_STORED_BYTES,
			hashes,
		)
		.unwrap();
		let manifest_response = PeerSyncPageResponseV1::new_signed(
			&manifest_request,
			vec![object],
			None,
			&source_pair(),
		)
		.unwrap();
		let manifest_admitted = manifest_store
			.attach_page_response(&manifest_plan.intent_key, &manifest_response.encode_wire())
			.unwrap();
		assert_eq!(
			manifest_admitted.admitted_page.as_ref().unwrap().objects[0].chunk_hashes.len(),
			MAX_CHUNKS
		);
	}

	#[test]
	fn full_page_progress_compacts_completed_proofs_without_poisoning_persistence() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mut identity = input(75);
		identity.candidate_start = 0;
		identity.candidate_count = 128;
		identity.candidate_predecessor_total = 0;
		let planned = store.plan(&identity).unwrap();
		let expectation = PeerPageExpectationV1::new(
			expected_peer_context(&planned.identity).unwrap(),
			PeerRequestIdentityV1::new(planned.identity.peer_operation_id, [75; 16]).unwrap(),
			None,
			128,
		)
		.unwrap();
		let request = PeerSyncPageRequestV1::new_signed(&expectation, &target_pair(75)).unwrap();
		let staged = store
			.stage_page_request_before_send(&planned.intent_key, &request.encode_wire())
			.unwrap();
		let items = (0..128u64)
			.map(|sequence| {
				let bytes = [sequence as u8];
				PeerObjectV1::new(
					&CanonicalCid::from_digest(blake2_256(&bytes)),
					1,
					sequence,
					sequence + 1,
					vec![blake2_256(&bytes)],
				)
				.unwrap()
			})
			.collect();
		let response =
			PeerSyncPageResponseV1::new_signed(&request, items, None, &source_pair()).unwrap();
		let mut record =
			store.attach_page_response(&staged.intent_key, &response.encode_wire()).unwrap();

		for sequence in 0..128u64 {
			let bytes = vec![sequence as u8];
			record = admit_object_chunks(&store, record, &bytes);
			let operation = derived_stream_id(&record.intent_key, sequence, OPERATION_DOMAIN);
			let (cid, length) = install_ready(&streaming, &record, sequence, bytes, operation);
			record = store
				.complete_object(&streaming, &record.intent_key, sequence, &cid, length)
				.unwrap();
			if let Some(page) = &record.admitted_page {
				assert!(page.objects[..page.next_object]
					.iter()
					.all(|object| object.verified_chunks.is_empty()));
			}
			assert!(store.select_tick().is_ok());
		}
		assert_eq!(record.next_sequence, 128);
		assert!(record.admitted_page.is_none());
		assert!(store.mark_installed(&record.intent_key).is_ok());
		drop(store);
		assert!(ReplicationIntentStore::open(temp.path()).unwrap().select_tick().is_ok());
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

	#[test]
	fn limited_resume_scheduler_advances_all_stalled_intents_across_restart() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		for value in 1..=200 {
			store.plan(&input(value)).unwrap();
		}
		let first = store.select_resume_tick_limit(64).unwrap();
		let second = store.select_resume_tick_limit(64).unwrap();
		assert_eq!(first.len(), 64);
		assert_eq!(second.len(), 64);
		drop(store);

		let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
		let third = reopened.select_resume_tick_limit(64).unwrap();
		let fourth = reopened.select_resume_tick_limit(64).unwrap();
		let mut seen = BTreeMap::new();
		for resume in first.into_iter().chain(second).chain(third).chain(fourth) {
			seen.insert(resume.intent_key, ());
		}
		assert_eq!(seen.len(), 200);
	}

	#[test]
	fn resume_descriptor_is_exact_across_restart() {
		let temp = tempfile::tempdir().unwrap();
		let store = ReplicationIntentStore::open(temp.path()).unwrap();
		let planned = store.plan(&input(91)).unwrap();
		let first = store.select_resume_tick_limit(MAX_TICK_WORK).unwrap();
		assert_eq!(first.len(), 1);
		assert_eq!(first[0].intent_key, planned.intent_key);
		assert_eq!(first[0].operation_id, planned.identity.peer_operation_id);
		assert_eq!(first[0].topology_snapshot_hash, planned.identity.topology_snapshot_hash);
		drop(store);
		let reopened = ReplicationIntentStore::open(temp.path()).unwrap();
		assert_eq!(reopened.select_resume_tick_limit(MAX_TICK_WORK).unwrap(), first);
	}
}
