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

//! Private durable outbox for exact `StorageProvider.submit_checkpoint` v2 calls.

use std::{
	collections::HashMap,
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
};

use codec::{Decode, Encode};
use pallet_orbis_storage_provider::{CheckpointContextV1, CommitmentPayloadV2, ReplicaSignature};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
use sp_crypto_hashing::blake2_256;

use super::checkpoint_quorum::{checkpoint_context_digest, checkpoint_digest};
use crate::ContentError;

const SUBMISSIONS_ROOT: &str = "checkpoint-submissions-v2";
const RECEIPTS_ROOT: &str = "checkpoint-receipts-v2";
const FINALIZED_RECEIPTS_ROOT: &str = "checkpoint-finalized-receipts-v2";
const VERSION: u8 = 2;
const ID_DOMAIN: &[u8] = b"cord/provider/checkpoint-submission-v2";
const RECORD_DOMAIN: &[u8] = b"cord/provider/checkpoint-submission-record/v2";
const RECEIPT_DOMAIN: &[u8] = b"cord/provider/checkpoint-receipt-record/v2";
const FINALIZED_RECEIPT_DOMAIN: &[u8] = b"cord/provider/checkpoint-finalized-receipt-record/v2";
const CHECKPOINT_DOMAIN: &[u8] = b"cord/storage/checkpoint/v2";
const MAX_RECORD_BYTES: usize = 128 * 1024;
const MAX_RECORDS: usize = 8_192;
const MAX_TEMP_ARTIFACTS: usize = 1;

