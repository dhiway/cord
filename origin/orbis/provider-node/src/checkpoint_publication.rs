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

//! Private finalized-state reconciliation for checkpoint publication.

use std::{
	collections::HashMap,
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
};

use codec::{Decode, Encode};
use orbis_storage_runtime_api::{CheckpointInfo, Versioned, RESPONSE_VERSION};
use pallet_orbis_storage_provider::{CommitmentPayloadV2, ReplicaSignature};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, H256};
use sp_crypto_hashing::blake2_256;

use super::checkpoint_outbox::{validate_submission, CheckpointSubmissionV2};
use crate::ContentError;

const ROOT: &str = "checkpoint-publications-v1";
const VERSION: u8 = 1;
const STATE: &str = "published";
const RECORD_DOMAIN: &[u8] = b"cord/provider/checkpoint-publication-record/v1";
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_RECORD_BYTES: usize = 256 * 1024;
const MAX_RECORDS: usize = 8_192;
const MAX_TEMP_ARTIFACTS: usize = 1;

type CheckpointResponse = Versioned<CheckpointInfo<AccountId32, H256, u32>>;

pub(crate) fn submission_bucket_id(
	submission: &CheckpointSubmissionV2,
) -> Result<[u8; 32], ContentError> {
	validate_submission(submission)?;
	let payload = decode_hex_scale::<CommitmentPayloadV2<H256, u32>>(&submission.payload_scale)?;
	Ok(payload.bucket_id.0)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FinalizedCheckpointPublicationInputV1 {
	pub submission: CheckpointSubmissionV2,
	pub finalized_hash: H256,
	pub finalized_number: u32,
	pub response_scale: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublishedCheckpointV1 {
	pub version: u8,
	pub submission_id: String,
	pub tuple_key: String,
	pub submission_record_hash: String,
	pub submission: CheckpointSubmissionV2,
	pub finalized_hash: String,
	pub finalized_number: u32,
	pub response_scale: String,
	pub state: String,
	pub record_hash: String,
}

impl PublishedCheckpointV1 {
	pub(crate) fn is_publishable(&self) -> bool {
		self.state == STATE
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckpointPublicationFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
}

pub(crate) struct CheckpointPublicationStoreV1 {
	root: PathBuf,
	records: RwLock<HashMap<String, PublishedCheckpointV1>>,
	by_tuple: RwLock<HashMap<String, String>>,
	fault: RwLock<Option<CheckpointPublicationFault>>,
	poisoned: RwLock<bool>,
}

impl CheckpointPublicationStoreV1 {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let root = root.as_ref().join(ROOT);
		fs::create_dir_all(&root).map_err(io_error)?;
		let mut records = HashMap::new();
		let mut by_tuple = HashMap::new();
		for item in read_records(&root)? {
			let record: PublishedCheckpointV1 =
				serde_json::from_slice(&item.bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_record(&record)?;
			if item.name != format!("{}.json", record.submission_id) ||
				records.insert(record.submission_id.clone(), record.clone()).is_some() ||
				by_tuple
					.insert(record.tuple_key.clone(), record.submission_id.clone())
					.is_some()
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
		fault: CheckpointPublicationFault,
	) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	pub(crate) fn contains(&self, submission_id: &str) -> Result<bool, ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(self.records.read().map_err(|_| lock_error())?.contains_key(submission_id))
	}

	pub(crate) fn records(&self) -> Result<Vec<PublishedCheckpointV1>, ContentError> {
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed);
		}
		let mut records = self
			.records
			.read()
			.map_err(|_| lock_error())?
			.values()
			.cloned()
			.collect::<Vec<_>>();
		records.sort_by(|left, right| left.submission_id.cmp(&right.submission_id));
		Ok(records)
	}

	pub(crate) fn publish(
		&self,
		input: &FinalizedCheckpointPublicationInputV1,
	) -> Result<PublishedCheckpointV1, ContentError> {
		let candidate = publication_record(input)?;
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
		by_tuple.insert(candidate.tuple_key.clone(), candidate.submission_id.clone());
		records.insert(candidate.submission_id.clone(), candidate.clone());
		Ok(candidate)
	}

	fn persist(&self, record: &PublishedCheckpointV1) -> Result<(), ContentError> {
		validate_record(record)?;
		let bytes = serde_json::to_vec(record).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed)
		}
		let key = &record.submission_id;
		let temp = self.root.join(format!("{key}.json.tmp-{}", std::process::id()));
		let mut file = File::create(&temp).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		self.trip(CheckpointPublicationFault::BeforeTempFsync)?;
		file.sync_all().map_err(io_error)?;
		self.trip(CheckpointPublicationFault::AfterTempFsync)?;
		fs::rename(&temp, self.root.join(format!("{key}.json"))).map_err(io_error)?;
		self.trip(CheckpointPublicationFault::AfterRename)?;
		File::open(&self.root)
			.and_then(|directory| directory.sync_all())
			.map_err(io_error)?;
		self.trip(CheckpointPublicationFault::AfterDirectoryFsync)
	}

	fn trip(&self, point: CheckpointPublicationFault) -> Result<(), ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			Err(ContentError::Io(format!("injected checkpoint publication fault: {point:?}")))
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

