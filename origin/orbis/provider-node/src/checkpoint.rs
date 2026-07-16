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

//! Private durable preparation of exact Commons checkpoint proposals.

#[cfg(feature = "checkpoint-live")]
#[path = "checkpoint_live.rs"]
pub(crate) mod checkpoint_live;
#[path = "checkpoint_outbox.rs"]
pub(crate) mod checkpoint_outbox;
#[path = "checkpoint_primary.rs"]
pub(crate) mod checkpoint_primary;
#[path = "checkpoint_promotion.rs"]
pub(crate) mod checkpoint_promotion;
#[path = "checkpoint_publication.rs"]
pub(crate) mod checkpoint_publication;
#[path = "checkpoint_quorum.rs"]
pub(crate) mod checkpoint_quorum;
#[cfg(feature = "checkpoint-consumer")]
#[path = "checkpoint_submitter.rs"]
pub(crate) mod checkpoint_submitter;

use std::{
	collections::HashMap,
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
};

use codec::{Decode, Encode};
use orbis_storage_runtime_api::{
	CheckpointDutyInfo, CheckpointDutyPhase as RuntimePhase, CommitmentInfo, RESPONSE_VERSION,
};
use pallet_orbis_storage_provider::{CheckpointContextV1, CommitmentPayloadV2};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
use sp_crypto_hashing::blake2_256;

use crate::{
	chain::validate_checkpoint_duty, storage::bucket_mmr::BucketMmrStore, BucketId, CheckpointDuty,
	CheckpointDutyPhase, ContentError, DiskStore, StreamingStore,
};

const ROOT: &str = "checkpoint-proposals-v2";
const VERSION: u16 = 2;
const DOMAIN: &[u8] = b"cord/storage/checkpoint/v2";
const CONTEXT_DOMAIN: &[u8] = b"cord/storage/checkpoint-context/v1";
const RECORD_DOMAIN: &[u8] = b"cord/storage/checkpoint-proposal-record/v2";
const MAX_PROPOSAL_BYTES: usize = 32_768;
const MAX_PROPOSALS: usize = 8_192;

pub(crate) trait ServiceKeySigner {
	fn public_key(&self) -> [u8; 32];
	fn sign_digest(&self, digest: [u8; 32]) -> [u8; 64];
	fn sign_message(&self, message: &[u8]) -> [u8; 64];
}

impl ServiceKeySigner for ed25519::Pair {
	fn public_key(&self) -> [u8; 32] {
		self.public().0
	}

	fn sign_digest(&self, digest: [u8; 32]) -> [u8; 64] {
		self.sign(&digest).0
	}

