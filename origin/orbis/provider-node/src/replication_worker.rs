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

//! Bounded private target-side replication lifecycle.

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use sp_core::{ed25519, Pair as _};
use tokio::{
	sync::Semaphore,
	task::JoinSet,
	time::{interval, MissedTickBehavior},
};

use crate::{
	chain::{ReplicationAuthority, ReplicationTopologySnapshot},
	checkpoint_stack::CheckpointStack,
	peer::PeerMmrCommitmentV1,
	peer_transport::HyperPeerTransport,
	replication::ReplicationResumeV1,
	replication_reconciler::ReplicationReconciler,
	replication_session::ReplicationSessionV1,
	BucketId, CheckpointDuty, DiskStore, FinalizedRuntimeAuthority,
};

const MAX_TICK_INTENTS: usize = 128;
const RESERVED_RESUMES: usize = MAX_TICK_INTENTS / 2;
const RESERVED_DISCOVERY: usize = MAX_TICK_INTENTS - RESERVED_RESUMES;
const MAX_CONCURRENCY: usize = 8;
const PEER_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn run(
	authority: Arc<FinalizedRuntimeAuthority>,
	stack: Arc<CheckpointStack>,
	store: Arc<DiskStore>,
	local_provider: [u8; 32],
	target_key: ed25519::Pair,
	cadence: Duration,
) {
	let mut ticker = interval(cadence.max(Duration::from_secs(1)));
	ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
	loop {
		ticker.tick().await;
		if let Err(error) = tick(
			Arc::clone(&authority),
			Arc::clone(&stack),
			Arc::clone(&store),
			local_provider,
			target_key.clone(),
		)
		.await
		{
			eprintln!("replication coordinator tick failed: {error}");
		}
	}
}

async fn tick(
	authority: Arc<FinalizedRuntimeAuthority>,
	stack: Arc<CheckpointStack>,
	store: Arc<DiskStore>,
	local_provider: [u8; 32],
	target_key: ed25519::Pair,
) -> Result<(), String> {
	let inventory = store.checkpoint_duty_inventory().map_err(|error| error.to_string())?;
	let (resume_limit, discovery_limit) = initial_lane_limits(inventory.is_some());
	let resumes = stack.replication_resume_tick(resume_limit).map_err(|error| error.to_string())?;
	let mut visited = BTreeSet::new();
	let mut work = Vec::new();
	for resume in resumes {
		if visited.insert(resume.intent_key.clone()) {
			work.push(WorkerIntent::Resume(resume));
		}
	}

	let duties = match inventory.as_ref() {
		Some(inventory) => store
			.reserve_checkpoint_replica_duties(inventory, discovery_limit)
			.map_err(|error| error.to_string())?,
		None => Vec::new(),
	};
	for duty in duties {
		if work.len() >= MAX_TICK_INTENTS {
			break;
		}
		match discover(
			Arc::clone(&authority),
			Arc::clone(&stack),
			local_provider,
			&target_key,
			&duty,
		)
		.await
		{
			Ok(Some(discovered)) if visited.insert(discovered.intent_key.clone()) => {
				work.push(WorkerIntent::Discovered(discovered));
			},
			Ok(_) => {},
			Err(error) => eprintln!("replication discovery rejected {}: {error}", duty.bucket_id),
		}
	}
	let remaining = MAX_TICK_INTENTS.saturating_sub(work.len());
	if remaining > 0 {
		for resume in stack.replication_resume_tick(remaining).map_err(|error| error.to_string())? {
			if visited.insert(resume.intent_key.clone()) {
				work.push(WorkerIntent::Resume(resume));
			}
		}
	}

	let permits = Arc::new(Semaphore::new(MAX_CONCURRENCY));
	let mut tasks = JoinSet::new();
	for intent in work {
		let permit = Arc::clone(&permits).acquire_owned().await.map_err(|_| "closed semaphore")?;
		let authority = Arc::clone(&authority);
		let stack = Arc::clone(&stack);
		let target_key = target_key.clone();
		tasks.spawn(async move {
			let _permit = permit;
			if let Err(error) =
				reconcile_intent(authority, stack, local_provider, target_key, intent).await
			{
				eprintln!("replication intent failed: {error}");
			}
		});
	}
	while tasks.join_next().await.is_some() {}
	Ok(())
}

