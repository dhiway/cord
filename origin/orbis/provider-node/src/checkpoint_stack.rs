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
	collections::HashMap,
	path::Path,
	sync::{Mutex, MutexGuard},
};

use sp_core::{crypto::AccountId32, ed25519, H256};

use crate::{
	checkpoint::{
		checkpoint_outbox::{
			CheckpointOutboxV2, CheckpointSubmissionV2, PreparedCheckpointOutboxV2,
		},
		checkpoint_primary::{
			submission_input, CheckpointPrimaryQuorumStore, PreparedCheckpointPrimaryQuorumStore,
			PrimaryQuorumSnapshotV1,
		},
		checkpoint_promotion::{CheckpointPromotionStoreV2, PreparedCheckpointPromotionStoreV2},
		checkpoint_publication::{
			CheckpointPublicationStoreV1, FinalizedCheckpointPublicationInputV1,
			PreparedCheckpointPublicationStoreV1, PublishedCheckpointV1,
		},
		checkpoint_quorum::{PreparedReplicaConfirmationStore, ReplicaConfirmationStore},
		CheckpointProposalStore, PreparedCheckpointProposalStore, PreparedCheckpointProposalV2,
		ServiceKeySigner,
	},
	peer::{
		PeerChunkRequestV1, PeerChunkResponseV1, PeerSyncPageRequestV1, PeerSyncPageResponseV1,
	},
	peer_reply::{PeerReplyFault, PeerReplyStore, PreparedPeerReplyStore},
	replication::{
		PreparedReplicationIntentStore, ReplicationActionV1, ReplicationIntentStore,
		ReplicationIntentV1, ReplicationResumeV1, VerifiedIncomingChunkV1,
	},
	replication_session::ReplicationSessionV1,
	storage::{
		bucket_mmr::{BucketMmrStore, PreparedBucketMmrStore},
		streaming::{ManifestDeletionEvidence, PreparedStreamingStore, ReplicationIngressState},
		StreamingStore,
	},
	BeginStreaming, BucketId, CheckpointDuty, ContentError, DiskStore, IntegritySummary,
	StreamingDescriptor,
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
	fallback_promotions: std::sync::Arc<CheckpointPromotionStoreV2>,
	#[cfg(feature = "checkpoint-live")]
	promotion_discovery:
		std::sync::Arc<crate::checkpoint_promotion_worker::PromotionDiscoveryScheduler>,
	replication: ReplicationIntentStore,
	peer_replies: PeerReplyStore,
}

/// Cohesive private checkpoint state owned through one synchronization boundary.
pub(crate) struct CheckpointStack {
	state: Mutex<CheckpointStackState>,
}

/// Root-wide validated checkpoint-kernel view with no recovery actions applied.
pub(crate) struct PreparedCheckpointStack {
	streaming: PreparedStreamingStore,
	bucket_mmr: PreparedBucketMmrStore,
	proposals: PreparedCheckpointProposalStore,
	replica_confirmations: PreparedReplicaConfirmationStore,
	primary_quorum: PreparedCheckpointPrimaryQuorumStore,
	outbox: PreparedCheckpointOutboxV2,
	publications: PreparedCheckpointPublicationStoreV1,
	fallback_promotions: PreparedCheckpointPromotionStoreV2,
	#[cfg(feature = "checkpoint-live")]
	promotion_discovery: crate::checkpoint_promotion_worker::PreparedPromotionDiscoveryScheduler,
	replication: PreparedReplicationIntentStore,
	peer_replies: PreparedPeerReplyStore,
}

