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

//! Bounded finality and exact finalized-state publication lifecycle.

use std::{sync::Arc, time::Duration};

use sp_core::ed25519;
use tokio::time::{interval, timeout, MissedTickBehavior};

use crate::{
	chain::{
		CheckpointPublicationAuthority, FinalizedCheckpointObservation, ReplicationAuthority,
	},
	checkpoint::{
		checkpoint_outbox::CheckpointFinalizedReceiptV2,
		checkpoint_promotion::FallbackPromotionFinalizedReceiptV2,
		checkpoint_promotion_submitter::PromotionFinalityLane,
		checkpoint_publication::{submission_bucket_id, PublishedCheckpointV1},
		checkpoint_submitter::CheckpointFinalityLane,
	},
	checkpoint_promotion_worker::PromotionDiscoveryScheduler,
	checkpoint_stack::CheckpointStack,
	ContentError, DiskStore,
};

const MAX_FINALITY_ATTEMPTS: usize = 8;
const MAX_PUBLICATIONS: usize = 8;
const FINALITY_TIMEOUT: Duration = Duration::from_secs(45);
const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(10);
const PROMOTION_FINALITY_TIMEOUT: Duration = Duration::from_secs(45);
const PROMOTION_TOPOLOGY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub(crate) struct CheckpointLiveTick {
	pub(crate) finalized: Option<CheckpointFinalizedReceiptV2>,
	pub(crate) promotion_finalized: Option<FallbackPromotionFinalizedReceiptV2>,
	pub(crate) published: Vec<PublishedCheckpointV1>,
}

pub(crate) async fn run<A, L>(
	authority: Arc<A>,
	stack: Arc<CheckpointStack>,
	store: Arc<DiskStore>,
	lane: L,
	local_provider: [u8; 32],
	local_key: ed25519::Pair,
	cadence: Duration,
) -> Result<(), ContentError>
where
	A: CheckpointPublicationAuthority + ReplicationAuthority + 'static,
	L: CheckpointFinalityLane + PromotionFinalityLane + 'static,
{
	let promotion_scheduler = stack.checkpoint_promotion_discovery_scheduler()?;
	let mut ticker = interval(cadence.max(Duration::from_secs(1)));
	ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
	loop {
		ticker.tick().await;
		match tick_with_promotions(
			&*authority,
			&stack,
			&store,
			&lane,
			&promotion_scheduler,
			local_provider,
			&local_key,
			PROMOTION_FINALITY_TIMEOUT,
			PROMOTION_TOPOLOGY_TIMEOUT,
			FINALITY_TIMEOUT,
			OBSERVATION_TIMEOUT,
		)
		.await
		{
			Ok(result) => {
				if let Some(receipt) = result.promotion_finalized {
					println!("checkpoint fallback promotion finalized: {}", receipt.intent_id);
				}
				if let Some(receipt) = result.finalized {
					println!("checkpoint finalized: {}", receipt.submission_id);
				}
				for record in result.published {
					println!("checkpoint publication reconciled: {}", record.submission_id);
				}
			},
			Err(error) => eprintln!("checkpoint live lifecycle tick failed: {error}"),
		}
	}
}

#[allow(clippy::too_many_arguments)]
async fn tick_with_promotions<A, L>(
	authority: &A,
	stack: &CheckpointStack,
	store: &DiskStore,
	lane: &L,
	scheduler: &PromotionDiscoveryScheduler,
	local_provider: [u8; 32],
	local_key: &ed25519::Pair,
	promotion_finality_timeout: Duration,
	promotion_topology_timeout: Duration,
	checkpoint_finality_timeout: Duration,
	observation_timeout: Duration,
) -> Result<CheckpointLiveTick, ContentError>
where
	A: CheckpointPublicationAuthority + ReplicationAuthority,
	L: CheckpointFinalityLane + PromotionFinalityLane,
{
	let checkpoint = tick_with_timeouts(
		authority,
		stack,
		lane,
		checkpoint_finality_timeout,
		observation_timeout,
	)
	.await;
	let mut first_error = checkpoint.as_ref().err().cloned();
	let promotion = crate::checkpoint_promotion_worker::tick(
		authority,
		stack,
		store,
		lane,
		scheduler,
		local_provider,
		local_key,
		promotion_finality_timeout,
		promotion_topology_timeout,
	)
	.await;
	if let Err(error) = &promotion {
		first_error.get_or_insert_with(|| error.clone());
	}
	if let Some(error) = first_error {
		return Err(error);
	}
	let mut checkpoint = checkpoint?;
	checkpoint.promotion_finalized = promotion?;
	Ok(checkpoint)
}

