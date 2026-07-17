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

//! Private authenticated replica confirmation kernel.

use std::{
	collections::HashMap,
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
};

use codec::{Decode, Encode};
use orbis_storage_runtime_api::{
	CheckpointDutyInfo, CheckpointDutyPhase, ProviderDutyRole, RESPONSE_VERSION,
};
use pallet_orbis_storage_provider::{CheckpointContextV1, CommitmentPayloadV2, ReplicaSignature};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
use sp_crypto_hashing::blake2_256;

use super::{ServiceKeySigner, CONTEXT_DOMAIN, DOMAIN};
use crate::{
	chain::validate_checkpoint_duty, storage::bucket_mmr::BucketMmrStore, BucketId, ContentError,
	DiskStore, StreamingStore,
};

const ROOT: &str = "checkpoint-confirmations-v1";
const VERSION: u8 = 1;
const AUTH_DOMAIN: &[u8] = b"cord/provider/checkpoint-confirmation-request/v1";
const RECORD_DOMAIN: &[u8] = b"cord/provider/checkpoint-confirmation-record/v1";
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024;
const MAX_RECORD_BYTES: usize = 160 * 1024;
const MAX_CONFIRMATIONS: usize = 8_192;
// Atomic replacement creates at most one process-specific temporary artifact.
const MAX_TEMP_ARTIFACTS: usize = 1;

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode)]
pub(crate) struct ReplicaConfirmationRequestV1 {
	pub version: u8,
	pub proposal_record_hash: [u8; 32],
	pub target_provider: AccountId32,
	pub target_service_key_version: u64,
	pub target_service_key: ed25519::Public,
	pub primary_provider: AccountId32,
	pub duty_id: H256,
	pub payload: CommitmentPayloadV2<H256, u32>,
	pub context: CheckpointContextV1<H256>,
	pub primary_service_key: ed25519::Public,
	pub primary_signature: ed25519::Signature,
	pub primary_context_signature: ed25519::Signature,
	pub auth_signature: ed25519::Signature,
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode)]
struct UnsignedReplicaConfirmationRequestV1 {
	version: u8,
	proposal_record_hash: [u8; 32],
	target_provider: AccountId32,
	target_service_key_version: u64,
	target_service_key: ed25519::Public,
	primary_provider: AccountId32,
	duty_id: H256,
	payload: CommitmentPayloadV2<H256, u32>,
	context: CheckpointContextV1<H256>,
	primary_service_key: ed25519::Public,
	primary_signature: ed25519::Signature,
	primary_context_signature: ed25519::Signature,
}

impl ReplicaConfirmationRequestV1 {
	fn unsigned(&self) -> UnsignedReplicaConfirmationRequestV1 {
		UnsignedReplicaConfirmationRequestV1 {
			version: self.version,
			proposal_record_hash: self.proposal_record_hash,
			target_provider: self.target_provider.clone(),
			target_service_key_version: self.target_service_key_version,
			target_service_key: self.target_service_key,
			primary_provider: self.primary_provider.clone(),
			duty_id: self.duty_id,
			payload: self.payload,
			context: self.context,
			primary_service_key: self.primary_service_key,
			primary_signature: self.primary_signature,
			primary_context_signature: self.primary_context_signature,
		}
	}

	pub(crate) fn auth_message(&self) -> Vec<u8> {
		let mut message = AUTH_DOMAIN.to_vec();
		self.unsigned().encode_to(&mut message);
		message
	}

	pub(crate) fn decode_canonical(bytes: &[u8]) -> Result<Self, ContentError> {
		if bytes.len() > MAX_REQUEST_BYTES {
			return Err(ContentError::IntegrityFailed);
		}
		let mut input = bytes;
		let request = Self::decode(&mut input).map_err(|_| ContentError::IntegrityFailed)?;
		if !input.is_empty() || request.encode() != bytes || request.version != VERSION {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(request)
	}
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode)]
pub(crate) struct ReplicaConfirmationResponseV1 {
	pub version: u8,
	pub proposal_record_hash: [u8; 32],
	pub target_provider: AccountId32,
	pub duty_id: H256,
	pub confirmation: ReplicaSignature<AccountId32>,
}