impl PreparedCheckpointStack {
	/// Apply only the retry-safe actions captured by the root-wide validated plan.
	pub(crate) fn apply(self) -> Result<CheckpointStack, ContentError> {
		let state = CheckpointStackState {
			streaming: self.streaming.apply()?,
			bucket_mmr: self.bucket_mmr.apply()?,
			proposals: self.proposals.apply()?,
			replica_confirmations: self.replica_confirmations.apply()?,
			primary_quorum: self.primary_quorum.apply()?,
			outbox: std::sync::Arc::new(self.outbox.apply()?),
			publications: self.publications.apply()?,
			fallback_promotions: std::sync::Arc::new(self.fallback_promotions.apply()?),
			#[cfg(feature = "checkpoint-live")]
			promotion_discovery: std::sync::Arc::new(self.promotion_discovery.apply()?),
			replication: self.replication.apply()?,
			peer_replies: self.peer_replies.apply()?,
		};
		Ok(CheckpointStack { state: Mutex::new(state) })
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckpointPublicationIntentV1 {
	pub(crate) submission: CheckpointSubmissionV2,
	pub(crate) finalized_hash: H256,
	pub(crate) finalized_number: u32,
}

impl CheckpointStack {
	/// Open every checkpoint kernel against the same provider root.
	#[cfg(any(test, feature = "evidence"))]
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		Self::prepare_open(root)?.apply()
	}

	/// Validate every kernel and cross-kernel relationship before any recovery action is applied.
	pub(crate) fn prepare_open(
		root: impl AsRef<Path>,
	) -> Result<PreparedCheckpointStack, ContentError> {
		let root = root.as_ref();
		let streaming = StreamingStore::prepare(root)?;
		let bucket_mmr = BucketMmrStore::prepare(root, &streaming)?;
		let outbox = CheckpointOutboxV2::prepare_open(root)?;
		let publications = CheckpointPublicationStoreV1::prepare_open(root)?;
		validate_prepared_publication_receipts(&outbox, &publications)?;
		Ok(PreparedCheckpointStack {
			streaming,
			bucket_mmr,
			proposals: CheckpointProposalStore::prepare_open(root)?,
			replica_confirmations: ReplicaConfirmationStore::prepare_open(root)?,
			primary_quorum: CheckpointPrimaryQuorumStore::prepare_open(root)?,
			outbox,
			publications,
			fallback_promotions: CheckpointPromotionStoreV2::prepare_open(root)?,
			#[cfg(feature = "checkpoint-live")]
			promotion_discovery:
				crate::checkpoint_promotion_worker::PromotionDiscoveryScheduler::prepare_open(root)?,
			replication: ReplicationIntentStore::prepare_open(root)?,
			peer_replies: PeerReplyStore::prepare_open(root)?,
		})
	}

	/// Audit the local byte plane and return only redacted readiness counts.
	pub(crate) fn integrity_summary(&self) -> Result<IntegritySummary, ContentError> {
		self.lock()?.streaming.integrity_summary()
	}

	/// Durably tombstone canonical manifest bytes while retaining bucket-MMR installation history.
	pub(crate) fn tombstone_manifest(
		&self,
		manifest: [u8; 32],
		bucket_id: BucketId,
		provider_commitment: [u8; 32],
		tombstoned_at: u32,
	) -> Result<ManifestDeletionEvidence, ContentError> {
		self.lock()?.streaming.tombstone_manifest(
			manifest,
			bucket_id,
			provider_commitment,
			tombstoned_at,
		)
	}

	/// Clone each independent bucket head so external finality work never borrows the stack guard.
	pub(crate) fn submission_heads(&self) -> Result<Vec<CheckpointSubmissionV2>, ContentError> {
		self.lock()?.outbox.pending_submission_heads()
	}

	/// Confirm one canonical private replica request against the exact current local inventory.
	pub(crate) fn confirm_checkpoint(
		&self,
		disk: &DiskStore,
		signer: &dyn ServiceKeySigner,
		request: &[u8],
	) -> Result<Vec<u8>, ContentError> {
		let state = self.lock()?;
		state.replica_confirmations.confirm(
			disk,
			&state.bucket_mmr,
			&state.streaming,
			signer,
			request,
		)
	}

	pub(crate) fn outstanding_checkpoint_quorums(
		&self,
	) -> Result<Vec<PreparedCheckpointProposalV2>, ContentError> {
		self.lock()?.primary_quorum.outstanding_proposals()
	}

	pub(crate) fn authorize_checkpoint_promotion(
		&self,
		provider: &AccountId32,
		duty_scale: &[u8],
		signer: &dyn ServiceKeySigner,
	) -> Result<crate::checkpoint::checkpoint_promotion::FallbackPromotionIntentV2, ContentError> {
		let store = std::sync::Arc::clone(&self.lock()?.fallback_promotions);
		store.authorize(provider, duty_scale, signer)
	}

	#[cfg(feature = "checkpoint-live")]
	pub(crate) fn checkpoint_promotion_discovery_scheduler(
		&self,
	) -> Result<
		std::sync::Arc<crate::checkpoint_promotion_worker::PromotionDiscoveryScheduler>,
		ContentError,
	> {
		Ok(std::sync::Arc::clone(&self.lock()?.promotion_discovery))
	}

	#[cfg(feature = "checkpoint-consumer")]
	pub(crate) async fn consume_checkpoint_promotion_with_lane_bounded(
		&self,
		lane: &impl crate::checkpoint::checkpoint_promotion_submitter::PromotionFinalityLane,
		max_attempts: usize,
	) -> Result<
		Option<crate::checkpoint::checkpoint_promotion::FallbackPromotionFinalizedReceiptV2>,
		ContentError,
	> {
		let store = std::sync::Arc::clone(&self.lock()?.fallback_promotions);
		crate::checkpoint::checkpoint_promotion_submitter::consume_one_with_lane_bounded(
			&store,
			lane,
			max_attempts,
		)
		.await
	}

	pub(crate) fn begin_checkpoint_quorum(
		&self,
		disk: &DiskStore,
		duty: &CheckpointDuty,
		signer: &dyn ServiceKeySigner,
	) -> Result<(PreparedCheckpointProposalV2, PrimaryQuorumSnapshotV1), ContentError> {
		let state = self.lock()?;
		let proposal = state.proposals.prepare(
			disk,
			&duty.duty_id,
			&state.bucket_mmr,
			&state.streaming,
			signer,
		)?;
		let snapshot = state.primary_quorum.begin(&proposal, signer)?;
		emit_quorum_if_ready(&state, &proposal, &snapshot)?;
		Ok((proposal, snapshot))
	}

	pub(crate) fn resume_checkpoint_quorum(
		&self,
		proposal: &PreparedCheckpointProposalV2,
		signer: &dyn ServiceKeySigner,
	) -> Result<PrimaryQuorumSnapshotV1, ContentError> {
		let state = self.lock()?;
		let snapshot = state.primary_quorum.begin(proposal, signer)?;
		emit_quorum_if_ready(&state, proposal, &snapshot)?;
		Ok(snapshot)
	}

	pub(crate) fn accept_checkpoint_confirmation(
		&self,
		proposal: &PreparedCheckpointProposalV2,
		response: &[u8],
	) -> Result<PrimaryQuorumSnapshotV1, ContentError> {
		let state = self.lock()?;
		let snapshot = state.primary_quorum.accept_response(proposal, response)?;
		emit_quorum_if_ready(&state, proposal, &snapshot)?;
		Ok(snapshot)
	}

	/// Replay an exact durable page reply without consulting mutable chain or content state.
	pub(crate) fn replay_peer_page(
		&self,
		request: &PeerSyncPageRequestV1,
	) -> Result<Option<Vec<u8>>, ContentError> {
		match self.lock()?.peer_replies.replay_page(request) {
			Ok(bytes) => Ok(Some(bytes)),
			Err(ContentError::NotFound) => Ok(None),
			Err(error) => Err(error),
		}
	}

	/// Replay an exact durable chunk reply without consulting mutable chain or content state.
	pub(crate) fn replay_peer_chunk(
		&self,
		request: &PeerChunkRequestV1,
	) -> Result<Option<Vec<u8>>, ContentError> {
		match self.lock()?.peer_replies.replay_chunk(request) {
			Ok(bytes) => Ok(Some(bytes)),
			Err(ContentError::NotFound) => Ok(None),
			Err(error) => Err(error),
		}
	}

	/// Build or replay one exact page response while holding the synchronous durability boundary.
	pub(crate) fn serve_peer_page(
		&self,
		request: &PeerSyncPageRequestV1,
		source: &ed25519::Pair,
	) -> Result<Vec<u8>, ContentError> {
		let state = self.lock()?;
		match state.peer_replies.replay_page(request) {
			Ok(bytes) => return Ok(bytes),
			Err(ContentError::NotFound) => {},
			Err(error) => return Err(error),
		}
		let context = request.context();
		let (cursor, limit) = request.page();
		let (items, next_cursor) = state.bucket_mmr.replication_page(
			&state.streaming,
			BucketId::from_bytes(context.bucket()),
			context.candidate_commitment(),
			cursor,
			limit,
		)?;
		let response =
			PeerSyncPageResponseV1::new_signed(request, items, next_cursor, source)?.encode_wire();
		state.peer_replies.record_page(request, &response)
	}

	/// Build or replay one exact chunk response after proving its committed object descriptor.
	pub(crate) fn serve_peer_chunk(
		&self,
		request: &PeerChunkRequestV1,
		source: &ed25519::Pair,
	) -> Result<Vec<u8>, ContentError> {
		let state = self.lock()?;
		match state.peer_replies.replay_chunk(request) {
			Ok(bytes) => return Ok(bytes),
			Err(ContentError::NotFound) => {},
			Err(error) => return Err(error),
		}
		let context = request.context();
		let bucket_id = BucketId::from_bytes(context.bucket());
		let (object, index, _) = request.chunk();
		state.bucket_mmr.verify_replication_object(
			&state.streaming,
			bucket_id,
			context.candidate_commitment(),
			object,
		)?;
		let bytes = state.streaming.read_chunk_verified(object.cid(), index)?;
		let response = PeerChunkResponseV1::new_signed(request, bytes, source)?.encode_wire();
		state.peer_replies.record_chunk(request, &response)
	}

	pub(crate) fn plan_replication(
		&self,
		session: &ReplicationSessionV1,
		operation_id: [u8; 16],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.lock()?.replication.plan_session(session, operation_id)
	}

	pub(crate) fn replan_replication_source(
		&self,
		intent_key: &str,
		session: &ReplicationSessionV1,
		operation_id: [u8; 16],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.lock()?.replication.replan_source(intent_key, session, operation_id)
	}

	pub(crate) fn replication_resume_tick(
		&self,
		limit: usize,
	) -> Result<Vec<ReplicationResumeV1>, ContentError> {
		self.lock()?.replication.select_resume_tick_limit(limit)
	}

	/// Reverify the complete local bucket and compare it with the current checkpoint's full MMR
	/// root and end. Integrity/not-found outcomes mean repair is required; durable I/O failures
	/// remain fatal to discovery.
	pub(crate) fn replication_checkpoint_ready(
		&self,
		bucket_id: BucketId,
		mmr_root: [u8; 32],
		end: u64,
	) -> Result<bool, ContentError> {
		let state = self.lock()?;
		match state.bucket_mmr.commitment_candidate(&state.streaming, bucket_id, 0) {
			Ok(candidate) => Ok(candidate.mmr_root.0 == mmr_root && candidate.leaf_count == end),
			Err(ContentError::IntegrityFailed | ContentError::NotFound) => Ok(false),
			Err(error) => Err(error),
		}
	}

	pub(crate) fn replication_predecessor_total(
		&self,
		bucket_id: BucketId,
		start: u64,
	) -> Result<u64, ContentError> {
		if start == 0 {
			return Ok(0);
		}
		self.lock()?.bucket_mmr.commitment_predecessor_total(bucket_id, start)
	}

	pub(crate) fn next_replication_action(
		&self,
		intent_key: &str,
		target: &ed25519::Pair,
	) -> Result<ReplicationActionV1, ContentError> {
		self.lock()?.replication.next_action(intent_key, target)
	}

	pub(crate) fn accept_replication_page(
		&self,
		intent_key: &str,
		response: &[u8],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.lock()?.replication.attach_page_response(intent_key, response)
	}

	/// Authenticate, classify and durably store one response chunk without advancing its proof.
	pub(crate) fn persist_replication_chunk(
		&self,
		intent_key: &str,
		response: &[u8],
	) -> Result<(), ContentError> {
		let state = self.lock()?;
		let incoming = state.replication.inspect_chunk_response(intent_key, response)?;
		let structurally_present = match state
			.bucket_mmr
			.replication_slot_structurally_matches(incoming.bucket_id, &incoming.object)
		{
			Ok(present) => present,
			Err(ContentError::NotFound) => false,
			Err(error) => return Err(error),
		};
		if structurally_present {
			match ensure_exact_chunk(&state.streaming, &incoming) {
				Ok(()) => return Ok(()),
				Err(ContentError::IntegrityFailed | ContentError::NotFound) => {},
				Err(error) => return Err(error),
			}
		}
		match state.streaming.classify_replication_ingress(
			incoming.bucket_id,
			incoming.object.cid(),
			incoming.object.position().0,
			incoming.install_operation_id,
			incoming.repair_operation_id,
		)? {
			ReplicationIngressState::ExactReady => {
				ensure_exact_chunk(&state.streaming, &incoming)?;
				match state.bucket_mmr.append_verified(
					&state.streaming,
					incoming.bucket_id,
					incoming.install_operation_id,
				) {
					Ok(()) => state.streaming.retire_completed_replication_repair(
						incoming.bucket_id,
						incoming.object.cid(),
						incoming.repair_operation_id,
						incoming.install_operation_id,
					)?,
					Err(ContentError::NotFound) => {
						state
							.bucket_mmr
							.revalidate_repaired_bucket(&state.streaming, incoming.bucket_id)?;
						if !state.bucket_mmr.replication_slot_matches(
							&state.streaming,
							incoming.bucket_id,
							&incoming.object,
						)? {
							install_derived_replication_object(
								&state.streaming,
								&state.bucket_mmr,
								&incoming,
							)?;
						}
					},
					Err(error) => return Err(error),
				}
			},
			ReplicationIngressState::Fresh => {
				let descriptor = StreamingDescriptor {
					operation_id: incoming.install_operation_id,
					bucket_id: incoming.bucket_id,
					expected_cid: incoming.object.cid().into(),
					object_len: incoming.object.position().0,
				};
				match state.streaming.begin(descriptor)? {
					BeginStreaming::Installed(_) =>
						ensure_exact_chunk(&state.streaming, &incoming)?,
					BeginStreaming::Receiving(_) => {
						let permit = state
							.streaming
							.try_acquire_ingress(
								incoming.bucket_id,
								incoming.install_operation_id,
								incoming.index,
								incoming.bytes.len(),
							)?
							.ok_or(ContentError::ProviderRecoveryTableFull)?;
						state.streaming.push_chunk(permit, &incoming.bytes)?;
					},
				}
				if incoming.index as usize + 1 == incoming.object.chunk_hashes().len() {
					state.streaming.finalize(incoming.bucket_id, incoming.install_operation_id)?;
					state.bucket_mmr.append_verified(
						&state.streaming,
						incoming.bucket_id,
						incoming.install_operation_id,
					)?;
				}
			},
			ReplicationIngressState::MaterializeRepaired => {
				install_derived_replication_object(&state.streaming, &state.bucket_mmr, &incoming)?;
			},
			ReplicationIngressState::Repair => {
				let progress = state
					.streaming
					.begin_repair(incoming.object.cid(), incoming.repair_operation_id)?;
				if incoming.index > progress.next_chunk {
					return Err(ContentError::ChunkOutOfOrder);
				}
				let progress = state.streaming.push_repair_chunk(
					incoming.object.cid(),
					incoming.repair_operation_id,
					incoming.index,
					&incoming.bytes,
				)?;
				if progress.ready_to_finalize {
					state
						.streaming
						.finalize_repair(incoming.object.cid(), incoming.repair_operation_id)?;
					state
						.bucket_mmr
						.revalidate_repaired_bucket(&state.streaming, incoming.bucket_id)?;
					if !state.bucket_mmr.replication_slot_matches(
						&state.streaming,
						incoming.bucket_id,
						&incoming.object,
					)? {
						install_derived_replication_object(
							&state.streaming,
							&state.bucket_mmr,
							&incoming,
						)?;
					}
				}
			},
		}
		Ok(())
	}

	pub(crate) fn accept_replication_chunk(
		&self,
		intent_key: &str,
		response: &[u8],
	) -> Result<ReplicationIntentV1, ContentError> {
		self.lock()?.replication.attach_chunk_response(intent_key, response)
	}

	pub(crate) fn finish_replication_object(
		&self,
		intent_key: &str,
		sequence: u64,
		cid: &str,
		length: u64,
	) -> Result<ReplicationIntentV1, ContentError> {
		let state = self.lock()?;
		if length == 0 {
			let install =
				state.replication.object_installation(intent_key, sequence, cid, length)?;
			let structurally_present =
				match state.bucket_mmr.zero_replication_slot_structurally_matches(
					install.bucket_id,
					sequence,
					cid,
					install.cumulative_total,
				) {
					Ok(present) => present,
					Err(ContentError::NotFound) => false,
					Err(error) => return Err(error),
				};
			if structurally_present {
				match state.streaming.verified_replication_ready(
					install.bucket_id,
					cid,
					0,
					install.operation_id,
					install.repair_operation_id,
				) {
					Ok(_) => {},
					Err(ContentError::IntegrityFailed | ContentError::NotFound) => {
						// Force an exact full-file audit so an unobserved missing/corrupt empty
						// object is quarantined before entering the durable repair lifecycle.
						let _ = state.streaming.read_range_verified(cid, 0, 0);
						let progress =
							state.streaming.begin_repair(cid, install.repair_operation_id)?;
						if !progress.ready_to_finalize {
							return Err(ContentError::ChunkMissing);
						}
						state.streaming.finalize_repair(cid, install.repair_operation_id)?;
						state
							.bucket_mmr
							.revalidate_repaired_bucket(&state.streaming, install.bucket_id)?;
					},
					Err(error) => return Err(error),
				}
				return state.replication.complete_object(
					&state.streaming,
					intent_key,
					sequence,
					cid,
					length,
				);
			}
			let descriptor = StreamingDescriptor {
				operation_id: install.operation_id,
				bucket_id: install.bucket_id,
				expected_cid: cid.into(),
				object_len: 0,
			};
			match state.streaming.begin(descriptor)? {
				BeginStreaming::Receiving(progress) if progress.next_chunk == 0 => {
					state.streaming.finalize(install.bucket_id, install.operation_id)?;
				},
				BeginStreaming::Installed(_) => {},
				BeginStreaming::Receiving(_) => return Err(ContentError::IntegrityFailed),
			}
			state.bucket_mmr.append_verified(
				&state.streaming,
				install.bucket_id,
				install.operation_id,
			)?;
		}
		state
			.replication
			.complete_object(&state.streaming, intent_key, sequence, cid, length)
	}

	pub(crate) fn mark_replication_installed(
		&self,
		intent_key: &str,
	) -> Result<ReplicationIntentV1, ContentError> {
		self.lock()?.replication.mark_installed(intent_key)
	}

	pub(crate) fn commit_replication_mmr(
		&self,
		intent_key: &str,
	) -> Result<ReplicationIntentV1, ContentError> {
		let state = self.lock()?;
		state
			.replication
			.commit_local_mmr(&state.bucket_mmr, &state.streaming, intent_key)
	}

	#[doc(hidden)]
	pub(crate) fn inject_peer_reply_fault_once(
		&self,
		fault: PeerReplyFault,
	) -> Result<(), ContentError> {
		self.lock()?.peer_replies.inject_fault_once(fault)
	}

	#[cfg(test)]
	pub(crate) fn inject_replication_intent_fault_once(
		&self,
		fault: crate::replication::ReplicationFault,
	) -> Result<(), ContentError> {
		self.lock()?.replication.inject_fault_once(fault)
	}

	#[cfg(test)]
	pub(crate) fn inject_replication_streaming_fault_once(
		&self,
		fault: crate::StreamingFault,
	) -> Result<(), ContentError> {
		self.lock()?.streaming.inject_fault_once(fault)
	}

	#[cfg(test)]
	pub(crate) fn guard_is_available(&self) -> bool {
		self.state.try_lock().is_ok()
	}

	#[cfg(all(test, feature = "checkpoint-live"))]
	pub(crate) fn enqueue_checkpoint_for_test(
		&self,
		input: &crate::checkpoint::checkpoint_outbox::CheckpointSubmissionInputV2,
	) -> Result<CheckpointSubmissionV2, ContentError> {
		Ok(self.lock()?.outbox.enqueue(input)?.submission)
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

	/// Consume at most `max_attempts` bucket heads without retaining the stack guard across
	/// finality.
	#[cfg(feature = "checkpoint-live")]
	pub(crate) async fn consume_one_with_lane_bounded(
		&self,
		lane: &impl crate::checkpoint::checkpoint_submitter::CheckpointFinalityLane,
		max_attempts: usize,
	) -> Result<
		Option<crate::checkpoint::checkpoint_outbox::CheckpointFinalizedReceiptV2>,
		ContentError,
	> {
		let outbox = std::sync::Arc::clone(&self.lock()?.outbox);
		crate::checkpoint::checkpoint_submitter::consume_one_with_lane_bounded(
			&outbox,
			lane,
			max_attempts,
		)
		.await
	}

	pub(crate) fn pending_checkpoint_publications(
		&self,
		limit: usize,
	) -> Result<Vec<CheckpointPublicationIntentV1>, ContentError> {
		if limit == 0 {
			return Err(ContentError::IntegrityFailed);
		}
		let state = self.lock()?;
		let mut pending = checkpoint_publication_intents(&state)?;
		pending.truncate(limit);
		Ok(pending)
	}

	/// Select a unique bounded batch after the durable cursor and reserve it before network work.
	pub(crate) fn reserve_checkpoint_publications(
		&self,
		limit: usize,
	) -> Result<Vec<CheckpointPublicationIntentV1>, ContentError> {
		if limit == 0 {
			return Err(ContentError::IntegrityFailed);
		}
		let state = self.lock()?;
		let mut pending = checkpoint_publication_intents(&state)?;
		if let Some(cursor) = state.publications.cursor()? {
			let split = pending
				.iter()
				.position(|intent| {
					(intent.finalized_number, intent.submission.submission_id.as_str()) >
						(cursor.finalized_number, cursor.submission_id.as_str())
				})
				.unwrap_or(0);
			pending.rotate_left(split);
		}
		pending.truncate(limit);
		if let Some(last) = pending.last() {
			state.publications.reserve_cursor(
				&last.submission,
				last.finalized_hash,
				last.finalized_number,
			)?;
		}
		Ok(pending)
	}

	pub(crate) fn publish_checkpoint_observation(
		&self,
		intent: &CheckpointPublicationIntentV1,
		response_scale: Vec<u8>,
	) -> Result<PublishedCheckpointV1, ContentError> {
		let state = self.lock()?;
		let receipt = state
			.outbox
			.finalized_receipt(&intent.submission.submission_id)?
			.ok_or(ContentError::IntegrityFailed)?;
		if receipt.submission_record_hash != intent.submission.record_hash ||
			decode_canonical_hash(&receipt.finalized_hash)? != intent.finalized_hash.0 ||
			receipt.finalized_number != intent.finalized_number
		{
			return Err(ContentError::IntegrityFailed);
		}
		state.publications.publish(&FinalizedCheckpointPublicationInputV1 {
			submission: intent.submission.clone(),
			finalized_hash: intent.finalized_hash,
			finalized_number: intent.finalized_number,
			response_scale,
		})
	}

	fn lock(&self) -> Result<MutexGuard<'_, CheckpointStackState>, ContentError> {
		self.state.lock().map_err(|_| ContentError::IntegrityFailed)
	}
}

fn decode_canonical_hash(value: &str) -> Result<[u8; 32], ContentError> {
	if value.len() != 64 ||
		value.bytes().any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
	{
		return Err(ContentError::IntegrityFailed);
	}
	hex::decode(value)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)
}