pub(crate) async fn tick<A, L>(
	authority: &A,
	stack: &CheckpointStack,
	lane: &L,
) -> Result<CheckpointLiveTick, ContentError>
where
	A: CheckpointPublicationAuthority,
	L: CheckpointFinalityLane,
{
	tick_with_timeouts(authority, stack, lane, FINALITY_TIMEOUT, OBSERVATION_TIMEOUT).await
}

async fn tick_with_timeouts<A, L>(
	authority: &A,
	stack: &CheckpointStack,
	lane: &L,
	finality_timeout: Duration,
	observation_timeout: Duration,
) -> Result<CheckpointLiveTick, ContentError>
where
	A: CheckpointPublicationAuthority,
	L: CheckpointFinalityLane,
{
	let finalized = match timeout(
		finality_timeout,
		stack.consume_one_with_lane_bounded(lane, MAX_FINALITY_ATTEMPTS),
	)
	.await
	{
		Ok(result) => result,
		Err(_) => Err(ContentError::Io("checkpoint finality attempt timed out".into())),
	};
	let mut first_error = finalized.as_ref().err().cloned();
	let mut published = Vec::new();
	for intent in stack.reserve_checkpoint_publications(MAX_PUBLICATIONS)? {
		let bucket_id = match submission_bucket_id(&intent.submission) {
			Ok(bucket_id) => bucket_id,
			Err(error) => {
				first_error.get_or_insert(error);
				continue;
			},
		};
		let observation = match timeout(
			observation_timeout,
			authority.checkpoint_observation_at(
				bucket_id,
				intent.finalized_hash.0,
				intent.finalized_number,
			),
		)
		.await
		{
			Ok(Ok(observation)) => observation,
			Ok(Err(_)) => {
				first_error.get_or_insert(ContentError::IntegrityFailed);
				continue;
			},
			Err(_) => {
				first_error
					.get_or_insert_with(|| ContentError::Io("checkpoint observation timed out".into()));
				continue;
			},
		};
		if !observation_matches(&observation, intent.finalized_hash.0, intent.finalized_number) {
			first_error.get_or_insert(ContentError::IntegrityFailed);
			continue;
		}
		match stack.publish_checkpoint_observation(&intent, observation.response_scale) {
			Ok(record) => published.push(record),
			Err(error) => {
				first_error.get_or_insert(error);
			},
		}
	}
	if let Some(error) = first_error {
		return Err(error);
	}
	Ok(CheckpointLiveTick {
		finalized: finalized?,
		promotion_finalized: None,
		published,
	})
}