fn publication_record(
	input: &FinalizedCheckpointPublicationInputV1,
) -> Result<PublishedCheckpointV1, ContentError> {
	validate_submission(&input.submission)?;
	validate_observation(&input.submission, input.finalized_number, &input.response_scale)?;
	let mut record = PublishedCheckpointV1 {
		version: VERSION,
		submission_id: input.submission.submission_id.clone(),
		tuple_key: input.submission.tuple_key.clone(),
		submission_record_hash: input.submission.record_hash.clone(),
		submission: input.submission.clone(),
		finalized_hash: hex::encode(input.finalized_hash.as_bytes()),
		finalized_number: input.finalized_number,
		response_scale: hex::encode(&input.response_scale),
		state: STATE.into(),
		record_hash: String::new(),
	};
	record.record_hash = record_hash(&record)?;
	validate_record(&record)?;
	Ok(record)
}

fn validate_record(record: &PublishedCheckpointV1) -> Result<(), ContentError> {
	if record.version != VERSION ||
		record.state != STATE ||
		record.submission_id != record.submission.submission_id ||
		record.tuple_key != record.submission.tuple_key ||
		record.submission_record_hash != record.submission.record_hash ||
		record.record_hash != record_hash(record)?
	{
		return Err(ContentError::IntegrityFailed)
	}
	validate_submission(&record.submission)?;
	let _: [u8; 32] = decode_hex(&record.finalized_hash)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)?;
	let response = decode_hex(&record.response_scale)?;
	validate_observation(&record.submission, record.finalized_number, &response)
}