fn checkpoint_publication_intents(
	state: &CheckpointStackState,
) -> Result<Vec<CheckpointPublicationIntentV1>, ContentError> {
	state
		.outbox
		.finalized_submissions()?
		.into_iter()
		.filter_map(|(submission, receipt)| {
			match state.publications.contains(&submission.submission_id) {
				Ok(true) => None,
				Ok(false) => Some(decode_canonical_hash(&receipt.finalized_hash).map(|hash| {
					CheckpointPublicationIntentV1 {
						submission,
						finalized_hash: H256::from(hash),
						finalized_number: receipt.finalized_number,
					}
				})),
				Err(error) => Some(Err(error)),
			}
		})
		.collect()
}

fn validate_prepared_publication_receipts(
	outbox: &PreparedCheckpointOutboxV2,
	publications: &PreparedCheckpointPublicationStoreV1,
) -> Result<(), ContentError> {
	let finalized = outbox
		.finalized_submissions()?
		.into_iter()
		.map(|(submission, receipt)| (submission.submission_id.clone(), (submission, receipt)))
		.collect::<HashMap<_, _>>();
	for publication in publications.records() {
		let (submission, receipt) =
			finalized.get(&publication.submission_id).ok_or(ContentError::IntegrityFailed)?;
		if submission != &publication.submission ||
			receipt.submission_record_hash != publication.submission_record_hash ||
			receipt.finalized_hash != publication.finalized_hash ||
			receipt.finalized_number != publication.finalized_number
		{
			return Err(ContentError::IntegrityFailed);
		}
	}
	if let Some(cursor) = publications.cursor() {
		let (submission, receipt) =
			finalized.get(&cursor.submission_id).ok_or(ContentError::IntegrityFailed)?;
		if submission.record_hash != cursor.submission_record_hash ||
			receipt.submission_record_hash != cursor.submission_record_hash ||
			receipt.finalized_hash != cursor.finalized_hash ||
			receipt.finalized_number != cursor.finalized_number
		{
			return Err(ContentError::IntegrityFailed);
		}
	}
	Ok(())
}