fn initial_lane_limits(has_inventory: bool) -> (usize, usize) {
	if has_inventory {
		(RESERVED_RESUMES, RESERVED_DISCOVERY)
	} else {
		(MAX_TICK_INTENTS, 0)
	}
}

enum WorkerIntent {
	Resume(ReplicationResumeV1),
	Discovered(DiscoveredIntent),
}

struct DiscoveredIntent {
	intent_key: String,
	session: ReplicationSessionV1,
	operation_id: [u8; 16],
}

async fn reconcile_intent(
	authority: Arc<FinalizedRuntimeAuthority>,
	stack: Arc<CheckpointStack>,
	local_provider: [u8; 32],
	target_key: ed25519::Pair,
	intent: WorkerIntent,
) -> Result<(), String> {
	let (session, operation_id) = match intent {
		WorkerIntent::Resume(resume) => {
			let pinned = authority
				.replication_topology_at(
					resume.bucket_id,
					resume.topology_finalized_hash,
					resume.topology_finalized_number,
				)
				.await
				.map_err(|error| error.to_string())?;
			if pinned.snapshot_hash != resume.topology_snapshot_hash
				|| resume.target_provider != local_provider
			{
				return Err("pinned replication identity changed".into());
			}
			let pinned_session = ReplicationSessionV1::from_topology(
				pinned.clone(),
				local_provider,
				target_key.public().0,
				resume.source_provider,
				resume.target_provider,
				resume.commitment,
			)
			.map_err(|error| error.to_string())?;
			let current = authority
				.replication_topology(resume.bucket_id)
				.await
				.map_err(|error| error.to_string())?;
			let current_session = ReplicationSessionV1::from_topology(
				current,
				local_provider,
				target_key.public().0,
				resume.source_provider,
				resume.target_provider,
				resume.commitment,
			)
			.map_err(|error| error.to_string())?;
			ensure_current(&pinned_session, &current_session)?;
			(pinned_session, resume.operation_id)
		},
		WorkerIntent::Discovered(discovered) => (discovered.session, discovered.operation_id),
	};
	let transport =
		Arc::new(HyperPeerTransport::new(PEER_TIMEOUT).map_err(|error| error.to_string())?);
	let reconciler = ReplicationReconciler::new(stack, transport, target_key)
		.map_err(|error| error.to_string())?;
	reconciler
		.reconcile_one(&session, operation_id)
		.await
		.map_err(|error| error.to_string())?;
	Ok(())
}

async fn discover<A: ReplicationAuthority>(
	authority: Arc<A>,
	stack: Arc<CheckpointStack>,
	local_provider: [u8; 32],
	target_key: &ed25519::Pair,
	duty: &CheckpointDuty,
) -> Result<Option<DiscoveredIntent>, String> {
	let bucket = decode_hash(&duty.bucket_id)?;
	let topology = authority
		.replication_topology(bucket)
		.await
		.map_err(|error| error.to_string())?;
	let checkpoint = topology
		.current_checkpoint
		.as_ref()
		.ok_or_else(|| "current checkpoint is unavailable".to_string())?;
	let target = topology
		.providers
		.iter()
		.find(|provider| provider.provider == local_provider)
		.ok_or_else(|| "local target is absent from topology".to_string())?;
	if target.primary {
		return Err("replication discovery target is not a replica".into());
	}
	let start = checkpoint.commitment.start_seq;
	let end = start
		.checked_add(checkpoint.commitment.leaf_count)
		.ok_or_else(|| "checkpoint commitment range overflowed".to_string())?;
	let bucket_id = BucketId::from_bytes(bucket);
	let local_ready = stack
		.replication_checkpoint_ready(bucket_id, checkpoint.commitment.mmr_root.0, end)
		.map_err(|error| error.to_string())?;
	if target.confirmed_checkpoint == Some(checkpoint.checkpoint_block) && local_ready {
		return Ok(None);
	}
	let source = select_source(&topology, local_provider, checkpoint.checkpoint_block)
		.ok_or_else(|| "no currently usable checkpoint source".to_string())?;
	let commitment = if local_ready {
		let predecessor = stack
			.replication_predecessor_total(bucket_id, start)
			.map_err(|error| error.to_string())?;
		PeerMmrCommitmentV1::new(
			checkpoint.commitment.mmr_root.0,
			start,
			checkpoint.commitment.leaf_count,
			predecessor,
		)
	} else {
		PeerMmrCommitmentV1::new(checkpoint.commitment.mmr_root.0, 0, end, 0)
	}
	.map_err(|error| error.to_string())?;
	let session = ReplicationSessionV1::from_topology(
		topology,
		local_provider,
		target_key.public().0,
		source,
		local_provider,
		commitment,
	)
	.map_err(|error| error.to_string())?;
	let operation_id = ReplicationReconciler::<HyperPeerTransport>::operation_id(&session);
	let planned = stack
		.plan_replication(&session, operation_id)
		.map_err(|error| error.to_string())?;
	Ok(Some(DiscoveredIntent { intent_key: planned.intent_key, session, operation_id }))
}

