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

//! Private durable queue for deterministic checkpoint fallback promotions.

use std::{
	collections::HashMap,
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
};

use codec::{Decode, Encode};
use orbis_storage_runtime_api::{
	CheckpointDutyInfo, CheckpointDutyMode, CheckpointDutyPhase, ProviderDutyAuthority,
	ProviderDutyRole, RESPONSE_VERSION,
};
use pallet_orbis_storage_provider::CheckpointFallbackPromotionV1;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
use sp_crypto_hashing::blake2_256;

use super::ServiceKeySigner;
use crate::ContentError;

const ROOT: &str = "checkpoint-promotions-v1";
const VERSION: u8 = 1;
const STATE: &str = "PromotionQueued";
const PROMOTION_DOMAIN: &[u8] = b"cord/storage/checkpoint-promotion/v1";
const ID_DOMAIN: &[u8] = b"cord/provider/checkpoint-promotion-queue/v1";
const TUPLE_DOMAIN: &[u8] = b"cord/provider/checkpoint-promotion-tuple/v1";
const RECORD_DOMAIN: &[u8] = b"cord/provider/checkpoint-promotion-record/v1";
const MAX_DUTY_BYTES: usize = 64 * 1024;
const MAX_RECORD_BYTES: usize = 192 * 1024;
const MAX_RECORDS: usize = 8_192;
const MAX_TEMP_ARTIFACTS: usize = 1;

type Duty = CheckpointDutyInfo<AccountId32, H256, u32>;
type Payload = CheckpointFallbackPromotionV1<H256, u32>;
type CallArgs = (Payload, ed25519::Public, ed25519::Signature);
type Authority = ProviderDutyAuthority<AccountId32, H256, u32>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QueuedFallbackPromotionV1 {
	pub version: u8,
	pub queue_id: String,
	pub tuple_key: String,
	pub provider: String,
	pub duty_scale: String,
	pub duty_fingerprint: String,
	pub snapshot_checkpoint: u32,
	pub bucket_id: String,
	pub duty_id: String,
	pub service_key_version: u64,
	pub payload_scale: String,
	pub service_key: String,
	pub signature: String,
	pub call_args_scale: String,
	pub state: String,
	pub record_hash: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckpointPromotionFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
}

pub(crate) struct CheckpointPromotionStoreV1 {
	root: PathBuf,
	records: RwLock<HashMap<String, QueuedFallbackPromotionV1>>,
	by_tuple: RwLock<HashMap<String, String>>,
	fault: RwLock<Option<CheckpointPromotionFault>>,
	poisoned: RwLock<bool>,
}

impl CheckpointPromotionStoreV1 {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let root = root.as_ref().join(ROOT);
		fs::create_dir_all(&root).map_err(io_error)?;
		let mut records = HashMap::new();
		let mut by_tuple = HashMap::new();
		for item in read_records(&root)? {
			let record: QueuedFallbackPromotionV1 =
				serde_json::from_slice(&item.bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_record(&record)?;
			if item.name != format!("{}.json", record.queue_id) ||
				records.insert(record.queue_id.clone(), record.clone()).is_some() ||
				by_tuple.insert(record.tuple_key.clone(), record.queue_id.clone()).is_some()
			{
				return Err(ContentError::IntegrityFailed)
			}
		}
		Ok(Self {
			root,
			records: RwLock::new(records),
			by_tuple: RwLock::new(by_tuple),
			fault: RwLock::new(None),
			poisoned: RwLock::new(false),
		})
	}