fn emit_quorum_if_ready(
	state: &CheckpointStackState,
	proposal: &PreparedCheckpointProposalV2,
	snapshot: &PrimaryQuorumSnapshotV1,
) -> Result<(), ContentError> {
	if snapshot.confirmations.is_some() {
		state.outbox.enqueue(&submission_input(proposal, snapshot)?)?;
	}
	Ok(())
}

fn ensure_exact_chunk(
	streaming: &StreamingStore,
	incoming: &VerifiedIncomingChunkV1,
) -> Result<(), ContentError> {
	if streaming.read_chunk_verified(incoming.object.cid(), incoming.index)? == incoming.bytes {
		Ok(())
	} else {
		Err(ContentError::IdempotencyConflict)
	}
}

fn install_derived_replication_object(
	streaming: &StreamingStore,
	mmr: &BucketMmrStore,
	incoming: &VerifiedIncomingChunkV1,
) -> Result<(), ContentError> {
	materialize_derived_replication_object(streaming, incoming)?;
	mmr.append_verified(streaming, incoming.bucket_id, incoming.install_operation_id)
}

fn materialize_derived_replication_object(
	streaming: &StreamingStore,
	incoming: &VerifiedIncomingChunkV1,
) -> Result<(), ContentError> {
	let descriptor = StreamingDescriptor {
		operation_id: incoming.install_operation_id,
		bucket_id: incoming.bucket_id,
		expected_cid: incoming.object.cid().into(),
		object_len: incoming.object.position().0,
	};
	match streaming.begin(descriptor)? {
		BeginStreaming::Installed(_) => {},
		BeginStreaming::Receiving(progress) => {
			for index in progress.next_chunk..incoming.object.chunk_hashes().len() as u16 {
				let bytes = streaming.read_chunk_verified(incoming.object.cid(), index)?;
				if incoming.object.chunk_hashes().get(usize::from(index)) !=
					Some(&sp_crypto_hashing::blake2_256(&bytes))
				{
					return Err(ContentError::IntegrityFailed);
				}
				let permit = streaming
					.try_acquire_ingress(
						incoming.bucket_id,
						incoming.install_operation_id,
						index,
						bytes.len(),
					)?
					.ok_or(ContentError::ProviderRecoveryTableFull)?;
				streaming.push_chunk(permit, &bytes)?;
			}
			streaming.finalize(incoming.bucket_id, incoming.install_operation_id)?;
		},
	}
	streaming.retire_completed_replication_repair(
		incoming.bucket_id,
		incoming.object.cid(),
		incoming.repair_operation_id,
		incoming.install_operation_id,
	)
}