fn observation_matches(
	observation: &FinalizedCheckpointObservation,
	finalized_hash: [u8; 32],
	finalized_number: u32,
) -> bool {
	observation.finalized_hash == finalized_hash && observation.finalized_number == finalized_number
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

	use async_trait::async_trait;
	use codec::Encode;
	use frame_metadata::v15::{
		CustomMetadata, ExtrinsicMetadata, OuterEnums, PalletCallMetadata, PalletMetadata,
		RuntimeMetadataV15,
	};
	use orbis_storage_runtime_api::{
		CheckpointDutyInfo, CheckpointDutyMode, CheckpointDutyPhase, CheckpointInfo,
		CommitmentInfo, ProviderDutyAuthority, ProviderDutyExclusion, ProviderDutyRole, Versioned,
		RESPONSE_VERSION,
	};
	use pallet_orbis_storage_provider::{
		CheckpointContextV1, CheckpointFallbackPromotionV1, CommitmentPayloadV2, CommitmentV1,
		ReplicaSignature,
	};
	use scale_info::{meta_type, TypeInfo};
	use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
	use tempfile::TempDir;

	use super::*;
	use crate::{
		chain::{
			ChainError, ReplicationAuthority, ReplicationTopologySnapshot,
		},
		checkpoint::{
			checkpoint_outbox::{CheckpointOutboxV2, CheckpointSubmissionInputV2},
			checkpoint_promotion::FallbackPromotionIntentV2,
			checkpoint_publication::{
				CheckpointPublicationStoreV1, FinalizedCheckpointPublicationInputV1,
			},
			checkpoint_quorum::{checkpoint_context_digest, checkpoint_digest},
			checkpoint_promotion_submitter::PromotionFinalityLane,
			checkpoint_submitter::FinalizedEvidence,
		},
		NodeProfile,
	};
	use crate::CheckpointDutyBatch;

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
		#[codec(index = 20)]
		promote_checkpoint_fallback {
			payload: CheckpointFallbackPromotionV1<H256, u32>,
			service_key: ed25519::Public,
			signature: ed25519::Signature,
		},
	}

	fn metadata() -> subxt::Metadata {
		let prefixed: frame_metadata::RuntimeMetadataPrefixed = RuntimeMetadataV15::new(
			vec![PalletMetadata {
				name: "StorageProvider",
				storage: None,
				calls: Some(PalletCallMetadata { ty: meta_type::<ExactCall>() }),
				event: None,
				constants: vec![],
				error: None,
				index: 37,
				docs: vec![],
			}],
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

	fn checkpoint_input() -> CheckpointSubmissionInputV2 {
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
			primary: AccountId32::new([1; 32]),
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
					provider: AccountId32::new([seed; 32]),
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

	fn checkpoint_input_for(seed: u8) -> CheckpointSubmissionInputV2 {
		let mut input = checkpoint_input();
		input.payload.bucket_id = H256::repeat_byte(seed);
		input.payload.commitment.mmr_root = H256::repeat_byte(seed.saturating_add(1));
		input.payload.commitment.start_seq = u64::from(seed);
		input.payload.nonce = 100 + u32::from(seed);
		input.context.v2_digest = checkpoint_digest(&input.payload);
		input.primary_signature = pair(1).sign(&checkpoint_digest(&input.payload));
		input.primary_context_signature = pair(1).sign(&checkpoint_context_digest(&input.context));
		input.confirmations = [2, 3]
			.into_iter()
			.map(|provider| ReplicaSignature {
				provider: AccountId32::new([provider; 32]),
				service_key: pair(provider).public(),
				signature: pair(provider).sign(&checkpoint_digest(&input.payload)),
				context_signature: pair(provider).sign(&checkpoint_context_digest(&input.context)),
			})
			.collect();
		input
	}

	fn promotion_duty() -> CheckpointDutyInfo<AccountId32, H256, u32> {
		let authority = |seed, role, order, eligible, may_initiate| ProviderDutyAuthority {
			provider: AccountId32::new([seed; 32]),
			role,
			order,
			active_service_key_version: 5,
			active_service_key: pair(seed).public().0,
			endpoint_hash: H256::repeat_byte(seed),
			organization_sla_eligible: true,
			overdue_challenge: false,
			eligible,
			may_sign: eligible,
			may_initiate,
			exclusion: (!eligible).then_some(ProviderDutyExclusion::Inactive),
			initiation_exclusion: None,
			confirmed_checkpoint: Some(100),
		};
		CheckpointDutyInfo {
			response_version: RESPONSE_VERSION,
			commons_genesis_hash: H256::repeat_byte(10),
			commons_spec_version: 11,
			commons_transaction_version: 12,
			commons_metadata_hash: H256::repeat_byte(13),
			duty_id: H256::repeat_byte(14),
			bucket_id: H256::repeat_byte(4),
			primary: AccountId32::new([9; 32]),
			replicas: vec![AccountId32::new([1; 32]), AccountId32::new([2; 32])],
			authorities: vec![
				authority(9, ProviderDutyRole::Primary, 0, false, false),
				authority(1, ProviderDutyRole::Replica, 1, true, true),
				authority(2, ProviderDutyRole::Replica, 2, true, false),
			],
			initiator: Some(AccountId32::new([1; 32])),
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
			previous_checkpoint: Some(100),
			expected_next_start_seq: 5,
			required_primary_confirmations: 1,
			required_replica_confirmations: 2,
		}
	}

	fn promotion_duty_for(seed: u8) -> CheckpointDutyInfo<AccountId32, H256, u32> {
		let mut duty = promotion_duty();
		duty.bucket_id = H256::repeat_byte(seed);
		duty.duty_id = H256::repeat_byte(seed.saturating_add(100));
		duty
	}

	fn valid_response() -> Vec<u8> {
		Versioned {
			version: RESPONSE_VERSION,
			value: Some(CheckpointInfo {
				bucket_id: H256::repeat_byte(4),
				commitment: CommitmentInfo {
					mmr_root: H256::repeat_byte(5),
					start_seq: 7,
					leaf_count: 3,
				},
				checkpoint_block: 115u32,
				primary_signers: 1,
				commitment_nonce: 100,
				replica_confirmations: vec![AccountId32::new([2; 32]), AccountId32::new([3; 32])],
			}),
		}
		.encode()
	}

	fn finality_digest(
		submission_id: &str,
		finalized_hash: [u8; 32],
		finalized_number: u32,
		extrinsic_hash: [u8; 32],
	) -> [u8; 32] {
		let submission_id: [u8; 32] = hex::decode(submission_id).unwrap().try_into().unwrap();
		let mut input = b"cord/provider/checkpoint-finality-attestation/v1".to_vec();
		1u8.encode_to(&mut input);
		submission_id.encode_to(&mut input);
		finalized_hash.encode_to(&mut input);
		finalized_number.encode_to(&mut input);
		extrinsic_hash.encode_to(&mut input);
		b"finalized".as_slice().encode_to(&mut input);
		sp_crypto_hashing::blake2_256(&input)
	}

	struct MockLane {
		stack: Arc<CheckpointStack>,
		metadata: subxt::Metadata,
		calls: AtomicUsize,
		guard_available: AtomicBool,
		reject: bool,
	}

	impl MockLane {
		fn new(stack: Arc<CheckpointStack>) -> Self {
			Self {
				stack,
				metadata: metadata(),
				calls: AtomicUsize::new(0),
				guard_available: AtomicBool::new(false),
				reject: false,
			}
		}
	}

	#[async_trait]
	impl CheckpointFinalityLane for MockLane {
		fn metadata(&self) -> &subxt::Metadata {
			&self.metadata
		}

		fn signer_account(&self) -> [u8; 32] {
			[1; 32]
		}

		fn service_key(&self) -> [u8; 32] {
			pair(1).public().0
		}

		async fn submit_and_finalize(
			&self,
			intent_id: &str,
			_payload: subxt::tx::DynamicPayload,
		) -> Result<FinalizedEvidence, ContentError> {
			self.calls.fetch_add(1, Ordering::SeqCst);
			self.guard_available.store(self.stack.guard_is_available(), Ordering::SeqCst);
			if self.reject {
				return Err(ContentError::Io("injected finality failure".into()));
			}
			let submission_id = intent_id
				.strip_prefix("orbis-checkpoint-v2-")
				.ok_or(ContentError::IntegrityFailed)?;
			let digest = finality_digest(submission_id, [22; 32], 120, [23; 32]);
			Ok(FinalizedEvidence {
				block_hash: [22; 32],
				block_number: 120,
				extrinsic_hash: [23; 32],
				finality_attestation_version: 1,
				finality_signature: pair(1).sign(&digest).0,
			})
		}
	}

	struct PendingLane {
		metadata: subxt::Metadata,
	}

	#[async_trait]
	impl CheckpointFinalityLane for PendingLane {
		fn metadata(&self) -> &subxt::Metadata {
			&self.metadata
		}

		fn signer_account(&self) -> [u8; 32] {
			[1; 32]
		}

		fn service_key(&self) -> [u8; 32] {
			pair(1).public().0
		}

		async fn submit_and_finalize(
			&self,
			_intent_id: &str,
			_payload: subxt::tx::DynamicPayload,
		) -> Result<FinalizedEvidence, ContentError> {
			std::future::pending().await
		}
	}

	#[async_trait]
	impl PromotionFinalityLane for PendingLane {
		fn metadata(&self) -> &subxt::Metadata {
			&self.metadata
		}

		fn signer_account(&self) -> [u8; 32] {
			[1; 32]
		}

		fn service_key(&self) -> [u8; 32] {
			pair(1).public().0
		}

		async fn submit_and_finalize(
			&self,
			_intent_id: &str,
			_intent: &FallbackPromotionIntentV2,
			_payload: subxt::tx::DynamicPayload,
		) -> Result<FinalizedEvidence, ContentError> {
			std::future::pending().await
		}
	}

	struct MockAuthority {
		stack: Arc<CheckpointStack>,
		observation: FinalizedCheckpointObservation,
		calls: AtomicUsize,
		guard_available: AtomicBool,
		topology_calls: AtomicUsize,
		publication_before_topology: AtomicBool,
		slow_topology: bool,
	}

	#[async_trait]
	impl CheckpointPublicationAuthority for MockAuthority {
		async fn checkpoint_observation_at(
			&self,
			bucket_id: [u8; 32],
			finalized_hash: [u8; 32],
			finalized_number: u32,
		) -> Result<FinalizedCheckpointObservation, ChainError> {
			self.calls.fetch_add(1, Ordering::SeqCst);
			self.guard_available.store(self.stack.guard_is_available(), Ordering::SeqCst);
			if bucket_id != [4; 32] || finalized_hash != [22; 32] || finalized_number != 120 {
				return Err(ChainError::Rejected(
					"worker did not bind the finalized receipt".into(),
				));
			}
			Ok(self.observation.clone())
		}
	}

	#[async_trait]
	impl ReplicationAuthority for MockAuthority {
		async fn replication_topology(
			&self,
			_bucket_id: [u8; 32],
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			self.topology_calls.fetch_add(1, Ordering::SeqCst);
			self.publication_before_topology
				.store(self.calls.load(Ordering::SeqCst) > 0, Ordering::SeqCst);
			if self.slow_topology {
				return std::future::pending().await;
			}
			Err(ChainError::Rejected("unexpected promotion topology read".into()))
		}

		async fn replication_topology_at(
			&self,
			_bucket_id: [u8; 32],
			_finalized_hash: [u8; 32],
			_finalized_number: u32,
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			self.topology_calls.fetch_add(1, Ordering::SeqCst);
			self.publication_before_topology
				.store(self.calls.load(Ordering::SeqCst) > 0, Ordering::SeqCst);
			if self.slow_topology {
				return std::future::pending().await;
			}
			Err(ChainError::Rejected("unexpected pinned promotion topology read".into()))
		}
	}

	struct PendingAuthority;

	#[async_trait]
	impl CheckpointPublicationAuthority for PendingAuthority {
		async fn checkpoint_observation_at(
			&self,
			_bucket_id: [u8; 32],
			_finalized_hash: [u8; 32],
			_finalized_number: u32,
		) -> Result<FinalizedCheckpointObservation, ChainError> {
			std::future::pending().await
		}
	}

	fn authority(
		stack: Arc<CheckpointStack>,
		finalized_hash: [u8; 32],
		finalized_number: u32,
		response_scale: Vec<u8>,
	) -> MockAuthority {
		MockAuthority {
			stack,
			observation: FinalizedCheckpointObservation {
				finalized_hash,
				finalized_number,
				response_scale,
			},
			calls: AtomicUsize::new(0),
			guard_available: AtomicBool::new(false),
			topology_calls: AtomicUsize::new(0),
			publication_before_topology: AtomicBool::new(false),
			slow_topology: false,
		}
	}

	async fn finalized_stack(temp: &TempDir) -> Arc<CheckpointStack> {
		let stack = Arc::new(CheckpointStack::open(temp.path()).unwrap());
		stack.enqueue_checkpoint_for_test(&checkpoint_input()).unwrap();
		let lane = MockLane::new(Arc::clone(&stack));
		stack.consume_one_with_lane_bounded(&lane, 1).await.unwrap().unwrap();
		stack
	}

	#[tokio::test]
	async fn live_consume_exact_observation_and_publication_release_the_guard() {
		let temp = TempDir::new().unwrap();
		let stack = Arc::new(CheckpointStack::open(temp.path()).unwrap());
		stack.enqueue_checkpoint_for_test(&checkpoint_input()).unwrap();
		let lane = MockLane::new(Arc::clone(&stack));
		let authority = authority(Arc::clone(&stack), [22; 32], 120, valid_response());

		let result = tick(&authority, &stack, &lane).await.unwrap();
		assert!(result.finalized.is_some());
		assert_eq!(result.published.len(), 1);
		assert!(lane.guard_available.load(Ordering::SeqCst));
		assert!(authority.guard_available.load(Ordering::SeqCst));
		assert!(stack.guard_is_available());
		assert!(stack.pending_checkpoint_publications(MAX_PUBLICATIONS).unwrap().is_empty());
	}

	#[tokio::test]
	async fn pending_finality_is_cancelled_without_blocking_publication_in_the_same_tick() {
		let temp = TempDir::new().unwrap();
		let stack = finalized_stack(&temp).await;
		stack.enqueue_checkpoint_for_test(&checkpoint_input_for(6)).unwrap();
		let lane = PendingLane { metadata: metadata() };
		let authority = authority(Arc::clone(&stack), [22; 32], 120, valid_response());

		assert!(matches!(
			tick_with_timeouts(&authority, &stack, &lane, Duration::ZERO, Duration::from_secs(1)).await,
			Err(ContentError::Io(message)) if message.contains("finality attempt timed out")
		));
		assert_eq!(authority.calls.load(Ordering::SeqCst), 1);
		assert!(stack.pending_checkpoint_publications(MAX_PUBLICATIONS).unwrap().is_empty());
	}

	#[tokio::test]
	async fn promotion_timeout_does_not_block_checkpoint_publication_on_the_shared_lane() {
		let temp = TempDir::new().unwrap();
		let stack = finalized_stack(&temp).await;
		stack
			.authorize_checkpoint_promotion(
				&AccountId32::new([1; 32]),
				&promotion_duty().encode(),
				&pair(1),
			)
			.unwrap();
		let store = DiskStore::open(
			temp.path(),
			NodeProfile {
				provider: hex::encode([1; 32]),
				endpoint: "https://provider.invalid".into(),
				service_key: hex::encode(pair(1).public().0),
				region: None,
			},
			1024,
		)
		.unwrap();
		let scheduler = PromotionDiscoveryScheduler::open(temp.path()).unwrap();
		let lane = PendingLane { metadata: metadata() };
		let authority = authority(Arc::clone(&stack), [22; 32], 120, valid_response());

		assert!(matches!(
			tick_with_promotions(
				&authority,
				&stack,
				&store,
				&lane,
				&scheduler,
				[1; 32],
				&pair(1),
				Duration::from_millis(1),
				Duration::from_secs(1),
				Duration::from_secs(1),
				Duration::from_secs(1),
			)
			.await,
			Err(ContentError::Io(message))
				if message.contains("checkpoint promotion finality timed out")
		));
		assert_eq!(authority.calls.load(Ordering::SeqCst), 1);
		assert!(stack.pending_checkpoint_publications(MAX_PUBLICATIONS).unwrap().is_empty());
	}

	#[tokio::test]
	async fn eight_slow_promotion_topologies_run_after_checkpoint_publication() {
		let temp = TempDir::new().unwrap();
		let stack = finalized_stack(&temp).await;
		let profile = NodeProfile {
			provider: hex::encode([1; 32]),
			endpoint: "https://provider.invalid".into(),
			service_key: hex::encode(pair(1).public().0),
			region: None,
		};
		let store = DiskStore::open(temp.path(), profile.clone(), 1024).unwrap();
		let duties = (20..28)
			.map(|seed| {
				crate::chain::validate_checkpoint_duty(
					promotion_duty_for(seed),
					&AccountId32::new([1; 32]),
					pair(1).public().0,
					120,
				)
				.unwrap()
			})
			.collect();
		store
			.stage_checkpoint_duty_page(CheckpointDutyBatch {
				finalized_hash: hex::encode([5; 32]),
				finalized_number: 130,
				provider: profile.provider,
				snapshot_checkpoint: 120,
				requested_cursor: None,
				next_cursor: None,
				duties,
			})
			.unwrap();
		let scheduler = PromotionDiscoveryScheduler::open(temp.path()).unwrap();
		let lane = PendingLane { metadata: metadata() };
		let mut authority = authority(Arc::clone(&stack), [22; 32], 120, valid_response());
		authority.slow_topology = true;

		assert!(matches!(
			tokio::time::timeout(
				Duration::from_millis(250),
				tick_with_promotions(
					&authority,
					&stack,
					&store,
					&lane,
					&scheduler,
					[1; 32],
					&pair(1),
					Duration::from_secs(1),
					Duration::from_millis(5),
					Duration::from_secs(1),
					Duration::from_secs(1),
				),
			)
			.await
			.unwrap(),
			Err(ContentError::Io(message))
				if message.contains("pinned promotion topology timed out")
		));
		assert_eq!(authority.calls.load(Ordering::SeqCst), 1);
		assert_eq!(authority.topology_calls.load(Ordering::SeqCst), 8);
		assert!(authority.publication_before_topology.load(Ordering::SeqCst));
		assert!(stack.pending_checkpoint_publications(MAX_PUBLICATIONS).unwrap().is_empty());
	}

	#[tokio::test]
	async fn pending_observation_is_cancelled_and_remains_durably_retryable() {
		let temp = TempDir::new().unwrap();
		let stack = finalized_stack(&temp).await;
		let lane = MockLane::new(Arc::clone(&stack));

		assert!(matches!(
			tick_with_timeouts(
				&PendingAuthority,
				&stack,
				&lane,
				Duration::from_secs(1),
				Duration::ZERO,
			)
			.await,
			Err(ContentError::Io(message)) if message.contains("observation timed out")
		));
		assert_eq!(stack.pending_checkpoint_publications(MAX_PUBLICATIONS).unwrap().len(), 1);
	}

	#[tokio::test]
	async fn eight_failed_publications_rotate_past_the_oldest_and_survive_reopen() {
		let temp = TempDir::new().unwrap();
		let stack = Arc::new(CheckpointStack::open(temp.path()).unwrap());
		let lane = MockLane::new(Arc::clone(&stack));
		for seed in 4..14 {
			stack.enqueue_checkpoint_for_test(&checkpoint_input_for(seed)).unwrap();
			stack.consume_one_with_lane_bounded(&lane, 1).await.unwrap().unwrap();
		}
		let first = stack.reserve_checkpoint_publications(MAX_PUBLICATIONS).unwrap();
		assert_eq!(first.len(), 8);
		let first_ids = first
			.iter()
			.map(|intent| intent.submission.submission_id.clone())
			.collect::<std::collections::HashSet<_>>();
		drop(stack);

		let reopened = CheckpointStack::open(temp.path()).unwrap();
		let second = reopened.reserve_checkpoint_publications(MAX_PUBLICATIONS).unwrap();
		assert_eq!(second.len(), 8);
		assert_eq!(
			second
				.iter()
				.filter(|intent| !first_ids.contains(&intent.submission.submission_id))
				.count(),
			2
		);
		assert_eq!(
			second
				.iter()
				.map(|intent| &intent.submission.submission_id)
				.collect::<std::collections::HashSet<_>>()
				.len(),
			8
		);
	}

	#[tokio::test]
	async fn publication_cursor_is_cross_validated_against_finality_on_reopen() {
		let temp = TempDir::new().unwrap();
		let stack = finalized_stack(&temp).await;
		let intent = stack.reserve_checkpoint_publications(1).unwrap().pop().unwrap();
		drop(stack);
		std::fs::remove_file(
			temp.path()
				.join("checkpoint-finalized-receipts-v2")
				.join(format!("{}.json", intent.submission.submission_id)),
		)
		.unwrap();

		assert!(matches!(CheckpointStack::open(temp.path()), Err(ContentError::IntegrityFailed)));
	}

	#[tokio::test]
	async fn publication_is_impossible_without_a_durable_finality_receipt() {
		let temp = TempDir::new().unwrap();
		let stack = Arc::new(CheckpointStack::open(temp.path()).unwrap());
		stack.enqueue_checkpoint_for_test(&checkpoint_input()).unwrap();
		let mut lane = MockLane::new(Arc::clone(&stack));
		lane.reject = true;
		let authority = authority(Arc::clone(&stack), [22; 32], 120, valid_response());

		assert!(matches!(tick(&authority, &stack, &lane).await, Err(ContentError::Io(_))));
		assert_eq!(authority.calls.load(Ordering::SeqCst), 0);
		assert!(stack.pending_checkpoint_publications(MAX_PUBLICATIONS).unwrap().is_empty());
	}

	#[test]
	fn restart_rejects_a_publication_without_its_authenticated_finality_receipt() {
		let temp = TempDir::new().unwrap();
		let outbox = CheckpointOutboxV2::open(temp.path()).unwrap();
		let submission = outbox.enqueue(&checkpoint_input()).unwrap().submission;
		let publications = CheckpointPublicationStoreV1::open(temp.path()).unwrap();
		publications
			.publish(&FinalizedCheckpointPublicationInputV1 {
				submission,
				finalized_hash: H256::repeat_byte(22),
				finalized_number: 120,
				response_scale: valid_response(),
			})
			.unwrap();
		drop(publications);
		drop(outbox);

		assert!(matches!(CheckpointStack::open(temp.path()), Err(ContentError::IntegrityFailed)));
	}

	#[tokio::test]
	async fn finalized_but_unpublished_record_is_recovered_after_reopen() {
		let temp = TempDir::new().unwrap();
		let stack = finalized_stack(&temp).await;
		assert_eq!(stack.pending_checkpoint_publications(MAX_PUBLICATIONS).unwrap().len(), 1);
		drop(stack);

		let reopened = Arc::new(CheckpointStack::open(temp.path()).unwrap());
		let lane = MockLane::new(Arc::clone(&reopened));
		let authority = authority(Arc::clone(&reopened), [22; 32], 120, valid_response());
		let result = tick(&authority, &reopened, &lane).await.unwrap();
		assert!(result.finalized.is_none());
		assert_eq!(result.published.len(), 1);
		assert_eq!(lane.calls.load(Ordering::SeqCst), 0);
		drop(reopened);

		let reopened = Arc::new(CheckpointStack::open(temp.path()).unwrap());
		assert!(reopened.pending_checkpoint_publications(MAX_PUBLICATIONS).unwrap().is_empty());
	}

	#[tokio::test]
	async fn mismatched_observation_identity_or_response_fails_closed() {
		for (hash, number, response) in [
			([24; 32], 120, valid_response()),
			([22; 32], 121, valid_response()),
			(
				[22; 32],
				120,
				Versioned::<CheckpointInfo<AccountId32, H256, u32>> {
					version: RESPONSE_VERSION,
					value: None,
				}
				.encode(),
			),
		] {
			let temp = TempDir::new().unwrap();
			let stack = finalized_stack(&temp).await;
			let lane = MockLane::new(Arc::clone(&stack));
			let authority = authority(Arc::clone(&stack), hash, number, response);
			assert!(matches!(
				tick(&authority, &stack, &lane).await,
				Err(ContentError::IntegrityFailed)
			));
			assert_eq!(stack.pending_checkpoint_publications(MAX_PUBLICATIONS).unwrap().len(), 1);
		}
	}
}