	pub(crate) fn inject_fault_once(
		&self,
		fault: CheckpointPromotionFault,
	) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	pub(crate) fn queue(
		&self,
		provider: &AccountId32,
		duty_scale: &[u8],
		signer: &dyn ServiceKeySigner,
	) -> Result<QueuedFallbackPromotionV1, ContentError> {
		let candidate = queue_record(provider, duty_scale, signer)?;
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		{
			let records = self.records.read().map_err(|_| lock_error())?;
			let by_tuple = self.by_tuple.read().map_err(|_| lock_error())?;
			if let Some(existing_id) = by_tuple.get(&candidate.tuple_key) {
				let existing = records.get(existing_id).ok_or(ContentError::IntegrityFailed)?;
				return if existing == &candidate {
					Ok(existing.clone())
				} else {
					Err(ContentError::IdempotencyConflict)
				}
			}
		}
		let mut records = self.records.write().map_err(|_| lock_error())?;
		let mut by_tuple = self.by_tuple.write().map_err(|_| lock_error())?;
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		if let Some(existing_id) = by_tuple.get(&candidate.tuple_key) {
			let existing = records.get(existing_id).ok_or(ContentError::IntegrityFailed)?;
			return if existing == &candidate {
				Ok(existing.clone())
			} else {
				Err(ContentError::IdempotencyConflict)
			}
		}
		if records.len() >= MAX_RECORDS {
			return Err(ContentError::ProviderRecoveryTableFull)
		}
		if let Err(error) = self.persist(&candidate) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error)
		}
		by_tuple.insert(candidate.tuple_key.clone(), candidate.queue_id.clone());
		records.insert(candidate.queue_id.clone(), candidate.clone());
		Ok(candidate)
	}

	fn persist(&self, record: &QueuedFallbackPromotionV1) -> Result<(), ContentError> {
		validate_record(record)?;
		let bytes = serde_json::to_vec(record).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed)
		}
		let key = &record.queue_id;
		let temp = self.root.join(format!("{key}.json.tmp-{}", std::process::id()));
		let mut file = File::create(&temp).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		self.trip(CheckpointPromotionFault::BeforeTempFsync)?;
		file.sync_all().map_err(io_error)?;
		self.trip(CheckpointPromotionFault::AfterTempFsync)?;
		fs::rename(&temp, self.root.join(format!("{key}.json"))).map_err(io_error)?;
		self.trip(CheckpointPromotionFault::AfterRename)?;
		File::open(&self.root)
			.and_then(|directory| directory.sync_all())
			.map_err(io_error)?;
		self.trip(CheckpointPromotionFault::AfterDirectoryFsync)
	}

	fn trip(&self, point: CheckpointPromotionFault) -> Result<(), ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			Err(ContentError::Io(format!("injected checkpoint promotion fault: {point:?}")))
		} else {
			Ok(())
		}
	}
}

struct RecordFile {
	name: String,
	bytes: Vec<u8>,
}

fn read_records(root: &Path) -> Result<Vec<RecordFile>, ContentError> {
	let mut records = Vec::new();
	let mut visited = 0usize;
	let mut temp_artifacts = 0usize;
	for item in fs::read_dir(root).map_err(io_error)? {
		visited = visited.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		if visited > MAX_RECORDS + MAX_TEMP_ARTIFACTS {
			return Err(ContentError::IntegrityFailed)
		}
		let item = item.map_err(io_error)?;
		let name = item.file_name().to_string_lossy().into_owned();
		if name.contains(".tmp-") {
			temp_artifacts = temp_artifacts.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			if temp_artifacts > MAX_TEMP_ARTIFACTS {
				return Err(ContentError::IntegrityFailed)
			}
			fs::remove_file(item.path()).map_err(io_error)?;
			continue
		}
		if records.len() >= MAX_RECORDS ||
			!name.ends_with(".json") ||
			!item.file_type().map_err(io_error)?.is_file()
		{
			return Err(ContentError::IntegrityFailed)
		}
		let bytes = fs::read(item.path()).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed)
		}
		records.push(RecordFile { name, bytes });
	}
	Ok(records)
}

fn queue_record(
	provider: &AccountId32,
	duty_scale: &[u8],
	signer: &dyn ServiceKeySigner,
) -> Result<QueuedFallbackPromotionV1, ContentError> {
	if duty_scale.len() > MAX_DUTY_BYTES {
		return Err(ContentError::IntegrityFailed)
	}
	let duty = decode_scale::<Duty>(duty_scale)?;
	let service_key = ed25519::Public::from_raw(signer.public_key());
	let local = validate_duty(&duty, provider, &service_key)?;
	let payload = Payload {
		version: VERSION,
		bucket_id: duty.bucket_id,
		snapshot_nonce: duty.snapshot_checkpoint,
		duty_id: duty.duty_id,
	};
	let digest = promotion_digest(&payload);
	let signature = ed25519::Signature::from_raw(signer.sign_digest(digest));
	if !ed25519::Pair::verify(&signature, &digest, &service_key) {
		return Err(ContentError::IntegrityFailed)
	}
	let args = (payload, service_key, signature);
	let mut record = QueuedFallbackPromotionV1 {
		version: VERSION,
		queue_id: queue_id(provider, &args),
		tuple_key: tuple_key(&payload),
		provider: account_hex(provider),
		duty_scale: hex::encode(duty_scale),
		duty_fingerprint: hex::encode(blake2_256(duty_scale)),
		snapshot_checkpoint: duty.snapshot_checkpoint,
		bucket_id: hex::encode(duty.bucket_id.as_bytes()),
		duty_id: hex::encode(duty.duty_id.as_bytes()),
		service_key_version: local.active_service_key_version,
		payload_scale: hex::encode(payload.encode()),
		service_key: hex::encode(service_key.0),
		signature: hex::encode(signature.0),
		call_args_scale: hex::encode(args.encode()),
		state: STATE.into(),
		record_hash: String::new(),
	};
	record.record_hash = record_hash(&record)?;
	validate_record(&record)?;
	Ok(record)
}

