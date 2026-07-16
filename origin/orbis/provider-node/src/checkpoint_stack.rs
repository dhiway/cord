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

//! Private ownership boundary for the provider checkpoint control plane.

use std::{
	path::Path,
	sync::{Mutex, MutexGuard},
};

use crate::{
	checkpoint::{
		checkpoint_outbox::{CheckpointOutboxV2, CheckpointSubmissionV2},
		checkpoint_primary::CheckpointPrimaryQuorumStore,
		checkpoint_promotion::CheckpointPromotionStoreV1,
		checkpoint_publication::CheckpointPublicationStoreV1,
		checkpoint_quorum::ReplicaConfirmationStore,
		CheckpointProposalStore,
	},
	replication::ReplicationIntentStore,
	storage::bucket_mmr::BucketMmrStore,
	ContentError, StreamingStore,
};

/// All durable checkpoint kernels opened against one provider root.
///
/// Operations which cross the chain or network boundary must first clone an owned intent from
/// this state, release the guard, perform the external work, and then reacquire the stack to
/// persist the result. The stack intentionally exposes no async guard.
struct CheckpointStackState {
	streaming: StreamingStore,
	bucket_mmr: BucketMmrStore,
	proposals: CheckpointProposalStore,
	replica_confirmations: ReplicaConfirmationStore,
	primary_quorum: CheckpointPrimaryQuorumStore,
	outbox: std::sync::Arc<CheckpointOutboxV2>,
	publications: CheckpointPublicationStoreV1,
	fallback_promotions: CheckpointPromotionStoreV1,
	replication: ReplicationIntentStore,
}

/// Cohesive private checkpoint state owned through one synchronization boundary.
pub(crate) struct CheckpointStack {
	state: Mutex<CheckpointStackState>,
}

impl CheckpointStack {
	/// Open every checkpoint kernel against the same provider root.
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let root = root.as_ref();
		let streaming = StreamingStore::open(root)?;
		let bucket_mmr = BucketMmrStore::open(root, &streaming)?;
		let state = CheckpointStackState {
			streaming,
			bucket_mmr,
			proposals: CheckpointProposalStore::open(root)?,
			replica_confirmations: ReplicaConfirmationStore::open(root)?,
			primary_quorum: CheckpointPrimaryQuorumStore::open(root)?,
			outbox: std::sync::Arc::new(CheckpointOutboxV2::open(root)?),
			publications: CheckpointPublicationStoreV1::open(root)?,
			fallback_promotions: CheckpointPromotionStoreV1::open(root)?,
			replication: ReplicationIntentStore::open(root)?,
		};
		Ok(Self { state: Mutex::new(state) })
	}

	/// Clone each independent bucket head so external finality work never borrows the stack guard.
	pub(crate) fn submission_heads(&self) -> Result<Vec<CheckpointSubmissionV2>, ContentError> {
		self.lock()?.outbox.pending_submission_heads()
	}

	/// Consume one durable v2 submission without retaining the stack guard across finality.
	#[cfg(feature = "checkpoint-live")]
	pub(crate) async fn consume_one_with_lane(
		&self,
		lane: &impl crate::checkpoint::checkpoint_submitter::CheckpointFinalityLane,
	) -> Result<
		Option<crate::checkpoint::checkpoint_outbox::CheckpointFinalizedReceiptV2>,
		ContentError,
	> {
		let outbox = std::sync::Arc::clone(&self.lock()?.outbox);
		crate::checkpoint::checkpoint_submitter::consume_one_with_lane(&outbox, lane).await
	}

	fn lock(&self) -> Result<MutexGuard<'_, CheckpointStackState>, ContentError> {
		self.state.lock().map_err(|_| ContentError::IntegrityFailed)
	}
}

#[cfg(test)]
mod tests {
	use std::{fs, path::Path, sync::Arc};

	use async_trait::async_trait;
	#[cfg(feature = "checkpoint-live")]
	use codec::Encode;
	#[cfg(feature = "checkpoint-live")]
	use frame_metadata::v15::{
		CustomMetadata, ExtrinsicMetadata, OuterEnums, PalletCallMetadata, PalletMetadata,
		RuntimeMetadataV15,
	};
	#[cfg(feature = "checkpoint-live")]
	use pallet_orbis_storage_provider::{
		CheckpointContextV1, CommitmentPayloadV2, CommitmentV1, ReplicaSignature,
	};
	#[cfg(feature = "checkpoint-live")]
	use scale_info::{meta_type, TypeInfo};
	use sp_core::Pair as _;
	#[cfg(feature = "checkpoint-live")]
	use sp_core::{crypto::AccountId32, ed25519, H256};
	#[cfg(feature = "checkpoint-live")]
	use std::sync::atomic::{AtomicBool, Ordering};
	use tempfile::TempDir;

