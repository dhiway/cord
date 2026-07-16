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

//! Private metadata-bound submission lane for durable checkpoint calls.

use async_trait::async_trait;
use codec::Encode;
use scale_decode::{DecodeAsFields, Field};
use scale_value::Composite;
use subxt::{tx::Payload as _, Metadata};

use super::checkpoint_outbox::{
	validate_submission, CheckpointFinalizedReceiptV2, CheckpointOutboxV2, CheckpointSubmissionV2,
};
use crate::ContentError;

const PALLET: &str = "StorageProvider";
const CALL: &str = "submit_checkpoint";
const CALL_FIELDS: [&str; 8] = [
	"domain",
	"payload",
	"window_start",
	"window_end",
	"service_key",
	"primary_signature",
	"primary_context_signature",
	"confirmations",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FinalizedEvidence {
	pub block_hash: [u8; 32],
	pub block_number: u32,
	pub extrinsic_hash: [u8; 32],
}

#[async_trait]
pub(crate) trait CheckpointFinalityLane: Send + Sync {
	fn metadata(&self) -> &Metadata;
	fn signer_account(&self) -> [u8; 32];

	async fn submit_and_finalize(
		&self,
		intent_id: &str,
		payload: subxt::tx::DynamicPayload,
	) -> Result<FinalizedEvidence, ContentError>;
}

pub(crate) async fn consume_one_with_lane(
	outbox: &CheckpointOutboxV2,
	lane: &impl CheckpointFinalityLane,
) -> Result<Option<CheckpointFinalizedReceiptV2>, ContentError> {
	let heads = outbox.pending_submission_heads()?;
	if heads.is_empty() {
		return Ok(None)
	}
	let mut first_lane_error = None;
	for submission in heads {
		if decode_fixed_hex::<32>(&submission.primary)? != lane.signer_account() {
			return Err(ContentError::IntegrityFailed)
		}
		let payload = checkpoint_payload(lane.metadata(), &submission)?;
		let intent_id = format!("orbis-checkpoint-v2-{}", submission.submission_id);
		let evidence = match lane.submit_and_finalize(&intent_id, payload).await {
			Ok(evidence) => evidence,
			Err(error) => {
				if first_lane_error.is_none() {
					first_lane_error = Some(error);
				}
				continue
			},
		};
		let receipt = outbox.record_finalized(
			&submission.submission_id,
			evidence.block_hash,
			evidence.block_number,
			evidence.extrinsic_hash,
		)?;
		return Ok(Some(receipt))
	}
	Err(first_lane_error.unwrap_or(ContentError::IntegrityFailed))
}

pub(crate) fn checkpoint_payload(
	metadata: &Metadata,
	submission: &CheckpointSubmissionV2,
) -> Result<subxt::tx::DynamicPayload, ContentError> {
	validate_submission(submission)?;
	let args = decode_canonical_hex(&submission.call_args_scale)?;
	let pallet = metadata.pallet_by_name(PALLET).ok_or(ContentError::IntegrityFailed)?;
	let call = pallet.call_variant_by_name(CALL).ok_or(ContentError::IntegrityFailed)?;
	if call.fields.len() != CALL_FIELDS.len() ||
		!call
			.fields
			.iter()
			.zip(CALL_FIELDS)
			.all(|(field, expected)| field.name.as_deref() == Some(expected))
	{
		return Err(ContentError::IntegrityFailed)
	}
	let mut fields = call.fields.iter().map(|field| Field::new(field.ty.id, field.name.as_deref()));
	let mut input = &args[..];
	let composite = Composite::<()>::decode_as_fields(&mut input, &mut fields, metadata.types())
		.map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || fields.next().is_some() {
		return Err(ContentError::IntegrityFailed)
	}
	let payload = subxt::dynamic::tx(PALLET, CALL, composite);
	let encoded = payload.encode_call_data(metadata).map_err(|_| ContentError::IntegrityFailed)?;
	let mut prefix = Vec::with_capacity(2);
	pallet.index().encode_to(&mut prefix);
	call.index.encode_to(&mut prefix);
	if encoded.get(..prefix.len()) != Some(prefix.as_slice()) ||
		encoded.get(prefix.len()..) != Some(args.as_slice())
	{
		return Err(ContentError::IntegrityFailed)
	}
	Ok(payload)
}

fn decode_fixed_hex<const N: usize>(value: &str) -> Result<[u8; N], ContentError> {
	decode_canonical_hex(value)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)
}