	fn sign_message(&self, message: &[u8]) -> [u8; 64] {
		self.sign(message).0
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProposalFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreparedCheckpointProposalV2 {
	pub version: u16,
	pub duty_id: String,
	pub duty_fingerprint: String,
	pub duty_scale: String,
	pub finalized_number: u32,
	pub finalized_hash: String,
	pub snapshot_checkpoint: u32,
	pub snapshot_hash: String,
	pub service_key_version: u64,
	pub service_key: String,
	pub primary_provider: String,
	pub bucket_id: String,
	pub window_start: u32,
	pub window_end: u32,
	pub nonce: u32,
	pub start_seq: u64,
	pub leaf_count: u64,
	pub mmr_root: String,
	pub payload_scale: String,
	pub digest: String,
	pub signature: String,
	pub context_scale: String,
	pub context_digest: String,
	pub context_signature: String,
	pub state: String,
	pub record_hash: String,
}

pub(crate) struct CheckpointProposalStore {
	root: PathBuf,
	state: RwLock<ProposalState>,
	fault: RwLock<Option<ProposalFault>>,
}

#[derive(Default)]
struct ProposalState {
	by_tuple: HashMap<String, PreparedCheckpointProposalV2>,
	by_duty: HashMap<String, String>,
	poisoned: bool,
}

impl CheckpointProposalStore {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let root = root.as_ref().join(ROOT);
		fs::create_dir_all(&root).map_err(io_error)?;
		let mut by_tuple = HashMap::new();
		let mut by_duty = HashMap::new();
		for item in fs::read_dir(&root).map_err(io_error)? {
			let item = item.map_err(io_error)?;
			let name = item.file_name().to_string_lossy().into_owned();
			if name.contains(".tmp-") {
				fs::remove_file(item.path()).map_err(io_error)?;
				continue;
			}
			if !name.ends_with(".json") || !item.file_type().map_err(io_error)?.is_file() {
				return Err(ContentError::IntegrityFailed);
			}
			let bytes = fs::read(item.path()).map_err(io_error)?;
			if bytes.len() > MAX_PROPOSAL_BYTES {
				return Err(ContentError::IntegrityFailed);
			}
			let proposal: PreparedCheckpointProposalV2 =
				serde_json::from_slice(&bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_proposal(&proposal)?;
			let tuple = tuple_key(&proposal.bucket_id, proposal.nonce, proposal.start_seq)?;
			if name != format!("{tuple}.json") || by_tuple.len() >= MAX_PROPOSALS {
				return Err(ContentError::IntegrityFailed);
			}
			if by_tuple.insert(tuple.clone(), proposal.clone()).is_some() ||
				by_duty.insert(proposal.duty_id.clone(), tuple).is_some()
			{
				return Err(ContentError::IdempotencyConflict);
			}
		}
		Ok(Self {
			root,
			state: RwLock::new(ProposalState { by_tuple, by_duty, poisoned: false }),
			fault: RwLock::new(None),
		})
	}

	pub(crate) fn inject_fault_once(&self, fault: ProposalFault) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	pub(crate) fn pending_checkpoint_proposals(
		&self,
	) -> Result<Vec<PreparedCheckpointProposalV2>, ContentError> {
		let state = self.state.read().map_err(|_| lock_error())?;
		if state.poisoned || state.by_tuple.len() > MAX_PROPOSALS {
			return Err(ContentError::IntegrityFailed);
		}
		let mut proposals = state.by_tuple.values().cloned().collect::<Vec<_>>();
		proposals.sort_by(|left, right| {
			(&left.bucket_id, left.nonce, left.start_seq, &left.duty_id).cmp(&(
				&right.bucket_id,
				right.nonce,
				right.start_seq,
				&right.duty_id,
			))
		});
		Ok(proposals)
	}

	#[allow(clippy::too_many_arguments)]
	pub(crate) fn prepare(
		&self,
		disk: &DiskStore,
		duty_id: &str,
		mmr: &BucketMmrStore,
		streaming: &StreamingStore,
		signer: &dyn ServiceKeySigner,
	) -> Result<PreparedCheckpointProposalV2, ContentError> {
		let duty = disk
			.pending_checkpoint_duties()
			.map_err(io_error)?
			.into_iter()
			.find(|duty| normalize_hash(&duty.duty_id).ok() == normalize_hash(duty_id).ok())
			.ok_or(ContentError::NotFound)?;
		let watermark = disk
			.checkpoint_duty_watermark()
			.map_err(io_error)?
			.ok_or(ContentError::IntegrityFailed)?;
		self.prepare_exact(&duty, &watermark, mmr, streaming, signer)
	}

	#[allow(clippy::too_many_arguments)]
	fn prepare_exact(
		&self,
		duty: &CheckpointDuty,
		watermark: &crate::CheckpointDutyWatermark,
		mmr: &BucketMmrStore,
		streaming: &StreamingStore,
		signer: &dyn ServiceKeySigner,
	) -> Result<PreparedCheckpointProposalV2, ContentError> {
		let encoded = hex::decode(duty.encoded_duty.trim_start_matches("0x"))
			.map_err(|_| ContentError::IntegrityFailed)?;
		if normalize_hash(&duty.duty_fingerprint)? != hex::encode(blake2_256(&encoded)) {
			return Err(ContentError::IntegrityFailed);
		}
		let mut input = &encoded[..];
		let decoded = CheckpointDutyInfo::<AccountId32, H256, u32>::decode(&mut input)
			.map_err(|_| ContentError::IntegrityFailed)?;
		if !input.is_empty() {
			return Err(ContentError::IntegrityFailed);
		}
		let provider = decode_account(&duty.provider)?;
		if signer.public_key() != decode_32(&duty.service_key)? {
			return Err(ContentError::IntegrityFailed);
		}
		let expected_start = match decoded.previous_commitment {
			None => 0,
			Some(CommitmentInfo { start_seq, leaf_count, .. }) =>
				start_seq.checked_add(leaf_count).ok_or(ContentError::IntegrityFailed)?,
		};
		if expected_start != decoded.expected_next_start_seq {
			return Err(ContentError::IntegrityFailed);
		}
		let bucket = BucketId::from_bytes(decoded.bucket_id.0);
		let tuple = tuple_key(
			&format!("{:#x}", decoded.bucket_id),
			decoded.expected_nonce,
			expected_start,
		)?;
		let mut state = self.state.write().map_err(|_| lock_error())?;
		if state.poisoned {
			return Err(ContentError::IntegrityFailed);
		}
		let duty_id = normalize_hash(&duty.duty_id)?;
		let duty_fingerprint = normalize_hash(&duty.duty_fingerprint)?;
		let duty_scale = hex::encode(&encoded);
		let snapshot_hash = normalize_hash(&duty.snapshot_hash)?;
		let service_key = hex::encode(signer.public_key());
		let primary_provider = hex::encode(<AccountId32 as AsRef<[u8]>>::as_ref(&provider));
		let bucket_id = normalize_hash(&duty.bucket_id)?;
		if let Some(existing) = state.by_tuple.get(&tuple).cloned() {
			if existing.duty_id == duty_id &&
				existing.duty_fingerprint == duty_fingerprint &&
				existing.duty_scale == duty_scale &&
				existing.snapshot_checkpoint == duty.snapshot_checkpoint &&
				existing.snapshot_hash == snapshot_hash &&
				existing.service_key_version == duty.service_key_version &&
				existing.service_key == service_key &&
				existing.primary_provider == primary_provider &&
				existing.bucket_id == bucket_id &&
				existing.window_start == decoded.due_at &&
				existing.window_end == decoded.grace_until &&
				existing.nonce == decoded.expected_nonce &&
				existing.start_seq == expected_start
			{
				return Ok(existing);
			}
			return Err(ContentError::IdempotencyConflict);
		}
		if state.by_duty.contains_key(&duty_id) || state.by_tuple.len() >= MAX_PROPOSALS {
			return Err(ContentError::IdempotencyConflict);
		}
		let projected = validate_checkpoint_duty(
			decoded.clone(),
			&provider,
			signer.public_key(),
			duty.snapshot_checkpoint,
		)
		.map_err(|_| ContentError::IntegrityFailed)?;
		if projected != *duty || watermark.snapshot_checkpoint != duty.snapshot_checkpoint {
			return Err(ContentError::IntegrityFailed);
		}
		let finalized_hash = normalize_hash(&watermark.finalized_hash)?;
		if decoded.snapshot_checkpoint != duty.snapshot_checkpoint ||
			decoded.expected_nonce != decoded.snapshot_checkpoint ||
			hex::encode(decoded.snapshot_hash.as_bytes()) != snapshot_hash
		{
			return Err(ContentError::IntegrityFailed);
		}
		if !duty.may_sign ||
			!duty.may_initiate ||
			decoded.initiator.as_ref() != Some(&provider) ||
			!matches!(
				duty.phase,
				CheckpointDutyPhase::Primary | CheckpointDutyPhase::ReplicaFallback
			) || decoded.required_primary_confirmations != 1 ||
			decoded.required_replica_confirmations != 2
		{
			return Err(ContentError::IntegrityFailed);
		}
		let local = decoded
			.authorities
			.iter()
			.find(|authority| authority.provider == provider)
			.ok_or(ContentError::IntegrityFailed)?;
		if local.active_service_key_version != duty.service_key_version ||
			local.active_service_key != signer.public_key() ||
			!local.may_sign ||
			!local.may_initiate ||
			!local.eligible ||
			!local.organization_sla_eligible ||
			local.overdue_challenge ||
			local.exclusion.is_some() ||
			local.initiation_exclusion.is_some()
		{
			return Err(ContentError::IntegrityFailed);
		}
		if !matches!(decoded.phase, RuntimePhase::Primary | RuntimePhase::ReplicaFallback) {
			return Err(ContentError::IntegrityFailed);
		}
		let commitment = mmr.commitment_candidate(streaming, bucket, expected_start)?;
		if commitment.leaf_count == 0 ||
			commitment.start_seq != expected_start ||
			commitment.range_end().is_none()
		{
			return Err(ContentError::IntegrityFailed);
		}
		let payload = CommitmentPayloadV2 {
			version: 2,
			bucket_id: decoded.bucket_id,
			commitment,
			nonce: decoded.expected_nonce,
		};
		let payload_scale = payload.encode();
		let mut digest_input = DOMAIN.to_vec();
		digest_input.extend_from_slice(&payload_scale);
		let digest = blake2_256(&digest_input);
		let signature = signer.sign_digest(digest);
		let context = CheckpointContextV1 {
			version: 1,
			genesis_hash: decoded.commons_genesis_hash,
			spec_version: decoded.commons_spec_version,
			transaction_version: decoded.commons_transaction_version,
			metadata_hash: decoded.commons_metadata_hash,
			finalized_hash: decoded.snapshot_hash,
			duty_id: decoded.duty_id,
			v2_digest: digest,
		};
		let context_scale = context.encode();
		let mut context_input = CONTEXT_DOMAIN.to_vec();
		context_input.extend_from_slice(&context_scale);
		let context_digest = blake2_256(&context_input);
		let context_signature = signer.sign_digest(context_digest);
		let mut proposal = PreparedCheckpointProposalV2 {
			version: VERSION,
			duty_id,
			duty_fingerprint,
			duty_scale,
			finalized_number: watermark.finalized_number,
			finalized_hash,
			snapshot_checkpoint: duty.snapshot_checkpoint,
			snapshot_hash,
			service_key_version: duty.service_key_version,
			service_key,
			primary_provider,
			bucket_id,
			window_start: decoded.due_at,
			window_end: decoded.grace_until,
			nonce: decoded.expected_nonce,
			start_seq: commitment.start_seq,
			leaf_count: commitment.leaf_count,
			mmr_root: hex::encode(commitment.mmr_root.as_bytes()),
			payload_scale: hex::encode(payload_scale),
			digest: hex::encode(digest),
			signature: hex::encode(signature),
			context_scale: hex::encode(context_scale),
			context_digest: hex::encode(context_digest),
			context_signature: hex::encode(context_signature),
			state: "prepared".into(),
			record_hash: String::new(),
		};
		proposal.record_hash = proposal_record_hash(&proposal)?;
		if let Err(error) = self.persist(&tuple, &proposal) {
			state.poisoned = true;
			return Err(error);
		}
		state.by_tuple.insert(tuple.clone(), proposal.clone());
		state.by_duty.insert(proposal.duty_id.clone(), tuple);
		Ok(proposal)
	}

	fn persist(
		&self,
		key: &str,
		proposal: &PreparedCheckpointProposalV2,
	) -> Result<(), ContentError> {
		validate_proposal(proposal)?;
		let bytes = serde_json::to_vec(proposal).map_err(io_error)?;
		if bytes.len() > MAX_PROPOSAL_BYTES {
			return Err(ContentError::IntegrityFailed);
		}
		let path = self.root.join(format!("{key}.json"));
		if path.exists() {
			let existing = fs::read(&path).map_err(io_error)?;
			if existing.len() > MAX_PROPOSAL_BYTES {
				return Err(ContentError::IntegrityFailed);
			}
			let existing: PreparedCheckpointProposalV2 =
				serde_json::from_slice(&existing).map_err(|_| ContentError::IntegrityFailed)?;
			validate_proposal(&existing)?;
			return if existing == *proposal {
				Ok(())
			} else {
				Err(ContentError::IdempotencyConflict)
			}
		}
		let temp = self.root.join(format!("{key}.json.tmp-{}", std::process::id()));
		let mut file = File::create(&temp).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		self.trip(ProposalFault::BeforeTempFsync)?;
		file.sync_all().map_err(io_error)?;
		self.trip(ProposalFault::AfterTempFsync)?;
		fs::rename(temp, path).map_err(io_error)?;
		self.trip(ProposalFault::AfterRename)?;
		File::open(&self.root).and_then(|dir| dir.sync_all()).map_err(io_error)?;
		self.trip(ProposalFault::AfterDirectoryFsync)
	}

	fn trip(&self, point: ProposalFault) -> Result<(), ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			Err(ContentError::Io(format!("injected proposal fault: {point:?}")))
		} else {
			Ok(())
		}
	}
}