#[cfg(test)]
mod tests {
	use std::{fs, path::Path, sync::Arc};

	use async_trait::async_trait;
	#[cfg(feature = "checkpoint-live")]
	use codec::{Decode, Encode};
	#[cfg(feature = "checkpoint-live")]
	use frame_metadata::v15::{
		CustomMetadata, ExtrinsicMetadata, OuterEnums, PalletCallMetadata, PalletMetadata,
		RuntimeMetadataV15,
	};
	#[cfg(feature = "checkpoint-live")]
	use orbis_storage_runtime_api::{
		CheckpointDutyInfo, CheckpointDutyMode, CheckpointDutyPhase, CheckpointInfo, Versioned,
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
	use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
	use tempfile::TempDir;

	use super::*;
	#[cfg(feature = "checkpoint-live")]
	use crate::chain::{CheckpointPublicationAuthority, FinalizedCheckpointObservation};
	#[cfg(feature = "checkpoint-live")]
	use crate::checkpoint::{
		checkpoint_outbox::CheckpointSubmissionInputV2,
		checkpoint_quorum::{checkpoint_context_digest, checkpoint_digest},
		checkpoint_submitter::{CheckpointFinalityLane, FinalizedEvidence},
	};
	use crate::{
		AgreementAuthorization, ChainAuthority, ChainError, ChallengeBatch, CheckpointDutyBatch,
		CheckpointDutyPageRequest, DiskStore, JsonlManifestDeletionOutbox, NodeProfile,
		OperationId, ProviderService,
	};

	#[cfg(feature = "checkpoint-live")]
	mod three_provider_continuity_fixture {
		include!(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/../test-fixtures/three_provider_continuity.rs"
		));
	}

