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

//! Metadata-bound finality adapter for durable checkpoint fallback promotions.

use async_trait::async_trait;
use codec::Encode;
use scale_decode::{DecodeAsFields, Field};
use scale_value::Composite;
use subxt::{tx::Payload as _, Metadata};

use super::{
	checkpoint_promotion::{
		validate_intent, CheckpointPromotionStoreV2, FallbackPromotionFinalizedReceiptV2,
		FallbackPromotionIntentV2,
	},
	checkpoint_submitter::FinalizedEvidence,
};
use crate::ContentError;

const PALLET: &str = "StorageProvider";
const CALL: &str = "promote_checkpoint_fallback";
const CALL_FIELDS: [&str; 3] = ["payload", "service_key", "signature"];
pub(super) const NATIVE_INTENT_PREFIX: &str = "orbis-checkpoint-promotion-v2-";

#[async_trait]
pub(crate) trait PromotionFinalityLane: Send + Sync {
	fn metadata(&self) -> &Metadata;
	fn signer_account(&self) -> [u8; 32];
	fn service_key(&self) -> [u8; 32];

	async fn submit_and_finalize(
		&self,
		native_intent_id: &str,
		intent: &FallbackPromotionIntentV2,
		payload: subxt::tx::DynamicPayload,
	) -> Result<FinalizedEvidence, ContentError>;
}

pub(crate) async fn consume_one_with_lane_bounded(
	store: &CheckpointPromotionStoreV2,
	lane: &impl PromotionFinalityLane,
	max_attempts: usize,
) -> Result<Option<FallbackPromotionFinalizedReceiptV2>, ContentError> {
	if max_attempts == 0 {
		return Err(ContentError::IntegrityFailed);
	}
	let pending = store.reserve_pending_intents(max_attempts)?;
	if pending.is_empty() {
		return Ok(None);
	}
	let mut first_error = None;
	for intent in pending.into_iter().take(max_attempts) {
		let (native_intent_id, payload) = match prepare_intent(lane, &intent) {
			Ok(prepared) => prepared,
			Err(error) => {
				first_error.get_or_insert(error);
				continue;
			},
		};
		let evidence = match lane.submit_and_finalize(&native_intent_id, &intent, payload).await {
			Ok(evidence) => evidence,
			Err(error) if first_error.is_none() => {
				first_error = Some(error);
				continue;
			},
			Err(_) => continue,
		};
		// A finalized chain transition has occurred. Any local validation or persistence failure
		// must stop this batch so a poisoned store cannot trigger another extrinsic.
		let receipt = store.record_finalized(
			&intent.intent_id,
			evidence.block_hash,
			evidence.block_number,
			evidence.extrinsic_hash,
			evidence.finality_attestation_version,
			evidence.finality_signature,
		)?;
		return Ok(Some(receipt));
	}
	Err(first_error.unwrap_or(ContentError::IntegrityFailed))
}

fn prepare_intent(
	lane: &impl PromotionFinalityLane,
	intent: &FallbackPromotionIntentV2,
) -> Result<(String, subxt::tx::DynamicPayload), ContentError> {
	validate_intent(intent)?;
	if decode_fixed_hex::<32>(&intent.provider)? != lane.signer_account()
		|| decode_fixed_hex::<32>(&intent.service_key)? != lane.service_key()
	{
		return Err(ContentError::IntegrityFailed);
	}
	let payload = promotion_payload(lane.metadata(), intent)?;
	let native_intent_id = format!("{NATIVE_INTENT_PREFIX}{}", intent.intent_id);
	Ok((native_intent_id, payload))
}