fn decode_canonical_hex(value: &str) -> Result<Vec<u8>, ContentError> {
	if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
		return Err(ContentError::IntegrityFailed)
	}
	hex::decode(value).map_err(|_| ContentError::IntegrityFailed)
}

#[cfg(test)]
mod tests {
	use std::sync::{
		atomic::{AtomicUsize, Ordering},
		Mutex,
	};

	use frame_metadata::v15::{
		CustomMetadata, ExtrinsicMetadata, OuterEnums, PalletCallMetadata, PalletMetadata,
		RuntimeMetadataV15,
	};
	use pallet_orbis_storage_provider::{
		CheckpointContextV1, CommitmentPayloadV2, CommitmentV1, ReplicaSignature,
	};
	use scale_info::{meta_type, TypeInfo};
	use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
	use tempfile::TempDir;

	use super::*;
	use crate::checkpoint::{
		checkpoint_outbox::{
			submission_record_hash, CheckpointOutboxFault, CheckpointSubmissionInputV2,
		},
		checkpoint_quorum::{checkpoint_context_digest, checkpoint_digest},
	};

	type Confirmations = Vec<ReplicaSignature<AccountId32>>;

	#[allow(non_camel_case_types, dead_code)]
	#[derive(TypeInfo)]
	enum ExactCall {
		#[codec(index = 19)]
		submit_checkpoint {
			domain: Vec<u8>,
			payload: CommitmentPayloadV2<H256, u32>,
			window_start: u32,
			window_end: u32,
			service_key: ed25519::Public,
			primary_signature: ed25519::Signature,
			primary_context_signature: ed25519::Signature,
			confirmations: Confirmations,
		},
	}

	#[allow(non_camel_case_types, dead_code)]
	#[derive(TypeInfo)]
	enum RenamedCall {
		#[codec(index = 19)]
		submit_checkpoint {
			wrong_domain: Vec<u8>,
			payload: CommitmentPayloadV2<H256, u32>,
			window_start: u32,
			window_end: u32,
			service_key: ed25519::Public,
			primary_signature: ed25519::Signature,
			primary_context_signature: ed25519::Signature,
			confirmations: Confirmations,
		},
	}

	#[allow(non_camel_case_types, dead_code)]
	#[derive(TypeInfo)]
	enum WrongShapeCall {
		#[codec(index = 19)]
		submit_checkpoint {
			domain: u32,
			payload: CommitmentPayloadV2<H256, u32>,
			window_start: u32,
			window_end: u32,
			service_key: ed25519::Public,
			primary_signature: ed25519::Signature,
			primary_context_signature: ed25519::Signature,
			confirmations: Confirmations,
		},
	}

	#[allow(non_camel_case_types, dead_code)]
	#[derive(TypeInfo)]
	enum RenamedCallVariant {
		#[codec(index = 19)]
		checkpoint_submit {
			domain: Vec<u8>,
			payload: CommitmentPayloadV2<H256, u32>,
			window_start: u32,
			window_end: u32,
			service_key: ed25519::Public,
			primary_signature: ed25519::Signature,
			primary_context_signature: ed25519::Signature,
			confirmations: Confirmations,
		},
	}

