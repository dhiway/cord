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

//! Private target-side replication reconciliation.

use std::sync::Arc;

use codec::Encode;
use sp_core::{ed25519, Pair as _};
use sp_crypto_hashing::blake2_256;

use crate::{
	checkpoint_stack::CheckpointStack,
	peer_transport::{PeerTransport, PeerTransportError},
	replication::{ReplicationActionV1, ReplicationPhase},
	replication_session::ReplicationSessionV1,
	ContentError,
};

const OPERATION_DOMAIN: &[u8] = b"cord/provider/replication-reconcile-operation/v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReconcileStepV1 {
	pub(crate) intent_key: String,
	pub(crate) phase: ReplicationPhase,
	pub(crate) used_network: bool,
}

/// Advances exactly one durable action and performs at most one external request.
pub(crate) struct ReplicationReconciler<T: PeerTransport> {
	stack: Arc<CheckpointStack>,
	transport: Arc<T>,
	target: ed25519::Pair,
}

impl<T: PeerTransport> ReplicationReconciler<T> {
	pub(crate) fn new(
		stack: Arc<CheckpointStack>,
		transport: Arc<T>,
		target: ed25519::Pair,
	) -> Result<Self, ContentError> {
		if target.public().0 == [0; 32] {
			return Err(ContentError::SchemaInvalid);
		}
		Ok(Self { stack, transport, target })
	}

	pub(crate) fn operation_id(session: &ReplicationSessionV1) -> [u8; 16] {
		let mut bytes = OPERATION_DOMAIN.to_vec();
		bytes.extend_from_slice(&session.context().encode());
		let digest = blake2_256(&bytes);
		let mut operation = [0; 16];
		operation.copy_from_slice(&digest[..16]);
		operation[0] = 1;
		operation
	}

	pub(crate) async fn reconcile_one(
		&self,
		session: &ReplicationSessionV1,
		operation_id: [u8; 16],
	) -> Result<ReconcileStepV1, ContentError> {
		if operation_id != Self::operation_id(session)
			|| session.target().service_key() != self.target.public().0
		{
			return Err(ContentError::IdempotencyConflict);
		}
		let planned = self.stack.plan_replication(session, operation_id)?;
		let intent_key = planned.intent_key.clone();
		let action = self.stack.next_replication_action(&intent_key, &self.target)?;
		let (record, used_network) = match action {
			ReplicationActionV1::SendPage { request_bytes, .. } => {
				let response =
					self.transport.page(session, &request_bytes).await.map_err(transport_error)?;
				(self.stack.accept_replication_page(&intent_key, &response)?, true)
			},
			ReplicationActionV1::SendChunk { request_bytes, .. } => {
				let response =
					self.transport.chunk(session, &request_bytes).await.map_err(transport_error)?;
				// Bytes cross their fsync/journal boundary before authenticated proof progress.
				self.stack.persist_replication_chunk(&intent_key, &response)?;
				(self.stack.accept_replication_chunk(&intent_key, &response)?, true)
			},
			ReplicationActionV1::FinishObject { sequence, cid, length, .. } => {
				(self.stack.finish_replication_object(&intent_key, sequence, &cid, length)?, false)
			},
			ReplicationActionV1::MarkInstalled { .. } => {
				(self.stack.mark_replication_installed(&intent_key)?, false)
			},
			ReplicationActionV1::CommitMmr { .. } => {
				(self.stack.commit_replication_mmr(&intent_key)?, false)
			},
			ReplicationActionV1::Complete => (planned, false),
		};
		Ok(ReconcileStepV1 { intent_key, phase: record.phase, used_network })
	}
}

fn transport_error(_: PeerTransportError) -> ContentError {
	ContentError::Io("replication peer exchange failed".into())
}

#[cfg(test)]
mod tests {
	use std::{
		fs,
		sync::{
			atomic::{AtomicBool, Ordering},
			Mutex,
		},
	};

	use async_trait::async_trait;
	use sp_core::Pair as _;
	use tempfile::TempDir;