	use super::*;
	#[cfg(feature = "checkpoint-live")]
	use crate::checkpoint::{
		checkpoint_outbox::CheckpointSubmissionInputV2,
		checkpoint_quorum::{checkpoint_context_digest, checkpoint_digest},
		checkpoint_submitter::{CheckpointFinalityLane, FinalizedEvidence},
	};
	use crate::{
		AgreementAuthorization, ChainAuthority, ChainError, ChallengeBatch, CheckpointDutyBatch,
		CheckpointDutyPageRequest, DiskStore, JsonlCheckpointOutbox, NodeProfile, ProviderService,
	};

	const DURABLE_ROOTS: [&str; 12] = [
		"streaming-v1",
		"bucket-mmr-v3",
		"checkpoint-proposals-v2",
		"checkpoint-confirmations-v1",
		"checkpoint-primary-quorum-v1",
		"checkpoint-submissions-v2",
		"checkpoint-receipts-v2",
		"checkpoint-finalized-receipts-v2",
		"checkpoint-scheduler-v1",
		"checkpoint-publications-v1",
		"checkpoint-promotions-v1",
		"replication-v1",
	];

	struct NoopAuthority;

	#[async_trait]
	impl ChainAuthority for NoopAuthority {
		async fn authorize_commit(
			&self,
			_agreement_id: [u8; 32],
			_content_commitment: [u8; 32],
			_bytes: u64,
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}

		async fn authorize_delete(
			&self,
			_agreement_id: [u8; 32],
			_content_commitment: [u8; 32],
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}

		async fn challenge_duties(
			&self,
			_after_block: Option<u32>,
		) -> Result<ChallengeBatch, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}