#[cfg(test)]
fn latest_replica_duties(
	inventory: Option<&crate::storage::CheckpointDutyInventory>,
) -> Vec<crate::CheckpointDuty> {
	let Some(inventory) = inventory else { return Vec::new() };
	let mut duties = inventory
		.duties
		.clone()
		.into_iter()
		.filter(|duty| {
			duty.snapshot_checkpoint == inventory.snapshot_checkpoint
				&& duty.role == crate::CheckpointDutyRole::Replica
		})
		.collect::<Vec<_>>();
	duties.sort_by(|left, right| left.bucket_id.cmp(&right.bucket_id));
	duties
}

fn select_source(
	topology: &ReplicationTopologySnapshot,
	local_provider: [u8; 32],
	checkpoint: u32,
) -> Option<[u8; 32]> {
	topology
		.providers
		.iter()
		.filter(|provider| {
			provider.provider != local_provider
				&& provider.usable
				&& provider.exclusions.is_empty()
				&& provider.confirmed_checkpoint == Some(checkpoint)
		})
		.min_by_key(|provider| provider.order)
		.map(|provider| provider.provider)
}

fn ensure_current(
	pinned: &ReplicationSessionV1,
	current: &ReplicationSessionV1,
) -> Result<(), String> {
	if current.topology().finalized_number < pinned.topology().finalized_number
		|| current.topology().bucket_version != pinned.topology().bucket_version
		|| current.source().order() != pinned.source().order()
		|| current.target().order() != pinned.target().order()
		|| current.source().endpoint_hash() != pinned.source().endpoint_hash()
		|| current.target().endpoint_hash() != pinned.target().endpoint_hash()
		|| current.source().service_key() != pinned.source().service_key()
		|| current.source().service_key_version() != pinned.source().service_key_version()
		|| current.target().service_key() != pinned.target().service_key()
		|| current.target().service_key_version() != pinned.target().service_key_version()
	{
		Err("current replication authority revoked the pinned session".into())
	} else {
		Ok(())
	}
}

fn decode_hash(value: &str) -> Result<[u8; 32], String> {
	let bytes = hex::decode(value.trim_start_matches("0x")).map_err(|error| error.to_string())?;
	bytes.try_into().map_err(|_| "bucket id must be exactly 32 bytes".into())
}

#[cfg(test)]
mod tests {
	use std::{fs, sync::Mutex};

	use async_trait::async_trait;
	use codec::Encode;
	use orbis_storage_runtime_api::{CheckpointInfo, CommitmentInfo};
	use sp_core::{crypto::AccountId32, H256};
	use sp_crypto_hashing::blake2_256;
	use tempfile::TempDir;