impl ReplicaConfirmationResponseV1 {
	pub(crate) fn decode_canonical(bytes: &[u8]) -> Result<Self, ContentError> {
		if bytes.len() > MAX_RESPONSE_BYTES {
			return Err(ContentError::IntegrityFailed);
		}
		let mut input = bytes;
		let response = Self::decode(&mut input).map_err(|_| ContentError::IntegrityFailed)?;
		if !input.is_empty() || response.encode() != bytes || response.version != VERSION {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(response)
	}
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmationRecordV1 {
	version: u8,
	duty_scale: String,
	duty_fingerprint: String,
	snapshot_checkpoint: u32,
	snapshot_hash: String,
	request: String,
	request_hash: String,
	response: String,
	response_hash: String,
	record_hash: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfirmationFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
}

pub(crate) struct ReplicaConfirmationStore {
	root: PathBuf,
	records: RwLock<HashMap<String, ConfirmationRecordV1>>,
	fault: RwLock<Option<ConfirmationFault>>,
	poisoned: RwLock<bool>,
}

pub(crate) struct PreparedReplicaConfirmationStore {
	root: PathBuf,
	root_missing: bool,
	records: HashMap<String, ConfirmationRecordV1>,
	temp_artifacts: Vec<PathBuf>,
}

impl PreparedReplicaConfirmationStore {
	pub(crate) fn apply(self) -> Result<ReplicaConfirmationStore, ContentError> {
		crate::bounded_io::create_prepared_directory(&self.root, self.root_missing)?;
		crate::bounded_io::remove_validated_temp_artifacts(&self.root, &self.temp_artifacts)?;
		Ok(ReplicaConfirmationStore {
			root: self.root,
			records: RwLock::new(self.records),
			fault: RwLock::new(None),
			poisoned: RwLock::new(false),
		})
	}
}

impl ReplicaConfirmationStore {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		Self::prepare_open(root)?.apply()
	}

	pub(crate) fn prepare_open(
		root: impl AsRef<Path>,
	) -> Result<PreparedReplicaConfirmationStore, ContentError> {
		let root = root.as_ref().join(ROOT);
		let root_missing = !crate::bounded_io::optional_directory_exists(&root)?;
		let mut records = HashMap::new();
		let mut visited = 0usize;
		let mut temp_artifacts = Vec::new();
		let entries =
			if root_missing { None } else { Some(fs::read_dir(&root).map_err(io_error)?) };
		for entry in entries.into_iter().flatten() {
			visited = visited.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			if visited > MAX_CONFIRMATIONS + MAX_TEMP_ARTIFACTS {
				return Err(ContentError::IntegrityFailed);
			}
			let entry = entry.map_err(io_error)?;
			let name = entry.file_name().to_string_lossy().into_owned();
			if crate::bounded_io::is_json_temp_artifact(&name) {
				if temp_artifacts.len() >= MAX_TEMP_ARTIFACTS
					|| !entry.file_type().map_err(io_error)?.is_file()
				{
					return Err(ContentError::IntegrityFailed);
				}
				temp_artifacts.push(entry.path());
				continue;
			}
			if !name.ends_with(".json") || !entry.file_type().map_err(io_error)?.is_file() {
				return Err(ContentError::IntegrityFailed);
			}
			let bytes =
				crate::bounded_io::read_regular_file(entry.path(), MAX_RECORD_BYTES as u64)?;
			if records.len() >= MAX_CONFIRMATIONS {
				return Err(ContentError::IntegrityFailed);
			}
			let record: ConfirmationRecordV1 =
				serde_json::from_slice(&bytes).map_err(|_| ContentError::IntegrityFailed)?;
			let key = validate_record(&record)?;
			if name != format!("{key}.json") || records.insert(key, record).is_some() {
				return Err(ContentError::IntegrityFailed);
			}
		}
		Ok(PreparedReplicaConfirmationStore { root, root_missing, records, temp_artifacts })
	}

	pub(crate) fn inject_fault_once(&self, fault: ConfirmationFault) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	pub(crate) fn confirm(
		&self,
		disk: &DiskStore,
		mmr: &BucketMmrStore,
		streaming: &StreamingStore,
		signer: &dyn ServiceKeySigner,
		request_bytes: &[u8],
	) -> Result<Vec<u8>, ContentError> {
		let request = ReplicaConfirmationRequestV1::decode_canonical(request_bytes)?;
		validate_request_signatures(&request)?;
		let key = confirmation_key(&request);
		{
			let poisoned = *self.poisoned.read().map_err(|_| lock_error())?;
			if poisoned {
				return Err(ContentError::IntegrityFailed);
			}
			let records = self.records.read().map_err(|_| lock_error())?;
			if let Some(record) = records.get(&key) {
				if record.request != hex::encode(request_bytes) {
					return Err(ContentError::IdempotencyConflict);
				}
				return hex::decode(&record.response).map_err(|_| ContentError::IntegrityFailed);
			}
		}

		let frozen = validate_fresh_request(&request, disk, mmr, streaming, signer)?;
		let digest = checkpoint_digest(&request.payload);
		let context_digest = checkpoint_context_digest(&request.context);
		let confirmation = ReplicaSignature {
			provider: request.target_provider.clone(),
			service_key: ed25519::Public::from_raw(signer.public_key()),
			signature: ed25519::Signature::from_raw(signer.sign_digest(digest)),
			context_signature: ed25519::Signature::from_raw(signer.sign_digest(context_digest)),
		};
		let response = ReplicaConfirmationResponseV1 {
			version: VERSION,
			proposal_record_hash: request.proposal_record_hash,
			target_provider: request.target_provider.clone(),
			duty_id: request.duty_id,
			confirmation,
		};
		let response_bytes = response.encode();
		let mut records = self.records.write().map_err(|_| lock_error())?;
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed);
		}
		if let Some(record) = records.get(&key) {
			return if record.request == hex::encode(request_bytes) {
				hex::decode(&record.response).map_err(|_| ContentError::IntegrityFailed)
			} else {
				Err(ContentError::IdempotencyConflict)
			};
		}
		if records.len() >= MAX_CONFIRMATIONS {
			return Err(ContentError::ProviderRecoveryTableFull);
		}
		let mut record = ConfirmationRecordV1 {
			version: VERSION,
			duty_scale: hex::encode(&frozen.encoded),
			duty_fingerprint: hex::encode(blake2_256(&frozen.encoded)),
			snapshot_checkpoint: frozen.decoded.snapshot_checkpoint,
			snapshot_hash: hex::encode(frozen.decoded.snapshot_hash.as_bytes()),
			request: hex::encode(request_bytes),
			request_hash: hex::encode(blake2_256(request_bytes)),
			response: hex::encode(&response_bytes),
			response_hash: hex::encode(blake2_256(&response_bytes)),
			record_hash: String::new(),
		};
		record.record_hash = record_hash(&record)?;
		if let Err(error) = self.persist(&key, &record) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error);
		}
		records.insert(key, record);
		Ok(response_bytes)
	}

	fn persist(&self, key: &str, record: &ConfirmationRecordV1) -> Result<(), ContentError> {
		let validated = validate_record(record)?;
		if validated != key {
			return Err(ContentError::IntegrityFailed);
		}
		let bytes = serde_json::to_vec(record).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed);
		}
		let path = self.root.join(format!("{key}.json"));
		if path.exists() {
			let existing = crate::bounded_io::read_regular_file(&path, MAX_RECORD_BYTES as u64)?;
			return if existing == bytes { Ok(()) } else { Err(ContentError::IdempotencyConflict) };
		}
		let temp = self.root.join(format!("{key}.json.tmp-{}", std::process::id()));
		let mut file = File::create(&temp).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		self.trip(ConfirmationFault::BeforeTempFsync)?;
		file.sync_all().map_err(io_error)?;
		self.trip(ConfirmationFault::AfterTempFsync)?;
		fs::rename(temp, path).map_err(io_error)?;
		self.trip(ConfirmationFault::AfterRename)?;
		File::open(&self.root)
			.and_then(|directory| directory.sync_all())
			.map_err(io_error)?;
		self.trip(ConfirmationFault::AfterDirectoryFsync)
	}

	fn trip(&self, point: ConfirmationFault) -> Result<(), ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			Err(ContentError::Io(format!("injected confirmation fault: {point:?}")))
		} else {
			Ok(())
		}
	}
}