type CallArgs = (
	Vec<u8>,
	CommitmentPayloadV2<H256, u32>,
	u32,
	u32,
	ed25519::Public,
	ed25519::Signature,
	ed25519::Signature,
	Vec<ReplicaSignature<AccountId32>>,
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckpointSubmissionInputV2 {
	pub primary: AccountId32,
	pub domain: Vec<u8>,
	pub payload: CommitmentPayloadV2<H256, u32>,
	pub context: CheckpointContextV1<H256>,
	pub window_start: u32,
	pub window_end: u32,
	pub service_key: ed25519::Public,
	pub primary_signature: ed25519::Signature,
	pub primary_context_signature: ed25519::Signature,
	pub confirmations: Vec<ReplicaSignature<AccountId32>>,
}

impl CheckpointSubmissionInputV2 {
	fn call_args(&self) -> CallArgs {
		(
			self.domain.clone(),
			self.payload,
			self.window_start,
			self.window_end,
			self.service_key,
			self.primary_signature,
			self.primary_context_signature,
			self.confirmations.clone(),
		)
	}
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckpointSubmissionV2 {
	pub version: u8,
	pub submission_id: String,
	pub tuple_key: String,
	pub primary: String,
	pub domain: String,
	pub payload_scale: String,
	pub context_scale: String,
	pub window_start: u32,
	pub window_end: u32,
	pub service_key: String,
	pub primary_signature: String,
	pub primary_context_signature: String,
	pub confirmations_scale: String,
	pub call_args_scale: String,
	pub record_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckpointReceiptV2 {
	pub version: u8,
	pub submission_id: String,
	pub submission_record_hash: String,
	pub state: String,
	pub receipt_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckpointFinalizedReceiptV2 {
	pub version: u8,
	pub submission_id: String,
	pub tuple_key: String,
	pub submission_record_hash: String,
	pub primary: String,
	pub finalized_hash: String,
	pub finalized_number: u32,
	pub extrinsic_hash: String,
	pub state: String,
	pub receipt_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckpointOutboxSnapshotV2 {
	pub submission: CheckpointSubmissionV2,
	pub receipt: Option<CheckpointReceiptV2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckpointOutboxFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
}

pub(crate) struct CheckpointOutboxV2 {
	submissions_root: PathBuf,
	receipts_root: PathBuf,
	finalized_receipts_root: PathBuf,
	submissions: RwLock<HashMap<String, CheckpointSubmissionV2>>,
	by_tuple: RwLock<HashMap<String, String>>,
	receipts: RwLock<HashMap<String, CheckpointReceiptV2>>,
	finalized_receipts: RwLock<HashMap<String, CheckpointFinalizedReceiptV2>>,
	fault: RwLock<Option<CheckpointOutboxFault>>,
	poisoned: RwLock<bool>,
	#[cfg(test)]
	ack_gate:
		RwLock<Option<(std::sync::Arc<std::sync::Barrier>, std::sync::Arc<std::sync::Barrier>)>>,
}

impl CheckpointOutboxV2 {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let submissions_root = root.as_ref().join(SUBMISSIONS_ROOT);
		let receipts_root = root.as_ref().join(RECEIPTS_ROOT);
		let finalized_receipts_root = root.as_ref().join(FINALIZED_RECEIPTS_ROOT);
		fs::create_dir_all(&submissions_root).map_err(io_error)?;
		fs::create_dir_all(&receipts_root).map_err(io_error)?;
		fs::create_dir_all(&finalized_receipts_root).map_err(io_error)?;
		let mut submissions = HashMap::new();
		let mut by_tuple = HashMap::new();
		for item in read_records(&submissions_root)? {
			let record: CheckpointSubmissionV2 =
				serde_json::from_slice(&item.bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_submission(&record)?;
			if item.name != format!("{}.json", record.submission_id) ||
				submissions.len() >= MAX_RECORDS ||
				submissions.insert(record.submission_id.clone(), record.clone()).is_some() ||
				by_tuple
					.insert(record.tuple_key.clone(), record.submission_id.clone())
					.is_some()
			{
				return Err(ContentError::IntegrityFailed)
			}
		}
		let mut receipts = HashMap::new();
		for item in read_records(&receipts_root)? {
			let receipt: CheckpointReceiptV2 =
				serde_json::from_slice(&item.bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_receipt(&receipt)?;
			let submission =
				submissions.get(&receipt.submission_id).ok_or(ContentError::IntegrityFailed)?;
			if receipt.submission_record_hash != submission.record_hash ||
				item.name != format!("{}.json", receipt.submission_id) ||
				receipts.len() >= MAX_RECORDS ||
				receipts.insert(receipt.submission_id.clone(), receipt).is_some()
			{
				return Err(ContentError::IntegrityFailed)
			}
		}
		let mut finalized_receipts = HashMap::new();
		for item in read_records(&finalized_receipts_root)? {
			let receipt: CheckpointFinalizedReceiptV2 =
				serde_json::from_slice(&item.bytes).map_err(|_| ContentError::IntegrityFailed)?;
			let submission =
				submissions.get(&receipt.submission_id).ok_or(ContentError::IntegrityFailed)?;
			validate_finalized_receipt(&receipt, submission)?;
			if item.name != format!("{}.json", receipt.submission_id) ||
				finalized_receipts.len() >= MAX_RECORDS ||
				finalized_receipts.insert(receipt.submission_id.clone(), receipt).is_some()
			{
				return Err(ContentError::IntegrityFailed)
			}
		}
		Ok(Self {
			submissions_root,
			receipts_root,
			finalized_receipts_root,
			submissions: RwLock::new(submissions),
			by_tuple: RwLock::new(by_tuple),
			receipts: RwLock::new(receipts),
			finalized_receipts: RwLock::new(finalized_receipts),
			fault: RwLock::new(None),
			poisoned: RwLock::new(false),
			#[cfg(test)]
			ack_gate: RwLock::new(None),
		})
	}

	/// Enumerate durable submissions without a durable finality receipt.
	pub(crate) fn pending_submissions(&self) -> Result<Vec<CheckpointSubmissionV2>, ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		let submissions = self.submissions.read().map_err(|_| lock_error())?;
		let finalized = self.finalized_receipts.read().map_err(|_| lock_error())?;
		let mut pending = submissions
			.values()
			.filter(|submission| !finalized.contains_key(&submission.submission_id))
			.cloned()
			.collect::<Vec<_>>();
		pending.sort_by(|left, right| {
			left.tuple_key
				.cmp(&right.tuple_key)
				.then_with(|| left.submission_id.cmp(&right.submission_id))
		});
		Ok(pending)
	}

	pub(crate) fn finalized_receipt(
		&self,
		submission_id: &str,
	) -> Result<Option<CheckpointFinalizedReceiptV2>, ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		Ok(self
			.finalized_receipts
			.read()
			.map_err(|_| lock_error())?
			.get(submission_id)
			.cloned())
	}

	pub(crate) fn record_finalized(
		&self,
		submission_id: &str,
		finalized_hash: [u8; 32],
		finalized_number: u32,
		extrinsic_hash: [u8; 32],
	) -> Result<CheckpointFinalizedReceiptV2, ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		let submission = self
			.submissions
			.read()
			.map_err(|_| lock_error())?
			.get(submission_id)
			.cloned()
			.ok_or(ContentError::NotFound)?;
		let mut receipt = CheckpointFinalizedReceiptV2 {
			version: VERSION,
			submission_id: submission.submission_id.clone(),
			tuple_key: submission.tuple_key.clone(),
			submission_record_hash: submission.record_hash.clone(),
			primary: submission.primary.clone(),
			finalized_hash: hex::encode(finalized_hash),
			finalized_number,
			extrinsic_hash: hex::encode(extrinsic_hash),
			state: "finalized".into(),
			receipt_hash: String::new(),
		};
		receipt.receipt_hash = finalized_receipt_hash(&receipt)?;
		validate_finalized_receipt(&receipt, &submission)?;
		let mut finalized = self.finalized_receipts.write().map_err(|_| lock_error())?;
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		if let Some(existing) = finalized.get(submission_id) {
			return if existing == &receipt {
				Ok(existing.clone())
			} else {
				Err(ContentError::IdempotencyConflict)
			}
		}
		if finalized.len() >= MAX_RECORDS {
			return Err(ContentError::ProviderRecoveryTableFull)
		}
		if let Err(error) = self.persist_finalized_receipt(&receipt) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error)
		}
		finalized.insert(submission_id.into(), receipt.clone());
		Ok(receipt)
	}

	pub(crate) fn inject_fault_once(
		&self,
		fault: CheckpointOutboxFault,
	) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	pub(crate) fn enqueue(
		&self,
		input: &CheckpointSubmissionInputV2,
	) -> Result<CheckpointOutboxSnapshotV2, ContentError> {
		validate_input(input)?;
		let tuple = tuple_key(&input.payload);
		let id = submission_id(&input.primary, &input.call_args());
		{
			if *self.poisoned.read().map_err(|_| lock_error())? {
				return Err(ContentError::IntegrityFailed)
			}
			// Keep the same lock order as the insertion path.
			let submissions = self.submissions.read().map_err(|_| lock_error())?;
			let by_tuple = self.by_tuple.read().map_err(|_| lock_error())?;
			if let Some(existing_id) = by_tuple.get(&tuple) {
				let existing = submissions.get(existing_id).ok_or(ContentError::IntegrityFailed)?;
				return if existing.submission_id == id {
					self.snapshot(existing)
				} else {
					Err(ContentError::IdempotencyConflict)
				}
			}
		}
		let mut record = submission_record(input, id, tuple)?;
		record.record_hash = submission_record_hash(&record)?;
		let mut submissions = self.submissions.write().map_err(|_| lock_error())?;
		let mut by_tuple = self.by_tuple.write().map_err(|_| lock_error())?;
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		if let Some(existing_id) = by_tuple.get(&record.tuple_key) {
			let existing = submissions.get(existing_id).ok_or(ContentError::IntegrityFailed)?;
			return if existing.submission_id == record.submission_id {
				self.snapshot(existing)
			} else {
				Err(ContentError::IdempotencyConflict)
			}
		}
		if submissions.len() >= MAX_RECORDS {
			return Err(ContentError::ProviderRecoveryTableFull)
		}
		if let Err(error) = self.persist_submission(&record) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error)
		}
		by_tuple.insert(record.tuple_key.clone(), record.submission_id.clone());
		submissions.insert(record.submission_id.clone(), record.clone());
		self.snapshot(&record)
	}

	pub(crate) fn acknowledge_enqueue(
		&self,
		submission_id: &str,
	) -> Result<CheckpointOutboxSnapshotV2, ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		let submission = self
			.submissions
			.read()
			.map_err(|_| lock_error())?
			.get(submission_id)
			.cloned()
			.ok_or(ContentError::NotFound)?;
		{
			let receipts = self.receipts.read().map_err(|_| lock_error())?;
			if let Some(receipt) = receipts.get(submission_id) {
				return Ok(CheckpointOutboxSnapshotV2 { submission, receipt: Some(receipt.clone()) })
			}
		}
		let mut receipt = CheckpointReceiptV2 {
			version: VERSION,
			submission_id: submission.submission_id.clone(),
			submission_record_hash: submission.record_hash.clone(),
			state: "durably_enqueued".into(),
			receipt_hash: String::new(),
		};
		receipt.receipt_hash = receipt_hash(&receipt)?;
		#[cfg(test)]
		if let Some((arrived, release)) = self.ack_gate.read().map_err(|_| lock_error())?.clone() {
			arrived.wait();
			release.wait();
		}
		let mut receipts = self.receipts.write().map_err(|_| lock_error())?;
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		if let Some(existing) = receipts.get(submission_id) {
			return Ok(CheckpointOutboxSnapshotV2 { submission, receipt: Some(existing.clone()) })
		}
		if receipts.len() >= MAX_RECORDS {
			return Err(ContentError::ProviderRecoveryTableFull)
		}
		if let Err(error) = self.persist_receipt(&receipt) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error)
		}
		receipts.insert(submission_id.into(), receipt.clone());
		Ok(CheckpointOutboxSnapshotV2 { submission, receipt: Some(receipt) })
	}

	fn snapshot(
		&self,
		submission: &CheckpointSubmissionV2,
	) -> Result<CheckpointOutboxSnapshotV2, ContentError> {
		validate_submission(submission)?;
		let receipt = self
			.receipts
			.read()
			.map_err(|_| lock_error())?
			.get(&submission.submission_id)
			.cloned();
		Ok(CheckpointOutboxSnapshotV2 { submission: submission.clone(), receipt })
	}

	fn persist_submission(&self, record: &CheckpointSubmissionV2) -> Result<(), ContentError> {
		validate_submission(record)?;
		self.persist(&self.submissions_root, &record.submission_id, record)
	}

	fn persist_receipt(&self, receipt: &CheckpointReceiptV2) -> Result<(), ContentError> {
		validate_receipt(receipt)?;
		self.persist(&self.receipts_root, &receipt.submission_id, receipt)
	}

	fn persist_finalized_receipt(
		&self,
		receipt: &CheckpointFinalizedReceiptV2,
	) -> Result<(), ContentError> {
		self.persist(&self.finalized_receipts_root, &receipt.submission_id, receipt)
	}

	fn persist<T: Serialize>(&self, root: &Path, key: &str, value: &T) -> Result<(), ContentError> {
		let bytes = serde_json::to_vec(value).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed)
		}
		let temp = root.join(format!("{key}.json.tmp-{}", std::process::id()));
		let mut file = File::create(&temp).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		self.trip(CheckpointOutboxFault::BeforeTempFsync)?;
		file.sync_all().map_err(io_error)?;
		self.trip(CheckpointOutboxFault::AfterTempFsync)?;
		fs::rename(&temp, root.join(format!("{key}.json"))).map_err(io_error)?;
		self.trip(CheckpointOutboxFault::AfterRename)?;
		File::open(root).and_then(|directory| directory.sync_all()).map_err(io_error)?;
		self.trip(CheckpointOutboxFault::AfterDirectoryFsync)
	}

	fn trip(&self, point: CheckpointOutboxFault) -> Result<(), ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			Err(ContentError::Io(format!("injected checkpoint outbox fault: {point:?}")))
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
		if !name.ends_with(".json") || !item.file_type().map_err(io_error)?.is_file() {
			return Err(ContentError::IntegrityFailed)
		}
		if records.len() >= MAX_RECORDS {
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

fn submission_record(
	input: &CheckpointSubmissionInputV2,
	submission_id: String,
	tuple_key: String,
) -> Result<CheckpointSubmissionV2, ContentError> {
	let args = input.call_args();
	Ok(CheckpointSubmissionV2 {
		version: VERSION,
		submission_id,
		tuple_key,
		primary: account_hex(&input.primary),
		domain: hex::encode(&input.domain),
		payload_scale: hex::encode(input.payload.encode()),
		context_scale: hex::encode(input.context.encode()),
		window_start: input.window_start,
		window_end: input.window_end,
		service_key: hex::encode(input.service_key.0),
		primary_signature: hex::encode(input.primary_signature.0),
		primary_context_signature: hex::encode(input.primary_context_signature.0),
		confirmations_scale: hex::encode(input.confirmations.encode()),
		call_args_scale: hex::encode(args.encode()),
		record_hash: String::new(),
	})
}

pub(super) fn validate_submission(record: &CheckpointSubmissionV2) -> Result<(), ContentError> {
	if record.version != VERSION ||
		record.record_hash != submission_record_hash(record)? ||
		record.submission_id.len() != 64 ||
		record.tuple_key.len() != 64
	{
		return Err(ContentError::IntegrityFailed)
	}
	let primary = decode_account(&record.primary)?;
	let domain = decode_hex(&record.domain)?;
	let payload = decode_scale::<CommitmentPayloadV2<H256, u32>>(&record.payload_scale)?;
	let context = decode_scale::<CheckpointContextV1<H256>>(&record.context_scale)?;
	let service_key = ed25519::Public::from_raw(decode_fixed(&record.service_key)?);
	let primary_signature = ed25519::Signature::from_raw(decode_fixed(&record.primary_signature)?);
	let primary_context_signature =
		ed25519::Signature::from_raw(decode_fixed(&record.primary_context_signature)?);
	let confirmations =
		decode_scale::<Vec<ReplicaSignature<AccountId32>>>(&record.confirmations_scale)?;
	let input = CheckpointSubmissionInputV2 {
		primary,
		domain,
		payload,
		context,
		window_start: record.window_start,
		window_end: record.window_end,
		service_key,
		primary_signature,
		primary_context_signature,
		confirmations,
	};
	validate_input(&input)?;
	let args = input.call_args();
	if hex::encode(args.encode()) != record.call_args_scale ||
		record.submission_id != submission_id(&input.primary, &args) ||
		record.tuple_key != tuple_key(&input.payload)
	{
		return Err(ContentError::IntegrityFailed)
	}
	Ok(())
}

fn validate_input(input: &CheckpointSubmissionInputV2) -> Result<(), ContentError> {
	if input.domain != CHECKPOINT_DOMAIN ||
		input.payload.version != 2 ||
		input.payload.commitment.leaf_count == 0 ||
		input.payload.commitment.range_end().is_none() ||
		input.window_start > input.window_end ||
		input.context.version != 1 ||
		input.context.v2_digest != checkpoint_digest(&input.payload) ||
		input.confirmations.len() != 2
	{
		return Err(ContentError::IntegrityFailed)
	}
	let payload_digest = checkpoint_digest(&input.payload);
	let context_digest = checkpoint_context_digest(&input.context);
	if !ed25519::Pair::verify(&input.primary_signature, &payload_digest, &input.service_key) ||
		!ed25519::Pair::verify(
			&input.primary_context_signature,
			&context_digest,
			&input.service_key,
		) {
		return Err(ContentError::IntegrityFailed)
	}
	let mut previous: Option<Vec<u8>> = None;
	for confirmation in &input.confirmations {
		let provider = confirmation.provider.encode();
		if confirmation.provider == input.primary ||
			previous.as_ref().is_some_and(|value| value >= &provider) ||
			!ed25519::Pair::verify(
				&confirmation.signature,
				&payload_digest,
				&confirmation.service_key,
			) || !ed25519::Pair::verify(
			&confirmation.context_signature,
			&context_digest,
			&confirmation.service_key,
		) {
			return Err(ContentError::IntegrityFailed)
		}
		previous = Some(provider);
	}
	Ok(())
}

fn submission_id(primary: &AccountId32, args: &CallArgs) -> String {
	let mut input = ID_DOMAIN.to_vec();
	primary.encode_to(&mut input);
	args.encode_to(&mut input);
	hex::encode(blake2_256(&input))
}

fn tuple_key(payload: &CommitmentPayloadV2<H256, u32>) -> String {
	let mut input = b"cord/provider/checkpoint-submission-tuple/v2".to_vec();
	payload.bucket_id.encode_to(&mut input);
	payload.nonce.encode_to(&mut input);
	payload.commitment.start_seq.encode_to(&mut input);
	hex::encode(blake2_256(&input))
}

pub(super) fn submission_record_hash(
	record: &CheckpointSubmissionV2,
) -> Result<String, ContentError> {
	let mut canonical = record.clone();
	canonical.record_hash.clear();
	let mut input = RECORD_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn validate_receipt(receipt: &CheckpointReceiptV2) -> Result<(), ContentError> {
	if receipt.version != VERSION ||
		receipt.state != "durably_enqueued" ||
		receipt.submission_id.len() != 64 ||
		receipt.submission_record_hash.len() != 64 ||
		receipt.receipt_hash != receipt_hash(receipt)?
	{
		return Err(ContentError::IntegrityFailed)
	}
	decode_hex(&receipt.submission_id)?;
	decode_hex(&receipt.submission_record_hash)?;
	Ok(())
}

fn receipt_hash(receipt: &CheckpointReceiptV2) -> Result<String, ContentError> {
	let mut canonical = receipt.clone();
	canonical.receipt_hash.clear();
	let mut input = RECEIPT_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn validate_finalized_receipt(
	receipt: &CheckpointFinalizedReceiptV2,
	submission: &CheckpointSubmissionV2,
) -> Result<(), ContentError> {
	if receipt.version != VERSION ||
		receipt.state != "finalized" ||
		receipt.submission_id != submission.submission_id ||
		receipt.tuple_key != submission.tuple_key ||
		receipt.submission_record_hash != submission.record_hash ||
		receipt.primary != submission.primary ||
		receipt.finalized_hash.len() != 64 ||
		receipt.extrinsic_hash.len() != 64 ||
		receipt.receipt_hash != finalized_receipt_hash(receipt)?
	{
		return Err(ContentError::IntegrityFailed)
	}
	decode_fixed::<32>(&receipt.finalized_hash)?;
	decode_fixed::<32>(&receipt.extrinsic_hash)?;
	Ok(())
}

fn finalized_receipt_hash(receipt: &CheckpointFinalizedReceiptV2) -> Result<String, ContentError> {
	let mut canonical = receipt.clone();
	canonical.receipt_hash.clear();
	let mut input = FINALIZED_RECEIPT_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn decode_scale<T: Decode + Encode>(value: &str) -> Result<T, ContentError> {
	let bytes = decode_hex(value)?;
	let mut input = &bytes[..];
	let decoded = T::decode(&mut input).map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || decoded.encode() != bytes {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(decoded)
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
	ContentError::Io("checkpoint v2 outbox lock poisoned".into())
}

#[cfg(test)]
mod tests {
	use pallet_orbis_storage_provider::CommitmentV1;
	use tempfile::TempDir;

	use super::*;

	fn pair(seed: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[seed; 32])
	}

	fn account(seed: u8) -> AccountId32 {
		AccountId32::new([seed; 32])
	}

	fn fixture() -> CheckpointSubmissionInputV2 {
		let payload = CommitmentPayloadV2 {
			version: 2,
			bucket_id: H256::repeat_byte(4),
			commitment: CommitmentV1 {
				mmr_root: H256::repeat_byte(5),
				start_seq: 7,
				leaf_count: 3,
			},
			nonce: 100,
		};
		let context = CheckpointContextV1 {
			version: 1,
			genesis_hash: H256::repeat_byte(10),
			spec_version: 11,
			transaction_version: 12,
			metadata_hash: H256::repeat_byte(13),
			finalized_hash: H256::repeat_byte(14),
			duty_id: H256::repeat_byte(15),
			v2_digest: checkpoint_digest(&payload),
		};
		let mut input = CheckpointSubmissionInputV2 {
			primary: account(1),
			domain: CHECKPOINT_DOMAIN.to_vec(),
			payload,
			context,
			window_start: 100,
			window_end: 110,
			service_key: pair(1).public(),
			primary_signature: ed25519::Signature::from_raw([0; 64]),
			primary_context_signature: ed25519::Signature::from_raw([0; 64]),
			confirmations: [2, 3]
				.into_iter()
				.map(|seed| ReplicaSignature {
					provider: account(seed),
					service_key: pair(seed).public(),
					signature: ed25519::Signature::from_raw([0; 64]),
					context_signature: ed25519::Signature::from_raw([0; 64]),
				})
				.collect(),
		};
		resign(&mut input);
		input
	}

	fn resign(input: &mut CheckpointSubmissionInputV2) {
		input.context.v2_digest = checkpoint_digest(&input.payload);
		input.primary_signature = pair(1).sign(&checkpoint_digest(&input.payload));
		input.primary_context_signature = pair(1).sign(&checkpoint_context_digest(&input.context));
		for confirmation in &mut input.confirmations {
			let seed = <AccountId32 as AsRef<[u8]>>::as_ref(&confirmation.provider)[0];
			confirmation.service_key = pair(seed).public();
			confirmation.signature = pair(seed).sign(&checkpoint_digest(&input.payload));
			confirmation.context_signature =
				pair(seed).sign(&checkpoint_context_digest(&input.context));
		}
	}

	fn record_path(temp: &TempDir, id: &str) -> PathBuf {
		temp.path().join(SUBMISSIONS_ROOT).join(format!("{id}.json"))
	}

	fn receipt_path(temp: &TempDir, id: &str) -> PathBuf {
		temp.path().join(RECEIPTS_ROOT).join(format!("{id}.json"))
	}

	fn finalized_receipt_path(temp: &TempDir, id: &str) -> PathBuf {
		temp.path().join(FINALIZED_RECEIPTS_ROOT).join(format!("{id}.json"))
	}

	#[test]
	fn exact_call_args_receipt_and_reopen_are_stable() {
		let temp = TempDir::new().unwrap();
		let input = fixture();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let first = outbox.enqueue(&input).unwrap();
		assert_eq!(first.receipt, None);
		assert_eq!(
			decode_scale::<CallArgs>(&first.submission.call_args_scale).unwrap(),
			input.call_args()
		);
		assert_eq!(outbox.enqueue(&input).unwrap(), first);

		let acknowledged = outbox.acknowledge_enqueue(&first.submission.submission_id).unwrap();
		assert_eq!(
			acknowledged.receipt.as_ref().unwrap().submission_record_hash,
			first.submission.record_hash
		);
		assert_eq!(
			outbox.acknowledge_enqueue(&first.submission.submission_id).unwrap(),
			acknowledged
		);
		assert!(record_path(&temp, &first.submission.submission_id).is_file());
		assert!(receipt_path(&temp, &first.submission.submission_id).is_file());

		drop(outbox);
		let reopened = CheckpointOutboxV2::open(temp.path()).unwrap();
		assert_eq!(reopened.enqueue(&input).unwrap(), acknowledged);
	}

	#[test]
	fn duplicates_conflicts_and_noncanonical_quorums_fail_closed() {
		let temp = TempDir::new().unwrap();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let input = fixture();
		let first = outbox.enqueue(&input).unwrap();
		assert_eq!(outbox.enqueue(&input).unwrap(), first);

		let mut changed = input.clone();
		changed.payload.commitment.mmr_root = H256::repeat_byte(99);
		resign(&mut changed);
		assert!(matches!(outbox.enqueue(&changed), Err(ContentError::IdempotencyConflict)));

		let mut swapped = input.clone();
		swapped.confirmations.swap(0, 1);
		assert!(matches!(outbox.enqueue(&swapped), Err(ContentError::IntegrityFailed)));
		let mut duplicate = input.clone();
		duplicate.confirmations[1] = duplicate.confirmations[0].clone();
		assert!(matches!(outbox.enqueue(&duplicate), Err(ContentError::IntegrityFailed)));
		let mut self_confirmation = input.clone();
		self_confirmation.confirmations[0].provider = input.primary.clone();
		assert!(matches!(outbox.enqueue(&self_confirmation), Err(ContentError::IntegrityFailed)));
		let mut wrong_domain = input.clone();
		wrong_domain.domain.push(0);
		assert!(matches!(outbox.enqueue(&wrong_domain), Err(ContentError::IntegrityFailed)));
	}

	#[test]
	fn retries_precede_capacity_and_recomputed_tampering_is_rejected() {
		let temp = TempDir::new().unwrap();
		let input = fixture();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let first = outbox.enqueue(&input).unwrap();
		{
			let mut records = outbox.submissions.write().unwrap();
			for index in records.len()..MAX_RECORDS {
				let mut dummy = first.submission.clone();
				dummy.submission_id = format!("{index:064x}");
				records.insert(dummy.submission_id.clone(), dummy);
			}
		}
		assert_eq!(outbox.enqueue(&input).unwrap(), first);
		let mut next = input.clone();
		next.payload.nonce += 1;
		resign(&mut next);
		assert!(matches!(outbox.enqueue(&next), Err(ContentError::ProviderRecoveryTableFull)));
		drop(outbox);

		let path = record_path(&temp, &first.submission.submission_id);
		let mut record: CheckpointSubmissionV2 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		let mut context = decode_scale::<CheckpointContextV1<H256>>(&record.context_scale).unwrap();
		context.duty_id = H256::repeat_byte(88);
		record.context_scale = hex::encode(context.encode());
		record.record_hash = submission_record_hash(&record).unwrap();
		fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(matches!(
			CheckpointOutboxV2::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn receipt_binding_tampering_is_rejected_even_with_a_recomputed_hash() {
		let temp = TempDir::new().unwrap();
		let input = fixture();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let first = outbox.enqueue(&input).unwrap();
		outbox.acknowledge_enqueue(&first.submission.submission_id).unwrap();
		drop(outbox);

		let path = receipt_path(&temp, &first.submission.submission_id);
		let mut receipt: CheckpointReceiptV2 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		receipt.submission_record_hash = hex::encode([91; 32]);
		receipt.receipt_hash = receipt_hash(&receipt).unwrap();
		fs::write(path, serde_json::to_vec(&receipt).unwrap()).unwrap();
		assert!(matches!(
			CheckpointOutboxV2::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn submission_and_receipt_crash_seams_recover_to_old_or_new_state() {
		let faults = [
			CheckpointOutboxFault::BeforeTempFsync,
			CheckpointOutboxFault::AfterTempFsync,
			CheckpointOutboxFault::AfterRename,
			CheckpointOutboxFault::AfterDirectoryFsync,
		];
		for fault in faults {
			let temp = TempDir::new().unwrap();
			let input = fixture();
			let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
			outbox.inject_fault_once(fault).unwrap();
			assert!(matches!(outbox.enqueue(&input), Err(ContentError::Io(_))));
			assert!(matches!(outbox.enqueue(&input), Err(ContentError::IntegrityFailed)));
			drop(outbox);
			let reopened = CheckpointOutboxV2::open(temp.path()).unwrap();
			let recovered = reopened.enqueue(&input).unwrap();

			reopened.inject_fault_once(fault).unwrap();
			assert!(matches!(
				reopened.acknowledge_enqueue(&recovered.submission.submission_id),
				Err(ContentError::Io(_))
			));
			assert!(matches!(
				reopened.acknowledge_enqueue(&recovered.submission.submission_id),
				Err(ContentError::IntegrityFailed)
			));
			drop(reopened);
			let reopened = CheckpointOutboxV2::open(temp.path()).unwrap();
			assert!(reopened
				.acknowledge_enqueue(&recovered.submission.submission_id)
				.unwrap()
				.receipt
				.is_some());
		}
	}

	#[test]
	fn recovery_rejects_more_than_the_hard_on_disk_record_bound_in_either_root() {
		for root_name in [SUBMISSIONS_ROOT, RECEIPTS_ROOT, FINALIZED_RECEIPTS_ROOT] {
			let temp = TempDir::new().unwrap();
			let root = temp.path().join(root_name);
			fs::create_dir_all(&root).unwrap();
			for index in 0..=MAX_RECORDS {
				fs::write(root.join(format!("{index:064x}.json")), []).unwrap();
			}
			assert!(matches!(read_records(&root), Err(ContentError::IntegrityFailed)));

			let temp = TempDir::new().unwrap();
			let root = temp.path().join(root_name);
			fs::create_dir_all(&root).unwrap();
			fs::write(root.join("first.json.tmp-1"), []).unwrap();
			fs::write(root.join("second.json.tmp-1"), []).unwrap();
			assert!(matches!(read_records(&root), Err(ContentError::IntegrityFailed)));
		}
	}

	#[test]
	fn acknowledgement_waiter_rechecks_poison_before_persisting() {
		let temp = TempDir::new().unwrap();
		let outbox = std::sync::Arc::new(CheckpointOutboxV2::open(temp.path()).unwrap());
		let submission = outbox.enqueue(&fixture()).unwrap().submission;
		let arrived = std::sync::Arc::new(std::sync::Barrier::new(2));
		let release = std::sync::Arc::new(std::sync::Barrier::new(2));
		*outbox.ack_gate.write().unwrap() = Some((arrived.clone(), release.clone()));

		let waiter = outbox.clone();
		let id = submission.submission_id.clone();
		let handle = std::thread::spawn(move || waiter.acknowledge_enqueue(&id));
		arrived.wait();
		*outbox.poisoned.write().unwrap() = true;
		release.wait();

		assert!(matches!(handle.join().unwrap(), Err(ContentError::IntegrityFailed)));
		assert!(!receipt_path(&temp, &submission.submission_id).exists());
	}

	#[test]
	fn pending_is_sorted_and_exact_finality_receipts_survive_reopen() {
		let temp = TempDir::new().unwrap();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		for nonce in [103, 101, 102] {
			let mut input = fixture();
			input.payload.nonce = nonce;
			resign(&mut input);
			outbox.enqueue(&input).unwrap();
		}
		let pending = outbox.pending_submissions().unwrap();
		let mut expected = pending.clone();
		expected.sort_by(|left, right| {
			left.tuple_key
				.cmp(&right.tuple_key)
				.then_with(|| left.submission_id.cmp(&right.submission_id))
		});
		assert_eq!(pending, expected);

		let submission = pending[0].clone();
		let receipt = outbox
			.record_finalized(&submission.submission_id, [8; 32], 44, [9; 32])
			.unwrap();
		assert_eq!(
			outbox
				.record_finalized(&submission.submission_id, [8; 32], 44, [9; 32])
				.unwrap(),
			receipt
		);
		assert!(matches!(
			outbox.record_finalized(&submission.submission_id, [7; 32], 44, [9; 32]),
			Err(ContentError::IdempotencyConflict)
		));
		assert!(!outbox
			.pending_submissions()
			.unwrap()
			.iter()
			.any(|item| item.submission_id == submission.submission_id));
		drop(outbox);

		let reopened = CheckpointOutboxV2::open(temp.path()).unwrap();
		assert_eq!(reopened.finalized_receipt(&submission.submission_id).unwrap(), Some(receipt));
	}

	#[test]
	fn finalized_receipt_capacity_and_recomputed_binding_tampering_fail_closed() {
		let temp = TempDir::new().unwrap();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let first = outbox.enqueue(&fixture()).unwrap().submission;
		let receipt = outbox.record_finalized(&first.submission_id, [1; 32], 2, [3; 32]).unwrap();
		{
			let mut receipts = outbox.finalized_receipts.write().unwrap();
			for index in receipts.len()..MAX_RECORDS {
				let mut dummy = receipt.clone();
				dummy.submission_id = format!("{index:064x}");
				receipts.insert(dummy.submission_id.clone(), dummy);
			}
		}
		assert_eq!(
			outbox.record_finalized(&first.submission_id, [1; 32], 2, [3; 32]).unwrap(),
			receipt
		);
		let mut next = fixture();
		next.payload.nonce += 1;
		resign(&mut next);
		let next = outbox.enqueue(&next).unwrap().submission;
		assert!(matches!(
			outbox.record_finalized(&next.submission_id, [1; 32], 2, [3; 32]),
			Err(ContentError::ProviderRecoveryTableFull)
		));
		drop(outbox);

		let path = finalized_receipt_path(&temp, &first.submission_id);
		let mut tampered: CheckpointFinalizedReceiptV2 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		tampered.primary = hex::encode([99; 32]);
		tampered.receipt_hash = finalized_receipt_hash(&tampered).unwrap();
		fs::write(path, serde_json::to_vec(&tampered).unwrap()).unwrap();
		assert!(matches!(
			CheckpointOutboxV2::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn finalized_receipt_crash_seams_replay_exactly_from_durable_state() {
		for fault in [
			CheckpointOutboxFault::BeforeTempFsync,
			CheckpointOutboxFault::AfterTempFsync,
			CheckpointOutboxFault::AfterRename,
			CheckpointOutboxFault::AfterDirectoryFsync,
		] {
			let temp = TempDir::new().unwrap();
			let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
			let submission = outbox.enqueue(&fixture()).unwrap().submission;
			outbox.inject_fault_once(fault).unwrap();
			assert!(matches!(
				outbox.record_finalized(&submission.submission_id, [4; 32], 5, [6; 32]),
				Err(ContentError::Io(_))
			));
			assert!(matches!(outbox.pending_submissions(), Err(ContentError::IntegrityFailed)));
			drop(outbox);

			let reopened = CheckpointOutboxV2::open(temp.path()).unwrap();
			let recovered = reopened
				.record_finalized(&submission.submission_id, [4; 32], 5, [6; 32])
				.unwrap();
			assert_eq!(recovered.finalized_hash, hex::encode([4; 32]));
			assert!(reopened.pending_submissions().unwrap().is_empty());
		}
	}
}