fn validate_observation(
	submission: &CheckpointSubmissionV2,
	finalized_number: u32,
	response_scale: &[u8],
) -> Result<(), ContentError> {
	if response_scale.len() > MAX_RESPONSE_BYTES {
		return Err(ContentError::IntegrityFailed)
	}
	let response = decode_scale::<CheckpointResponse>(response_scale)?;
	if response.version != RESPONSE_VERSION {
		return Err(ContentError::IntegrityFailed)
	}
	let observed = response.value.ok_or(ContentError::IntegrityFailed)?;
	let payload = decode_hex_scale::<CommitmentPayloadV2<H256, u32>>(&submission.payload_scale)?;
	let confirmations =
		decode_hex_scale::<Vec<ReplicaSignature<AccountId32>>>(&submission.confirmations_scale)?;
	let expected_providers: Vec<_> =
		confirmations.into_iter().map(|confirmation| confirmation.provider).collect();
	if observed.bucket_id != payload.bucket_id ||
		observed.commitment.mmr_root != payload.commitment.mmr_root ||
		observed.commitment.start_seq != payload.commitment.start_seq ||
		observed.commitment.leaf_count != payload.commitment.leaf_count ||
		observed.commitment_nonce != payload.nonce ||
		observed.primary_signers != 1 ||
		observed.replica_confirmations.len() != 2 ||
		expected_providers.len() != 2 ||
		observed.replica_confirmations != expected_providers ||
		observed.checkpoint_block < payload.nonce ||
		finalized_number < observed.checkpoint_block
	{
		return Err(ContentError::IntegrityFailed)
	}
	let first = observed.replica_confirmations[0].encode();
	let second = observed.replica_confirmations[1].encode();
	if first >= second {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(())
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

fn record_hash(record: &PublishedCheckpointV1) -> Result<String, ContentError> {
	let mut canonical = record.clone();
	canonical.record_hash.clear();
	let mut input = RECORD_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

fn lock_error() -> ContentError {
	ContentError::Io("checkpoint publication lock poisoned".into())
}

#[cfg(test)]
mod tests {
	use orbis_storage_runtime_api::CommitmentInfo;
	use pallet_orbis_storage_provider::{CheckpointContextV1, CommitmentV1};
	use sp_core::{ed25519, Pair as _};
	use tempfile::TempDir;

	use super::*;
	use crate::checkpoint::{
		checkpoint_outbox::{CheckpointOutboxV2, CheckpointSubmissionInputV2},
		checkpoint_quorum::{checkpoint_context_digest, checkpoint_digest},
	};

	fn pair(seed: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[seed; 32])
	}

	fn account(seed: u8) -> AccountId32 {
		AccountId32::new([seed; 32])
	}

	fn submission_input() -> CheckpointSubmissionInputV2 {
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
			domain: b"cord/storage/checkpoint/v2".to_vec(),
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

	fn durable_submission(
		input: &CheckpointSubmissionInputV2,
	) -> (TempDir, CheckpointSubmissionV2) {
		let temp = TempDir::new().unwrap();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let submission = outbox.enqueue(input).unwrap().submission;
		(temp, submission)
	}

	fn publication_input(
		submission: CheckpointSubmissionV2,
	) -> FinalizedCheckpointPublicationInputV1 {
		let payload =
			decode_hex_scale::<CommitmentPayloadV2<H256, u32>>(&submission.payload_scale).unwrap();
		let confirmations =
			decode_hex_scale::<Vec<ReplicaSignature<AccountId32>>>(&submission.confirmations_scale)
				.unwrap();
		let response = CheckpointResponse {
			version: RESPONSE_VERSION,
			value: Some(CheckpointInfo {
				bucket_id: payload.bucket_id,
				commitment: CommitmentInfo {
					mmr_root: payload.commitment.mmr_root,
					start_seq: payload.commitment.start_seq,
					leaf_count: payload.commitment.leaf_count,
				},
				checkpoint_block: 115,
				primary_signers: 1,
				commitment_nonce: payload.nonce,
				replica_confirmations: confirmations
					.into_iter()
					.map(|confirmation| confirmation.provider)
					.collect(),
			}),
		};
		FinalizedCheckpointPublicationInputV1 {
			submission,
			finalized_hash: H256::repeat_byte(22),
			finalized_number: 120,
			response_scale: response.encode(),
		}
	}

	fn response(input: &FinalizedCheckpointPublicationInputV1) -> CheckpointResponse {
		decode_scale(&input.response_scale).unwrap()
	}

	fn replace_response(
		input: &mut FinalizedCheckpointPublicationInputV1,
		mutate: impl FnOnce(&mut CheckpointResponse),
	) {
		let mut value = response(input);
		mutate(&mut value);
		input.response_scale = value.encode();
	}

	#[test]
	fn exact_finalized_observation_becomes_publishable_and_reopens() {
		let temp = TempDir::new().unwrap();
		let (_submission_temp, submission) = durable_submission(&submission_input());
		let input = publication_input(submission);
		let store = CheckpointPublicationStoreV1::open(temp.path()).unwrap();
		let published = store.publish(&input).unwrap();
		assert!(published.is_publishable());
		assert_eq!(decode_hex(&published.response_scale).unwrap(), input.response_scale);
		assert_eq!(published.submission, input.submission);
		assert_eq!(store.publish(&input).unwrap(), published);
		drop(store);

		let reopened = CheckpointPublicationStoreV1::open(temp.path()).unwrap();
		assert_eq!(reopened.publish(&input).unwrap(), published);
	}

	#[test]
	fn wrong_missing_or_noncanonical_runtime_responses_fail_closed() {
		let (_submission_temp, submission) = durable_submission(&submission_input());
		let valid = publication_input(submission);
		let mut cases = Vec::new();

		let mut wrong_version = valid.clone();
		replace_response(&mut wrong_version, |response| response.version -= 1);
		cases.push(wrong_version);
		let mut missing = valid.clone();
		replace_response(&mut missing, |response| response.value = None);
		cases.push(missing);
		let mut trailing = valid.clone();
		trailing.response_scale.push(0);
		cases.push(trailing);
		let mut oversized = valid.clone();
		oversized.response_scale.resize(MAX_RESPONSE_BYTES + 1, 0);
		cases.push(oversized);

		for input in cases {
			let temp = TempDir::new().unwrap();
			let store = CheckpointPublicationStoreV1::open(temp.path()).unwrap();
			assert!(matches!(store.publish(&input), Err(ContentError::IntegrityFailed)));
			assert!(store.records.read().unwrap().is_empty());
		}
	}

	#[test]
	fn every_checkpoint_field_quorum_and_finality_mismatch_fails_closed() {
		let (_submission_temp, submission) = durable_submission(&submission_input());
		let valid = publication_input(submission);
		let mut cases = Vec::new();
		for mutation in 0..11 {
			let mut input = valid.clone();
			replace_response(&mut input, |response| {
				let checkpoint = response.value.as_mut().unwrap();
				match mutation {
					0 => checkpoint.bucket_id = H256::repeat_byte(31),
					1 => checkpoint.commitment.mmr_root = H256::repeat_byte(32),
					2 => checkpoint.commitment.start_seq += 1,
					3 => checkpoint.commitment.leaf_count += 1,
					4 => checkpoint.commitment_nonce += 1,
					5 => checkpoint.primary_signers = 2,
					6 => {
						checkpoint.replica_confirmations.pop();
					},
					7 => checkpoint.replica_confirmations.swap(0, 1),
					8 => checkpoint.replica_confirmations[1] = account(9),
					9 =>
						checkpoint.replica_confirmations[1] =
							checkpoint.replica_confirmations[0].clone(),
					10 => checkpoint.checkpoint_block = checkpoint.commitment_nonce - 1,
					_ => unreachable!(),
				}
			});
			cases.push(input);
		}
		let mut stale_finality = valid;
		stale_finality.finalized_number = 114;
		cases.push(stale_finality);

		for input in cases {
			let temp = TempDir::new().unwrap();
			let store = CheckpointPublicationStoreV1::open(temp.path()).unwrap();
			assert!(matches!(store.publish(&input), Err(ContentError::IntegrityFailed)));
		}
	}

	#[test]
	fn exact_retry_precedes_capacity_but_changed_observation_or_submission_conflicts() {
		let temp = TempDir::new().unwrap();
		let base_submission_input = submission_input();
		let (_submission_temp, submission) = durable_submission(&base_submission_input);
		let input = publication_input(submission);
		let store = CheckpointPublicationStoreV1::open(temp.path()).unwrap();
		let published = store.publish(&input).unwrap();

		let mut changed_observation = input.clone();
		changed_observation.finalized_hash = H256::repeat_byte(23);
		assert!(matches!(
			store.publish(&changed_observation),
			Err(ContentError::IdempotencyConflict)
		));

		{
			let mut records = store.records.write().unwrap();
			for index in records.len()..MAX_RECORDS {
				let mut dummy = published.clone();
				dummy.submission_id = format!("{index:064x}");
				records.insert(dummy.submission_id.clone(), dummy);
			}
		}
		assert_eq!(store.publish(&input).unwrap(), published);

		let mut next_submission_input = base_submission_input.clone();
		next_submission_input.payload.nonce += 1;
		resign(&mut next_submission_input);
		let (_next_temp, next_submission) = durable_submission(&next_submission_input);
		let next = publication_input(next_submission);
		assert!(matches!(store.publish(&next), Err(ContentError::ProviderRecoveryTableFull)));
		drop(store);

		let changed_store_temp = TempDir::new().unwrap();
		let changed_store = CheckpointPublicationStoreV1::open(changed_store_temp.path()).unwrap();
		let (_changed_temp, changed_submission) = {
			let mut changed = base_submission_input;
			changed.payload.commitment.mmr_root = H256::repeat_byte(77);
			resign(&mut changed);
			durable_submission(&changed)
		};
		changed_store.publish(&input).unwrap();
		assert!(matches!(
			changed_store.publish(&publication_input(changed_submission)),
			Err(ContentError::IdempotencyConflict)
		));
	}

	#[test]
	fn recomputed_record_and_embedded_submission_tampering_fail_on_reopen() {
		let temp = TempDir::new().unwrap();
		let (_submission_temp, submission) = durable_submission(&submission_input());
		let input = publication_input(submission);
		let store = CheckpointPublicationStoreV1::open(temp.path()).unwrap();
		let published = store.publish(&input).unwrap();
		drop(store);

		let path = temp.path().join(ROOT).join(format!("{}.json", published.submission_id));
		let mut record: PublishedCheckpointV1 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		let mut observed =
			decode_scale::<CheckpointResponse>(&decode_hex(&record.response_scale).unwrap())
				.unwrap();
		observed.value.as_mut().unwrap().commitment.mmr_root = H256::repeat_byte(90);
		record.response_scale = hex::encode(observed.encode());
		record.record_hash = record_hash(&record).unwrap();
		fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(matches!(
			CheckpointPublicationStoreV1::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));

		record.response_scale = hex::encode(&input.response_scale);
		record.submission_record_hash = hex::encode([44; 32]);
		record.record_hash = record_hash(&record).unwrap();
		fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(matches!(
			CheckpointPublicationStoreV1::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn every_publication_crash_seam_recovers_exact_old_or_new_state() {
		let faults = [
			CheckpointPublicationFault::BeforeTempFsync,
			CheckpointPublicationFault::AfterTempFsync,
			CheckpointPublicationFault::AfterRename,
			CheckpointPublicationFault::AfterDirectoryFsync,
		];
		for fault in faults {
			let temp = TempDir::new().unwrap();
			let (_submission_temp, submission) = durable_submission(&submission_input());
			let input = publication_input(submission);
			let store = CheckpointPublicationStoreV1::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(store.publish(&input), Err(ContentError::Io(_))));
			assert!(matches!(store.publish(&input), Err(ContentError::IntegrityFailed)));
			drop(store);

			let reopened = CheckpointPublicationStoreV1::open(temp.path()).unwrap();
			assert_eq!(reopened.publish(&input).unwrap(), publication_record(&input).unwrap());
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