	const DURABLE_ROOTS: [&str; 17] = [
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
		"checkpoint-publication-cursor-v1",
		"checkpoint-promotion-intents-v2",
		"checkpoint-promotion-finalized-receipts-v2",
		"checkpoint-promotion-scheduler-v2",
		"checkpoint-promotion-discovery-v1",
		"replication-v3",
		"peer-replies-v1",
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
		ProviderService::new_preopened(
			store,
			Arc::new(NoopAuthority),
			sp_core::ed25519::Pair::from_seed(&[7u8; 32]),
			Arc::new(JsonlManifestDeletionOutbox::new(root.join("test-outbox.jsonl"))),
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

	#[cfg(feature = "checkpoint-live")]
	struct ContinuityLane {
		metadata: subxt::Metadata,
		calls: AtomicUsize,
		signer_account: [u8; 32],
		signer: ed25519::Pair,
		finalized_number: u32,
	}

	#[cfg(feature = "checkpoint-live")]
	#[async_trait]
	impl CheckpointFinalityLane for ContinuityLane {
		fn metadata(&self) -> &subxt::Metadata {
			&self.metadata
		}

		fn signer_account(&self) -> [u8; 32] {
			self.signer_account
		}

		fn service_key(&self) -> [u8; 32] {
			self.signer.public().0
		}

		async fn submit_and_finalize(
			&self,
			intent_id: &str,
			_payload: subxt::tx::DynamicPayload,
		) -> Result<FinalizedEvidence, ContentError> {
			self.calls.fetch_add(1, Ordering::SeqCst);
			let submission_id = intent_id
				.strip_prefix("orbis-checkpoint-v2-")
				.ok_or(ContentError::IntegrityFailed)?;
			let block_hash = [8; 32];
			let block_number = self.finalized_number;
			let extrinsic_hash = [9; 32];
			Ok(FinalizedEvidence {
				block_hash,
				block_number,
				extrinsic_hash,
				finality_attestation_version: 1,
				finality_signature: self
					.signer
					.sign(&finality_digest(submission_id, block_hash, block_number, extrinsic_hash))
					.0,
			})
		}
	}

	#[cfg(feature = "checkpoint-live")]
	struct ContinuityPublicationAuthority {
		calls: AtomicUsize,
		bucket_id: [u8; 32],
		finalized_number: u32,
		response_scale: Vec<u8>,
	}

	#[cfg(feature = "checkpoint-live")]
	#[async_trait]
	impl CheckpointPublicationAuthority for ContinuityPublicationAuthority {
		async fn checkpoint_observation_at(
			&self,
			bucket_id: [u8; 32],
			finalized_hash: [u8; 32],
			finalized_number: u32,
		) -> Result<FinalizedCheckpointObservation, ChainError> {
			self.calls.fetch_add(1, Ordering::SeqCst);
			if bucket_id != self.bucket_id ||
				finalized_hash != [8; 32] ||
				finalized_number != self.finalized_number
			{
				return Err(ChainError::Rejected("unexpected promoted checkpoint identity".into()));
			}
			Ok(FinalizedCheckpointObservation {
				finalized_hash,
				finalized_number,
				response_scale: self.response_scale.clone(),
			})
		}
	}

	#[test]
	fn late_kernel_rejection_preserves_prepared_streaming_cleanup_byte_for_byte() {
		for later in ["bucket-mmr-v3", "checkpoint-proposals-v2"] {
			let temp = tempfile::tempdir().unwrap();
			drop(CheckpointStack::open(temp.path()).unwrap());
			let streaming = temp.path().join("streaming-v1");
			let journal_temp = streaming.join("journal.json.tmp-77");
			let journal_temp_bytes = b"stale-but-bounded-journal-temp";
			std::fs::write(&journal_temp, journal_temp_bytes).unwrap();
			let staging_orphan = streaming.join("staging").join("unowned-part");
			let staging_bytes = b"unowned-staging-evidence";
			std::fs::write(&staging_orphan, staging_bytes).unwrap();
			let later_root = temp.path().join(later);
			if later == "bucket-mmr-v3" {
				std::fs::write(later_root.join("not-a-bucket-directory"), b"late-invalid").unwrap();
			} else {
				std::fs::write(later_root.join("invalid.json"), b"not-json").unwrap();
			}

			assert!(matches!(
				CheckpointStack::prepare_open(temp.path()),
				Err(ContentError::IntegrityFailed)
			));
			assert_eq!(std::fs::read(&journal_temp).unwrap(), journal_temp_bytes, "{later}");
			assert_eq!(std::fs::read(&staging_orphan).unwrap(), staging_bytes, "{later}");
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
			&state.peer_replies,
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

	#[test]
	fn accepted_quorum_emits_one_stack_owned_outbox_record_across_reopen() {
		use crate::checkpoint::checkpoint_primary::tests::{pair, proposal, response};

		let temp = TempDir::new().unwrap();
		let proposal = proposal(false, 2);
		let stack = CheckpointStack::open(temp.path()).unwrap();
		let started = stack.resume_checkpoint_quorum(&proposal, &pair(1)).unwrap();
		let first = response(&started.requests[0]);
		let second = response(&started.requests[1]);
		stack.accept_checkpoint_confirmation(&proposal, &first).unwrap();
		assert!(stack.submission_heads().unwrap().is_empty());
		stack.accept_checkpoint_confirmation(&proposal, &second).unwrap();
		let submission = stack.submission_heads().unwrap();
		assert_eq!(submission.len(), 1);
		drop(stack);

		let reopened = CheckpointStack::open(temp.path()).unwrap();
		let ready = reopened.resume_checkpoint_quorum(&proposal, &pair(1)).unwrap();
		assert!(ready.requests.is_empty());
		reopened.accept_checkpoint_confirmation(&proposal, &second).unwrap();
		assert_eq!(reopened.submission_heads().unwrap(), submission);
	}

	#[cfg(feature = "checkpoint-live")]
	#[tokio::test]
	async fn three_provider_promoted_quorum_finality_and_publication_are_exact_once() {
		use crate::{
			chain::validate_checkpoint_duty,
			checkpoint::{
				checkpoint_primary::PrimaryQuorumState,
				checkpoint_quorum::{
					checkpoint_context_digest, checkpoint_digest, ReplicaConfirmationRequestV1,
					ReplicaConfirmationResponseV1,
				},
			},
			CanonicalCid,
		};
		use three_provider_continuity_fixture as fixture;

		fn decode_exact<T: Decode + Encode>(encoded: &str) -> T {
			let bytes = hex::decode(encoded).unwrap();
			let mut input = &bytes[..];
			let value = T::decode(&mut input).unwrap();
			assert!(input.is_empty());
			assert_eq!(value.encode(), bytes);
			value
		}

		fn response(request_bytes: &[u8]) -> Vec<u8> {
			let request = ReplicaConfirmationRequestV1::decode_canonical(request_bytes).unwrap();
			let signer = [10u8, 11, 12]
				.into_iter()
				.map(pair)
				.find(|candidate| candidate.public() == request.target_service_key)
				.expect("frozen runtime authority has a fixture signing key");
			ReplicaConfirmationResponseV1 {
				version: 1,
				proposal_record_hash: request.proposal_record_hash,
				target_provider: request.target_provider.clone(),
				duty_id: request.duty_id,
				confirmation: ReplicaSignature {
					provider: request.target_provider,
					service_key: signer.public(),
					signature: signer.sign(&checkpoint_digest(&request.payload)),
					context_signature: signer.sign(&checkpoint_context_digest(&request.context)),
				},
			}
			.encode()
		}

		let duty_bytes = hex::decode(fixture::REPAIRED_DUTY_SCALE).unwrap();
		assert_eq!(
			hex::encode(sp_crypto_hashing::blake2_256(&duty_bytes)),
			fixture::REPAIRED_DUTY_HASH
		);
		let typed = decode_exact::<CheckpointDutyInfo<AccountId32, H256, u32>>(
			fixture::REPAIRED_DUTY_SCALE,
		);
		assert_eq!(typed.mode, CheckpointDutyMode::PromotionPending);
		assert_eq!(typed.phase, CheckpointDutyPhase::Primary);
		assert_eq!(typed.initiator.as_ref(), Some(&typed.primary));
		assert_eq!(typed.replicas.len(), 2);
		let signer = [10u8, 11, 12]
			.into_iter()
			.map(pair)
			.find(|candidate| AccountId32::new(candidate.public().0) == typed.primary)
			.expect("frozen promoted primary has a fixture signing key");
		let duty = validate_checkpoint_duty(
			typed.clone(),
			&typed.primary,
			signer.public().0,
			typed.snapshot_checkpoint,
		)
		.unwrap();
		assert_eq!(duty.encoded_duty.trim_start_matches("0x"), fixture::REPAIRED_DUTY_SCALE);

		let response_scale = hex::decode(fixture::CHECKPOINT_INFO_SCALE).unwrap();
		assert_eq!(
			hex::encode(sp_crypto_hashing::blake2_256(&response_scale)),
			fixture::CHECKPOINT_INFO_HASH
		);
		let checkpoint = decode_exact::<Versioned<CheckpointInfo<AccountId32, H256, u32>>>(
			fixture::CHECKPOINT_INFO_SCALE,
		);
		let checkpoint = checkpoint.value.expect("runtime froze an accepted checkpoint");
		assert_eq!(checkpoint.bucket_id, typed.bucket_id);
		assert_eq!(checkpoint.replica_confirmations.len(), 2);

		let temp = TempDir::new().unwrap();
		let stack = CheckpointStack::open(temp.path()).unwrap();
		let provider_bytes: &[u8] = typed.primary.as_ref();
		let store = DiskStore::open(
			temp.path().join("duty-store"),
			NodeProfile {
				provider: hex::encode(provider_bytes),
				endpoint: "http://127.0.0.1:8080".into(),
				service_key: hex::encode(signer.public().0),
				region: None,
			},
			1024,
		)
		.unwrap();
		store
			.stage_checkpoint_duty_page(CheckpointDutyBatch {
				finalized_hash: duty.snapshot_hash.clone(),
				finalized_number: typed.snapshot_checkpoint,
				provider: duty.provider.clone(),
				snapshot_checkpoint: typed.snapshot_checkpoint,
				requested_cursor: None,
				next_cursor: None,
				duties: vec![duty.clone()],
			})
			.unwrap();

		let bucket = BucketId::from_bytes(typed.bucket_id.0);
		{
			let state = stack.lock().unwrap();
			for (operation, bytes) in [(1u8, fixture::PRIOR_OBJECT), (2, fixture::NEXT_OBJECT)] {
				let operation_id = OperationId::from_bytes([operation; 16]);
				let cid = CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(bytes));
				state
					.streaming
					.put_chunks(
						StreamingDescriptor {
							operation_id,
							bucket_id: bucket,
							expected_cid: cid.as_str().into(),
							object_len: bytes.len() as u64,
						},
						[bytes.to_vec()],
					)
					.unwrap();
				state
					.bucket_mmr
					.append_verified(&state.streaming, bucket, operation_id)
					.unwrap();
				if operation == 1 {
					let previous =
						state.bucket_mmr.commitment_candidate(&state.streaming, bucket, 0).unwrap();
					let frozen_previous = typed.previous_commitment.as_ref().unwrap();
					assert_eq!(previous.mmr_root, frozen_previous.mmr_root);
					assert_eq!(previous.start_seq, frozen_previous.start_seq);
					assert_eq!(previous.leaf_count, frozen_previous.leaf_count);
				}
			}
			let candidate = state
				.bucket_mmr
				.commitment_candidate(&state.streaming, bucket, typed.expected_next_start_seq)
				.unwrap();
			assert_eq!(candidate.mmr_root, checkpoint.commitment.mmr_root);
			assert_eq!(candidate.start_seq, checkpoint.commitment.start_seq);
			assert_eq!(candidate.leaf_count, checkpoint.commitment.leaf_count);
		}

		let (proposal, collecting) = stack.begin_checkpoint_quorum(&store, &duty, &signer).unwrap();
		assert_eq!(collecting.state, PrimaryQuorumState::Collecting);
		assert_eq!(collecting.requests.len(), 2);
		assert_eq!(proposal.duty_scale, fixture::REPAIRED_DUTY_SCALE);
		let payload = decode_exact::<CommitmentPayloadV2<H256, u32>>(&proposal.payload_scale);
		assert_eq!(payload.bucket_id, checkpoint.bucket_id);
		assert_eq!(payload.commitment.mmr_root, checkpoint.commitment.mmr_root);
		assert_eq!(payload.commitment.start_seq, checkpoint.commitment.start_seq);
		assert_eq!(payload.commitment.leaf_count, checkpoint.commitment.leaf_count);
		assert_eq!(payload.nonce, checkpoint.commitment_nonce);

		let requests = collecting
			.requests
			.iter()
			.map(|bytes| ReplicaConfirmationRequestV1::decode_canonical(bytes).unwrap())
			.collect::<Vec<_>>();
		assert_eq!(
			requests
				.iter()
				.map(|request| request.target_provider.clone())
				.collect::<Vec<_>>(),
			checkpoint.replica_confirmations
		);
		let first = response(&collecting.requests[0]);
		let second = response(&collecting.requests[1]);
		let partial = stack.accept_checkpoint_confirmation(&proposal, &first).unwrap();
		assert_eq!(partial.state, PrimaryQuorumState::Collecting);
		assert!(stack.submission_heads().unwrap().is_empty());
		let ready = stack.accept_checkpoint_confirmation(&proposal, &second).unwrap();
		assert_eq!(ready.state, PrimaryQuorumState::QuorumReady);
		let submissions = stack.submission_heads().unwrap();
		assert_eq!(submissions.len(), 1);
		stack.accept_checkpoint_confirmation(&proposal, &second).unwrap();
		assert_eq!(stack.submission_heads().unwrap(), submissions);
		drop(stack);

		let stack = CheckpointStack::open(temp.path()).unwrap();
		let reopened_ready = stack.resume_checkpoint_quorum(&proposal, &signer).unwrap();
		assert_eq!(reopened_ready.state, PrimaryQuorumState::QuorumReady);
		assert!(reopened_ready.requests.is_empty());
		stack.accept_checkpoint_confirmation(&proposal, &first).unwrap();
		assert_eq!(stack.submission_heads().unwrap(), submissions);

		let mut signer_account = [0u8; 32];
		signer_account.copy_from_slice(typed.primary.as_ref());
		let lane = ContinuityLane {
			metadata: checkpoint_metadata(),
			calls: AtomicUsize::new(0),
			signer_account,
			signer,
			finalized_number: checkpoint.checkpoint_block,
		};
		let authority = ContinuityPublicationAuthority {
			calls: AtomicUsize::new(0),
			bucket_id: typed.bucket_id.0,
			finalized_number: checkpoint.checkpoint_block,
			response_scale,
		};
		let lifecycle =
			crate::checkpoint_live_worker::tick(&authority, &stack, &lane).await.unwrap();
		assert_eq!(
			lifecycle.finalized.as_ref().unwrap().submission_id,
			submissions[0].submission_id
		);
		assert_eq!(lifecycle.published.len(), 1);
		assert_eq!(lifecycle.published[0].submission_id, submissions[0].submission_id);
		assert_eq!(lane.calls.load(Ordering::SeqCst), 1);
		assert_eq!(authority.calls.load(Ordering::SeqCst), 1);
		assert!(stack.submission_heads().unwrap().is_empty());
		assert!(stack.pending_checkpoint_publications(8).unwrap().is_empty());

		let repeated =
			crate::checkpoint_live_worker::tick(&authority, &stack, &lane).await.unwrap();
		assert!(repeated.finalized.is_none());
		assert!(repeated.published.is_empty());
		assert_eq!(lane.calls.load(Ordering::SeqCst), 1);
		assert_eq!(authority.calls.load(Ordering::SeqCst), 1);
		drop(stack);

		let reopened = CheckpointStack::open(temp.path()).unwrap();
		let replay =
			crate::checkpoint_live_worker::tick(&authority, &reopened, &lane).await.unwrap();
		assert!(replay.finalized.is_none());
		assert!(replay.published.is_empty());
		assert_eq!(lane.calls.load(Ordering::SeqCst), 1);
		assert_eq!(authority.calls.load(Ordering::SeqCst), 1);
		let state = reopened.lock().unwrap();
		assert_eq!(state.publications.records().unwrap().len(), 1);
		assert_eq!(
			state.outbox.finalized_receipt(&submissions[0].submission_id).unwrap(),
			lifecycle.finalized
		);
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
