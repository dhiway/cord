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
	BucketId, CheckpointDuty, CheckpointDutyRole, CheckpointDutyWatermark, DiskStore,
	FinalizedRuntimeAuthority,
};

const MAX_TICK_INTENTS: usize = 128;
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
	let resumes = stack.replication_resume_tick().map_err(|error| error.to_string())?;
	let mut visited = BTreeSet::new();
	let mut work = Vec::new();
	for resume in resumes.into_iter().take(MAX_TICK_INTENTS) {
		visited.insert(resume.intent_key.clone());
		work.push(WorkerIntent::Resume(resume));
	}

	let watermark = store.checkpoint_duty_watermark().map_err(|error| error.to_string())?;
	let duties = store.pending_checkpoint_duties().map_err(|error| error.to_string())?;
	for duty in latest_replica_duties(watermark.as_ref(), duties).into_iter() {
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

async fn discover(
	authority: Arc<FinalizedRuntimeAuthority>,
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
	if target.confirmed_checkpoint == Some(checkpoint.checkpoint_block) {
		return Ok(None);
	}
	let source = select_source(&topology, local_provider, checkpoint.checkpoint_block)
		.ok_or_else(|| "no currently usable checkpoint source".to_string())?;
	let start = checkpoint.commitment.start_seq;
	let predecessor = stack
		.replication_predecessor_total(BucketId::from_bytes(bucket), start)
		.map_err(|error| error.to_string())?;
	let commitment = PeerMmrCommitmentV1::new(
		checkpoint.commitment.mmr_root.0,
		start,
		checkpoint.commitment.leaf_count,
		predecessor,
	)
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

fn latest_replica_duties(
	watermark: Option<&CheckpointDutyWatermark>,
	duties: Vec<CheckpointDuty>,
) -> Vec<CheckpointDuty> {
	let Some(watermark) = watermark else { return Vec::new() };
	let mut duties = duties
		.into_iter()
		.filter(|duty| {
			duty.snapshot_checkpoint == watermark.snapshot_checkpoint
				&& duty.role == CheckpointDutyRole::Replica
		})
		.collect::<Vec<_>>();
	duties.sort_by(|left, right| left.bucket_id.cmp(&right.bucket_id));
	duties.truncate(MAX_TICK_INTENTS);
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
	use super::*;
	use crate::{chain::ReplicationProviderSnapshot, CheckpointDutyMode, CheckpointDutyPhase};

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

	#[test]
	fn discovery_uses_only_latest_installed_replica_snapshot_and_caps_work() {
		let watermark = CheckpointDutyWatermark {
			finalized_hash: format!("0x{}", hex::encode([2; 32])),
			finalized_number: 60,
			snapshot_checkpoint: 50,
			cursor: None,
		};
		let mut duties = vec![duty(49, CheckpointDutyRole::Replica, 1)];
		duties.push(duty(50, CheckpointDutyRole::Primary, 2));
		duties.extend((3..=140).map(|value| duty(50, CheckpointDutyRole::Replica, value)));
		let selected = latest_replica_duties(Some(&watermark), duties);
		assert_eq!(selected.len(), MAX_TICK_INTENTS);
		assert!(selected.iter().all(|item| {
			item.snapshot_checkpoint == 50 && item.role == CheckpointDutyRole::Replica
		}));
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