		async fn checkpoint_duties(
			&self,
			_request: Option<CheckpointDutyPageRequest>,
		) -> Result<CheckpointDutyBatch, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}
	}

	fn profile() -> NodeProfile {
		let service_key = sp_core::ed25519::Pair::from_seed(&[7u8; 32]).public();
		NodeProfile {
			provider: "01".repeat(32),
			endpoint: "http://127.0.0.1:8080".into(),
			service_key: hex::encode(service_key.0),
			region: None,
		}
	}

	fn service(
		store: Arc<DiskStore>,
		root: &Path,
	) -> Result<ProviderService<NoopAuthority>, ContentError> {
		ProviderService::new(
			store,
			Arc::new(NoopAuthority),
			sp_core::ed25519::Pair::from_seed(&[7u8; 32]),
			Arc::new(JsonlCheckpointOutbox::new(root.join("test-outbox.jsonl"))),
		)
	}

	#[cfg(feature = "checkpoint-live")]
	type Confirmations = Vec<ReplicaSignature<AccountId32>>;

	#[cfg(feature = "checkpoint-live")]
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

	#[cfg(feature = "checkpoint-live")]
	fn checkpoint_metadata() -> subxt::Metadata {
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

	#[cfg(feature = "checkpoint-live")]
	fn pair(seed: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[seed; 32])
	}

	#[cfg(feature = "checkpoint-live")]
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

	#[cfg(feature = "checkpoint-live")]
	fn finality_digest(
		submission_id: &str,
		block_hash: [u8; 32],
		block_number: u32,
		extrinsic_hash: [u8; 32],
	) -> [u8; 32] {
		let submission_id: [u8; 32] = hex::decode(submission_id).unwrap().try_into().unwrap();
		let mut input = b"cord/provider/checkpoint-finality-attestation/v1".to_vec();
		1u8.encode_to(&mut input);
		submission_id.encode_to(&mut input);
		block_hash.encode_to(&mut input);
		block_number.encode_to(&mut input);
		extrinsic_hash.encode_to(&mut input);
		b"finalized".as_slice().encode_to(&mut input);
		sp_crypto_hashing::blake2_256(&input)
	}

	#[cfg(feature = "checkpoint-live")]
	struct GuardCheckingLane<'a> {
		stack: &'a CheckpointStack,
		metadata: subxt::Metadata,
		observed_released_guard: AtomicBool,
	}

	#[cfg(feature = "checkpoint-live")]
	#[async_trait]
	impl CheckpointFinalityLane for GuardCheckingLane<'_> {
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
			let state = self.stack.state.try_lock().map_err(|_| ContentError::IntegrityFailed)?;
			if Arc::strong_count(&state.outbox) != 2 {
				return Err(ContentError::IntegrityFailed);
			}
			drop(state);
			self.observed_released_guard.store(true, Ordering::SeqCst);
			let submission_id = intent_id
				.strip_prefix("orbis-checkpoint-v2-")
				.ok_or(ContentError::IntegrityFailed)?;
			let block_hash = [8; 32];
			let block_number = 44;
			let extrinsic_hash = [9; 32];
			Ok(FinalizedEvidence {
				block_hash,
				block_number,
				extrinsic_hash,
				finality_attestation_version: 1,
				finality_signature: pair(1)
					.sign(&finality_digest(submission_id, block_hash, block_number, extrinsic_hash))
					.0,
			})
		}
	}

	#[test]
	fn one_stack_opens_every_kernel_and_releases_owned_intents() {
		let temp = TempDir::new().unwrap();
		let stack = CheckpointStack::open(temp.path()).unwrap();

		for root in DURABLE_ROOTS {
			assert!(temp.path().join(root).is_dir(), "missing durable root {root}");
		}
		assert!(stack.submission_heads().unwrap().is_empty());
		assert!(stack.state.try_lock().is_ok());
		let state = stack.lock().unwrap();
		assert_eq!(Arc::strong_count(&state.outbox), 1);
		assert!(state.proposals.pending_checkpoint_proposals().unwrap().is_empty());
		assert!(state.outbox.pending_submissions().unwrap().is_empty());
		let _owned_kernels = (
			&state.streaming,
			&state.bucket_mmr,
			&state.replica_confirmations,
			&state.primary_quorum,
			&state.publications,
			&state.fallback_promotions,
			&state.replication,
		);
	}

	#[test]
	fn stacks_are_isolated_by_provider_root() {
		let first = TempDir::new().unwrap();
		let second = TempDir::new().unwrap();
		drop(CheckpointStack::open(first.path()).unwrap());
		drop(CheckpointStack::open(second.path()).unwrap());

		fs::write(first.path().join("checkpoint-submissions-v2/unbounded.bin"), b"invalid")
			.unwrap();
		assert!(matches!(CheckpointStack::open(first.path()), Err(ContentError::IntegrityFailed)));
		assert!(CheckpointStack::open(second.path()).is_ok());
	}

	#[test]
	fn provider_service_owns_all_checkpoint_roots_under_its_disk_store() {
		let temp = TempDir::new().unwrap();
		let store = Arc::new(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let service = service(store, temp.path()).unwrap();

		assert!(service.checkpoint_stack().submission_heads().unwrap().is_empty());
		assert_eq!(service.store().root(), temp.path());
		for durable_root in DURABLE_ROOTS {
			let path = temp.path().join(durable_root);
			assert!(path.is_dir(), "missing durable root {durable_root}");
			assert_eq!(path.parent(), Some(temp.path()));
		}
	}

	#[test]
	fn provider_service_fails_closed_for_every_corrupt_checkpoint_root() {
		for durable_root in DURABLE_ROOTS {
			let temp = TempDir::new().unwrap();
			let store = Arc::new(DiskStore::open(temp.path(), profile(), 1024).unwrap());
			drop(service(store.clone(), temp.path()).unwrap());
			let corrupt_path = temp.path().join(durable_root);
			fs::remove_dir_all(&corrupt_path).unwrap();
			fs::write(&corrupt_path, b"corrupt durable-root shape").unwrap();

			assert!(
				service(store, temp.path()).is_err(),
				"service opened corrupt durable root {durable_root}"
			);
		}
	}

	#[cfg(feature = "checkpoint-live")]
	#[tokio::test]
	async fn stack_consumer_releases_guard_and_persists_finalized_receipt_across_restart() {
		let temp = TempDir::new().unwrap();
		let stack = CheckpointStack::open(temp.path()).unwrap();
		let submission =
			stack.lock().unwrap().outbox.enqueue(&checkpoint_input()).unwrap().submission;
		let lane = GuardCheckingLane {
			stack: &stack,
			metadata: checkpoint_metadata(),
			observed_released_guard: AtomicBool::new(false),
		};

		let receipt = stack.consume_one_with_lane(&lane).await.unwrap().unwrap();
		assert!(lane.observed_released_guard.load(Ordering::SeqCst));
		assert_eq!(receipt.submission_id, submission.submission_id);
		assert!(stack.submission_heads().unwrap().is_empty());
		drop(stack);

		let reopened = CheckpointStack::open(temp.path()).unwrap();
		assert!(reopened.submission_heads().unwrap().is_empty());
		assert_eq!(
			reopened
				.lock()
				.unwrap()
				.outbox
				.finalized_receipt(&submission.submission_id)
				.unwrap(),
			Some(receipt)
		);
	}
}