struct FrozenDuty {
	encoded: Vec<u8>,
	decoded: CheckpointDutyInfo<AccountId32, H256, u32>,
}

fn validate_fresh_request(
	request: &ReplicaConfirmationRequestV1,
	disk: &DiskStore,
	mmr: &BucketMmrStore,
	streaming: &StreamingStore,
	signer: &dyn ServiceKeySigner,
) -> Result<FrozenDuty, ContentError> {
	let duty = disk
		.pending_checkpoint_duties()
		.map_err(io_error)?
		.into_iter()
		.find(|duty| normalize_hash(&duty.duty_id).ok() == Some(hex::encode(request.duty_id)))
		.ok_or(ContentError::NotFound)?;
	let watermark = disk
		.checkpoint_duty_watermark()
		.map_err(io_error)?
		.ok_or(ContentError::IntegrityFailed)?;
	let encoded = hex::decode(duty.encoded_duty.trim_start_matches("0x"))
		.map_err(|_| ContentError::IntegrityFailed)?;
	if hex::encode(blake2_256(&encoded)) != normalize_hash(&duty.duty_fingerprint)? {
		return Err(ContentError::IntegrityFailed);
	}
	let decoded = validate_frozen_duty(request, &encoded)?;
	let projected = validate_checkpoint_duty(
		decoded.clone(),
		&request.target_provider,
		signer.public_key(),
		duty.snapshot_checkpoint,
	)
	.map_err(|_| ContentError::IntegrityFailed)?;
	if projected != duty
		|| watermark.snapshot_checkpoint != duty.snapshot_checkpoint
		|| !duty.may_sign
		|| duty.service_key_version == 0
		|| duty.service_key.trim_start_matches("0x") != hex::encode(signer.public_key())
		|| decoded.duty_id != request.duty_id
		|| decoded.initiator.as_ref() != Some(&request.primary_provider)
		|| decoded.due_at > decoded.grace_until
		|| !matches!(
			decoded.phase,
			CheckpointDutyPhase::Primary | CheckpointDutyPhase::ReplicaFallback
		) {
		return Err(ContentError::IntegrityFailed);
	}
	let local = decoded
		.authorities
		.iter()
		.find(|authority| authority.provider == request.target_provider)
		.ok_or(ContentError::IntegrityFailed)?;
	if local.active_service_key_version != duty.service_key_version
		|| local.active_service_key_version != request.target_service_key_version
		|| local.active_service_key != signer.public_key()
		|| local.active_service_key != request.target_service_key.0
		|| !local.may_sign
		|| !local.eligible
		|| !local.organization_sla_eligible
		|| local.overdue_challenge
		|| local.exclusion.is_some()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let primary = decoded
		.authorities
		.iter()
		.find(|authority| authority.provider == request.primary_provider)
		.ok_or(ContentError::IntegrityFailed)?;
	if primary.active_service_key != request.primary_service_key.0
		|| !primary.may_sign
		|| !primary.may_initiate
		|| !primary.eligible
		|| !primary.organization_sla_eligible
		|| primary.overdue_challenge
		|| primary.exclusion.is_some()
		|| primary.initiation_exclusion.is_some()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let expected_start = match decoded.previous_commitment.as_ref() {
		Some(previous) => previous
			.start_seq
			.checked_add(previous.leaf_count)
			.ok_or(ContentError::IntegrityFailed)?,
		None => 0,
	};
	if request.payload.version != 2
		|| request.payload.bucket_id != decoded.bucket_id
		|| request.payload.nonce != decoded.expected_nonce
		|| request.payload.commitment.start_seq != expected_start
		|| request.payload.commitment.range_end().is_none()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let candidate = mmr.commitment_candidate(
		streaming,
		BucketId::from_bytes(decoded.bucket_id.0),
		expected_start,
	)?;
	if request.payload.commitment != candidate {
		return Err(ContentError::IntegrityFailed);
	}
	let expected_context = CheckpointContextV1 {
		version: 1,
		genesis_hash: decoded.commons_genesis_hash,
		spec_version: decoded.commons_spec_version,
		transaction_version: decoded.commons_transaction_version,
		metadata_hash: decoded.commons_metadata_hash,
		finalized_hash: decoded.snapshot_hash,
		duty_id: decoded.duty_id,
		v2_digest: checkpoint_digest(&request.payload),
	};
	if request.context != expected_context
		|| hex::encode(decoded.snapshot_hash.as_bytes()) != normalize_hash(&duty.snapshot_hash)?
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(FrozenDuty { encoded, decoded })
}