fn validate_proposal(proposal: &PreparedCheckpointProposalV2) -> Result<(), ContentError> {
	if proposal.version != VERSION ||
		proposal.state != "prepared" ||
		proposal.leaf_count == 0 ||
		proposal.nonce != proposal.snapshot_checkpoint ||
		proposal.start_seq.checked_add(proposal.leaf_count).is_none()
	{
		return Err(ContentError::IntegrityFailed);
	}
	for value in [
		&proposal.duty_id,
		&proposal.duty_fingerprint,
		&proposal.finalized_hash,
		&proposal.snapshot_hash,
		&proposal.service_key,
		&proposal.primary_provider,
		&proposal.bucket_id,
		&proposal.mmr_root,
		&proposal.digest,
		&proposal.context_digest,
		&proposal.record_hash,
	] {
		if normalize_hash(value)? != *value {
			return Err(ContentError::IntegrityFailed);
		}
	}
	if proposal.record_hash != proposal_record_hash(proposal)? {
		return Err(ContentError::IntegrityFailed);
	}
	canonical_hex(&proposal.signature, 64)?;
	canonical_hex(&proposal.context_signature, 64)?;
	let duty_bytes =
		hex::decode(&proposal.duty_scale).map_err(|_| ContentError::IntegrityFailed)?;
	if duty_bytes.is_empty() ||
		hex::encode(&duty_bytes) != proposal.duty_scale ||
		hex::encode(blake2_256(&duty_bytes)) != proposal.duty_fingerprint
	{
		return Err(ContentError::IntegrityFailed);
	}
	let mut duty_input = &duty_bytes[..];
	let duty = CheckpointDutyInfo::<AccountId32, H256, u32>::decode(&mut duty_input)
		.map_err(|_| ContentError::IntegrityFailed)?;
	if !duty_input.is_empty() ||
		duty.encode() != duty_bytes ||
		duty.response_version != RESPONSE_VERSION ||
		hex::encode(duty.duty_id.as_bytes()) != proposal.duty_id ||
		hex::encode(duty.bucket_id.as_bytes()) != proposal.bucket_id ||
		hex::encode(duty.snapshot_hash.as_bytes()) != proposal.snapshot_hash ||
		duty.snapshot_checkpoint != proposal.snapshot_checkpoint ||
		duty.expected_nonce != proposal.nonce ||
		duty.due_at != proposal.window_start ||
		duty.grace_until != proposal.window_end ||
		proposal.window_start > proposal.window_end ||
		!matches!(duty.phase, RuntimePhase::Primary | RuntimePhase::ReplicaFallback) ||
		duty.required_primary_confirmations != 1 ||
		duty.required_replica_confirmations != 2
	{
		return Err(ContentError::IntegrityFailed);
	}
	let initiator = duty.initiator.as_ref().ok_or(ContentError::IntegrityFailed)?;
	if hex::encode(<AccountId32 as AsRef<[u8]>>::as_ref(initiator)) != proposal.primary_provider {
		return Err(ContentError::IntegrityFailed);
	}
	let authority = duty
		.authorities
		.iter()
		.find(|authority| &authority.provider == initiator)
		.ok_or(ContentError::IntegrityFailed)?;
	if authority.active_service_key_version != proposal.service_key_version ||
		authority.active_service_key != decode_32(&proposal.service_key)? ||
		!authority.may_sign ||
		!authority.may_initiate ||
		!authority.eligible ||
		!authority.organization_sla_eligible ||
		authority.overdue_challenge ||
		authority.exclusion.is_some() ||
		authority.initiation_exclusion.is_some()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let payload_bytes =
		hex::decode(&proposal.payload_scale).map_err(|_| ContentError::IntegrityFailed)?;
	if hex::encode(&payload_bytes) != proposal.payload_scale {
		return Err(ContentError::IntegrityFailed);
	}
	let mut payload_input = &payload_bytes[..];
	let payload = CommitmentPayloadV2::<H256, u32>::decode(&mut payload_input)
		.map_err(|_| ContentError::IntegrityFailed)?;
	if !payload_input.is_empty() ||
		payload.version != 2 ||
		hex::encode(payload.bucket_id.as_bytes()) != proposal.bucket_id ||
		payload.nonce != proposal.nonce ||
		payload.commitment.start_seq != proposal.start_seq ||
		payload.commitment.leaf_count != proposal.leaf_count ||
		hex::encode(payload.commitment.mmr_root.as_bytes()) != proposal.mmr_root ||
		payload.commitment.range_end().is_none()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let mut input = DOMAIN.to_vec();
	input.extend_from_slice(&payload_bytes);
	let digest = blake2_256(&input);
	if proposal.digest != hex::encode(digest) {
		return Err(ContentError::IntegrityFailed);
	}
	let key = ed25519::Public::from_raw(decode_32(&proposal.service_key)?);
	let signature: [u8; 64] = hex::decode(&proposal.signature)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)?;
	if !ed25519::Pair::verify(&ed25519::Signature::from_raw(signature), &digest, &key) {
		return Err(ContentError::IntegrityFailed);
	}
	let context_bytes =
		hex::decode(&proposal.context_scale).map_err(|_| ContentError::IntegrityFailed)?;
	if context_bytes.is_empty() || hex::encode(&context_bytes) != proposal.context_scale {
		return Err(ContentError::IntegrityFailed);
	}
	let mut context_input = &context_bytes[..];
	let context = CheckpointContextV1::<H256>::decode(&mut context_input)
		.map_err(|_| ContentError::IntegrityFailed)?;
	let expected_context = CheckpointContextV1 {
		version: 1,
		genesis_hash: duty.commons_genesis_hash,
		spec_version: duty.commons_spec_version,
		transaction_version: duty.commons_transaction_version,
		metadata_hash: duty.commons_metadata_hash,
		finalized_hash: duty.snapshot_hash,
		duty_id: duty.duty_id,
		v2_digest: digest,
	};
	if !context_input.is_empty() || context.encode() != context_bytes || context != expected_context
	{
		return Err(ContentError::IntegrityFailed);
	}
	let mut signed_context = CONTEXT_DOMAIN.to_vec();
	signed_context.extend_from_slice(&context_bytes);
	let context_digest = blake2_256(&signed_context);
	if proposal.context_digest != hex::encode(context_digest) {
		return Err(ContentError::IntegrityFailed);
	}
	let context_signature: [u8; 64] = hex::decode(&proposal.context_signature)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)?;
	if !ed25519::Pair::verify(
		&ed25519::Signature::from_raw(context_signature),
		&context_digest,
		&key,
	) {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn proposal_record_hash(proposal: &PreparedCheckpointProposalV2) -> Result<String, ContentError> {
	let mut canonical = proposal.clone();
	canonical.record_hash.clear();
	let bytes = serde_json::to_vec(&canonical).map_err(io_error)?;
	let mut input = RECORD_DOMAIN.to_vec();
	input.extend_from_slice(&bytes);
	Ok(hex::encode(blake2_256(&input)))
}

fn tuple_key(bucket: &str, nonce: u32, start: u64) -> Result<String, ContentError> {
	let bytes = format!("{}:{nonce}:{start}", normalize_hash(bucket)?);
	Ok(hex::encode(blake2_256(bytes.as_bytes())))
}

fn normalize_hash(value: &str) -> Result<String, ContentError> {
	let value = value.trim_start_matches("0x");
	if value.len() != 64 || value.bytes().any(|byte| byte.is_ascii_uppercase()) {
		return Err(ContentError::IntegrityFailed);
	}
	hex::decode(value).map_err(|_| ContentError::IntegrityFailed)?;
	Ok(value.into())
}

fn decode_32(value: &str) -> Result<[u8; 32], ContentError> {
	hex::decode(normalize_hash(value)?)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)
}