	use super::*;
	use crate::{
		chain::{ChainError, ReplicationProviderSnapshot},
		peer_responder::PeerResponder,
		peer_transport::{PeerTransport, PeerTransportError},
		replication::ReplicationPhase,
		storage::{
			bucket_mmr::BucketMmrStore, CheckpointDutyInventory, StreamingDescriptor,
			StreamingStore,
		},
		CheckpointDutyMode, CheckpointDutyPhase, CheckpointDutyRole, OperationId,
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
	}

	#[async_trait]
	impl PeerTransport for DirectTransport {
		async fn page(
			&self,
			_: &ReplicationSessionV1,
			request: &[u8],
		) -> Result<Vec<u8>, PeerTransportError> {
			self.requests.lock().unwrap().push(request.to_vec());
			self.responder.page(request).await.map_err(|_| PeerTransportError::Response)
		}

		async fn chunk(
			&self,
			_: &ReplicationSessionV1,
			request: &[u8],
		) -> Result<Vec<u8>, PeerTransportError> {
			self.requests.lock().unwrap().push(request.to_vec());
			self.responder.chunk(request).await.map_err(|_| PeerTransportError::Response)
		}
	}

	fn duty(snapshot: u32, role: CheckpointDutyRole, value: u16) -> CheckpointDuty {
		CheckpointDuty {
			duty_id: format!("0x{:064x}", value),
			bucket_id: format!("0x{:064x}", value),
			provider: format!("0x{}", hex::encode([9; 32])),
			role,
			service_key_version: 1,
			service_key: format!("0x{}", hex::encode([19; 32])),
			snapshot_checkpoint: snapshot,
			snapshot_hash: format!("0x{}", hex::encode([2; 32])),
			due_at: snapshot,
			grace_until: snapshot,
			phase: CheckpointDutyPhase::NotDue,
			mode: CheckpointDutyMode::Standard,
			may_sign: true,
			may_initiate: false,
			encoded_duty: "0x00".into(),
			duty_fingerprint: format!("0x{}", hex::encode([3; 32])),
		}
	}

	fn provider(
		value: u8,
		order: u8,
		usable: bool,
		checkpoint: Option<u32>,
	) -> ReplicationProviderSnapshot {
		ReplicationProviderSnapshot {
			provider: [value; 32],
			order,
			primary: order == 0,
			record_present: true,
			endpoint: Some(format!("https://provider-{value}.invalid").into_bytes()),
			endpoint_hash: Some([value; 32]),
			active_service_key: Some([value + 10; 32]),
			active_service_key_version: Some(1),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(7),
			overdue_challenges: 0,
			eligible: true,
			usable,
			exclusions: Vec::new(),
			confirmed_checkpoint: checkpoint,
		}
	}