fn validate_request_signatures(request: &ReplicaConfirmationRequestV1) -> Result<(), ContentError> {
	let digest = checkpoint_digest(&request.payload);
	let context_digest = checkpoint_context_digest(&request.context);
	if request.context.v2_digest != digest
		|| !ed25519::Pair::verify(&request.primary_signature, &digest, &request.primary_service_key)
		|| !ed25519::Pair::verify(
			&request.primary_context_signature,
			&context_digest,
			&request.primary_service_key,
		) || !ed25519::Pair::verify(
		&request.auth_signature,
		&request.auth_message(),
		&request.primary_service_key,
	) {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn validate_frozen_duty(
	request: &ReplicaConfirmationRequestV1,
	encoded: &[u8],
) -> Result<CheckpointDutyInfo<AccountId32, H256, u32>, ContentError> {
	let mut input = encoded;
	let duty = CheckpointDutyInfo::<AccountId32, H256, u32>::decode(&mut input)
		.map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty()
		|| duty.encode() != encoded
		|| duty.response_version != RESPONSE_VERSION
		|| duty.duty_id != request.duty_id
		|| duty.initiator.as_ref() != Some(&request.primary_provider)
		|| duty.due_at > duty.grace_until
		|| !matches!(
			duty.phase,
			CheckpointDutyPhase::Primary | CheckpointDutyPhase::ReplicaFallback
		) {
		return Err(ContentError::IntegrityFailed);
	}
	validate_authority_projection(&duty)?;
	let initiator = duty.initiator.as_ref().ok_or(ContentError::IntegrityFailed)?;
	if !valid_confirmation_target(&duty, &request.target_provider, initiator) {
		return Err(ContentError::IntegrityFailed);
	}
	let local = duty
		.authorities
		.iter()
		.find(|authority| authority.provider == request.target_provider)
		.ok_or(ContentError::IntegrityFailed)?;
	if local.active_service_key_version != request.target_service_key_version
		|| local.active_service_key != request.target_service_key.0
		|| !local.may_sign
		|| !local.eligible
		|| !local.organization_sla_eligible
		|| local.overdue_challenge
		|| local.exclusion.is_some()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let primary = duty
		.authorities
		.iter()
		.find(|authority| authority.provider == request.primary_provider)
		.ok_or(ContentError::IntegrityFailed)?;
	if primary.active_service_key != request.primary_service_key.0
		|| !primary.may_sign
		|| !primary.may_initiate
		|| !primary.eligible
		|| !primary.organization_sla_eligible
		|| primary.overdue_challenge
		|| primary.exclusion.is_some()
		|| primary.initiation_exclusion.is_some()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let expected_start = match duty.previous_commitment.as_ref() {
		Some(previous) => previous
			.start_seq
			.checked_add(previous.leaf_count)
			.ok_or(ContentError::IntegrityFailed)?,
		None => 0,
	};
	if duty.expected_nonce != duty.snapshot_checkpoint
		|| duty.expected_next_start_seq != expected_start
		|| duty.required_primary_confirmations != 1
		|| duty.required_replica_confirmations != 2
		|| request.payload.version != 2
		|| request.payload.bucket_id != duty.bucket_id
		|| request.payload.nonce != duty.expected_nonce
		|| request.payload.commitment.leaf_count == 0
		|| request.payload.commitment.start_seq != expected_start
		|| request.payload.commitment.range_end().is_none()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let expected_context = CheckpointContextV1 {
		version: 1,
		genesis_hash: duty.commons_genesis_hash,
		spec_version: duty.commons_spec_version,
		transaction_version: duty.commons_transaction_version,
		metadata_hash: duty.commons_metadata_hash,
		finalized_hash: duty.snapshot_hash,
		duty_id: duty.duty_id,
		v2_digest: checkpoint_digest(&request.payload),
	};
	if request.context != expected_context {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(duty)
}

fn validate_authority_projection(
	duty: &CheckpointDutyInfo<AccountId32, H256, u32>,
) -> Result<(), ContentError> {
	if duty.replicas.iter().any(|replica| replica == &duty.primary)
		|| duty
			.replicas
			.iter()
			.enumerate()
			.any(|(index, replica)| duty.replicas[..index].contains(replica))
		|| duty.authorities.len() != duty.replicas.len().saturating_add(1)
	{
		return Err(ContentError::IntegrityFailed);
	}
	let primary = duty.authorities.first().ok_or(ContentError::IntegrityFailed)?;
	if primary.provider != duty.primary
		|| primary.role != ProviderDutyRole::Primary
		|| primary.order != 0
	{
		return Err(ContentError::IntegrityFailed);
	}
	for (index, (replica, authority)) in
		duty.replicas.iter().zip(duty.authorities.iter().skip(1)).enumerate()
	{
		let order =
			u8::try_from(index.saturating_add(1)).map_err(|_| ContentError::IntegrityFailed)?;
		if authority.provider != *replica
			|| authority.role != ProviderDutyRole::Replica
			|| authority.order != order
		{
			return Err(ContentError::IntegrityFailed);
		}
	}
	Ok(())
}

fn valid_confirmation_target(
	duty: &CheckpointDutyInfo<AccountId32, H256, u32>,
	target: &AccountId32,
	initiator: &AccountId32,
) -> bool {
	if target == initiator {
		return false;
	}
	match duty.phase {
		CheckpointDutyPhase::Primary => {
			initiator == &duty.primary && duty.replicas.contains(target)
		},
		CheckpointDutyPhase::ReplicaFallback => {
			duty.replicas.contains(initiator)
				&& (target == &duty.primary
					|| duty
						.replicas
						.iter()
						.any(|replica| replica == target && replica != initiator))
		},
		_ => false,
	}
}

fn validate_record(record: &ConfirmationRecordV1) -> Result<String, ContentError> {
	if record.version != VERSION || record.record_hash != record_hash(record)? {
		return Err(ContentError::IntegrityFailed);
	}
	let duty_bytes = hex::decode(&record.duty_scale).map_err(|_| ContentError::IntegrityFailed)?;
	if duty_bytes.is_empty()
		|| hex::encode(&duty_bytes) != record.duty_scale
		|| record.duty_fingerprint != hex::encode(blake2_256(&duty_bytes))
	{
		return Err(ContentError::IntegrityFailed);
	}
	let request_bytes = hex::decode(&record.request).map_err(|_| ContentError::IntegrityFailed)?;
	let response_bytes =
		hex::decode(&record.response).map_err(|_| ContentError::IntegrityFailed)?;
	if hex::encode(&request_bytes) != record.request
		|| hex::encode(&response_bytes) != record.response
		|| record.request_hash != hex::encode(blake2_256(&request_bytes))
		|| record.response_hash != hex::encode(blake2_256(&response_bytes))
	{
		return Err(ContentError::IntegrityFailed);
	}
	let request = ReplicaConfirmationRequestV1::decode_canonical(&request_bytes)?;
	validate_request_signatures(&request)?;
	let duty = validate_frozen_duty(&request, &duty_bytes)?;
	if duty.snapshot_checkpoint != record.snapshot_checkpoint
		|| record.snapshot_hash != hex::encode(duty.snapshot_hash.as_bytes())
	{
		return Err(ContentError::IntegrityFailed);
	}
	let response = ReplicaConfirmationResponseV1::decode_canonical(&response_bytes)?;
	if response.proposal_record_hash != request.proposal_record_hash
		|| response.target_provider != request.target_provider
		|| response.duty_id != request.duty_id
		|| response.confirmation.provider != request.target_provider
		|| response.confirmation.service_key != request.target_service_key
		|| !ed25519::Pair::verify(
			&response.confirmation.signature,
			&checkpoint_digest(&request.payload),
			&response.confirmation.service_key,
		) || !ed25519::Pair::verify(
		&response.confirmation.context_signature,
		&checkpoint_context_digest(&request.context),
		&response.confirmation.service_key,
	) {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(confirmation_key(&request))
}

pub(crate) fn checkpoint_digest(payload: &CommitmentPayloadV2<H256, u32>) -> [u8; 32] {
	let mut message = DOMAIN.to_vec();
	payload.encode_to(&mut message);
	blake2_256(&message)
}

pub(crate) fn checkpoint_context_digest(context: &CheckpointContextV1<H256>) -> [u8; 32] {
	let mut message = CONTEXT_DOMAIN.to_vec();
	context.encode_to(&mut message);
	blake2_256(&message)
}

fn confirmation_key(request: &ReplicaConfirmationRequestV1) -> String {
	let mut input = b"cord/provider/checkpoint-confirmation-key/v1".to_vec();
	input.extend_from_slice(request.duty_id.as_bytes());
	input.extend_from_slice(<AccountId32 as AsRef<[u8]>>::as_ref(&request.target_provider));
	hex::encode(blake2_256(&input))
}

fn record_hash(record: &ConfirmationRecordV1) -> Result<String, ContentError> {
	let mut canonical = record.clone();
	canonical.record_hash.clear();
	let mut input = RECORD_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn normalize_hash(value: &str) -> Result<String, ContentError> {
	let value = value.trim_start_matches("0x");
	if value.len() != 64 || value.bytes().any(|byte| byte.is_ascii_uppercase()) {
		return Err(ContentError::IntegrityFailed);
	}
	hex::decode(value).map_err(|_| ContentError::IntegrityFailed)?;
	Ok(value.into())
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

fn lock_error() -> ContentError {
	ContentError::Io("checkpoint confirmation lock poisoned".into())
}

#[cfg(test)]
mod tests {
	use super::*;
	use orbis_storage_runtime_api::{
		CheckpointDutyMode, CommitmentInfo, ProviderDutyAuthority, ProviderDutyExclusion,
	};
	use tempfile::TempDir;

	use crate::{CheckpointDutyBatch, NodeProfile, OperationId, StreamingDescriptor};

	struct Fixture {
		_temp: TempDir,
		streaming: StreamingStore,
		mmr: BucketMmrStore,
		disk: DiskStore,
		store: ReplicaConfirmationStore,
		local: ed25519::Pair,
		primary: ed25519::Pair,
		other: ed25519::Pair,
		request: ReplicaConfirmationRequestV1,
	}

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

	fn resign(request: &mut ReplicaConfirmationRequestV1, signer: &ed25519::Pair) {
		request.primary_service_key = signer.public();
		request.primary_signature = signer.sign(&checkpoint_digest(&request.payload));
		request.primary_context_signature =
			signer.sign(&checkpoint_context_digest(&request.context));
		request.auth_signature = ed25519::Signature::from_raw([0; 64]);
		request.auth_signature = signer.sign(&request.auth_message());
	}

	fn fixture(fallback: bool, authority_fault: u8) -> Fixture {
		fixture_variant(fallback, authority_fault, 0, 0)
	}

	fn fixture_variant(
		fallback: bool,
		authority_fault: u8,
		target_kind: u8,
		duty_fault: u8,
	) -> Fixture {
		let temp = TempDir::new().unwrap();
		let replica = ed25519::Pair::from_seed(&[8; 32]);
		let primary = ed25519::Pair::from_seed(&[7; 32]);
		let other = ed25519::Pair::from_seed(&[9; 32]);
		let old_primary = AccountId32::new([1; 32]);
		let configured_replica = AccountId32::new([2; 32]);
		let promoted = AccountId32::new([3; 32]);
		let mut authorities = vec![
			authority(
				old_primary.clone(),
				ProviderDutyRole::Primary,
				0,
				primary.public().0,
				!fallback,
			),
			authority(
				configured_replica.clone(),
				ProviderDutyRole::Replica,
				1,
				replica.public().0,
				false,
			),
			authority(promoted.clone(), ProviderDutyRole::Replica, 2, other.public().0, fallback),
		];
		let target = match target_kind {
			1 => old_primary.clone(),
			2 => promoted.clone(),
			_ => configured_replica.clone(),
		};
		let local = match target_kind {
			1 => ed25519::Pair::from_seed(&[7; 32]),
			2 => ed25519::Pair::from_seed(&[9; 32]),
			_ => ed25519::Pair::from_seed(&[8; 32]),
		};
		let target_index = match target_kind {
			1 => 0,
			2 => 2,
			_ => 1,
		};
		match authority_fault {
			1 => authorities[target_index].eligible = false,
			2 => authorities[target_index].overdue_challenge = true,
			3 => authorities[target_index].exclusion = Some(ProviderDutyExclusion::Inactive),
			4 => {
				authorities[target_index].initiation_exclusion =
					Some(ProviderDutyExclusion::ReplicaCheckpointMissingOrStale)
			},
			_ => {},
		}
		let initiator = if fallback { promoted.clone() } else { old_primary.clone() };
		let mut typed = CheckpointDutyInfo {
			response_version: RESPONSE_VERSION,
			commons_genesis_hash: H256::repeat_byte(10),
			commons_spec_version: 1,
			commons_transaction_version: 1,
			commons_metadata_hash: H256::repeat_byte(11),
			duty_id: H256::repeat_byte(5),
			bucket_id: H256::repeat_byte(4),
			primary: old_primary,
			replicas: vec![configured_replica, promoted],
			authorities,
			initiator: Some(initiator.clone()),
			phase: if fallback {
				CheckpointDutyPhase::ReplicaFallback
			} else {
				CheckpointDutyPhase::Primary
			},
			mode: CheckpointDutyMode::Standard,
			snapshot_checkpoint: 100,
			snapshot_hash: H256::repeat_byte(12),
			due_at: 100,
			grace_until: 110,
			expected_nonce: 100,
			scheduled_at: 99,
			previous_commitment: None::<CommitmentInfo<H256>>,
			previous_checkpoint: None,
			expected_next_start_seq: 0,
			required_primary_confirmations: 1,
			required_replica_confirmations: 2,
		};
		match duty_fault {
			1 => typed.expected_nonce = 99,
			2 => typed.expected_next_start_seq = 1,
			3 => typed.required_primary_confirmations = 0,
			4 => typed.required_replica_confirmations = 1,
			5 => {
				let duplicate = typed.replicas[1].clone();
				typed.replicas.push(duplicate.clone());
				typed.authorities.push(authority(
					duplicate,
					ProviderDutyRole::Replica,
					3,
					other.public().0,
					false,
				));
			},
			_ => {},
		}
		let duty = validate_checkpoint_duty(typed.clone(), &target, local.public().0, 100).unwrap();
		let disk = DiskStore::open(
			temp.path().join("disk"),
			NodeProfile {
				provider: duty.provider.clone(),
				endpoint: "http://127.0.0.1:8080".into(),
				service_key: duty.service_key.clone(),
				region: None,
			},
			1_000_000,
		)
		.unwrap();
		disk.stage_checkpoint_duty_page(CheckpointDutyBatch {
			finalized_hash: format!("0x{}", "0d".repeat(32)),
			finalized_number: 105,
			provider: duty.provider.clone(),
			snapshot_checkpoint: 100,
			requested_cursor: None,
			next_cursor: None,
			duties: vec![duty],
		})
		.unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		install(&streaming, &mmr, 1, b"first");
		let payload = CommitmentPayloadV2 {
			version: 2,
			bucket_id: typed.bucket_id,
			commitment: mmr
				.commitment_candidate(&streaming, BucketId::from_bytes([4; 32]), 0)
				.unwrap(),
			nonce: typed.expected_nonce,
		};
		let context = CheckpointContextV1 {
			version: 1,
			genesis_hash: typed.commons_genesis_hash,
			spec_version: typed.commons_spec_version,
			transaction_version: typed.commons_transaction_version,
			metadata_hash: typed.commons_metadata_hash,
			finalized_hash: typed.snapshot_hash,
			duty_id: typed.duty_id,
			v2_digest: checkpoint_digest(&payload),
		};
		let signing_pair = if fallback { &other } else { &primary };
		let mut request = ReplicaConfirmationRequestV1 {
			version: VERSION,
			proposal_record_hash: [55; 32],
			target_provider: target,
			target_service_key_version: 5,
			target_service_key: local.public(),
			primary_provider: initiator,
			duty_id: typed.duty_id,
			payload,
			context,
			primary_service_key: signing_pair.public(),
			primary_signature: ed25519::Signature::from_raw([0; 64]),
			primary_context_signature: ed25519::Signature::from_raw([0; 64]),
			auth_signature: ed25519::Signature::from_raw([0; 64]),
		};
		resign(&mut request, signing_pair);
		let store = ReplicaConfirmationStore::open(temp.path()).unwrap();
		Fixture { _temp: temp, streaming, mmr, disk, store, local, primary, other, request }
	}

	fn install(streaming: &StreamingStore, mmr: &BucketMmrStore, operation: u8, bytes: &[u8]) {
		let bucket = BucketId::from_bytes([4; 32]);
		let operation = OperationId::from_bytes([operation; 16]);
		let cid = crate::CanonicalCid::from_digest(blake2_256(bytes));
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: operation,
					bucket_id: bucket,
					expected_cid: cid.as_str().into(),
					object_len: bytes.len() as u64,
				},
				[bytes.to_vec()],
			)
			.unwrap();
		mmr.append_verified(streaming, bucket, operation).unwrap();
	}

	#[test]
	fn primary_and_fallback_confirmations_are_exact_and_dual_signed() {
		for fallback in [false, true] {
			let fixture = fixture(fallback, 0);
			let request = fixture.request.encode();
			let response = fixture
				.store
				.confirm(&fixture.disk, &fixture.mmr, &fixture.streaming, &fixture.local, &request)
				.unwrap();
			let decoded = ReplicaConfirmationResponseV1::decode_canonical(&response).unwrap();
			assert_eq!(decoded.target_provider, fixture.request.target_provider);
			assert_eq!(decoded.confirmation.provider, fixture.request.target_provider);
			assert_eq!(decoded.confirmation.service_key, fixture.local.public());
			assert!(ed25519::Pair::verify(
				&decoded.confirmation.signature,
				&checkpoint_digest(&fixture.request.payload),
				&fixture.local.public(),
			));
			assert!(ed25519::Pair::verify(
				&decoded.confirmation.context_signature,
				&checkpoint_context_digest(&fixture.request.context),
				&fixture.local.public(),
			));
		}
	}

	#[test]
	fn fallback_predecessor_confirms_but_promoted_initiator_cannot_confirm_itself() {
		let predecessor = fixture_variant(true, 0, 1, 0);
		assert!(predecessor
			.store
			.confirm(
				&predecessor.disk,
				&predecessor.mmr,
				&predecessor.streaming,
				&predecessor.local,
				&predecessor.request.encode(),
			)
			.is_ok());

		let promoted = fixture_variant(true, 0, 2, 0);
		assert_eq!(
			promoted.store.confirm(
				&promoted.disk,
				&promoted.mmr,
				&promoted.streaming,
				&promoted.local,
				&promoted.request.encode(),
			),
			Err(ContentError::IntegrityFailed)
		);
	}

	#[test]
	fn frozen_duty_requires_nonce_sequence_quorum_and_unique_authorities() {
		for duty_fault in 1..=5 {
			let fixture = fixture_variant(false, 0, 0, duty_fault);
			assert!(
				fixture
					.store
					.confirm(
						&fixture.disk,
						&fixture.mmr,
						&fixture.streaming,
						&fixture.local,
						&fixture.request.encode(),
					)
					.is_err(),
				"duty fault {duty_fault}"
			);
		}

		let fixture = fixture(false, 0);
		let duty = fixture.disk.pending_checkpoint_duties().unwrap().pop().unwrap();
		let mut encoded = &hex::decode(duty.encoded_duty.trim_start_matches("0x")).unwrap()[..];
		let mut decoded =
			CheckpointDutyInfo::<AccountId32, H256, u32>::decode(&mut encoded).unwrap();
		decoded.initiator = Some(decoded.replicas[0].clone());
		decoded.authorities[1].may_initiate = true;
		assert!(!valid_confirmation_target(
			&decoded,
			&decoded.replicas[1],
			decoded.initiator.as_ref().unwrap(),
		));
	}

	#[test]
	fn reopen_rejects_rehashed_semantically_invalid_frozen_duties() {
		for duty_fault in 1..=5 {
			let fixture = fixture(false, 0);
			fixture
				.store
				.confirm(
					&fixture.disk,
					&fixture.mmr,
					&fixture.streaming,
					&fixture.local,
					&fixture.request.encode(),
				)
				.unwrap();
			drop(fixture.store);
			let path = fs::read_dir(fixture._temp.path().join(ROOT))
				.unwrap()
				.next()
				.unwrap()
				.unwrap()
				.path();
			let mut record: ConfirmationRecordV1 =
				serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
			let bytes = hex::decode(&record.duty_scale).unwrap();
			let mut input = &bytes[..];
			let mut duty =
				CheckpointDutyInfo::<AccountId32, H256, u32>::decode(&mut input).unwrap();
			match duty_fault {
				1 => duty.expected_nonce = duty.snapshot_checkpoint.saturating_sub(1),
				2 => duty.expected_next_start_seq = 1,
				3 => duty.required_primary_confirmations = 0,
				4 => duty.required_replica_confirmations = 1,
				5 => {
					let duplicate = duty.replicas[1].clone();
					let mut duplicate_authority = duty.authorities[2].clone();
					duplicate_authority.order = 3;
					duty.replicas.push(duplicate);
					duty.authorities.push(duplicate_authority);
				},
				_ => unreachable!(),
			}
			let changed = duty.encode();
			record.duty_scale = hex::encode(&changed);
			record.duty_fingerprint = hex::encode(blake2_256(&changed));
			record.record_hash = record_hash(&record).unwrap();
			fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
			assert!(
				ReplicaConfirmationStore::open(fixture._temp.path()).is_err(),
				"reopen duty fault {duty_fault}"
			);
		}
	}

	#[test]
	fn replica_may_sign_when_only_failover_initiation_is_excluded() {
		let fixture = fixture(false, 4);
		assert!(fixture
			.store
			.confirm(
				&fixture.disk,
				&fixture.mmr,
				&fixture.streaming,
				&fixture.local,
				&fixture.request.encode(),
			)
			.is_ok());
	}

	#[test]
	fn exact_retry_survives_head_and_key_advancement_but_changed_reuse_fails() {
		let fixture = fixture(false, 0);
		let bytes = fixture.request.encode();
		let first = fixture
			.store
			.confirm(&fixture.disk, &fixture.mmr, &fixture.streaming, &fixture.local, &bytes)
			.unwrap();
		install(&fixture.streaming, &fixture.mmr, 2, b"second");
		let rotated = ed25519::Pair::from_seed(&[44; 32]);
		assert_eq!(
			fixture
				.store
				.confirm(&fixture.disk, &fixture.mmr, &fixture.streaming, &rotated, &bytes)
				.unwrap(),
			first
		);
		let mut changed = fixture.request.clone();
		changed.payload.commitment.mmr_root = H256::repeat_byte(99);
		changed.context.v2_digest = checkpoint_digest(&changed.payload);
		resign(&mut changed, &fixture.primary);
		assert_eq!(
			fixture.store.confirm(
				&fixture.disk,
				&fixture.mmr,
				&fixture.streaming,
				&fixture.local,
				&changed.encode(),
			),
			Err(ContentError::IdempotencyConflict)
		);
	}

	#[test]
	fn fresh_confirmation_rejects_audience_authority_context_range_and_bytes_failures() {
		let canonical = fixture(false, 0).request.encode();
		let mut trailing = canonical.clone();
		trailing.push(0);
		assert!(ReplicaConfirmationRequestV1::decode_canonical(&trailing).is_err());
		assert!(ReplicaConfirmationRequestV1::decode_canonical(&vec![0; MAX_REQUEST_BYTES + 1])
			.is_err());
		for case in 0..16 {
			let fault = match case {
				8 => 1,
				9 => 2,
				10 => 3,
				_ => 0,
			};
			let fixture = fixture(false, fault);
			let mut request = fixture.request.clone();
			let mut local = &fixture.local;
			let rotated;
			match case {
				0 => request.target_provider = AccountId32::new([3; 32]),
				1 => request.primary_provider = AccountId32::new([3; 32]),
				2 => {
					rotated = ed25519::Pair::from_seed(&[44; 32]);
					local = &rotated;
				},
				3 => request.primary_signature = ed25519::Signature::from_raw([1; 64]),
				4 => request.context.metadata_hash = H256::repeat_byte(99),
				5 => request.payload.commitment.mmr_root = H256::repeat_byte(99),
				6 => request.payload.commitment.start_seq = 1,
				7 => request.payload.commitment.leaf_count = u64::MAX,
				11 => {
					let object = fs::read_dir(fixture._temp.path().join("streaming-v1/objects"))
						.unwrap()
						.next()
						.unwrap()
						.unwrap()
						.path();
					fs::write(object, b"corrupt").unwrap();
				},
				12 => {
					let object = fs::read_dir(fixture._temp.path().join("streaming-v1/objects"))
						.unwrap()
						.next()
						.unwrap()
						.unwrap()
						.path();
					fs::remove_file(object).unwrap();
				},
				13 => request.target_service_key_version += 1,
				14 => request.target_service_key = ed25519::Pair::from_seed(&[66; 32]).public(),
				15 => request.payload.commitment.leaf_count = 0,
				_ => {},
			}
			if !matches!(case, 2 | 3 | 8 | 9 | 10 | 11 | 12) {
				request.context.v2_digest = checkpoint_digest(&request.payload);
				resign(&mut request, &fixture.primary);
			}
			assert!(
				fixture
					.store
					.confirm(
						&fixture.disk,
						&fixture.mmr,
						&fixture.streaming,
						local,
						&request.encode(),
					)
					.is_err(),
				"case {case}"
			);
		}
	}

	#[test]
	fn record_tamper_capacity_and_crash_seams_fail_closed() {
		let tamper_fixture = fixture(false, 0);
		let bytes = tamper_fixture.request.encode();
		let response = tamper_fixture
			.store
			.confirm(
				&tamper_fixture.disk,
				&tamper_fixture.mmr,
				&tamper_fixture.streaming,
				&tamper_fixture.local,
				&bytes,
			)
			.unwrap();
		drop(tamper_fixture.store);
		let root = tamper_fixture._temp.path().join(ROOT);
		let path = fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
		let mut record: ConfirmationRecordV1 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		record.response_hash = "aa".repeat(32);
		record.record_hash = record_hash(&record).unwrap();
		fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(ReplicaConfirmationStore::open(tamper_fixture._temp.path()).is_err());
		assert!(!response.is_empty());

		let key_tamper = fixture(false, 0);
		key_tamper
			.store
			.confirm(
				&key_tamper.disk,
				&key_tamper.mmr,
				&key_tamper.streaming,
				&key_tamper.local,
				&key_tamper.request.encode(),
			)
			.unwrap();
		drop(key_tamper.store);
		let path = fs::read_dir(key_tamper._temp.path().join(ROOT))
			.unwrap()
			.next()
			.unwrap()
			.unwrap()
			.path();
		let mut record: ConfirmationRecordV1 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		let mut decoded = ReplicaConfirmationResponseV1::decode_canonical(
			&hex::decode(&record.response).unwrap(),
		)
		.unwrap();
		let attacker = ed25519::Pair::from_seed(&[66; 32]);
		decoded.confirmation.service_key = attacker.public();
		decoded.confirmation.signature =
			attacker.sign(&checkpoint_digest(&key_tamper.request.payload));
		decoded.confirmation.context_signature =
			attacker.sign(&checkpoint_context_digest(&key_tamper.request.context));
		let changed_response = decoded.encode();
		record.response = hex::encode(&changed_response);
		record.response_hash = hex::encode(blake2_256(&changed_response));
		record.record_hash = record_hash(&record).unwrap();
		fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(ReplicaConfirmationStore::open(key_tamper._temp.path()).is_err());

		for case in 0..4 {
			let frozen_tamper = fixture(false, 0);
			frozen_tamper
				.store
				.confirm(
					&frozen_tamper.disk,
					&frozen_tamper.mmr,
					&frozen_tamper.streaming,
					&frozen_tamper.local,
					&frozen_tamper.request.encode(),
				)
				.unwrap();
			drop(frozen_tamper.store);
			let path = fs::read_dir(frozen_tamper._temp.path().join(ROOT))
				.unwrap()
				.next()
				.unwrap()
				.unwrap()
				.path();
			let mut record: ConfirmationRecordV1 =
				serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
			match case {
				0 => {
					record.duty_scale.push_str("00");
					record.duty_fingerprint =
						hex::encode(blake2_256(&hex::decode(&record.duty_scale).unwrap()));
				},
				1 => record.duty_fingerprint = "aa".repeat(32),
				2 => record.snapshot_checkpoint += 1,
				3 => record.snapshot_hash = "aa".repeat(32),
				_ => unreachable!(),
			}
			record.record_hash = record_hash(&record).unwrap();
			fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
			assert!(
				ReplicaConfirmationStore::open(frozen_tamper._temp.path()).is_err(),
				"frozen case {case}"
			);
		}

		let capacity = fixture(false, 0);
		let request = capacity.request.encode();
		let first = capacity
			.store
			.confirm(&capacity.disk, &capacity.mmr, &capacity.streaming, &capacity.local, &request)
			.unwrap();
		let mut changed_proposal = capacity.request.clone();
		changed_proposal.proposal_record_hash = [77; 32];
		resign(&mut changed_proposal, &capacity.primary);
		assert_eq!(
			capacity.store.confirm(
				&capacity.disk,
				&capacity.mmr,
				&capacity.streaming,
				&capacity.local,
				&changed_proposal.encode(),
			),
			Err(ContentError::IdempotencyConflict)
		);
		assert_eq!(capacity.store.records.read().unwrap().len(), 1);
		let existing = capacity.store.records.read().unwrap().values().next().unwrap().clone();
		let mut records = capacity.store.records.write().unwrap();
		let mut index = 0u64;
		while records.len() < MAX_CONFIRMATIONS {
			records.entry(format!("{index:064x}")).or_insert_with(|| existing.clone());
			index += 1;
		}
		drop(records);
		assert_eq!(
			capacity
				.store
				.confirm(
					&capacity.disk,
					&capacity.mmr,
					&capacity.streaming,
					&capacity.local,
					&request
				)
				.unwrap(),
			first
		);
		assert_eq!(
			capacity.store.confirm(
				&capacity.disk,
				&capacity.mmr,
				&capacity.streaming,
				&capacity.local,
				&changed_proposal.encode(),
			),
			Err(ContentError::IdempotencyConflict)
		);

		for (fault, persisted) in [
			(ConfirmationFault::BeforeTempFsync, false),
			(ConfirmationFault::AfterTempFsync, false),
			(ConfirmationFault::AfterRename, true),
			(ConfirmationFault::AfterDirectoryFsync, true),
		] {
			let fixture = fixture(false, 0);
			let bytes = fixture.request.encode();
			fixture.store.inject_fault_once(fault).unwrap();
			assert!(fixture
				.store
				.confirm(&fixture.disk, &fixture.mmr, &fixture.streaming, &fixture.local, &bytes)
				.is_err());
			assert!(fixture
				.store
				.confirm(&fixture.disk, &fixture.mmr, &fixture.streaming, &fixture.local, &bytes)
				.is_err());
			drop(fixture.store);
			let reopened = ReplicaConfirmationStore::open(fixture._temp.path()).unwrap();
			assert_eq!(reopened.records.read().unwrap().len(), usize::from(persisted));
			assert!(reopened
				.confirm(&fixture.disk, &fixture.mmr, &fixture.streaming, &fixture.local, &bytes)
				.is_ok());
		}
	}

	#[test]
	fn recovery_rejects_temp_artifact_flood() {
		let temp = TempDir::new().unwrap();
		let root = temp.path().join(ROOT);
		fs::create_dir_all(&root).unwrap();
		fs::write(root.join("first.json.tmp-1"), b"partial").unwrap();
		fs::write(root.join("second.json.tmp-1"), b"partial").unwrap();
		assert!(matches!(
			ReplicaConfirmationStore::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
		assert!(root.join("first.json.tmp-1").exists());
		assert!(root.join("second.json.tmp-1").exists());
	}
}