	fn metadata<Call: TypeInfo + 'static>() -> Metadata {
		metadata_with_pallet::<Call>(PALLET)
	}

	fn metadata_with_pallet<Call: TypeInfo + 'static>(pallet_name: &'static str) -> Metadata {
		metadata_from_pallets(vec![PalletMetadata {
			name: pallet_name,
			storage: None,
			calls: Some(PalletCallMetadata { ty: meta_type::<Call>() }),
			event: None,
			constants: vec![],
			error: None,
			index: 37,
			docs: vec![],
		}])
	}

	fn metadata_without_calls() -> Metadata {
		metadata_from_pallets(vec![PalletMetadata {
			name: PALLET,
			storage: None,
			calls: None,
			event: None,
			constants: vec![],
			error: None,
			index: 37,
			docs: vec![],
		}])
	}

	fn metadata_from_pallets(pallets: Vec<PalletMetadata>) -> Metadata {
		let prefixed: frame_metadata::RuntimeMetadataPrefixed = RuntimeMetadataV15::new(
			pallets,
			ExtrinsicMetadata {
				version: 4,
				address_ty: meta_type::<()>(),
				call_ty: meta_type::<()>(),
				signature_ty: meta_type::<()>(),
				extra_ty: meta_type::<()>(),
				signed_extensions: vec![],
			},
			meta_type::<()>(),
			vec![],
			OuterEnums {
				call_enum_ty: meta_type::<()>(),
				event_enum_ty: meta_type::<()>(),
				error_enum_ty: meta_type::<()>(),
			},
			CustomMetadata { map: Default::default() },
		)
		.into();
		prefixed.try_into().unwrap()
	}

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
					signature: pair(seed).sign(&checkpoint_digest(&payload)),
					context_signature: pair(seed).sign(&checkpoint_context_digest(&context)),
				})
				.collect(),
		};
		input.primary_signature = pair(1).sign(&checkpoint_digest(&input.payload));
		input.primary_context_signature = pair(1).sign(&checkpoint_context_digest(&input.context));
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

	fn submission(temp: &TempDir) -> (CheckpointOutboxV2, CheckpointSubmissionV2) {
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let submission = outbox.enqueue(&fixture()).unwrap().submission;
		(outbox, submission)
	}

	fn enqueue_checkpoint(
		outbox: &CheckpointOutboxV2,
		bucket: u8,
		nonce: u32,
		start_seq: u64,
	) -> CheckpointSubmissionV2 {
		let mut input = fixture();
		input.payload.bucket_id = H256::repeat_byte(bucket);
		input.payload.nonce = nonce;
		input.payload.commitment.start_seq = start_seq;
		resign(&mut input);
		outbox.enqueue(&input).unwrap().submission
	}

	struct MockLane {
		metadata: Metadata,
		signer: [u8; 32],
		result: Mutex<Result<FinalizedEvidence, ContentError>>,
		reject_intents: Vec<String>,
		calls: AtomicUsize,
		intents: Mutex<Vec<String>>,
	}

	impl MockLane {
		fn successful(signer: [u8; 32]) -> Self {
			Self {
				metadata: metadata::<ExactCall>(),
				signer,
				result: Mutex::new(Ok(FinalizedEvidence {
					block_hash: [8; 32],
					block_number: 44,
					extrinsic_hash: [9; 32],
				})),
				reject_intents: Vec::new(),
				calls: AtomicUsize::new(0),
				intents: Mutex::new(Vec::new()),
			}
		}
	}

	#[async_trait]
	impl CheckpointFinalityLane for MockLane {
		fn metadata(&self) -> &Metadata {
			&self.metadata
		}

		fn signer_account(&self) -> [u8; 32] {
			self.signer
		}

		async fn submit_and_finalize(
			&self,
			intent_id: &str,
			_payload: subxt::tx::DynamicPayload,
		) -> Result<FinalizedEvidence, ContentError> {
			self.calls.fetch_add(1, Ordering::SeqCst);
			self.intents.lock().unwrap().push(intent_id.into());
			if self.reject_intents.iter().any(|rejected| rejected == intent_id) {
				return Err(ContentError::Io(format!("checkpoint rejected: {intent_id}")))
			}
			self.result.lock().unwrap().clone()
		}
	}

	#[test]
	fn exact_metadata_round_trip_rejects_name_shape_and_trailing_bytes() {
		let temp = TempDir::new().unwrap();
		let (_outbox, mut submission) = submission(&temp);
		let exact = metadata::<ExactCall>();
		let payload = checkpoint_payload(&exact, &submission).unwrap();
		let encoded = payload.encode_call_data(&exact).unwrap();
		assert_eq!(&encoded[2..], hex::decode(&submission.call_args_scale).unwrap());
		assert!(matches!(
			checkpoint_payload(&metadata_from_pallets(vec![]), &submission),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			checkpoint_payload(
				&metadata_with_pallet::<ExactCall>("RenamedStorageProvider"),
				&submission
			),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			checkpoint_payload(&metadata_without_calls(), &submission),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			checkpoint_payload(&metadata::<RenamedCallVariant>(), &submission),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			checkpoint_payload(&metadata::<RenamedCall>(), &submission),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			checkpoint_payload(&metadata::<WrongShapeCall>(), &submission),
			Err(ContentError::IntegrityFailed)
		));

		submission.call_args_scale.push_str("00");
		submission.record_hash = submission_record_hash(&submission).unwrap();
		assert!(matches!(
			checkpoint_payload(&exact, &submission),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[tokio::test]
	async fn signer_and_finality_failures_never_create_receipts() {
		let temp = TempDir::new().unwrap();
		let (outbox, submission) = submission(&temp);
		let wrong_signer = MockLane::successful([99; 32]);
		assert!(matches!(
			consume_one_with_lane(&outbox, &wrong_signer).await,
			Err(ContentError::IntegrityFailed)
		));
		assert_eq!(wrong_signer.calls.load(Ordering::SeqCst), 0);
		assert_eq!(outbox.finalized_receipt(&submission.submission_id).unwrap(), None);

		let failed = MockLane {
			result: Mutex::new(Err(ContentError::Io("finality failed".into()))),
			..MockLane::successful([1; 32])
		};
		assert!(matches!(consume_one_with_lane(&outbox, &failed).await, Err(ContentError::Io(_))));
		assert_eq!(failed.calls.load(Ordering::SeqCst), 1);
		assert_eq!(
			failed.intents.lock().unwrap().as_slice(),
			[format!("orbis-checkpoint-v2-{}", submission.submission_id)]
		);
		assert_eq!(outbox.finalized_receipt(&submission.submission_id).unwrap(), None);
	}

	#[tokio::test]
	async fn later_hash_precedence_and_rejection_cannot_starve_predecessor() {
		let temp = TempDir::new().unwrap();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let mut later_input = fixture();
		later_input.payload.nonce = 104;
		resign(&mut later_input);
		let later = outbox.enqueue(&later_input).unwrap().submission;
		let mut predecessor_input = fixture();
		predecessor_input.payload.nonce = 103;
		resign(&mut predecessor_input);
		let predecessor = outbox.enqueue(&predecessor_input).unwrap().submission;
		assert!(later.tuple_key < predecessor.tuple_key);

		let predecessor_intent = format!("orbis-checkpoint-v2-{}", predecessor.submission_id);
		let later_intent = format!("orbis-checkpoint-v2-{}", later.submission_id);
		let lane = MockLane {
			reject_intents: vec![later_intent.clone()],
			..MockLane::successful([1; 32])
		};
		let receipt = consume_one_with_lane(&outbox, &lane).await.unwrap().unwrap();
		assert_eq!(receipt.submission_id, predecessor.submission_id);
		assert!(outbox.finalized_receipt(&predecessor.submission_id).unwrap().is_some());
		assert_eq!(
			outbox
				.pending_submissions()
				.unwrap()
				.iter()
				.map(|submission| submission.submission_id.as_str())
				.collect::<Vec<_>>(),
			[later.submission_id.as_str()]
		);

		assert!(matches!(consume_one_with_lane(&outbox, &lane).await, Err(ContentError::Io(_))));
		assert_eq!(lane.intents.lock().unwrap().as_slice(), [predecessor_intent, later_intent]);
		assert_eq!(outbox.finalized_receipt(&later.submission_id).unwrap(), None);
	}

	#[tokio::test]
	async fn rejected_bucket_head_cannot_starve_an_independent_bucket() {
		let temp = TempDir::new().unwrap();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let first_a = enqueue_checkpoint(&outbox, 4, 103, 7);
		let successor_a = enqueue_checkpoint(&outbox, 4, 104, 10);
		let first_b = enqueue_checkpoint(&outbox, 5, 99, 2);
		let first_a_intent = format!("orbis-checkpoint-v2-{}", first_a.submission_id);
		let first_b_intent = format!("orbis-checkpoint-v2-{}", first_b.submission_id);
		let lane = MockLane {
			reject_intents: vec![first_a_intent.clone()],
			..MockLane::successful([1; 32])
		};

		let receipt = consume_one_with_lane(&outbox, &lane).await.unwrap().unwrap();
		assert_eq!(receipt.submission_id, first_b.submission_id);
		assert_eq!(lane.intents.lock().unwrap().as_slice(), [first_a_intent, first_b_intent]);
		assert_eq!(outbox.finalized_receipt(&first_a.submission_id).unwrap(), None);
		assert_eq!(outbox.finalized_receipt(&successor_a.submission_id).unwrap(), None);
		assert!(outbox.finalized_receipt(&first_b.submission_id).unwrap().is_some());
		assert_eq!(
			outbox
				.pending_submissions()
				.unwrap()
				.iter()
				.map(|submission| submission.submission_id.as_str())
				.collect::<Vec<_>>(),
			[first_a.submission_id.as_str(), successor_a.submission_id.as_str()]
		);
	}

	#[tokio::test]
	async fn all_bucket_head_failures_return_the_first_deterministic_error() {
		let temp = TempDir::new().unwrap();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let first_a = enqueue_checkpoint(&outbox, 4, 103, 7);
		let successor_a = enqueue_checkpoint(&outbox, 4, 104, 10);
		let first_b = enqueue_checkpoint(&outbox, 5, 99, 2);
		let first_a_intent = format!("orbis-checkpoint-v2-{}", first_a.submission_id);
		let successor_a_intent = format!("orbis-checkpoint-v2-{}", successor_a.submission_id);
		let first_b_intent = format!("orbis-checkpoint-v2-{}", first_b.submission_id);
		let lane = MockLane {
			reject_intents: vec![
				first_a_intent.clone(),
				successor_a_intent,
				first_b_intent.clone(),
			],
			..MockLane::successful([1; 32])
		};

		assert_eq!(
			consume_one_with_lane(&outbox, &lane).await,
			Err(ContentError::Io(format!("checkpoint rejected: {first_a_intent}")))
		);
		assert_eq!(lane.intents.lock().unwrap().as_slice(), [first_a_intent, first_b_intent]);
		for submission in [&first_a, &successor_a, &first_b] {
			assert_eq!(outbox.finalized_receipt(&submission.submission_id).unwrap(), None);
		}
	}

	#[tokio::test]
	async fn finality_then_persist_crash_replays_the_exact_pending_submission() {
		let temp = TempDir::new().unwrap();
		let (outbox, submission) = submission(&temp);
		let lane = MockLane::successful([1; 32]);
		outbox.inject_fault_once(CheckpointOutboxFault::AfterTempFsync).unwrap();
		assert!(matches!(consume_one_with_lane(&outbox, &lane).await, Err(ContentError::Io(_))));
		assert_eq!(lane.calls.load(Ordering::SeqCst), 1);
		drop(outbox);

		let reopened = CheckpointOutboxV2::open(temp.path()).unwrap();
		let receipt = consume_one_with_lane(&reopened, &lane).await.unwrap().unwrap();
		assert_eq!(lane.calls.load(Ordering::SeqCst), 2);
		assert_eq!(receipt.submission_id, submission.submission_id);
		assert!(reopened.pending_submissions().unwrap().is_empty());
	}
}