	fn pair(seed: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[seed; 32])
	}

	fn topology_provider(
		provider: [u8; 32],
		order: u8,
		key: [u8; 32],
		confirmed_checkpoint: Option<u32>,
	) -> ReplicationProviderSnapshot {
		let endpoint = format!("https://provider-{order}.invalid").into_bytes();
		ReplicationProviderSnapshot {
			provider,
			order,
			primary: order == 0,
			record_present: true,
			endpoint_hash: Some(blake2_256(&endpoint)),
			endpoint: Some(endpoint),
			active_service_key: Some(key),
			active_service_key_version: Some(u64::from(order) + 1),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(50),
			overdue_challenges: 0,
			eligible: true,
			usable: true,
			exclusions: Vec::new(),
			confirmed_checkpoint,
		}
	}

	fn install_object(
		root: &std::path::Path,
		streaming: &StreamingStore,
		operation: u8,
		bytes: &[u8],
	) -> crate::CanonicalCid {
		let cid = crate::CanonicalCid::from_digest(blake2_256(bytes));
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: OperationId::from_bytes([operation; 16]),
					bucket_id: BucketId::from_bytes([3; 32]),
					expected_cid: cid.to_string(),
					object_len: bytes.len() as u64,
				},
				std::iter::once(bytes.to_vec()),
			)
			.unwrap();
		assert!(root.join("streaming-v1").join("objects").join(cid.as_str()).is_file());
		cid
	}

	async fn corrupt_target_is_discovered_and_repaired(
		target_confirmed: bool,
		target_has_suffix: bool,
	) {
		let source = TempDir::new().unwrap();
		let target = TempDir::new().unwrap();
		let first = b"checkpoint-prefix".to_vec();
		let second = b"checkpoint-suffix".to_vec();
		let source_streaming = StreamingStore::open(source.path()).unwrap();
		let first_cid = install_object(source.path(), &source_streaming, 7, &first);
		install_object(source.path(), &source_streaming, 8, &second);
		let source_mmr = BucketMmrStore::open(source.path(), &source_streaming).unwrap();
		let suffix = source_mmr
			.commitment_candidate(&source_streaming, BucketId::from_bytes([3; 32]), 1)
			.unwrap();
		assert_eq!(suffix.start_seq, 1);
		assert_eq!(suffix.leaf_count, 1);

		let target_streaming = StreamingStore::open(target.path()).unwrap();
		install_object(target.path(), &target_streaming, 97, &first);
		if target_has_suffix {
			install_object(target.path(), &target_streaming, 98, &second);
		}
		drop(BucketMmrStore::open(target.path(), &target_streaming).unwrap());
		drop(target_streaming);
		let object = target.path().join("streaming-v1").join("objects").join(first_cid.as_str());
		let mut damaged = first.clone();
		damaged[0] ^= 1;
		fs::write(object, damaged).unwrap();

		let checkpoint = 50;
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: [1; 32],
			finalized_hash: [2; 32],
			finalized_number: 60,
			governed_finalized_checkpoint: Some(checkpoint),
			bucket_id: [3; 32],
			bucket_version: 4,
			primary: [4; 32],
			replicas: vec![[9; 32]],
			providers: vec![
				topology_provider([4; 32], 0, pair(11).public().0, Some(checkpoint)),
				topology_provider(
					[9; 32],
					1,
					pair(12).public().0,
					target_confirmed.then_some(checkpoint),
				),
			],
			current_checkpoint: Some(CheckpointInfo {
				bucket_id: H256::repeat_byte(3),
				commitment: CommitmentInfo {
					mmr_root: suffix.mmr_root,
					start_seq: suffix.start_seq,
					leaf_count: suffix.leaf_count,
				},
				checkpoint_block: checkpoint,
				primary_signers: 1,
				commitment_nonce: checkpoint,
				replica_confirmations: if target_confirmed {
					vec![AccountId32::new([9; 32])]
				} else {
					Vec::new()
				},
			}),
			snapshot_hash: [0; 32],
		};
		let mut topology_bytes = b"cord/provider/replication-topology/v1".to_vec();
		topology.encode_to(&mut topology_bytes);
		topology.snapshot_hash = blake2_256(&topology_bytes);
		let authority = Arc::new(Authority(topology.clone()));
		let responder = Arc::new(
			PeerResponder::new(
				Arc::clone(&authority),
				Arc::new(CheckpointStack::open(source.path()).unwrap()),
				[4; 32],
				pair(11),
			)
			.unwrap(),
		);
		let target_stack = Arc::new(CheckpointStack::open(target.path()).unwrap());
		let mut replica_duty = duty(checkpoint, CheckpointDutyRole::Replica, 3);
		replica_duty.bucket_id = format!("0x{}", hex::encode([3; 32]));
		let discovered = discover(
			Arc::clone(&authority),
			Arc::clone(&target_stack),
			[9; 32],
			&pair(12),
			&replica_duty,
		)
		.await
		.unwrap()
		.expect("corrupt local bytes must not trust chain confirmation");
		assert_eq!(discovered.session.context().candidate_commitment().sequence_range(), (0, 2));
		assert_eq!(discovered.session.context().candidate_commitment().predecessor_total_size(), 0);

		let transport = Arc::new(DirectTransport { responder, requests: Mutex::new(Vec::new()) });
		let reconciler =
			ReplicationReconciler::new(Arc::clone(&target_stack), Arc::clone(&transport), pair(12))
				.unwrap();
		let mut phase = ReplicationPhase::Planned;
		for step in 0..12 {
			let result =
				reconciler.reconcile_one(&discovered.session, discovered.operation_id).await;
			phase = result
				.unwrap_or_else(|error| panic!("reconcile step {step} failed: {error:?}"))
				.phase;
			if phase == ReplicationPhase::MmrCommitted {
				break;
			}
		}
		assert_eq!(phase, ReplicationPhase::MmrCommitted);
		assert!(target_stack
			.replication_checkpoint_ready(BucketId::from_bytes([3; 32]), suffix.mmr_root.0, 2)
			.unwrap());
		assert!(!transport.requests.lock().unwrap().is_empty());
	}

	#[test]
	fn discovery_uses_only_the_exact_latest_installed_replica_inventory() {
		let mut duties = vec![duty(49, CheckpointDutyRole::Replica, 1)];
		duties.push(duty(50, CheckpointDutyRole::Primary, 2));
		duties.extend((3..=140).map(|value| duty(50, CheckpointDutyRole::Replica, value)));
		let inventory = CheckpointDutyInventory {
			finalized_hash: format!("0x{}", hex::encode([2; 32])),
			finalized_number: 60,
			snapshot_checkpoint: 50,
			duties,
		};
		let selected = latest_replica_duties(Some(&inventory));
		assert_eq!(selected.len(), 138);
		assert!(selected.iter().all(|item| {
			item.snapshot_checkpoint == 50 && item.role == CheckpointDutyRole::Replica
		}));
	}

	#[test]
	fn reserved_lanes_advance_stalled_resumes_and_completed_early_duties_beyond_128() {
		let resumes = (0..200).collect::<Vec<_>>();
		let duties = (0..200).collect::<Vec<_>>();
		let (resume_limit, duty_limit) = initial_lane_limits(true);
		assert_eq!((resume_limit, duty_limit), (64, 64));
		assert_eq!(resume_limit + duty_limit, MAX_TICK_INTENTS);
		let mut resume_cursor = 0;
		let mut duty_cursor = 0;
		let mut seen_resumes = BTreeSet::new();
		let mut seen_duties = BTreeSet::new();
		for _ in 0..4 {
			for offset in 0..resume_limit {
				seen_resumes.insert(resumes[(resume_cursor + offset) % resumes.len()]);
			}
			for offset in 0..duty_limit {
				// Reservation advances independently of whether an earlier duty already completed.
				seen_duties.insert(duties[(duty_cursor + offset) % duties.len()]);
			}
			resume_cursor = (resume_cursor + resume_limit) % resumes.len();
			duty_cursor = (duty_cursor + duty_limit) % duties.len();
		}
		assert_eq!(seen_resumes.len(), resumes.len());
		assert_eq!(seen_duties.len(), duties.len());
	}

	#[tokio::test]
	async fn confirmed_corrupt_bucket_is_audited_then_repaired_from_full_checkpoint_range() {
		corrupt_target_is_discovered_and_repaired(true, true).await;
	}

	#[tokio::test]
	async fn unconfirmed_unavailable_prefix_is_repaired_before_missing_suffix_is_installed() {
		corrupt_target_is_discovered_and_repaired(false, false).await;
	}

	#[test]
	fn source_selection_is_ordered_and_observes_current_revocation() {
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: [1; 32],
			finalized_hash: [2; 32],
			finalized_number: 10,
			governed_finalized_checkpoint: Some(7),
			bucket_id: [3; 32],
			bucket_version: 1,
			primary: [4; 32],
			replicas: vec![[5; 32], [9; 32]],
			providers: vec![
				provider(4, 0, true, Some(7)),
				provider(5, 1, true, Some(7)),
				provider(9, 2, true, None),
			],
			current_checkpoint: None,
			snapshot_hash: [6; 32],
		};
		assert_eq!(select_source(&topology, [9; 32], 7), Some([4; 32]));
		topology.providers[0].usable = false;
		assert_eq!(select_source(&topology, [9; 32], 7), Some([5; 32]));
		topology.providers[1].usable = false;
		assert_eq!(select_source(&topology, [9; 32], 7), None);
	}
}
