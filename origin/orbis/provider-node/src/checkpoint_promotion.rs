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

//! Private durable intents and finalized receipts for deterministic checkpoint promotions.

use std::{
	collections::{BTreeMap, HashMap},
	fs::{self, File, OpenOptions},
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

const LEGACY_ROOT: &str = "checkpoint-promotions-v1";
const INTENTS_ROOT: &str = "checkpoint-promotion-intents-v2";
const FINALIZED_RECEIPTS_ROOT: &str = "checkpoint-promotion-finalized-receipts-v2";
const SCHEDULER_ROOT: &str = "checkpoint-promotion-scheduler-v2";
const SCHEDULER_KEY: &str = "cursor";
const STORE_VERSION: u8 = 2;
const SCHEDULER_VERSION: u8 = 2;
const PAYLOAD_VERSION: u8 = 1;
const FINALITY_ATTESTATION_VERSION: u8 = 1;
const AUTHORIZED_STATE: &str = "authorized";
const FINALIZED_STATE: &str = "finalized";
const PROMOTION_DOMAIN: &[u8] = b"cord/storage/checkpoint-promotion/v1";
const ID_DOMAIN: &[u8] = b"cord/provider/checkpoint-promotion-intent/v2";
const TUPLE_DOMAIN: &[u8] = b"cord/provider/checkpoint-promotion-tuple/v2";
const INTENT_RECORD_DOMAIN: &[u8] = b"cord/provider/checkpoint-promotion-intent-record/v2";
const FINALIZED_RECEIPT_DOMAIN: &[u8] = b"cord/provider/checkpoint-promotion-finalized-receipt/v2";
const FINALITY_ATTESTATION_DOMAIN: &[u8] =
	b"cord/provider/checkpoint-promotion-finality-attestation/v1";
const SCHEDULER_RECORD_DOMAIN: &[u8] = b"cord/provider/checkpoint-promotion-scheduler/v2";
const MAX_DUTY_BYTES: usize = 64 * 1024;
const MAX_INTENT_BYTES: usize = 192 * 1024;
const MAX_RECEIPT_BYTES: usize = 8 * 1024;
const MAX_RECORDS: usize = 8_192;
const MAX_TEMP_ARTIFACTS: usize = 1;
const MAX_RESERVATION_SCAN: usize = 64;
const MAX_RESERVATION_ACTIONS: usize = 8;

type Duty = CheckpointDutyInfo<AccountId32, H256, u32>;
type Payload = CheckpointFallbackPromotionV1<H256, u32>;
type CallArgs = (Payload, ed25519::Public, ed25519::Signature);
type Authority = ProviderDutyAuthority<AccountId32, H256, u32>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FallbackPromotionIntentV2 {
	pub version: u8,
	pub intent_id: String,
	pub tuple_key: String,
	pub provider: String,
	pub duty_scale: String,
	pub duty_fingerprint: String,
	pub inventory_finalized_hash: String,
	pub inventory_finalized_number: u32,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FallbackPromotionFinalizedReceiptV2 {
	pub version: u8,
	pub intent_id: String,
	pub intent_record_hash: String,
	pub tuple_key: String,
	pub provider: String,
	pub finalized_hash: String,
	pub finalized_number: u32,
	pub extrinsic_hash: String,
	pub state: String,
	pub finality_attestation_version: u8,
	pub finality_signature: String,
	pub receipt_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointPromotionSchedulerCursorV2 {
	version: u8,
	intent_id: String,
	intent_record_hash: String,
	tuple_key: String,
	inventory_finalized_hash: String,
	inventory_finalized_number: u32,
	record_hash: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckpointPromotionFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
}

pub(crate) struct CheckpointPromotionStoreV2 {
	intents_root: PathBuf,
	finalized_receipts_root: PathBuf,
	scheduler_root: PathBuf,
	intents: RwLock<HashMap<String, FallbackPromotionIntentV2>>,
	intent_order: RwLock<BTreeMap<(u32, String), String>>,
	by_tuple: RwLock<HashMap<String, String>>,
	finalized_receipts: RwLock<HashMap<String, FallbackPromotionFinalizedReceiptV2>>,
	scheduler_cursor: RwLock<Option<CheckpointPromotionSchedulerCursorV2>>,
	fault: RwLock<Option<CheckpointPromotionFault>>,
	poisoned: RwLock<bool>,
}

impl CheckpointPromotionStoreV2 {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let provider_root = root.as_ref();
		if provider_root.join(LEGACY_ROOT).try_exists().map_err(io_error)? {
			return Err(ContentError::IntegrityFailed);
		}
		let intents_root = provider_root.join(INTENTS_ROOT);
		let finalized_receipts_root = provider_root.join(FINALIZED_RECEIPTS_ROOT);
		let scheduler_root = provider_root.join(SCHEDULER_ROOT);
		fs::create_dir_all(&intents_root).map_err(io_error)?;
		fs::create_dir_all(&finalized_receipts_root).map_err(io_error)?;
		fs::create_dir_all(&scheduler_root).map_err(io_error)?;

		let mut intents = HashMap::new();
		let mut intent_order = BTreeMap::new();
		let mut by_tuple = HashMap::new();
		for item in read_records(&intents_root, MAX_INTENT_BYTES)? {
			let intent: FallbackPromotionIntentV2 =
				serde_json::from_slice(&item.bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_intent(&intent)?;
			let order_key = (intent.inventory_finalized_number, intent.intent_id.clone());
			if item.name != format!("{}.json", intent.intent_id) ||
				intents.len() >= MAX_RECORDS ||
				intents.insert(intent.intent_id.clone(), intent.clone()).is_some() ||
				intent_order.insert(order_key, intent.intent_id.clone()).is_some() ||
				by_tuple.insert(intent.tuple_key.clone(), intent.intent_id.clone()).is_some()
			{
				return Err(ContentError::IntegrityFailed);
			}
		}

		let mut finalized_receipts = HashMap::new();
		for item in read_records(&finalized_receipts_root, MAX_RECEIPT_BYTES)? {
			let receipt: FallbackPromotionFinalizedReceiptV2 =
				serde_json::from_slice(&item.bytes).map_err(|_| ContentError::IntegrityFailed)?;
			let intent = intents.get(&receipt.intent_id).ok_or(ContentError::IntegrityFailed)?;
			validate_finalized_receipt(&receipt, intent)?;
			if item.name != format!("{}.json", receipt.intent_id) ||
				finalized_receipts.len() >= MAX_RECORDS ||
				finalized_receipts.insert(receipt.intent_id.clone(), receipt).is_some()
			{
				return Err(ContentError::IntegrityFailed);
			}
		}
		let scheduler_cursor = read_scheduler_cursor(&scheduler_root)?;
		if let Some(cursor) = &scheduler_cursor {
			let intent = intents.get(&cursor.intent_id).ok_or(ContentError::IntegrityFailed)?;
			validate_scheduler_cursor_against_intent(cursor, intent)?;
		}

		Ok(Self {
			intents_root,
			finalized_receipts_root,
			scheduler_root,
			intents: RwLock::new(intents),
			intent_order: RwLock::new(intent_order),
			by_tuple: RwLock::new(by_tuple),
			finalized_receipts: RwLock::new(finalized_receipts),
			scheduler_cursor: RwLock::new(scheduler_cursor),
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

	pub(crate) fn authorize(
		&self,
		provider: &AccountId32,
		duty_scale: &[u8],
		signer: &dyn ServiceKeySigner,
	) -> Result<FallbackPromotionIntentV2, ContentError> {
		let candidate = intent_record(provider, duty_scale, signer)?;
		self.ensure_healthy()?;
		{
			let intents = self.intents.read().map_err(|_| lock_error())?;
			let by_tuple = self.by_tuple.read().map_err(|_| lock_error())?;
			if let Some(existing_id) = by_tuple.get(&candidate.tuple_key) {
				let existing = intents.get(existing_id).ok_or(ContentError::IntegrityFailed)?;
				return exact_or_conflict(existing, &candidate);
			}
		}

		let mut intents = self.intents.write().map_err(|_| lock_error())?;
		let mut by_tuple = self.by_tuple.write().map_err(|_| lock_error())?;
		let mut intent_order = self.intent_order.write().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		if let Some(existing_id) = by_tuple.get(&candidate.tuple_key) {
			let existing = intents.get(existing_id).ok_or(ContentError::IntegrityFailed)?;
			return exact_or_conflict(existing, &candidate);
		}
		if intents.len() >= MAX_RECORDS {
			return Err(ContentError::ProviderRecoveryTableFull);
		}
		if let Err(error) = self.persist_intent(&candidate) {
			self.poison()?;
			return Err(error);
		}
		by_tuple.insert(candidate.tuple_key.clone(), candidate.intent_id.clone());
		intent_order.insert(
			(candidate.inventory_finalized_number, candidate.intent_id.clone()),
			candidate.intent_id.clone(),
		);
		intents.insert(candidate.intent_id.clone(), candidate.clone());
		Ok(candidate)
	}

	/// Reserve one bounded fair batch and fsync its cursor before the caller performs network work.
	pub(crate) fn reserve_pending_intents(
		&self,
		limit: usize,
	) -> Result<Vec<FallbackPromotionIntentV2>, ContentError> {
		if limit == 0 || limit > MAX_RESERVATION_ACTIONS {
			return Err(ContentError::IntegrityFailed);
		}
		self.ensure_healthy()?;
		let intents = self.intents.read().map_err(|_| lock_error())?;
		let intent_order = self.intent_order.read().map_err(|_| lock_error())?;
		let receipts = self.finalized_receipts.read().map_err(|_| lock_error())?;
		let mut cursor = self.scheduler_cursor.write().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		let previous_key = cursor
			.as_ref()
			.map(|previous| (previous.inventory_finalized_number, previous.intent_id.clone()));
		let mut ordered_ids = Vec::with_capacity(MAX_RESERVATION_SCAN);
		if let Some(previous_key) = previous_key {
			use std::ops::Bound::{Excluded, Included, Unbounded};
			ordered_ids.extend(
				intent_order
					.range((Excluded(previous_key.clone()), Unbounded))
					.chain(intent_order.range((Unbounded, Included(previous_key))))
					.take(MAX_RESERVATION_SCAN)
					.map(|(_, intent_id)| intent_id.clone()),
			);
		} else {
			ordered_ids.extend(intent_order.values().take(MAX_RESERVATION_SCAN).cloned());
		}
		let mut pending = Vec::with_capacity(limit);
		let mut last_considered = None;
		for intent_id in ordered_ids {
			let intent = intents.get(&intent_id).ok_or(ContentError::IntegrityFailed)?;
			last_considered = Some(intent);
			if !receipts.contains_key(&intent_id) {
				pending.push(intent.clone());
				if pending.len() == limit {
					break;
				}
			}
		}
		if let Some(last) = last_considered {
			let candidate = scheduler_cursor(last)?;
			if cursor.as_ref() != Some(&candidate) {
				if let Err(error) = self.persist_scheduler_cursor(&candidate) {
					self.poison()?;
					return Err(error);
				}
				*cursor = Some(candidate);
			}
		}
		Ok(pending)
	}

	fn unreserved_pending_intents(&self) -> Result<Vec<FallbackPromotionIntentV2>, ContentError> {
		self.ensure_healthy()?;
		let intents = self.intents.read().map_err(|_| lock_error())?;
		let receipts = self.finalized_receipts.read().map_err(|_| lock_error())?;
		let mut pending = intents
			.values()
			.filter(|intent| !receipts.contains_key(&intent.intent_id))
			.cloned()
			.collect::<Vec<_>>();
		pending.sort_by(|left, right| {
			left.inventory_finalized_number
				.cmp(&right.inventory_finalized_number)
				.then_with(|| left.intent_id.cmp(&right.intent_id))
		});
		Ok(pending)
	}

	#[cfg(test)]
	fn pending_intents(&self) -> Result<Vec<FallbackPromotionIntentV2>, ContentError> {
		self.unreserved_pending_intents()
	}

	pub(crate) fn record_finalized(
		&self,
		intent_id: &str,
		finalized_hash: [u8; 32],
		finalized_number: u32,
		extrinsic_hash: [u8; 32],
		finality_attestation_version: u8,
		finality_signature: [u8; 64],
	) -> Result<FallbackPromotionFinalizedReceiptV2, ContentError> {
		self.ensure_healthy()?;
		let intent = self
			.intents
			.read()
			.map_err(|_| lock_error())?
			.get(intent_id)
			.cloned()
			.ok_or(ContentError::NotFound)?;
		let mut receipt = FallbackPromotionFinalizedReceiptV2 {
			version: STORE_VERSION,
			intent_id: intent.intent_id.clone(),
			intent_record_hash: intent.record_hash.clone(),
			tuple_key: intent.tuple_key.clone(),
			provider: intent.provider.clone(),
			finalized_hash: hex::encode(finalized_hash),
			finalized_number,
			extrinsic_hash: hex::encode(extrinsic_hash),
			state: FINALIZED_STATE.into(),
			finality_attestation_version,
			finality_signature: hex::encode(finality_signature),
			receipt_hash: String::new(),
		};
		receipt.receipt_hash = finalized_receipt_hash(&receipt)?;
		validate_finalized_receipt(&receipt, &intent)?;

		let mut receipts = self.finalized_receipts.write().map_err(|_| lock_error())?;
		self.ensure_healthy()?;
		if let Some(existing) = receipts.get(intent_id) {
			return exact_or_conflict(existing, &receipt);
		}
		if receipts.len() >= MAX_RECORDS {
			return Err(ContentError::ProviderRecoveryTableFull);
		}
		if let Err(error) = self.persist_finalized_receipt(&receipt) {
			self.poison()?;
			return Err(error);
		}
		receipts.insert(intent_id.into(), receipt.clone());
		Ok(receipt)
	}

	fn persist_intent(&self, intent: &FallbackPromotionIntentV2) -> Result<(), ContentError> {
		validate_intent(intent)?;
		persist_record(
			&self.intents_root,
			&intent.intent_id,
			&serde_json::to_vec(intent).map_err(io_error)?,
			MAX_INTENT_BYTES,
			&self.fault,
		)
	}

	fn persist_finalized_receipt(
		&self,
		receipt: &FallbackPromotionFinalizedReceiptV2,
	) -> Result<(), ContentError> {
		persist_record(
			&self.finalized_receipts_root,
			&receipt.intent_id,
			&serde_json::to_vec(receipt).map_err(io_error)?,
			MAX_RECEIPT_BYTES,
			&self.fault,
		)
	}

	fn persist_scheduler_cursor(
		&self,
		cursor: &CheckpointPromotionSchedulerCursorV2,
	) -> Result<(), ContentError> {
		validate_scheduler_cursor(cursor)?;
		let bytes = serde_json::to_vec(cursor).map_err(io_error)?;
		persist_record(&self.scheduler_root, SCHEDULER_KEY, &bytes, MAX_RECEIPT_BYTES, &self.fault)
	}

	fn ensure_healthy(&self) -> Result<(), ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			Err(ContentError::IntegrityFailed)
		} else {
			Ok(())
		}
	}

	fn poison(&self) -> Result<(), ContentError> {
		*self.poisoned.write().map_err(|_| lock_error())? = true;
		Ok(())
	}
}

fn exact_or_conflict<T: Clone + Eq>(existing: &T, candidate: &T) -> Result<T, ContentError> {
	if existing == candidate {
		Ok(existing.clone())
	} else {
		Err(ContentError::IdempotencyConflict)
	}
}

struct RecordFile {
	name: String,
	bytes: Vec<u8>,
}

fn read_records(root: &Path, max_record_bytes: usize) -> Result<Vec<RecordFile>, ContentError> {
	let mut records = Vec::new();
	let mut visited = 0usize;
	let mut temp_artifacts = 0usize;
	let mut removed_temp = false;
	for item in fs::read_dir(root).map_err(io_error)? {
		visited = visited.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		if visited > MAX_RECORDS + MAX_TEMP_ARTIFACTS {
			return Err(ContentError::IntegrityFailed);
		}
		let item = item.map_err(io_error)?;
		let name = item.file_name().to_string_lossy().into_owned();
		if name.contains(".tmp-") {
			temp_artifacts = temp_artifacts.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			if temp_artifacts > MAX_TEMP_ARTIFACTS || !item.file_type().map_err(io_error)?.is_file()
			{
				return Err(ContentError::IntegrityFailed);
			}
			fs::remove_file(item.path()).map_err(io_error)?;
			removed_temp = true;
			continue;
		}
		if records.len() >= MAX_RECORDS ||
			!name.ends_with(".json") ||
			!item.file_type().map_err(io_error)?.is_file()
		{
			return Err(ContentError::IntegrityFailed);
		}
		let bytes = fs::read(item.path()).map_err(io_error)?;
		if bytes.len() > max_record_bytes {
			return Err(ContentError::IntegrityFailed);
		}
		records.push(RecordFile { name, bytes });
	}
	if removed_temp {
		File::open(root).and_then(|directory| directory.sync_all()).map_err(io_error)?;
	}
	Ok(records)
}

fn read_scheduler_cursor(
	root: &Path,
) -> Result<Option<CheckpointPromotionSchedulerCursorV2>, ContentError> {
	let expected = format!("{SCHEDULER_KEY}.json");
	let temp_prefix = format!("{SCHEDULER_KEY}.json.tmp-");
	let mut cursor = None;
	let mut temp_artifacts = 0usize;
	let mut removed_temp = false;
	for item in fs::read_dir(root).map_err(io_error)? {
		let item = item.map_err(io_error)?;
		let name = item.file_name().to_string_lossy().into_owned();
		if name.starts_with(&temp_prefix) {
			temp_artifacts = temp_artifacts.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			if temp_artifacts > MAX_TEMP_ARTIFACTS || !item.file_type().map_err(io_error)?.is_file()
			{
				return Err(ContentError::IntegrityFailed);
			}
			fs::remove_file(item.path()).map_err(io_error)?;
			removed_temp = true;
			continue;
		}
		if name != expected || cursor.is_some() || !item.file_type().map_err(io_error)?.is_file() {
			return Err(ContentError::IntegrityFailed);
		}
		let bytes = fs::read(item.path()).map_err(io_error)?;
		if bytes.len() > MAX_RECEIPT_BYTES {
			return Err(ContentError::IntegrityFailed);
		}
		let decoded: CheckpointPromotionSchedulerCursorV2 =
			serde_json::from_slice(&bytes).map_err(|_| ContentError::IntegrityFailed)?;
		validate_scheduler_cursor(&decoded)?;
		cursor = Some(decoded);
	}
	if removed_temp {
		File::open(root).and_then(|directory| directory.sync_all()).map_err(io_error)?;
	}
	Ok(cursor)
}

fn scheduler_cursor(
	intent: &FallbackPromotionIntentV2,
) -> Result<CheckpointPromotionSchedulerCursorV2, ContentError> {
	validate_intent(intent)?;
	let mut cursor = CheckpointPromotionSchedulerCursorV2 {
		version: SCHEDULER_VERSION,
		intent_id: intent.intent_id.clone(),
		intent_record_hash: intent.record_hash.clone(),
		tuple_key: intent.tuple_key.clone(),
		inventory_finalized_hash: intent.inventory_finalized_hash.clone(),
		inventory_finalized_number: intent.inventory_finalized_number,
		record_hash: String::new(),
	};
	cursor.record_hash = scheduler_cursor_hash(&cursor)?;
	validate_scheduler_cursor_against_intent(&cursor, intent)?;
	Ok(cursor)
}

fn validate_scheduler_cursor(
	cursor: &CheckpointPromotionSchedulerCursorV2,
) -> Result<(), ContentError> {
	if cursor.version != SCHEDULER_VERSION || cursor.record_hash != scheduler_cursor_hash(cursor)? {
		return Err(ContentError::IntegrityFailed);
	}
	for value in [
		&cursor.intent_id,
		&cursor.intent_record_hash,
		&cursor.tuple_key,
		&cursor.inventory_finalized_hash,
		&cursor.record_hash,
	] {
		let _: [u8; 32] =
			decode_hex(value)?.try_into().map_err(|_| ContentError::IntegrityFailed)?;
	}
	Ok(())
}

fn validate_scheduler_cursor_against_intent(
	cursor: &CheckpointPromotionSchedulerCursorV2,
	intent: &FallbackPromotionIntentV2,
) -> Result<(), ContentError> {
	validate_scheduler_cursor(cursor)?;
	if cursor.intent_id != intent.intent_id ||
		cursor.intent_record_hash != intent.record_hash ||
		cursor.tuple_key != intent.tuple_key ||
		cursor.inventory_finalized_hash != intent.inventory_finalized_hash ||
		cursor.inventory_finalized_number != intent.inventory_finalized_number
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn scheduler_cursor_hash(
	cursor: &CheckpointPromotionSchedulerCursorV2,
) -> Result<String, ContentError> {
	let mut canonical = cursor.clone();
	canonical.record_hash.clear();
	let mut input = SCHEDULER_RECORD_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn persist_record(
	root: &Path,
	key: &str,
	bytes: &[u8],
	max_record_bytes: usize,
	fault: &RwLock<Option<CheckpointPromotionFault>>,
) -> Result<(), ContentError> {
	if bytes.len() > max_record_bytes {
		return Err(ContentError::IntegrityFailed);
	}
	let temp = root.join(format!("{key}.json.tmp-{}", std::process::id()));
	let mut file = OpenOptions::new().write(true).create_new(true).open(&temp).map_err(io_error)?;
	file.write_all(bytes).map_err(io_error)?;
	trip(fault, CheckpointPromotionFault::BeforeTempFsync)?;
	file.sync_all().map_err(io_error)?;
	trip(fault, CheckpointPromotionFault::AfterTempFsync)?;
	fs::rename(&temp, root.join(format!("{key}.json"))).map_err(io_error)?;
	trip(fault, CheckpointPromotionFault::AfterRename)?;
	File::open(root).and_then(|directory| directory.sync_all()).map_err(io_error)?;
	trip(fault, CheckpointPromotionFault::AfterDirectoryFsync)
}

fn trip(
	fault: &RwLock<Option<CheckpointPromotionFault>>,
	point: CheckpointPromotionFault,
) -> Result<(), ContentError> {
	let mut fault = fault.write().map_err(|_| lock_error())?;
	if fault.as_ref() == Some(&point) {
		*fault = None;
		Err(ContentError::Io(format!("injected checkpoint promotion fault: {point:?}")))
	} else {
		Ok(())
	}
}

fn intent_record(
	provider: &AccountId32,
	duty_scale: &[u8],
	signer: &dyn ServiceKeySigner,
) -> Result<FallbackPromotionIntentV2, ContentError> {
	if duty_scale.len() > MAX_DUTY_BYTES {
		return Err(ContentError::IntegrityFailed);
	}
	let duty = decode_scale::<Duty>(duty_scale)?;
	let service_key = ed25519::Public::from_raw(signer.public_key());
	let local = validate_duty(&duty, provider, &service_key)?;
	let payload = Payload {
		version: PAYLOAD_VERSION,
		bucket_id: duty.bucket_id,
		snapshot_nonce: duty.snapshot_checkpoint,
		duty_id: duty.duty_id,
	};
	let digest = promotion_digest(&payload);
	let signature = ed25519::Signature::from_raw(signer.sign_digest(digest));
	if !ed25519::Pair::verify(&signature, &digest, &service_key) {
		return Err(ContentError::IntegrityFailed);
	}
	let args = (payload, service_key, signature);
	let mut intent = FallbackPromotionIntentV2 {
		version: STORE_VERSION,
		intent_id: intent_id(provider, duty_scale, &args),
		tuple_key: tuple_key(&payload),
		provider: account_hex(provider),
		duty_scale: hex::encode(duty_scale),
		duty_fingerprint: hex::encode(blake2_256(duty_scale)),
		inventory_finalized_hash: hex::encode(duty.snapshot_hash.as_bytes()),
		inventory_finalized_number: duty.snapshot_checkpoint,
		snapshot_checkpoint: duty.snapshot_checkpoint,
		bucket_id: hex::encode(duty.bucket_id.as_bytes()),
		duty_id: hex::encode(duty.duty_id.as_bytes()),
		service_key_version: local.active_service_key_version,
		payload_scale: hex::encode(payload.encode()),
		service_key: hex::encode(service_key.0),
		signature: hex::encode(signature.0),
		call_args_scale: hex::encode(args.encode()),
		state: AUTHORIZED_STATE.into(),
		record_hash: String::new(),
	};
	intent.record_hash = intent_record_hash(&intent)?;
	validate_intent(&intent)?;
	Ok(intent)
}

pub(crate) fn validate_intent(intent: &FallbackPromotionIntentV2) -> Result<(), ContentError> {
	if intent.version != STORE_VERSION ||
		intent.state != AUTHORIZED_STATE ||
		intent.intent_id.len() != 64 ||
		intent.tuple_key.len() != 64 ||
		intent.record_hash != intent_record_hash(intent)?
	{
		return Err(ContentError::IntegrityFailed);
	}
	let provider = decode_account(&intent.provider)?;
	let duty_bytes = decode_hex(&intent.duty_scale)?;
	if duty_bytes.len() > MAX_DUTY_BYTES ||
		intent.duty_fingerprint != hex::encode(blake2_256(&duty_bytes))
	{
		return Err(ContentError::IntegrityFailed);
	}
	let duty = decode_scale::<Duty>(&duty_bytes)?;
	let service_key = ed25519::Public::from_raw(decode_fixed(&intent.service_key)?);
	let local = validate_duty(&duty, &provider, &service_key)?;
	let payload = decode_hex_scale::<Payload>(&intent.payload_scale)?;
	let expected_payload = Payload {
		version: PAYLOAD_VERSION,
		bucket_id: duty.bucket_id,
		snapshot_nonce: duty.snapshot_checkpoint,
		duty_id: duty.duty_id,
	};
	let signature = ed25519::Signature::from_raw(decode_fixed(&intent.signature)?);
	let args = (payload, service_key, signature);
	if payload != expected_payload ||
		intent.inventory_finalized_hash != hex::encode(duty.snapshot_hash.as_bytes()) ||
		intent.inventory_finalized_number != duty.snapshot_checkpoint ||
		intent.snapshot_checkpoint != duty.snapshot_checkpoint ||
		intent.bucket_id != hex::encode(duty.bucket_id.as_bytes()) ||
		intent.duty_id != hex::encode(duty.duty_id.as_bytes()) ||
		intent.service_key_version != local.active_service_key_version ||
		intent.call_args_scale != hex::encode(args.encode()) ||
		intent.intent_id != intent_id(&provider, &duty_bytes, &args) ||
		intent.tuple_key != tuple_key(&payload) ||
		!ed25519::Pair::verify(&signature, &promotion_digest(&payload), &service_key)
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn validate_finalized_receipt(
	receipt: &FallbackPromotionFinalizedReceiptV2,
	intent: &FallbackPromotionIntentV2,
) -> Result<(), ContentError> {
	if receipt.version != STORE_VERSION ||
		receipt.intent_id != intent.intent_id ||
		receipt.intent_record_hash != intent.record_hash ||
		receipt.tuple_key != intent.tuple_key ||
		receipt.provider != intent.provider ||
		receipt.state != FINALIZED_STATE ||
		receipt.finalized_hash.len() != 64 ||
		receipt.extrinsic_hash.len() != 64 ||
		receipt.finality_attestation_version != FINALITY_ATTESTATION_VERSION ||
		receipt.finality_signature.len() != 128 ||
		receipt.finalized_number < intent.inventory_finalized_number ||
		receipt.finalized_number < intent.snapshot_checkpoint ||
		receipt.receipt_hash != finalized_receipt_hash(receipt)?
	{
		return Err(ContentError::IntegrityFailed);
	}
	let finalized_hash = decode_fixed::<32>(&receipt.finalized_hash)?;
	let extrinsic_hash = decode_fixed::<32>(&receipt.extrinsic_hash)?;
	let signature = ed25519::Signature::from_raw(decode_fixed::<64>(&receipt.finality_signature)?);
	let service_key = ed25519::Public::from_raw(decode_fixed::<32>(&intent.service_key)?);
	let digest = promotion_finality_attestation_digest(
		receipt.finality_attestation_version,
		&receipt.intent_id,
		&receipt.intent_record_hash,
		&receipt.tuple_key,
		finalized_hash,
		receipt.finalized_number,
		extrinsic_hash,
		&receipt.state,
	)?;
	if !ed25519::Pair::verify(&signature, &digest, &service_key) {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

pub(crate) fn promotion_finality_attestation_digest(
	version: u8,
	intent_id: &str,
	intent_record_hash: &str,
	tuple_key: &str,
	finalized_hash: [u8; 32],
	finalized_number: u32,
	extrinsic_hash: [u8; 32],
	state: &str,
) -> Result<[u8; 32], ContentError> {
	if version != FINALITY_ATTESTATION_VERSION || state != FINALIZED_STATE {
		return Err(ContentError::IntegrityFailed);
	}
	let intent_id = decode_fixed::<32>(intent_id)?;
	let intent_record_hash = decode_fixed::<32>(intent_record_hash)?;
	let tuple_key = decode_fixed::<32>(tuple_key)?;
	let mut input = FINALITY_ATTESTATION_DOMAIN.to_vec();
	version.encode_to(&mut input);
	intent_id.encode_to(&mut input);
	intent_record_hash.encode_to(&mut input);
	tuple_key.encode_to(&mut input);
	finalized_hash.encode_to(&mut input);
	finalized_number.encode_to(&mut input);
	extrinsic_hash.encode_to(&mut input);
	state.as_bytes().encode_to(&mut input);
	Ok(blake2_256(&input))
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
		return Err(ContentError::IntegrityFailed);
	}
	let expected_len = duty.replicas.len().checked_add(1).ok_or(ContentError::IntegrityFailed)?;
	if duty.authorities.len() != expected_len {
		return Err(ContentError::IntegrityFailed);
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
			return Err(ContentError::IntegrityFailed);
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
		return Err(ContentError::IntegrityFailed);
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

fn intent_id(provider: &AccountId32, duty_scale: &[u8], args: &CallArgs) -> String {
	let mut input = ID_DOMAIN.to_vec();
	provider.encode_to(&mut input);
	duty_scale.encode_to(&mut input);
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

fn intent_record_hash(intent: &FallbackPromotionIntentV2) -> Result<String, ContentError> {
	let mut canonical = intent.clone();
	canonical.record_hash.clear();
	let mut input = INTENT_RECORD_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn finalized_receipt_hash(
	receipt: &FallbackPromotionFinalizedReceiptV2,
) -> Result<String, ContentError> {
	let mut canonical = receipt.clone();
	canonical.receipt_hash.clear();
	let mut input = FINALIZED_RECEIPT_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn decode_scale<T: Decode + Encode>(bytes: &[u8]) -> Result<T, ContentError> {
	let mut input = bytes;
	let decoded = T::decode(&mut input).map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || decoded.encode() != bytes {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(decoded)
}

fn decode_hex_scale<T: Decode + Encode>(value: &str) -> Result<T, ContentError> {
	decode_scale(&decode_hex(value)?)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ContentError> {
	if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
		return Err(ContentError::IntegrityFailed);
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

	fn duty_with(seed: u8) -> Duty {
		let mut duty = duty();
		duty.duty_id = H256::repeat_byte(seed);
		duty.bucket_id = H256::repeat_byte(seed.wrapping_add(100));
		duty
	}

	fn authorize_with(store: &CheckpointPromotionStoreV2, seed: u8) -> FallbackPromotionIntentV2 {
		store.authorize(&account(2), &duty_with(seed).encode(), &pair(2)).unwrap()
	}

	fn intent_path(temp: &TempDir, intent_id: &str) -> PathBuf {
		temp.path().join(INTENTS_ROOT).join(format!("{intent_id}.json"))
	}

	fn receipt_path(temp: &TempDir, intent_id: &str) -> PathBuf {
		temp.path().join(FINALIZED_RECEIPTS_ROOT).join(format!("{intent_id}.json"))
	}

	fn finality_signature(
		intent: &FallbackPromotionIntentV2,
		block_hash: [u8; 32],
		block_number: u32,
		extrinsic_hash: [u8; 32],
	) -> [u8; 64] {
		let digest = promotion_finality_attestation_digest(
			FINALITY_ATTESTATION_VERSION,
			&intent.intent_id,
			&intent.record_hash,
			&intent.tuple_key,
			block_hash,
			block_number,
			extrinsic_hash,
			FINALIZED_STATE,
		)
		.unwrap();
		pair(2).sign(&digest).0
	}

	fn finalize(
		store: &CheckpointPromotionStoreV2,
		intent: &FallbackPromotionIntentV2,
	) -> FallbackPromotionFinalizedReceiptV2 {
		let block_hash = [21; 32];
		let block_number = 130;
		let extrinsic_hash = [22; 32];
		store
			.record_finalized(
				&intent.intent_id,
				block_hash,
				block_number,
				extrinsic_hash,
				FINALITY_ATTESTATION_VERSION,
				finality_signature(intent, block_hash, block_number, extrinsic_hash),
			)
			.unwrap()
	}

	#[test]
	fn exact_intent_and_finalized_receipt_replay_and_reopen_stably() {
		let temp = TempDir::new().unwrap();
		let duty = duty();
		let duty_scale = duty.encode();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let intent = store.authorize(&account(2), &duty_scale, &pair(2)).unwrap();
		assert_eq!(intent.state, AUTHORIZED_STATE);
		assert_eq!(intent.inventory_finalized_hash, hex::encode(duty.snapshot_hash));
		assert_eq!(intent.inventory_finalized_number, duty.snapshot_checkpoint);
		assert_eq!(decode_hex(&intent.duty_scale).unwrap(), duty_scale);
		let args = decode_hex_scale::<CallArgs>(&intent.call_args_scale).unwrap();
		assert_eq!(args.0.version, PAYLOAD_VERSION);
		assert_eq!(args.0.bucket_id, duty.bucket_id);
		assert_eq!(args.0.snapshot_nonce, duty.snapshot_checkpoint);
		assert_eq!(args.0.duty_id, duty.duty_id);
		assert_eq!(store.pending_intents().unwrap(), vec![intent.clone()]);
		assert_eq!(store.authorize(&account(2), &duty_scale, &pair(2)).unwrap(), intent);

		let receipt = finalize(&store, &intent);
		assert!(store.pending_intents().unwrap().is_empty());
		assert_eq!(finalize(&store, &intent), receipt);
		drop(store);

		let reopened = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		assert_eq!(reopened.authorize(&account(2), &duty_scale, &pair(2)).unwrap(), intent);
		assert_eq!(finalize(&reopened, &intent), receipt);
		assert!(reopened.pending_intents().unwrap().is_empty());
	}

	#[test]
	fn same_tuple_with_changed_duty_conflicts_and_changed_finality_conflicts() {
		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let base = duty();
		let intent = store.authorize(&account(2), &base.encode(), &pair(2)).unwrap();
		let mut changed = base.clone();
		changed.snapshot_hash = H256::repeat_byte(88);
		assert!(matches!(
			store.authorize(&account(2), &changed.encode(), &pair(2)),
			Err(ContentError::IdempotencyConflict)
		));
		finalize(&store, &intent);
		assert!(matches!(
			store.record_finalized(
				&intent.intent_id,
				[31; 32],
				131,
				[32; 32],
				FINALITY_ATTESTATION_VERSION,
				[0; 64],
			),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn legacy_root_is_rejected_without_creating_v2_roots() {
		let temp = TempDir::new().unwrap();
		fs::create_dir(temp.path().join(LEGACY_ROOT)).unwrap();
		assert!(matches!(
			CheckpointPromotionStoreV2::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
		assert!(!temp.path().join(INTENTS_ROOT).exists());
		assert!(!temp.path().join(FINALIZED_RECEIPTS_ROOT).exists());
	}

	#[test]
	fn rehashed_semantic_tampering_is_rejected_on_reopen() {
		let temp = TempDir::new().unwrap();
		let base = duty();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let intent = store.authorize(&account(2), &base.encode(), &pair(2)).unwrap();
		drop(store);
		let path = intent_path(&temp, &intent.intent_id);
		let mut record: FallbackPromotionIntentV2 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		record.call_args_scale.push_str("00");
		record.record_hash = intent_record_hash(&record).unwrap();
		fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(matches!(
			CheckpointPromotionStoreV2::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));

		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let intent = store.authorize(&account(2), &base.encode(), &pair(2)).unwrap();
		finalize(&store, &intent);
		drop(store);
		let path = receipt_path(&temp, &intent.intent_id);
		let mut record: FallbackPromotionFinalizedReceiptV2 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		record.finalized_number += 1;
		record.receipt_hash = finalized_receipt_hash(&record).unwrap();
		fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(matches!(
			CheckpointPromotionStoreV2::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn every_intent_and_receipt_crash_seam_recovers_exact_old_or_new_state() {
		let faults = [
			CheckpointPromotionFault::BeforeTempFsync,
			CheckpointPromotionFault::AfterTempFsync,
			CheckpointPromotionFault::AfterRename,
			CheckpointPromotionFault::AfterDirectoryFsync,
		];
		for fault in faults {
			let temp = TempDir::new().unwrap();
			let base = duty();
			let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(
				store.authorize(&account(2), &base.encode(), &pair(2)),
				Err(ContentError::Io(_))
			));
			drop(store);
			let reopened = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
			let intent = reopened.authorize(&account(2), &base.encode(), &pair(2)).unwrap();

			reopened.inject_fault_once(fault).unwrap();
			let block_hash = [21; 32];
			let block_number = 130;
			let extrinsic_hash = [22; 32];
			assert!(matches!(
				reopened.record_finalized(
					&intent.intent_id,
					block_hash,
					block_number,
					extrinsic_hash,
					FINALITY_ATTESTATION_VERSION,
					finality_signature(&intent, block_hash, block_number, extrinsic_hash),
				),
				Err(ContentError::Io(_))
			));
			drop(reopened);
			let reopened = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
			finalize(&reopened, &intent);
			assert!(reopened.pending_intents().unwrap().is_empty());
		}
	}

	#[test]
	fn directories_records_and_capacity_are_hard_bounded() {
		let temp = TempDir::new().unwrap();
		let root = temp.path().join(INTENTS_ROOT);
		fs::create_dir_all(&root).unwrap();
		for index in 0..=MAX_RECORDS {
			fs::write(root.join(format!("{index:064x}.json")), []).unwrap();
		}
		assert!(matches!(
			read_records(&root, MAX_INTENT_BYTES),
			Err(ContentError::IntegrityFailed)
		));

		let temp = TempDir::new().unwrap();
		let root = temp.path().join(FINALIZED_RECEIPTS_ROOT);
		fs::create_dir_all(&root).unwrap();
		fs::write(root.join("first.json.tmp-1"), []).unwrap();
		fs::write(root.join("second.json.tmp-1"), []).unwrap();
		assert!(matches!(
			read_records(&root, MAX_RECEIPT_BYTES),
			Err(ContentError::IntegrityFailed)
		));

		let temp = TempDir::new().unwrap();
		let root = temp.path().join(INTENTS_ROOT);
		fs::create_dir_all(&root).unwrap();
		fs::write(root.join("oversized.json"), vec![0; MAX_INTENT_BYTES + 1]).unwrap();
		assert!(matches!(
			read_records(&root, MAX_INTENT_BYTES),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn invalid_finality_attestation_and_unknown_intent_fail_closed() {
		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let intent = store.authorize(&account(2), &duty().encode(), &pair(2)).unwrap();
		assert!(matches!(
			store.record_finalized(
				&intent.intent_id,
				[21; 32],
				130,
				[22; 32],
				FINALITY_ATTESTATION_VERSION,
				pair(3).sign(b"wrong").0,
			),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			store.record_finalized(
				&"ff".repeat(32),
				[21; 32],
				130,
				[22; 32],
				FINALITY_ATTESTATION_VERSION,
				[0; 64],
			),
			Err(ContentError::NotFound)
		));

		let block_hash = [21; 32];
		let block_number = intent.inventory_finalized_number - 1;
		let extrinsic_hash = [22; 32];
		assert!(matches!(
			store.record_finalized(
				&intent.intent_id,
				block_hash,
				block_number,
				extrinsic_hash,
				FINALITY_ATTESTATION_VERSION,
				finality_signature(&intent, block_hash, block_number, extrinsic_hash),
			),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn scheduler_bounds_and_persistent_failures_rotate_across_restart() {
		let temp = TempDir::new().unwrap();
		let mut store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let mut all = std::collections::HashSet::new();
		for seed in 20..90 {
			all.insert(authorize_with(&store, seed).intent_id);
		}
		assert!(matches!(store.reserve_pending_intents(0), Err(ContentError::IntegrityFailed)));
		assert!(matches!(
			store.reserve_pending_intents(MAX_RESERVATION_ACTIONS + 1),
			Err(ContentError::IntegrityFailed)
		));

		let mut seen = std::collections::HashSet::new();
		for _ in 0..9 {
			let batch = store.reserve_pending_intents(MAX_RESERVATION_ACTIONS).unwrap();
			assert!(!batch.is_empty());
			assert!(batch.len() <= MAX_RESERVATION_ACTIONS);
			assert_eq!(
				batch
					.iter()
					.map(|intent| &intent.intent_id)
					.collect::<std::collections::HashSet<_>>()
					.len(),
				batch.len()
			);
			seen.extend(batch.into_iter().map(|intent| intent.intent_id));
			drop(store);
			store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		}
		assert_eq!(seen, all);
	}

	#[test]
	fn scheduler_scans_past_sixty_four_finalized_entries_after_restart() {
		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		for seed in 20..86 {
			authorize_with(&store, seed);
		}
		let ordered = store.pending_intents().unwrap();
		for intent in ordered.iter().take(MAX_RESERVATION_SCAN) {
			let block_hash = [21; 32];
			let block_number = 130;
			let extrinsic_hash = [22; 32];
			store
				.record_finalized(
					&intent.intent_id,
					block_hash,
					block_number,
					extrinsic_hash,
					FINALITY_ATTESTATION_VERSION,
					finality_signature(intent, block_hash, block_number, extrinsic_hash),
				)
				.unwrap();
		}
		assert!(store.reserve_pending_intents(MAX_RESERVATION_ACTIONS).unwrap().is_empty());
		drop(store);

		let reopened = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let batch = reopened.reserve_pending_intents(MAX_RESERVATION_ACTIONS).unwrap();
		assert_eq!(batch.len(), 2);
		assert_eq!(
			batch
				.iter()
				.map(|intent| &intent.intent_id)
				.collect::<std::collections::HashSet<_>>()
				.len(),
			2
		);
	}

	#[test]
	fn scheduler_cursor_tamper_and_every_crash_seam_fail_closed() {
		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		authorize_with(&store, 20);
		store.reserve_pending_intents(1).unwrap();
		drop(store);
		let path = temp.path().join(SCHEDULER_ROOT).join(format!("{SCHEDULER_KEY}.json"));
		let mut cursor: CheckpointPromotionSchedulerCursorV2 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		cursor.tuple_key = "ff".repeat(32);
		cursor.record_hash = scheduler_cursor_hash(&cursor).unwrap();
		fs::write(&path, serde_json::to_vec(&cursor).unwrap()).unwrap();
		assert!(matches!(
			CheckpointPromotionStoreV2::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));

		for fault in [
			CheckpointPromotionFault::BeforeTempFsync,
			CheckpointPromotionFault::AfterTempFsync,
			CheckpointPromotionFault::AfterRename,
			CheckpointPromotionFault::AfterDirectoryFsync,
		] {
			let temp = TempDir::new().unwrap();
			let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
			let expected =
				[authorize_with(&store, 20).intent_id, authorize_with(&store, 21).intent_id]
					.into_iter()
					.collect::<std::collections::HashSet<_>>();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(store.reserve_pending_intents(1), Err(ContentError::Io(_))));
			drop(store);
			let reopened = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
			let batch = reopened.reserve_pending_intents(1).unwrap();
			assert_eq!(batch.len(), 1);
			assert!(expected.contains(&batch[0].intent_id));
		}
	}
}