fn validate_record(record: &QueuedFallbackPromotionV1) -> Result<(), ContentError> {
	if record.version != VERSION ||
		record.state != STATE ||
		record.queue_id.len() != 64 ||
		record.tuple_key.len() != 64 ||
		record.record_hash != record_hash(record)?
	{
		return Err(ContentError::IntegrityFailed)
	}
	let provider = decode_account(&record.provider)?;
	let duty_bytes = decode_hex(&record.duty_scale)?;
	if duty_bytes.len() > MAX_DUTY_BYTES ||
		record.duty_fingerprint != hex::encode(blake2_256(&duty_bytes))
	{
		return Err(ContentError::IntegrityFailed)
	}
	let duty = decode_scale::<Duty>(&duty_bytes)?;
	let service_key = ed25519::Public::from_raw(decode_fixed(&record.service_key)?);
	let local = validate_duty(&duty, &provider, &service_key)?;
	let payload = decode_hex_scale::<Payload>(&record.payload_scale)?;
	let expected_payload = Payload {
		version: VERSION,
		bucket_id: duty.bucket_id,
		snapshot_nonce: duty.snapshot_checkpoint,
		duty_id: duty.duty_id,
	};
	let signature = ed25519::Signature::from_raw(decode_fixed(&record.signature)?);
	let args = (payload, service_key, signature);
	if payload != expected_payload ||
		record.snapshot_checkpoint != duty.snapshot_checkpoint ||
		record.bucket_id != hex::encode(duty.bucket_id.as_bytes()) ||
		record.duty_id != hex::encode(duty.duty_id.as_bytes()) ||
		record.service_key_version != local.active_service_key_version ||
		record.call_args_scale != hex::encode(args.encode()) ||
		record.queue_id != queue_id(&provider, &args) ||
		record.tuple_key != tuple_key(&payload) ||
		!ed25519::Pair::verify(&signature, &promotion_digest(&payload), &service_key)
	{
		return Err(ContentError::IntegrityFailed)
	}
	Ok(())
}