pub(crate) fn promotion_payload(
	metadata: &Metadata,
	intent: &FallbackPromotionIntentV2,
) -> Result<subxt::tx::DynamicPayload, ContentError> {
	validate_intent(intent)?;
	let args = decode_canonical_hex(&intent.call_args_scale)?;
	let pallet = metadata.pallet_by_name(PALLET).ok_or(ContentError::IntegrityFailed)?;
	let call = pallet.call_variant_by_name(CALL).ok_or(ContentError::IntegrityFailed)?;
	if call.fields.len() != CALL_FIELDS.len()
		|| !call
			.fields
			.iter()
			.zip(CALL_FIELDS)
			.all(|(field, expected)| field.name.as_deref() == Some(expected))
	{
		return Err(ContentError::IntegrityFailed);
	}
	let mut fields = call.fields.iter().map(|field| Field::new(field.ty.id, field.name.as_deref()));
	let mut input = &args[..];
	let composite = Composite::<()>::decode_as_fields(&mut input, &mut fields, metadata.types())
		.map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || fields.next().is_some() {
		return Err(ContentError::IntegrityFailed);
	}
	let payload = subxt::dynamic::tx(PALLET, CALL, composite);
	let encoded = payload.encode_call_data(metadata).map_err(|_| ContentError::IntegrityFailed)?;
	let mut prefix = Vec::with_capacity(2);
	pallet.index().encode_to(&mut prefix);
	call.index.encode_to(&mut prefix);
	if encoded.get(..prefix.len()) != Some(prefix.as_slice())
		|| encoded.get(prefix.len()..) != Some(args.as_slice())
	{
		return Err(ContentError::IntegrityFailed);
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
		return Err(ContentError::IntegrityFailed);
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
	use orbis_storage_runtime_api::{
		CheckpointDutyInfo, CheckpointDutyMode, CheckpointDutyPhase, CommitmentInfo,
		ProviderDutyAuthority, ProviderDutyExclusion, ProviderDutyRole, RESPONSE_VERSION,
	};
	use pallet_orbis_storage_provider::CheckpointFallbackPromotionV1;
	use scale_info::{meta_type, TypeInfo};
	use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
	use tempfile::TempDir;

	use super::*;
	use crate::checkpoint::{
		checkpoint_outbox::{FINALITY_ATTESTATION_VERSION, FINALIZED_STATE},
		checkpoint_promotion::{
			promotion_finality_attestation_digest, CheckpointPromotionFault,
		},
	};

	type Duty = CheckpointDutyInfo<AccountId32, H256, u32>;

	#[allow(non_camel_case_types, dead_code)]
	#[derive(TypeInfo)]
	enum ExactCall {
		#[codec(index = 26)]
		promote_checkpoint_fallback {
			payload: CheckpointFallbackPromotionV1<H256, u32>,
			service_key: ed25519::Public,
			signature: ed25519::Signature,
		},
	}

	#[allow(non_camel_case_types, dead_code)]
	#[derive(TypeInfo)]
	enum RenamedFieldCall {
		#[codec(index = 26)]
		promote_checkpoint_fallback {
			wrong_payload: CheckpointFallbackPromotionV1<H256, u32>,
			service_key: ed25519::Public,
			signature: ed25519::Signature,
		},
	}

	#[allow(non_camel_case_types, dead_code)]
	#[derive(TypeInfo)]
	enum WrongShapeCall {
		#[codec(index = 26)]
		promote_checkpoint_fallback {
			payload: u32,
			service_key: ed25519::Public,
			signature: ed25519::Signature,
		},
	}

	#[allow(non_camel_case_types, dead_code)]
	#[derive(TypeInfo)]
	enum RenamedCall {
		#[codec(index = 26)]
		checkpoint_fallback_promote {
			payload: CheckpointFallbackPromotionV1<H256, u32>,
			service_key: ed25519::Public,
			signature: ed25519::Signature,
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

	fn duty(seed: u8) -> Duty {
		Duty {
			response_version: RESPONSE_VERSION,
			commons_genesis_hash: H256::repeat_byte(10),
			commons_spec_version: 11,
			commons_transaction_version: 12,
			commons_metadata_hash: H256::repeat_byte(13),
			duty_id: H256::repeat_byte(seed),
			bucket_id: H256::repeat_byte(seed.saturating_add(40)),
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

	fn authorize(
		store: &CheckpointPromotionStoreV2,
		seed: u8,
	) -> FallbackPromotionIntentV2 {
		store.authorize(&account(2), &duty(seed).encode(), &pair(2)).unwrap()
	}

	struct MockLane {
		metadata: Metadata,
		signer: [u8; 32],
		service_seed: u8,
		rejected: Vec<String>,
		calls: AtomicUsize,
		intents: Mutex<Vec<String>>,
	}

	impl MockLane {
		fn valid() -> Self {
			Self {
				metadata: metadata::<ExactCall>(),
				signer: [2; 32],
				service_seed: 2,
				rejected: Vec::new(),
				calls: AtomicUsize::new(0),
				intents: Mutex::new(Vec::new()),
			}
		}
	}

	#[async_trait]
	impl PromotionFinalityLane for MockLane {
		fn metadata(&self) -> &Metadata {
			&self.metadata
		}

		fn signer_account(&self) -> [u8; 32] {
			self.signer
		}

		fn service_key(&self) -> [u8; 32] {
			pair(self.service_seed).public().0
		}

		async fn submit_and_finalize(
			&self,
			native_intent_id: &str,
			intent: &FallbackPromotionIntentV2,
			_payload: subxt::tx::DynamicPayload,
		) -> Result<FinalizedEvidence, ContentError> {
			self.calls.fetch_add(1, Ordering::SeqCst);
			self.intents.lock().unwrap().push(native_intent_id.into());
			if self.rejected.iter().any(|rejected| rejected == native_intent_id) {
				return Err(ContentError::Io(format!("promotion rejected: {native_intent_id}")));
			}
			let block_hash = [21; 32];
			let block_number = 130;
			let extrinsic_hash = [22; 32];
			let digest = promotion_finality_attestation_digest(
				FINALITY_ATTESTATION_VERSION,
				&intent.intent_id,
				&intent.record_hash,
				&intent.tuple_key,
				block_hash,
				block_number,
				extrinsic_hash,
				FINALIZED_STATE,
			)?;
			Ok(FinalizedEvidence {
				block_hash,
				block_number,
				extrinsic_hash,
				finality_attestation_version: FINALITY_ATTESTATION_VERSION,
				finality_signature: pair(self.service_seed).sign(&digest).0,
			})
		}
	}

	#[test]
	fn metadata_and_call_bytes_are_exact_and_fail_closed() {
		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let mut intent = authorize(&store, 14);
		let exact = metadata::<ExactCall>();
		let payload = promotion_payload(&exact, &intent).unwrap();
		let encoded = payload.encode_call_data(&exact).unwrap();
		assert_eq!(&encoded[2..], hex::decode(&intent.call_args_scale).unwrap());
		assert!(matches!(
			promotion_payload(&metadata_from_pallets(vec![]), &intent),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			promotion_payload(&metadata_with_pallet::<ExactCall>("RenamedStorageProvider"), &intent),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			promotion_payload(&metadata::<RenamedCall>(), &intent),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			promotion_payload(&metadata::<RenamedFieldCall>(), &intent),
			Err(ContentError::IntegrityFailed)
		));
		assert!(matches!(
			promotion_payload(&metadata::<WrongShapeCall>(), &intent),
			Err(ContentError::IntegrityFailed)
		));
		intent.call_args_scale.push_str("00");
		assert!(matches!(
			promotion_payload(&exact, &intent),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[tokio::test]
	async fn wrong_metadata_account_and_service_key_never_submit() {
		for lane in [
			MockLane { metadata: metadata::<RenamedCall>(), ..MockLane::valid() },
			MockLane { signer: [99; 32], ..MockLane::valid() },
			MockLane { service_seed: 99, ..MockLane::valid() },
		] {
			let temp = TempDir::new().unwrap();
			let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
			authorize(&store, 14);
			assert!(matches!(
				consume_one_with_lane_bounded(&store, &lane, 1).await,
				Err(ContentError::IntegrityFailed)
			));
			assert_eq!(lane.calls.load(Ordering::SeqCst), 0);
			assert_eq!(store.reserve_pending_intents(8).unwrap().len(), 1);
		}
	}

	#[tokio::test]
	async fn bounded_attempts_isolate_failures_and_use_stable_native_ids() {
		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		for seed in 14..17 {
			authorize(&store, seed);
		}
		let pending = store.reserve_pending_intents(3).unwrap();
		let first_native = format!("{NATIVE_INTENT_PREFIX}{}", pending[0].intent_id);
		let lane = MockLane { rejected: vec![first_native.clone()], ..MockLane::valid() };
		let receipt = consume_one_with_lane_bounded(&store, &lane, 2).await.unwrap().unwrap();
		assert_eq!(lane.calls.load(Ordering::SeqCst), 2);
		assert_eq!(
			lane.intents.lock().unwrap().as_slice(),
			[
				first_native,
				format!("{NATIVE_INTENT_PREFIX}{}", pending[1].intent_id)
			]
		);
		assert_eq!(receipt.intent_id, pending[1].intent_id);
		assert_eq!(store.reserve_pending_intents(8).unwrap().len(), 2);
	}

	#[tokio::test]
	async fn finalized_receipt_has_no_checkpoint_outbox_or_publication_side_effects() {
		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let intent = authorize(&store, 14);
		let receipt = consume_one_with_lane_bounded(&store, &MockLane::valid(), 1)
			.await
			.unwrap()
			.unwrap();
		assert_eq!(receipt.intent_id, intent.intent_id);
		assert!(store.reserve_pending_intents(8).unwrap().is_empty());
		for root in [
			"checkpoint-submissions-v2",
			"checkpoint-finalized-receipts-v2",
			"checkpoint-publications-v1",
		] {
			assert!(!temp.path().join(root).exists(), "unexpected checkpoint side effect: {root}");
		}
	}

	#[tokio::test]
	async fn chain_finality_before_local_receipt_replays_the_exact_native_intent() {
		let temp = TempDir::new().unwrap();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let intent = authorize(&store, 14);
		let lane = MockLane::valid();
		assert_eq!(store.reserve_pending_intents(1).unwrap(), vec![intent.clone()]);
		store.inject_fault_once(CheckpointPromotionFault::BeforeTempFsync).unwrap();
		assert!(matches!(
			consume_one_with_lane_bounded(&store, &lane, 1).await,
			Err(ContentError::Io(_))
		));
		drop(store);

		let reopened = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		let receipt = consume_one_with_lane_bounded(&reopened, &lane, 1)
			.await
			.unwrap()
			.unwrap();
		let native = format!("{NATIVE_INTENT_PREFIX}{}", intent.intent_id);
		assert_eq!(lane.calls.load(Ordering::SeqCst), 2);
		assert_eq!(lane.intents.lock().unwrap().as_slice(), [native.clone(), native]);
		assert_eq!(receipt.intent_id, intent.intent_id);
		assert!(reopened.reserve_pending_intents(8).unwrap().is_empty());
	}

	#[tokio::test]
	async fn post_finality_receipt_failure_aborts_batch_and_reopens_old_or_new() {
		for fault in [
			CheckpointPromotionFault::BeforeTempFsync,
			CheckpointPromotionFault::AfterTempFsync,
			CheckpointPromotionFault::AfterRename,
			CheckpointPromotionFault::AfterDirectoryFsync,
		] {
			let temp = TempDir::new().unwrap();
			let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
			authorize(&store, 14);
			authorize(&store, 15);
			let pending = store.reserve_pending_intents(2).unwrap();
			let lane = MockLane::valid();
			store.inject_fault_once(fault).unwrap();
			assert!(matches!(
				consume_one_with_lane_bounded(&store, &lane, 2).await,
				Err(ContentError::Io(_))
			));
			assert_eq!(lane.calls.load(Ordering::SeqCst), 1);
			assert_eq!(
				lane.intents.lock().unwrap().as_slice(),
				[format!("{NATIVE_INTENT_PREFIX}{}", pending[0].intent_id)]
			);
			drop(store);

			let reopened = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
			let receipt = consume_one_with_lane_bounded(&reopened, &lane, 2)
				.await
				.unwrap()
				.unwrap();
			let expected = match fault {
				CheckpointPromotionFault::BeforeTempFsync |
				CheckpointPromotionFault::AfterTempFsync => &pending[0],
				CheckpointPromotionFault::AfterRename |
				CheckpointPromotionFault::AfterDirectoryFsync => &pending[1],
			};
			assert_eq!(lane.calls.load(Ordering::SeqCst), 2);
			assert_eq!(receipt.intent_id, expected.intent_id);
			assert_eq!(
				lane.intents.lock().unwrap().last(),
				Some(&format!("{NATIVE_INTENT_PREFIX}{}", expected.intent_id))
			);
		}
	}
}