	use super::*;
	use crate::{
		chain::{
			ChainError, ReplicationAuthority, ReplicationProviderSnapshot,
			ReplicationTopologySnapshot,
		},
		peer::PeerMmrCommitmentV1,
		peer_responder::PeerResponder,
		storage::{bucket_mmr::BucketMmrStore, StreamingDescriptor, StreamingStore},
		BucketId, CanonicalCid, OperationId, CHUNK_BYTES,
	};

	#[derive(Clone)]
	struct Authority(ReplicationTopologySnapshot);

	#[async_trait]
	impl ReplicationAuthority for Authority {
		async fn replication_topology(
			&self,
			bucket: [u8; 32],
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			(bucket == self.0.bucket_id)
				.then(|| self.0.clone())
				.ok_or_else(|| ChainError::Rejected("wrong bucket".into()))
		}

		async fn replication_topology_at(
			&self,
			bucket: [u8; 32],
			finalized_hash: [u8; 32],
			finalized_number: u32,
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			(bucket == self.0.bucket_id
				&& finalized_hash == self.0.finalized_hash
				&& finalized_number == self.0.finalized_number)
				.then(|| self.0.clone())
				.ok_or_else(|| ChainError::Rejected("wrong topology".into()))
		}
	}

	struct DirectTransport {
		responder: Arc<PeerResponder<Authority>>,
		requests: Mutex<Vec<Vec<u8>>>,
		fail_once: AtomicBool,
		corrupt_once: AtomicBool,
	}

	impl DirectTransport {
		fn new(responder: Arc<PeerResponder<Authority>>) -> Self {
			Self {
				responder,
				requests: Mutex::new(Vec::new()),
				fail_once: AtomicBool::new(false),
				corrupt_once: AtomicBool::new(false),
			}
		}
	}

	#[async_trait]
	impl PeerTransport for DirectTransport {
		async fn page(
			&self,
			_: &ReplicationSessionV1,
			request: &[u8],
		) -> Result<Vec<u8>, PeerTransportError> {
			self.requests.lock().unwrap().push(request.to_vec());
			if self.fail_once.swap(false, Ordering::SeqCst) {
				return Err(PeerTransportError::Transport);
			}
			let mut response =
				self.responder.page(request).await.map_err(|_| PeerTransportError::Response)?;
			if self.corrupt_once.swap(false, Ordering::SeqCst) {
				response[0] ^= 1;
			}
			Ok(response)
		}

		async fn chunk(
			&self,
			_: &ReplicationSessionV1,
			request: &[u8],
		) -> Result<Vec<u8>, PeerTransportError> {
			self.requests.lock().unwrap().push(request.to_vec());
			if self.fail_once.swap(false, Ordering::SeqCst) {
				return Err(PeerTransportError::Transport);
			}
			let mut response =
				self.responder.chunk(request).await.map_err(|_| PeerTransportError::Response)?;
			if self.corrupt_once.swap(false, Ordering::SeqCst) {
				response[0] ^= 1;
			}
			Ok(response)
		}
	}