fn validate_duty<'a>(
	duty: &'a Duty,
	provider: &AccountId32,
	service_key: &ed25519::Public,
) -> Result<&'a Authority, ContentError> {
	if duty.response_version != RESPONSE_VERSION ||
		duty.phase != CheckpointDutyPhase::ReplicaFallbackPromotion ||
		duty.mode != CheckpointDutyMode::Standard ||
		duty.expected_nonce != duty.snapshot_checkpoint ||
		duty.due_at > duty.grace_until ||
		duty.snapshot_checkpoint < duty.grace_until ||
		duty.initiator.as_ref() != Some(provider) ||
		!duty.replicas.contains(provider) ||
		!(2..=4).contains(&duty.replicas.len()) ||
		duty.required_primary_confirmations != 1 ||
		duty.required_replica_confirmations != 2
	{
		return Err(ContentError::IntegrityFailed)
	}
	let expected_len = duty.replicas.len().checked_add(1).ok_or(ContentError::IntegrityFailed)?;
	if duty.authorities.len() != expected_len {
		return Err(ContentError::IntegrityFailed)
	}
	let mut seen = Vec::<Vec<u8>>::with_capacity(expected_len);
	for (index, authority) in duty.authorities.iter().enumerate() {
		let (expected_provider, expected_role) = if index == 0 {
			(&duty.primary, ProviderDutyRole::Primary)
		} else {
			(&duty.replicas[index - 1], ProviderDutyRole::Replica)
		};
		let expected_order = u8::try_from(index).map_err(|_| ContentError::IntegrityFailed)?;
		let encoded = authority.provider.encode();
		if authority.provider != *expected_provider ||
			authority.role != expected_role ||
			authority.order != expected_order ||
			seen.iter().any(|item| item == &encoded)
		{
			return Err(ContentError::IntegrityFailed)
		}
		seen.push(encoded);
	}
	let mut candidates: Vec<_> = duty
		.authorities
		.iter()
		.filter(|authority| authority.role == ProviderDutyRole::Replica)
		.filter(|authority| selection_eligible(authority))
		.map(|authority| (authority.confirmed_checkpoint, authority.provider.encode(), authority))
		.collect();
	candidates.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
	let selected = candidates
		.first()
		.map(|candidate| candidate.2)
		.ok_or(ContentError::IntegrityFailed)?;
	let nonselected_initiator = duty
		.authorities
		.iter()
		.any(|authority| authority.provider != selected.provider && authority.may_initiate);
	let eligible_others = duty
		.authorities
		.iter()
		.filter(|authority| authority.eligible && authority.provider != selected.provider)
		.count();
	if &selected.provider != provider ||
		!promotion_eligible(selected) ||
		nonselected_initiator ||
		eligible_others >= 2 ||
		selected.active_service_key != service_key.0
	{
		return Err(ContentError::IntegrityFailed)
	}
	Ok(selected)
}

fn promotion_eligible(authority: &Authority) -> bool {
	selection_eligible(authority) && authority.may_sign && authority.may_initiate
}

fn selection_eligible(authority: &Authority) -> bool {
	authority.eligible &&
		authority.organization_sla_eligible &&
		!authority.overdue_challenge &&
		authority.exclusion.is_none() &&
		authority.initiation_exclusion.is_none() &&
		authority.active_service_key_version > 0 &&
		authority.active_service_key != [0; 32]
}

fn promotion_digest(payload: &Payload) -> [u8; 32] {
	let mut input = PROMOTION_DOMAIN.to_vec();
	payload.encode_to(&mut input);
	blake2_256(&input)
}

fn queue_id(provider: &AccountId32, args: &CallArgs) -> String {
	let mut input = ID_DOMAIN.to_vec();
	provider.encode_to(&mut input);
	args.encode_to(&mut input);
	hex::encode(blake2_256(&input))
}

fn tuple_key(payload: &Payload) -> String {
	let mut input = TUPLE_DOMAIN.to_vec();
	payload.bucket_id.encode_to(&mut input);
	payload.snapshot_nonce.encode_to(&mut input);
	payload.duty_id.encode_to(&mut input);
	hex::encode(blake2_256(&input))
}