fn canonical_hex(value: &str, bytes: usize) -> Result<(), ContentError> {
	if value.len() != bytes.saturating_mul(2) ||
		value.bytes().any(|byte| byte.is_ascii_uppercase()) ||
		hex::decode(value).map_err(|_| ContentError::IntegrityFailed)?.len() != bytes
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn decode_account(value: &str) -> Result<AccountId32, ContentError> {
	Ok(AccountId32::new(decode_32(value)?))
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

fn lock_error() -> ContentError {
	ContentError::Io("checkpoint proposal lock poisoned".into())
}

#[cfg(test)]
mod tests {
	use super::*;
	use orbis_storage_runtime_api::{
		CheckpointDutyMode, CheckpointDutyPhase, ProviderDutyAuthority, ProviderDutyRole,
		RESPONSE_VERSION,
	};
	use tempfile::TempDir;

	use crate::{CheckpointDutyBatch, NodeProfile, OperationId, StreamingDescriptor};

	fn authority(
		provider: AccountId32,
		role: ProviderDutyRole,
		order: u8,
		key: [u8; 32],
		may_initiate: bool,
	) -> ProviderDutyAuthority<AccountId32, H256, u32> {
		ProviderDutyAuthority {
			provider,
			role,
			order,
			active_service_key_version: 5,
			active_service_key: key,
			endpoint_hash: H256::repeat_byte(order),
			organization_sla_eligible: true,
			overdue_challenge: false,
			eligible: true,
			may_sign: true,
			may_initiate,
			exclusion: None,
			initiation_exclusion: None,
			confirmed_checkpoint: None,
		}
	}

	fn typed_duty(key: [u8; 32], duty_id: u8) -> CheckpointDutyInfo<AccountId32, H256, u32> {
		let primary = AccountId32::new([1; 32]);
		let first = AccountId32::new([2; 32]);
		let second = AccountId32::new([3; 32]);
		CheckpointDutyInfo {
			response_version: RESPONSE_VERSION,
			commons_genesis_hash: H256::repeat_byte(10),
			commons_spec_version: 1,
			commons_transaction_version: 1,
			commons_metadata_hash: H256::repeat_byte(11),
			duty_id: H256::repeat_byte(duty_id),
			bucket_id: H256::repeat_byte(4),
			primary: primary.clone(),
			replicas: vec![first.clone(), second.clone()],
			authorities: vec![
				authority(primary.clone(), ProviderDutyRole::Primary, 0, key, true),
				authority(first, ProviderDutyRole::Replica, 1, [8; 32], false),
				authority(second, ProviderDutyRole::Replica, 2, [9; 32], false),
			],
			initiator: Some(primary),
			phase: CheckpointDutyPhase::Primary,
			mode: CheckpointDutyMode::Standard,
			snapshot_checkpoint: 100,
			snapshot_hash: H256::repeat_byte(12),
			due_at: 100,
			grace_until: 110,
			expected_nonce: 100,
			scheduled_at: 99,
			previous_commitment: None,
			previous_checkpoint: None,
			expected_next_start_seq: 0,
			required_primary_confirmations: 1,
			required_replica_confirmations: 2,
		}
	}

	fn project(typed: CheckpointDutyInfo<AccountId32, H256, u32>, key: [u8; 32]) -> CheckpointDuty {
		validate_checkpoint_duty(typed, &AccountId32::new([1; 32]), key, 100).unwrap()
	}

	fn watermark() -> crate::CheckpointDutyWatermark {
		crate::CheckpointDutyWatermark {
			finalized_hash: format!("0x{}", "0d".repeat(32)),
			finalized_number: 111,
			snapshot_checkpoint: 100,
			cursor: None,
		}
	}

	fn install(streaming: &StreamingStore, mmr: &BucketMmrStore, operation: u8, bytes: &[u8]) {
		let bucket_id = BucketId::from_bytes([4; 32]);
		let operation_id = OperationId::from_bytes([operation; 16]);
		let cid = crate::CanonicalCid::from_digest(blake2_256(bytes));
		streaming
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
		mmr.append_verified(streaming, bucket_id, operation_id).unwrap();
	}

	#[test]
	fn checkpoint_v2_fixture_matches_exact_scale_digest_and_signature() {
		let signed_message = hex::decode("636f72642f73746f726167652f636865636b706f696e742f763202000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f07000000000000000300000000000000e8030000").unwrap();
		assert!(signed_message.starts_with(DOMAIN));
		let payload_bytes = &signed_message[DOMAIN.len()..];
		let mut input = payload_bytes;
		let payload = CommitmentPayloadV2::<H256, u32>::decode(&mut input).unwrap();
		assert!(input.is_empty());
		assert_eq!(payload.encode(), payload_bytes);
		assert_eq!(payload.version, 2);
		assert_eq!(payload.commitment.start_seq, 7);
		assert_eq!(payload.commitment.leaf_count, 3);
		assert_eq!(payload.nonce, 1000);
		let digest = blake2_256(&signed_message);
		assert_eq!(
			hex::encode(digest),
			"ace62a2f3c3887586e55ca13c4ba2313583801f9874380aae8a583a119c89920"
		);
		let public = ed25519::Public::from_raw(
			hex::decode("d759793bbc13a2819a827c76adb6fba8a49aee007f49f2d0992d99b825ad2c48")
				.unwrap()
				.try_into()
				.unwrap(),
		);
		let signature = ed25519::Signature::from_raw(
			hex::decode("dedf813237ae6e7e953b1bfe1114206e24c5a26d1c591e8cab2fdbd3532bee8f2226e0d809c0b4c2869f8ce7998e6cc7edf7543a4e8f19af0a2c06fdb46d2501")
				.unwrap()
				.try_into()
				.unwrap(),
		);
		assert!(ed25519::Pair::verify(&signature, &digest, &public));
	}

	#[test]
	fn checkpoint_context_fixture_matches_runtime_scale_digest_and_signature() {
		let context = CheckpointContextV1 {
			version: 1,
			genesis_hash: H256::repeat_byte(10),
			spec_version: 1,
			transaction_version: 1,
			metadata_hash: H256::repeat_byte(11),
			finalized_hash: H256::repeat_byte(12),
			duty_id: H256::repeat_byte(5),
			v2_digest: hex::decode(
				"ace62a2f3c3887586e55ca13c4ba2313583801f9874380aae8a583a119c89920",
			)
			.unwrap()
			.try_into()
			.unwrap(),
		};
		let scale = context.encode();
		let mut input = CONTEXT_DOMAIN.to_vec();
		input.extend_from_slice(&scale);
		let digest = blake2_256(&input);
		let signer = ed25519::Pair::from_seed(&[7; 32]);
		let signature = signer.sign_digest(digest);
		assert_eq!(hex::encode(&scale), "010a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a01000000010000000b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0505050505050505050505050505050505050505050505050505050505050505ace62a2f3c3887586e55ca13c4ba2313583801f9874380aae8a583a119c89920");
		assert_eq!(
			hex::encode(digest),
			"813859d02f56c76f8ecb615542729b4fb6628d9534048a2bc6c4713ff42137a0"
		);
		assert_eq!(hex::encode(signature), "5ff25804878efd113e2d4667b406d671e916bae819f4177eaade713ecdf5bbba79bce7db5387d12ed1efd7d98459631ce61a6b54d472561bfaf152f0b18ebd04");
		let mut decoded = &scale[..];
		assert_eq!(CheckpointContextV1::<H256>::decode(&mut decoded).unwrap(), context);
		assert!(decoded.is_empty());
	}

	#[test]
	fn prepares_from_distinct_finalized_and_snapshot_contexts_and_retries_after_growth() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		install(&streaming, &mmr, 1, b"first");
		let signer = ed25519::Pair::from_seed(&[7; 32]);
		let duty = project(typed_duty(signer.public().0, 5), signer.public().0);
		let disk = DiskStore::open(
			temp.path().join("disk"),
			NodeProfile {
				provider: duty.provider.clone(),
				endpoint: "http://127.0.0.1:8080".into(),
				service_key: duty.service_key.clone(),
				region: None,
			},
			1024,
		)
		.unwrap();
		let context = watermark();
		disk.stage_checkpoint_duty_page(CheckpointDutyBatch {
			finalized_hash: context.finalized_hash.clone(),
			finalized_number: context.finalized_number,
			provider: duty.provider.clone(),
			snapshot_checkpoint: context.snapshot_checkpoint,
			requested_cursor: None,
			next_cursor: None,
			duties: vec![duty.clone()],
		})
		.unwrap();
		let store = CheckpointProposalStore::open(temp.path()).unwrap();
		let first = store.prepare(&disk, &duty.duty_id, &mmr, &streaming, &signer).unwrap();
		assert_eq!(first.finalized_number, 111);
		assert_eq!(first.snapshot_checkpoint, 100);
		assert_eq!(first.primary_provider, "01".repeat(32));
		assert_eq!((first.window_start, first.window_end), (100, 110));
		assert!(!first.context_scale.is_empty());
		assert!(!first.context_digest.is_empty());
		assert!(!first.context_signature.is_empty());
		install(&streaming, &mmr, 2, b"second");
		let retry = store.prepare(&disk, &duty.duty_id, &mmr, &streaming, &signer).unwrap();
		assert_eq!(retry, first);
		disk.stage_checkpoint_duty_page(CheckpointDutyBatch {
			finalized_hash: format!("0x{}", "ee".repeat(32)),
			finalized_number: 222,
			provider: duty.provider.clone(),
			snapshot_checkpoint: 101,
			requested_cursor: None,
			next_cursor: None,
			duties: Vec::new(),
		})
		.unwrap();
		let advanced = store.prepare(&disk, &duty.duty_id, &mmr, &streaming, &signer).unwrap();
		assert_eq!(advanced, first);
		assert_eq!(store.pending_checkpoint_proposals().unwrap(), vec![first]);
		assert_eq!(
			CheckpointProposalStore::open(temp.path())
				.unwrap()
				.state
				.read()
				.unwrap()
				.by_tuple
				.len(),
			1
		);
	}

	#[test]
	fn replica_fallback_persists_local_initiator_not_stale_primary() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		install(&streaming, &mmr, 1, b"first");
		let signer = ed25519::Pair::from_seed(&[7; 32]);
		let mut typed = typed_duty([6; 32], 5);
		let replica = AccountId32::new([2; 32]);
		typed.initiator = Some(replica.clone());
		typed.phase = CheckpointDutyPhase::ReplicaFallback;
		typed.authorities[1].active_service_key = signer.public().0;
		typed.authorities[1].may_initiate = true;
		let duty = validate_checkpoint_duty(typed, &replica, signer.public().0, 100).unwrap();
		let store = CheckpointProposalStore::open(temp.path()).unwrap();
		let proposal = store.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).unwrap();
		assert_eq!(proposal.primary_provider, "02".repeat(32));
		assert_ne!(proposal.primary_provider, "01".repeat(32));
		let context = CheckpointContextV1::<H256>::decode(
			&mut &hex::decode(&proposal.context_scale).unwrap()[..],
		)
		.unwrap();
		assert_eq!(context.duty_id, H256::repeat_byte(5));
	}

	#[test]
	fn rejects_changed_duty_and_malformed_exact_encoding() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		install(&streaming, &mmr, 1, b"first");
		let signer = ed25519::Pair::from_seed(&[7; 32]);
		let duty = project(typed_duty(signer.public().0, 5), signer.public().0);
		let store = CheckpointProposalStore::open(temp.path()).unwrap();
		store.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).unwrap();
		let changed = project(typed_duty(signer.public().0, 6), signer.public().0);
		assert_eq!(
			store.prepare_exact(&changed, &watermark(), &mmr, &streaming, &signer),
			Err(ContentError::IdempotencyConflict)
		);
		let mut trailing = changed;
		let encoded = format!("{}00", trailing.encoded_duty);
		trailing.encoded_duty = encoded.clone();
		trailing.duty_fingerprint =
			format!("0x{}", hex::encode(blake2_256(&hex::decode(&encoded[2..]).unwrap())));
		assert!(store.prepare_exact(&trailing, &watermark(), &mmr, &streaming, &signer).is_err());
	}

	#[test]
	fn rejects_non_initiator_phase_quorum_key_version_and_sequence_gaps() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let signer = ed25519::Pair::from_seed(&[7; 32]);
		let store = CheckpointProposalStore::open(temp.path()).unwrap();
		let mut variants = Vec::new();

		let mut no_initiator = typed_duty(signer.public().0, 1);
		no_initiator.initiator = None;
		variants.push(project(no_initiator, signer.public().0));

		let mut not_due = typed_duty(signer.public().0, 2);
		not_due.phase = CheckpointDutyPhase::NotDue;
		variants.push(project(not_due, signer.public().0));

		let mut weak_quorum = typed_duty(signer.public().0, 3);
		weak_quorum.required_replica_confirmations = 1;
		variants.push(project(weak_quorum, signer.public().0));
		let mut missing_primary = typed_duty(signer.public().0, 7);
		missing_primary.required_primary_confirmations = 0;
		variants.push(project(missing_primary, signer.public().0));

		let mut gap = typed_duty(signer.public().0, 4);
		gap.previous_commitment =
			Some(CommitmentInfo { mmr_root: H256::repeat_byte(20), start_seq: 0, leaf_count: 1 });
		gap.expected_next_start_seq = 2;
		variants.push(project(gap, signer.public().0));

		let mut changed_version = project(typed_duty(signer.public().0, 5), signer.public().0);
		changed_version.service_key_version += 1;
		variants.push(changed_version);
		let mut denied = project(typed_duty(signer.public().0, 8), signer.public().0);
		denied.may_initiate = false;
		variants.push(denied);
		let mut fingerprint = project(typed_duty(signer.public().0, 9), signer.public().0);
		fingerprint.duty_fingerprint = format!("0x{}", "aa".repeat(32));
		variants.push(fingerprint);
		let mut wrong_nonce = typed_duty(signer.public().0, 10);
		wrong_nonce.expected_nonce = 99;
		variants.push(project(wrong_nonce, signer.public().0));
		let mut cannot_sign = typed_duty(signer.public().0, 11);
		cannot_sign.authorities[0].may_sign = false;
		variants.push(project(cannot_sign, signer.public().0));
		let mut ineligible = typed_duty(signer.public().0, 12);
		ineligible.authorities[0].eligible = false;
		variants.push(project(ineligible, signer.public().0));
		let mut promotion = typed_duty(signer.public().0, 13);
		promotion.phase = CheckpointDutyPhase::ReplicaFallbackPromotion;
		promotion.mode = CheckpointDutyMode::PromotionPending;
		variants.push(project(promotion, signer.public().0));

		for duty in variants {
			assert!(store.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).is_err());
			assert!(store.pending_checkpoint_proposals().unwrap().is_empty());
		}
		let duty = project(typed_duty(signer.public().0, 6), signer.public().0);
		let mut wrong_snapshot = watermark();
		wrong_snapshot.snapshot_checkpoint = 99;
		assert!(store.prepare_exact(&duty, &wrong_snapshot, &mmr, &streaming, &signer).is_err());
		let wrong_signer = ed25519::Pair::from_seed(&[6; 32]);
		assert!(store
			.prepare_exact(&duty, &watermark(), &mmr, &streaming, &wrong_signer)
			.is_err());
		assert!(store.pending_checkpoint_proposals().unwrap().is_empty());
		assert!(store.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).is_err());
		assert!(store.pending_checkpoint_proposals().unwrap().is_empty());
	}

	#[test]
	fn refuses_corrupt_verified_bytes_without_writing_a_proposal() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		install(&streaming, &mmr, 1, b"first");
		let object = fs::read_dir(temp.path().join("streaming-v1/objects"))
			.unwrap()
			.next()
			.unwrap()
			.unwrap()
			.path();
		fs::write(object, b"wrong").unwrap();
		let signer = ed25519::Pair::from_seed(&[7; 32]);
		let duty = project(typed_duty(signer.public().0, 5), signer.public().0);
		let store = CheckpointProposalStore::open(temp.path()).unwrap();
		assert!(store.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).is_err());
		assert!(store.pending_checkpoint_proposals().unwrap().is_empty());
	}

	#[test]
	fn reopening_rejects_field_payload_signature_filename_and_size_corruption() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		install(&streaming, &mmr, 1, b"first");
		let signer = ed25519::Pair::from_seed(&[7; 32]);
		let duty = project(typed_duty(signer.public().0, 5), signer.public().0);
		let store = CheckpointProposalStore::open(temp.path()).unwrap();
		store.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).unwrap();
		drop(store);
		let root = temp.path().join(ROOT);
		let path = fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
		let proposal: PreparedCheckpointProposalV2 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		let mut field = proposal.clone();
		field.finalized_number += 1;
		let mut payload = proposal.clone();
		payload.payload_scale.push_str("00");
		let mut signature = proposal.clone();
		signature.signature.replace_range(0..2, "aa");
		signature.record_hash = proposal_record_hash(&signature).unwrap();
		payload.record_hash = proposal_record_hash(&payload).unwrap();
		let mut bucket = proposal.clone();
		bucket.bucket_id = "aa".repeat(32);
		bucket.record_hash = proposal_record_hash(&bucket).unwrap();
		let mut nonce = proposal.clone();
		nonce.nonce += 1;
		nonce.record_hash = proposal_record_hash(&nonce).unwrap();
		let mut start = proposal.clone();
		start.start_seq += 1;
		start.record_hash = proposal_record_hash(&start).unwrap();
		let mut snapshot = proposal.clone();
		snapshot.snapshot_checkpoint += 1;
		snapshot.record_hash = proposal_record_hash(&snapshot).unwrap();
		let mut duty_scale = proposal.clone();
		duty_scale.duty_scale.push_str("00");
		duty_scale.record_hash = proposal_record_hash(&duty_scale).unwrap();
		let mut primary_provider = proposal.clone();
		primary_provider.primary_provider = "aa".repeat(32);
		primary_provider.record_hash = proposal_record_hash(&primary_provider).unwrap();
		let mut window_start = proposal.clone();
		window_start.window_start += 1;
		window_start.record_hash = proposal_record_hash(&window_start).unwrap();
		let mut window_end = proposal.clone();
		window_end.window_end += 1;
		window_end.record_hash = proposal_record_hash(&window_end).unwrap();
		let mut context_scale = proposal.clone();
		context_scale.context_scale.push_str("00");
		context_scale.record_hash = proposal_record_hash(&context_scale).unwrap();
		let mut context_digest = proposal.clone();
		context_digest.context_digest = "aa".repeat(32);
		context_digest.record_hash = proposal_record_hash(&context_digest).unwrap();
		let mut context_signature = proposal.clone();
		context_signature.context_signature.replace_range(0..2, "aa");
		context_signature.record_hash = proposal_record_hash(&context_signature).unwrap();
		for (record, wrong_filename) in [
			(field, false),
			(payload, false),
			(signature, false),
			(bucket, false),
			(nonce, false),
			(start, false),
			(snapshot, false),
			(duty_scale, false),
			(primary_provider, false),
			(window_start, false),
			(window_end, false),
			(context_scale, false),
			(context_digest, false),
			(context_signature, false),
			(proposal.clone(), true),
		] {
			let case = TempDir::new().unwrap();
			let case_root = case.path().join(ROOT);
			fs::create_dir_all(&case_root).unwrap();
			let key = if wrong_filename {
				"ff".repeat(32)
			} else {
				tuple_key(&record.bucket_id, record.nonce, record.start_seq).unwrap()
			};
			fs::write(case_root.join(format!("{key}.json")), serde_json::to_vec(&record).unwrap())
				.unwrap();
			assert!(CheckpointProposalStore::open(case.path()).is_err());
		}
		let oversized = TempDir::new().unwrap();
		let oversized_root = oversized.path().join(ROOT);
		fs::create_dir_all(&oversized_root).unwrap();
		fs::write(
			oversized_root.join(format!(
				"{}.json",
				tuple_key(&proposal.bucket_id, proposal.nonce, proposal.start_seq).unwrap()
			)),
			vec![b'0'; MAX_PROPOSAL_BYTES + 1],
		)
		.unwrap();
		assert!(CheckpointProposalStore::open(oversized.path()).is_err());
	}

	#[test]
	fn bounded_proposal_table_refuses_new_work_without_an_extra_record() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		install(&streaming, &mmr, 1, b"first");
		let signer = ed25519::Pair::from_seed(&[7; 32]);
		let duty = project(typed_duty(signer.public().0, 5), signer.public().0);
		let store = CheckpointProposalStore::open(temp.path()).unwrap();
		let proposal = store.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).unwrap();
		let mut index = 0u64;
		let mut state = store.state.write().unwrap();
		while state.by_tuple.len() < MAX_PROPOSALS {
			state
				.by_tuple
				.entry(format!("{index:064x}"))
				.or_insert_with(|| proposal.clone());
			index += 1;
		}
		drop(state);
		let mut next = typed_duty(signer.public().0, 6);
		next.expected_nonce = 8;
		let next = project(next, signer.public().0);
		assert_eq!(
			store.prepare_exact(&next, &watermark(), &mmr, &streaming, &signer),
			Err(ContentError::IdempotencyConflict)
		);
		assert_eq!(fs::read_dir(temp.path().join(ROOT)).unwrap().count(), 1);
	}

	#[test]
	fn crash_points_recover_only_an_immutable_old_or_new_record() {
		for (fault, persisted) in [
			(ProposalFault::BeforeTempFsync, false),
			(ProposalFault::AfterTempFsync, false),
			(ProposalFault::AfterRename, true),
			(ProposalFault::AfterDirectoryFsync, true),
		] {
			let temp = TempDir::new().unwrap();
			let streaming = StreamingStore::open(temp.path()).unwrap();
			let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
			install(&streaming, &mmr, 1, b"first");
			let signer = ed25519::Pair::from_seed(&[7; 32]);
			let duty = project(typed_duty(signer.public().0, 5), signer.public().0);
			let store = CheckpointProposalStore::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(store.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).is_err());
			assert!(store.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).is_err());
			let reopened = CheckpointProposalStore::open(temp.path()).unwrap();
			assert_eq!(reopened.state.read().unwrap().by_tuple.len(), usize::from(persisted));
			assert!(reopened.prepare_exact(&duty, &watermark(), &mmr, &streaming, &signer).is_ok());
		}
	}
}