	fn pair(seed: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[seed; 32])
	}

	fn provider(id: u8, order: u8, key: [u8; 32]) -> ReplicationProviderSnapshot {
		let endpoint = format!("https://provider-{id}.invalid").into_bytes();
		ReplicationProviderSnapshot {
			provider: [id; 32],
			order,
			primary: order == 0,
			record_present: true,
			endpoint_hash: Some(blake2_256(&endpoint)),
			endpoint: Some(endpoint),
			active_service_key: Some(key),
			active_service_key_version: Some(u64::from(order) + 1),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(8),
			overdue_challenges: 0,
			eligible: true,
			usable: true,
			exclusions: Vec::new(),
			confirmed_checkpoint: None,
		}
	}

	fn topology() -> ReplicationTopologySnapshot {
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: [1; 32],
			finalized_hash: [2; 32],
			finalized_number: 10,
			governed_finalized_checkpoint: Some(8),
			bucket_id: [3; 32],
			bucket_version: 4,
			primary: [4; 32],
			replicas: vec![[5; 32]],
			providers: vec![
				provider(4, 0, pair(11).public().0),
				provider(5, 1, pair(12).public().0),
			],
			current_checkpoint: None,
			snapshot_hash: [0; 32],
		};
		let mut encoded = b"cord/provider/replication-topology/v1".to_vec();
		topology.encode_to(&mut encoded);
		topology.snapshot_hash = blake2_256(&encoded);
		topology
	}

	fn fixture_with_bytes(
		bytes: Vec<u8>,
	) -> (
		TempDir,
		TempDir,
		ReplicationSessionV1,
		Arc<PeerResponder<Authority>>,
		CanonicalCid,
		Vec<u8>,
	) {
		let source = TempDir::new().unwrap();
		let target = TempDir::new().unwrap();
		let cid = CanonicalCid::from_digest(blake2_256(&bytes));
		let streaming = StreamingStore::open(source.path()).unwrap();
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: OperationId::from_bytes([7; 16]),
					bucket_id: BucketId::from_bytes([3; 32]),
					expected_cid: cid.to_string(),
					object_len: bytes.len() as u64,
				},
				bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec),
			)
			.unwrap();
		let mmr = BucketMmrStore::open(source.path(), &streaming).unwrap();
		let candidate =
			mmr.commitment_candidate(&streaming, BucketId::from_bytes([3; 32]), 0).unwrap();
		let commitment = PeerMmrCommitmentV1::new(candidate.mmr_root.0, 0, 1, 0).unwrap();
		let topology = topology();
		let session = ReplicationSessionV1::from_topology(
			topology.clone(),
			[5; 32],
			pair(12).public().0,
			[4; 32],
			[5; 32],
			commitment,
		)
		.unwrap();
		let responder = Arc::new(
			PeerResponder::new(
				Arc::new(Authority(topology)),
				Arc::new(CheckpointStack::open(source.path()).unwrap()),
				[4; 32],
				pair(11),
			)
			.unwrap(),
		);
		(source, target, session, responder, cid, bytes)
	}

	fn fixture() -> (
		TempDir,
		TempDir,
		ReplicationSessionV1,
		Arc<PeerResponder<Authority>>,
		CanonicalCid,
		Vec<u8>,
	) {
		fixture_with_bytes(vec![31; CHUNK_BYTES + 7])
	}

	fn duplicate_suffix_fixture() -> (
		TempDir,
		TempDir,
		ReplicationSessionV1,
		Arc<PeerResponder<Authority>>,
		CanonicalCid,
		Vec<u8>,
	) {
		let source = TempDir::new().unwrap();
		let target = TempDir::new().unwrap();
		let bytes = vec![41; CHUNK_BYTES + 11];
		let cid = CanonicalCid::from_digest(blake2_256(&bytes));
		let source_streaming = StreamingStore::open(source.path()).unwrap();
		for operation in [7u8, 8] {
			source_streaming
				.put_chunks(
					StreamingDescriptor {
						operation_id: OperationId::from_bytes([operation; 16]),
						bucket_id: BucketId::from_bytes([3; 32]),
						expected_cid: cid.to_string(),
						object_len: bytes.len() as u64,
					},
					bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec),
				)
				.unwrap();
		}
		let source_mmr = BucketMmrStore::open(source.path(), &source_streaming).unwrap();
		let candidate = source_mmr
			.commitment_candidate(&source_streaming, BucketId::from_bytes([3; 32]), 1)
			.unwrap();
		let predecessor = source_mmr
			.commitment_predecessor_total(BucketId::from_bytes([3; 32]), 1)
			.unwrap();
		let commitment = PeerMmrCommitmentV1::new(
			candidate.mmr_root.0,
			candidate.start_seq,
			candidate.leaf_count,
			predecessor,
		)
		.unwrap();
		let topology = topology();
		let session = ReplicationSessionV1::from_topology(
			topology.clone(),
			[5; 32],
			pair(12).public().0,
			[4; 32],
			[5; 32],
			commitment,
		)
		.unwrap();
		let responder = Arc::new(
			PeerResponder::new(
				Arc::new(Authority(topology)),
				Arc::new(CheckpointStack::open(source.path()).unwrap()),
				[4; 32],
				pair(11),
			)
			.unwrap(),
		);
		let target_streaming = StreamingStore::open(target.path()).unwrap();
		target_streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: OperationId::from_bytes([99; 16]),
					bucket_id: BucketId::from_bytes([3; 32]),
					expected_cid: cid.to_string(),
					object_len: bytes.len() as u64,
				},
				bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec),
			)
			.unwrap();
		let target_mmr = BucketMmrStore::open(target.path(), &target_streaming).unwrap();
		let mut damaged = bytes.clone();
		damaged[0] ^= 1;
		fs::write(target.path().join("streaming-v1").join("objects").join(cid.as_str()), damaged)
			.unwrap();
		assert_eq!(
			target_streaming.verify_installed(cid.as_str()),
			Err(ContentError::IntegrityFailed)
		);
		drop(target_mmr);
		drop(target_streaming);
		(source, target, session, responder, cid, bytes)
	}

	#[tokio::test]
	async fn page_and_two_chunks_reach_exact_local_mmr() {
		let (_source, target, session, responder, cid, bytes) = fixture();
		let stack = Arc::new(CheckpointStack::open(target.path()).unwrap());
		let transport = Arc::new(DirectTransport::new(responder));
		let reconciler = ReplicationReconciler::new(stack, transport.clone(), pair(12)).unwrap();
		let operation = ReplicationReconciler::<DirectTransport>::operation_id(&session);
		let mut final_step = None;
		for _ in 0..8 {
			let step = reconciler.reconcile_one(&session, operation).await.unwrap();
			if step.phase == ReplicationPhase::MmrCommitted {
				final_step = Some(step);
				break;
			}
		}
		assert!(final_step.is_some());
		assert_eq!(transport.requests.lock().unwrap().len(), 3);
		let installed = StreamingStore::open(target.path()).unwrap();
		let mut durable = installed.read_chunk_verified(cid.as_str(), 0).unwrap();
		durable.extend(installed.read_chunk_verified(cid.as_str(), 1).unwrap());
		assert_eq!(durable, bytes);
	}

	#[tokio::test]
	async fn restart_resends_the_exact_durable_request() {
		let (_source, target, session, responder, _, _) = fixture();
		let operation = ReplicationReconciler::<DirectTransport>::operation_id(&session);
		let first_transport = Arc::new(DirectTransport::new(responder.clone()));
		first_transport.fail_once.store(true, Ordering::SeqCst);
		let first = ReplicationReconciler::new(
			Arc::new(CheckpointStack::open(target.path()).unwrap()),
			first_transport.clone(),
			pair(12),
		)
		.unwrap();
		assert!(first.reconcile_one(&session, operation).await.is_err());
		let exact = first_transport.requests.lock().unwrap()[0].clone();
		drop(first);

		let second_transport = Arc::new(DirectTransport::new(responder));
		let second = ReplicationReconciler::new(
			Arc::new(CheckpointStack::open(target.path()).unwrap()),
			second_transport.clone(),
			pair(12),
		)
		.unwrap();
		second.reconcile_one(&session, operation).await.unwrap();
		assert_eq!(second_transport.requests.lock().unwrap()[0], exact);
	}

	#[tokio::test]
	async fn zero_byte_object_is_installed_and_appended_before_completion() {
		let (_source, target, session, responder, cid, _) = fixture_with_bytes(Vec::new());
		let transport = Arc::new(DirectTransport::new(responder));
		let reconciler = ReplicationReconciler::new(
			Arc::new(CheckpointStack::open(target.path()).unwrap()),
			transport.clone(),
			pair(12),
		)
		.unwrap();
		let operation = ReplicationReconciler::<DirectTransport>::operation_id(&session);
		let mut phase = ReplicationPhase::Planned;
		for _ in 0..6 {
			phase = reconciler.reconcile_one(&session, operation).await.unwrap().phase;
			if phase == ReplicationPhase::MmrCommitted {
				break;
			}
		}
		assert_eq!(phase, ReplicationPhase::MmrCommitted);
		assert_eq!(transport.requests.lock().unwrap().len(), 1);
		let installed = StreamingStore::open(target.path()).unwrap();
		assert_eq!(
			installed.read_chunk_verified(cid.as_str(), 0),
			Err(ContentError::ChunkOutOfOrder)
		);
	}

	#[tokio::test]
	async fn crash_after_chunk_fsync_replays_request_then_attaches_proof() {
		let (_source, target, session, responder, _, _) = fixture();
		let stack = Arc::new(CheckpointStack::open(target.path()).unwrap());
		let transport = Arc::new(DirectTransport::new(responder.clone()));
		let reconciler = ReplicationReconciler::new(stack.clone(), transport, pair(12)).unwrap();
		let operation = ReplicationReconciler::<DirectTransport>::operation_id(&session);
		reconciler.reconcile_one(&session, operation).await.unwrap();
		let intent = stack.plan_replication(&session, operation).unwrap();
		let request = match stack.next_replication_action(&intent.intent_key, &pair(12)).unwrap() {
			ReplicationActionV1::SendChunk { request_bytes, .. } => request_bytes,
			other => panic!("expected chunk, got {other:?}"),
		};
		let response = responder.chunk(&request).await.unwrap();
		stack.persist_replication_chunk(&intent.intent_key, &response).unwrap();
		drop(reconciler);
		drop(stack);

		let retry_transport = Arc::new(DirectTransport::new(responder));
		let retry = ReplicationReconciler::new(
			Arc::new(CheckpointStack::open(target.path()).unwrap()),
			retry_transport.clone(),
			pair(12),
		)
		.unwrap();
		retry.reconcile_one(&session, operation).await.unwrap();
		assert_eq!(retry_transport.requests.lock().unwrap().as_slice(), [request]);
	}

	#[tokio::test]
	async fn invalid_response_makes_no_progress_and_retries_exact_request() {
		let (_source, target, session, responder, _, _) = fixture();
		let transport = Arc::new(DirectTransport::new(responder));
		let reconciler = ReplicationReconciler::new(
			Arc::new(CheckpointStack::open(target.path()).unwrap()),
			transport.clone(),
			pair(12),
		)
		.unwrap();
		let operation = ReplicationReconciler::<DirectTransport>::operation_id(&session);
		reconciler.reconcile_one(&session, operation).await.unwrap();
		transport.corrupt_once.store(true, Ordering::SeqCst);
		assert!(reconciler.reconcile_one(&session, operation).await.is_err());
		reconciler.reconcile_one(&session, operation).await.unwrap();
		let requests = transport.requests.lock().unwrap();
		assert_eq!(requests.len(), 3);
		assert_eq!(requests[1], requests[2]);
	}

	#[tokio::test]
	async fn quarantined_duplicate_is_repaired_and_live_mmr_is_revalidated() {
		let (_source, target, session, responder, cid, bytes) = fixture();
		let streaming = StreamingStore::open(target.path()).unwrap();
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: OperationId::from_bytes([99; 16]),
					bucket_id: BucketId::from_bytes([3; 32]),
					expected_cid: cid.to_string(),
					object_len: bytes.len() as u64,
				},
				bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec),
			)
			.unwrap();
		let mmr = BucketMmrStore::open(target.path(), &streaming).unwrap();
		let mut damaged = bytes.clone();
		damaged[0] ^= 1;
		fs::write(target.path().join("streaming-v1").join("objects").join(cid.as_str()), damaged)
			.unwrap();
		assert_eq!(streaming.verify_installed(cid.as_str()), Err(ContentError::IntegrityFailed));
		drop(mmr);
		drop(streaming);

		let reconciler = ReplicationReconciler::new(
			Arc::new(CheckpointStack::open(target.path()).unwrap()),
			Arc::new(DirectTransport::new(responder)),
			pair(12),
		)
		.unwrap();
		let operation = ReplicationReconciler::<DirectTransport>::operation_id(&session);
		let mut phase = ReplicationPhase::Planned;
		for _ in 0..8 {
			phase = reconciler.reconcile_one(&session, operation).await.unwrap().phase;
			if phase == ReplicationPhase::MmrCommitted {
				break;
			}
		}
		assert_eq!(phase, ReplicationPhase::MmrCommitted);
		let repaired = StreamingStore::open(target.path()).unwrap();
		let mut durable = repaired.read_chunk_verified(cid.as_str(), 0).unwrap();
		durable.extend(repaired.read_chunk_verified(cid.as_str(), 1).unwrap());
		assert_eq!(durable, bytes);
	}

	#[tokio::test]
	async fn quarantined_prior_duplicate_repairs_then_appends_exact_suffix_leaf() {
		let (_source, target, session, responder, cid, bytes) = duplicate_suffix_fixture();
		let reconciler = ReplicationReconciler::new(
			Arc::new(CheckpointStack::open(target.path()).unwrap()),
			Arc::new(DirectTransport::new(responder)),
			pair(12),
		)
		.unwrap();
		let operation = ReplicationReconciler::<DirectTransport>::operation_id(&session);
		let mut phase = ReplicationPhase::Planned;
		for _ in 0..8 {
			phase = reconciler.reconcile_one(&session, operation).await.unwrap().phase;
			if phase == ReplicationPhase::MmrCommitted {
				break;
			}
		}
		assert_eq!(phase, ReplicationPhase::MmrCommitted);
		let streaming = StreamingStore::open(target.path()).unwrap();
		let mmr = BucketMmrStore::open(target.path(), &streaming).unwrap();
		let candidate =
			mmr.commitment_candidate(&streaming, BucketId::from_bytes([3; 32]), 1).unwrap();
		assert_eq!(candidate.mmr_root.0, session.context().candidate_commitment().mmr_root());
		assert_eq!(candidate.leaf_count, 1);
		let mut durable = streaming.read_chunk_verified(cid.as_str(), 0).unwrap();
		durable.extend(streaming.read_chunk_verified(cid.as_str(), 1).unwrap());
		assert_eq!(durable, bytes);
	}

	#[tokio::test]
	async fn crash_mid_repaired_materialization_resumes_before_proof_attach() {
		let (_source, target, session, responder, _, _) = duplicate_suffix_fixture();
		let stack = Arc::new(CheckpointStack::open(target.path()).unwrap());
		let transport = Arc::new(DirectTransport::new(responder.clone()));
		let reconciler = ReplicationReconciler::new(stack.clone(), transport, pair(12)).unwrap();
		let operation = ReplicationReconciler::<DirectTransport>::operation_id(&session);
		reconciler.reconcile_one(&session, operation).await.unwrap();
		reconciler.reconcile_one(&session, operation).await.unwrap();
		stack
			.inject_replication_streaming_fault_once(crate::StreamingFault::AfterChunkSync)
			.unwrap();
		assert!(matches!(
			reconciler.reconcile_one(&session, operation).await,
			Err(ContentError::Io(_))
		));
		drop(reconciler);
		drop(stack);

		let retry = ReplicationReconciler::new(
			Arc::new(CheckpointStack::open(target.path()).unwrap()),
			Arc::new(DirectTransport::new(responder)),
			pair(12),
		)
		.unwrap();
		let mut phase = ReplicationPhase::Receiving;
		for _ in 0..6 {
			phase = retry.reconcile_one(&session, operation).await.unwrap().phase;
			if phase == ReplicationPhase::MmrCommitted {
				break;
			}
		}
		assert_eq!(phase, ReplicationPhase::MmrCommitted);
	}
}