fn record_hash(record: &QueuedFallbackPromotionV1) -> Result<String, ContentError> {
	let mut canonical = record.clone();
	canonical.record_hash.clear();
	let mut input = RECORD_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn decode_scale<T: Decode + Encode>(bytes: &[u8]) -> Result<T, ContentError> {
	let mut input = bytes;
	let decoded = T::decode(&mut input).map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || decoded.encode() != bytes {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(decoded)
}

fn decode_hex_scale<T: Decode + Encode>(value: &str) -> Result<T, ContentError> {
	decode_scale(&decode_hex(value)?)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ContentError> {
	if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
		return Err(ContentError::IntegrityFailed)
	}
	hex::decode(value).map_err(|_| ContentError::IntegrityFailed)
}

fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N], ContentError> {
	decode_hex(value)?.try_into().map_err(|_| ContentError::IntegrityFailed)
}

fn decode_account(value: &str) -> Result<AccountId32, ContentError> {
	Ok(AccountId32::new(decode_fixed(value)?))
}

fn account_hex(account: &AccountId32) -> String {
	hex::encode(<AccountId32 as AsRef<[u8]>>::as_ref(account))
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

fn lock_error() -> ContentError {
	ContentError::Io("checkpoint promotion lock poisoned".into())
}

#[cfg(test)]
mod tests {
	use orbis_storage_runtime_api::{
		CommitmentInfo, ProviderDutyAuthority, ProviderDutyExclusion, ProviderDutyRole,
	};
	use sp_core::Pair as _;
	use tempfile::TempDir;

	use super::*;

	fn pair(id: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[id.saturating_add(10); 32])
	}

	fn account(id: u8) -> AccountId32 {
		AccountId32::new([id; 32])
	}

	fn authority(
		id: u8,
		role: ProviderDutyRole,
		order: u8,
		eligible: bool,
		may_initiate: bool,
		confirmed_checkpoint: Option<u32>,
	) -> ProviderDutyAuthority<AccountId32, H256, u32> {
		ProviderDutyAuthority {
			provider: account(id),
			role,
			order,
			active_service_key_version: 5,
			active_service_key: pair(id).public().0,
			endpoint_hash: H256::repeat_byte(id),
			organization_sla_eligible: eligible,
			overdue_challenge: false,
			eligible,
			may_sign: eligible,
			may_initiate,
			exclusion: (!eligible).then_some(ProviderDutyExclusion::Inactive),
			initiation_exclusion: None,
			confirmed_checkpoint,
		}
	}

	fn duty() -> Duty {
		Duty {
			response_version: RESPONSE_VERSION,
			commons_genesis_hash: H256::repeat_byte(10),
			commons_spec_version: 11,
			commons_transaction_version: 12,
			commons_metadata_hash: H256::repeat_byte(13),
			duty_id: H256::repeat_byte(14),
			bucket_id: H256::repeat_byte(4),
			primary: account(1),
			replicas: vec![account(2), account(3), account(4)],
			authorities: vec![
				authority(1, ProviderDutyRole::Primary, 0, false, false, None),
				authority(2, ProviderDutyRole::Replica, 1, true, true, Some(100)),
				authority(3, ProviderDutyRole::Replica, 2, true, false, Some(90)),
				authority(4, ProviderDutyRole::Replica, 3, false, false, None),
			],
			initiator: Some(account(2)),
			phase: CheckpointDutyPhase::ReplicaFallbackPromotion,
			mode: CheckpointDutyMode::Standard,
			snapshot_checkpoint: 120,
			snapshot_hash: H256::repeat_byte(15),
			due_at: 100,
			grace_until: 110,
			expected_nonce: 120,
			scheduled_at: 90,
			previous_commitment: Some(CommitmentInfo {
				mmr_root: H256::repeat_byte(5),
				start_seq: 0,
				leaf_count: 5,
			}),
			previous_checkpoint: Some(90),
			expected_next_start_seq: 5,
			required_primary_confirmations: 1,
			required_replica_confirmations: 2,
		}
	}

	fn path(temp: &TempDir, queue_id: &str) -> PathBuf {
		temp.path().join(ROOT).join(format!("{queue_id}.json"))
	}

	#[test]
	fn exact_call_args_are_queued_signed_and_reopen_stably() {
		let temp = TempDir::new().unwrap();
		let duty = duty();
		let duty_scale = duty.encode();
		let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
		let queued = store.queue(&account(2), &duty_scale, &pair(2)).unwrap();
		assert_eq!(queued.state, STATE);
		assert_eq!(decode_hex(&queued.duty_scale).unwrap(), duty_scale);
		let args = decode_hex_scale::<CallArgs>(&queued.call_args_scale).unwrap();
		assert_eq!(args.0.bucket_id, duty.bucket_id);
		assert_eq!(args.0.snapshot_nonce, duty.snapshot_checkpoint);
		assert_eq!(args.0.duty_id, duty.duty_id);
		assert_eq!(args.1, pair(2).public());
		assert!(ed25519::Pair::verify(&args.2, &promotion_digest(&args.0), &args.1));
		assert_eq!(store.queue(&account(2), &duty_scale, &pair(2)).unwrap(), queued);
		drop(store);

		let reopened = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
		assert_eq!(reopened.queue(&account(2), &duty_scale, &pair(2)).unwrap(), queued);
	}

	#[test]
	fn highest_checkpoint_then_scale_smallest_deterministically_wins() {
		let temp = TempDir::new().unwrap();
		let mut tie = duty();
		tie.authorities[2].confirmed_checkpoint = Some(100);
		let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
		store.queue(&account(2), &tie.encode(), &pair(2)).unwrap();

		let temp = TempDir::new().unwrap();
		let mut higher = duty();
		higher.authorities[1].may_initiate = false;
		higher.authorities[2].may_initiate = true;
		higher.authorities[2].confirmed_checkpoint = Some(101);
		higher.initiator = Some(account(3));
		let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
		store.queue(&account(3), &higher.encode(), &pair(3)).unwrap();

		let temp = TempDir::new().unwrap();
		let mut forged_lower = duty();
		forged_lower.authorities[2].confirmed_checkpoint = Some(101);
		let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
		assert!(matches!(
			store.queue(&account(2), &forged_lower.encode(), &pair(2)),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn wrong_phase_mode_time_quorum_version_and_encoding_are_rejected() {
		let base = duty();
		let mut variants = Vec::new();
		for phase in [
			CheckpointDutyPhase::NotDue,
			CheckpointDutyPhase::Primary,
			CheckpointDutyPhase::ReplicaFallback,
			CheckpointDutyPhase::BlockedInsufficientFallbackQuorum,
			CheckpointDutyPhase::Unavailable,
		] {
			let mut changed = base.clone();
			changed.phase = phase;
			variants.push(changed.encode());
		}
		for fault in 0..9 {
			let mut changed = base.clone();
			match fault {
				0 => changed.mode = CheckpointDutyMode::PromotionPending,
				1 => changed.expected_nonce -= 1,
				2 => changed.snapshot_checkpoint = changed.grace_until - 1,
				3 => changed.due_at = changed.grace_until + 1,
				4 => changed.required_primary_confirmations = 0,
				5 => changed.required_replica_confirmations = 1,
				6 => changed.response_version -= 1,
				7 => {
					changed.replicas.pop();
					changed.replicas.pop();
					changed.authorities.pop();
					changed.authorities.pop();
				},
				8 => {
					changed.replicas.extend([account(5), account(6)]);
					changed.authorities.extend([
						authority(5, ProviderDutyRole::Replica, 4, false, false, None),
						authority(6, ProviderDutyRole::Replica, 5, false, false, None),
					]);
				},
				_ => unreachable!(),
			}
			variants.push(changed.encode());
		}
		let mut noncanonical = base.encode();
		noncanonical.push(0);
		variants.push(noncanonical);
		let mut oversized = base.encode();
		oversized.resize(MAX_DUTY_BYTES + 1, 0);
		variants.push(oversized);

		for duty_scale in variants {
			let temp = TempDir::new().unwrap();
			let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
			assert!(matches!(
				store.queue(&account(2), &duty_scale, &pair(2)),
				Err(ContentError::IntegrityFailed)
			));
		}
	}

	#[test]
	fn wrong_self_key_authority_projection_and_eligibility_fail_closed() {
		let base = duty();
		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
		assert!(matches!(
			store.queue(&account(3), &base.encode(), &pair(3)),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			store.queue(&account(2), &base.encode(), &pair(3)),
			Err(ContentError::IntegrityFailed)
		));

		let mut variants = Vec::new();
		let mut projection = base.clone();
		projection.authorities.swap(1, 2);
		variants.push(projection);
		let mut duplicate = base.clone();
		duplicate.replicas[1] = duplicate.replicas[0].clone();
		duplicate.authorities[2].provider = duplicate.replicas[0].clone();
		variants.push(duplicate);
		let mut extra_initiator = base.clone();
		extra_initiator.authorities[2].may_initiate = true;
		variants.push(extra_initiator);
		for fault in 0..8 {
			let mut changed = base.clone();
			let local = &mut changed.authorities[1];
			match fault {
				0 => local.eligible = false,
				1 => local.may_sign = false,
				2 => local.organization_sla_eligible = false,
				3 => local.overdue_challenge = true,
				4 => local.exclusion = Some(ProviderDutyExclusion::Inactive),
				5 =>
					local.initiation_exclusion =
						Some(ProviderDutyExclusion::ReplicaCheckpointMissingOrStale),
				6 => local.active_service_key_version = 0,
				7 => local.active_service_key = [0; 32],
				_ => unreachable!(),
			}
			variants.push(changed);
		}

		for changed in variants {
			let temp = TempDir::new().unwrap();
			let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
			assert!(matches!(
				store.queue(&account(2), &changed.encode(), &pair(2)),
				Err(ContentError::IntegrityFailed)
			));
		}
	}

	#[test]
	fn exact_retry_precedes_capacity_while_changed_duty_conflicts() {
		let temp = TempDir::new().unwrap();
		let base = duty();
		let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
		let queued = store.queue(&account(2), &base.encode(), &pair(2)).unwrap();
		let mut changed = base.clone();
		changed.snapshot_hash = H256::repeat_byte(88);
		assert!(matches!(
			store.queue(&account(2), &changed.encode(), &pair(2)),
			Err(ContentError::IdempotencyConflict)
		));

		{
			let mut records = store.records.write().unwrap();
			for index in records.len()..MAX_RECORDS {
				let mut dummy = queued.clone();
				dummy.queue_id = format!("{index:064x}");
				records.insert(dummy.queue_id.clone(), dummy);
			}
		}
		assert_eq!(store.queue(&account(2), &base.encode(), &pair(2)).unwrap(), queued);
		let mut next = base;
		next.duty_id = H256::repeat_byte(89);
		assert!(matches!(
			store.queue(&account(2), &next.encode(), &pair(2)),
			Err(ContentError::ProviderRecoveryTableFull)
		));
	}

	#[test]
	fn rehashed_semantic_tampering_is_rejected_on_reopen() {
		let temp = TempDir::new().unwrap();
		let base = duty();
		let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
		let queued = store.queue(&account(2), &base.encode(), &pair(2)).unwrap();
		drop(store);
		let reopened = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
		assert_eq!(reopened.queue(&account(2), &base.encode(), &pair(2)).unwrap(), queued);
		drop(reopened);

		let path = path(&temp, &queued.queue_id);
		let mut record: QueuedFallbackPromotionV1 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		let mut changed = decode_hex_scale::<Duty>(&record.duty_scale).unwrap();
		changed.authorities[2].confirmed_checkpoint = Some(101);
		record.duty_scale = hex::encode(changed.encode());
		record.duty_fingerprint = hex::encode(blake2_256(&changed.encode()));
		record.record_hash = record_hash(&record).unwrap();
		fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(matches!(
			CheckpointPromotionStoreV1::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));

		record.duty_scale = hex::encode(base.encode());
		record.duty_fingerprint = hex::encode(blake2_256(&base.encode()));
		record.call_args_scale.push_str("00");
		record.record_hash = record_hash(&record).unwrap();
		fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(matches!(
			CheckpointPromotionStoreV1::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn every_promotion_crash_seam_recovers_exact_old_or_new_state() {
		let faults = [
			CheckpointPromotionFault::BeforeTempFsync,
			CheckpointPromotionFault::AfterTempFsync,
			CheckpointPromotionFault::AfterRename,
			CheckpointPromotionFault::AfterDirectoryFsync,
		];
		for fault in faults {
			let temp = TempDir::new().unwrap();
			let base = duty();
			let store = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(
				store.queue(&account(2), &base.encode(), &pair(2)),
				Err(ContentError::Io(_))
			));
			assert!(matches!(
				store.queue(&account(2), &base.encode(), &pair(2)),
				Err(ContentError::IntegrityFailed)
			));
			drop(store);
			let reopened = CheckpointPromotionStoreV1::open(temp.path()).unwrap();
			reopened.queue(&account(2), &base.encode(), &pair(2)).unwrap();
		}
	}

	#[test]
	fn recovery_directory_and_record_bytes_are_hard_bounded() {
		let temp = TempDir::new().unwrap();
		let root = temp.path().join(ROOT);
		fs::create_dir_all(&root).unwrap();
		for index in 0..=MAX_RECORDS {
			fs::write(root.join(format!("{index:064x}.json")), []).unwrap();
		}
		assert!(matches!(read_records(&root), Err(ContentError::IntegrityFailed)));

		let temp = TempDir::new().unwrap();
		let root = temp.path().join(ROOT);
		fs::create_dir_all(&root).unwrap();
		fs::write(root.join("first.json.tmp-1"), []).unwrap();
		fs::write(root.join("second.json.tmp-1"), []).unwrap();
		assert!(matches!(read_records(&root), Err(ContentError::IntegrityFailed)));

		let temp = TempDir::new().unwrap();
		let root = temp.path().join(ROOT);
		fs::create_dir_all(&root).unwrap();
		fs::write(root.join("oversized.json"), vec![0; MAX_RECORD_BYTES + 1]).unwrap();
		assert!(matches!(read_records(&root), Err(ContentError::IntegrityFailed)));
	}
}
