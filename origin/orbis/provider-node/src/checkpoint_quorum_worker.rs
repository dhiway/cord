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

//! Bounded initiator-side checkpoint quorum coordinator.

use std::{
	collections::BTreeSet,
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::{Arc, Mutex},
	time::Duration,
};

use codec::Decode;
use orbis_storage_runtime_api::CheckpointDutyInfo;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
use tokio::{
	task::JoinSet,
	time::{interval, MissedTickBehavior},
};

use crate::{
	chain::{ReplicationAuthority, ReplicationTopologySnapshot},
	checkpoint::{checkpoint_quorum::ReplicaConfirmationRequestV1, PreparedCheckpointProposalV2},
	checkpoint_stack::CheckpointStack,
	checkpoint_transport::{
		CheckpointConfirmationEndpoint, CheckpointConfirmationTransport,
		HyperCheckpointConfirmationTransport,
	},
	CheckpointDuty, CheckpointDutyPhase, ContentError, DiskStore, FinalizedRuntimeAuthority,
};

const MAX_RESUMES: usize = 64;
const MAX_CANDIDATES: usize = 64;
const MAX_SELECTIONS: usize = MAX_RESUMES + MAX_CANDIDATES;
const MAX_ACTIONS: usize = 8;
const TRANSPORT_TIMEOUT: Duration = Duration::from_secs(10);
const SCHEDULER_ROOT: &str = "checkpoint-quorum-scheduler-v1";
const SCHEDULER_VERSION: u8 = 1;
const SCHEDULER_DOMAIN: &[u8] = b"cord/provider/checkpoint-quorum-scheduler/v1";

pub(crate) async fn run(
	authority: Arc<FinalizedRuntimeAuthority>,
	stack: Arc<CheckpointStack>,
	store: Arc<DiskStore>,
	local_provider: [u8; 32],
	local_key: ed25519::Pair,
	cadence: Duration,
) {
	let transport = match HyperCheckpointConfirmationTransport::new(TRANSPORT_TIMEOUT) {
		Ok(transport) => Arc::new(transport),
		Err(error) => {
			eprintln!("checkpoint quorum transport initialization failed: {error}");
			return;
		},
	};
	let scheduler = match CheckpointQuorumScheduler::open(store.root()) {
		Ok(scheduler) => Arc::new(scheduler),
		Err(error) => {
			eprintln!("checkpoint quorum scheduler initialization failed: {error}");
			return;
		},
	};
	let mut ticker = interval(cadence.max(Duration::from_secs(1)));
	ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
	loop {
		ticker.tick().await;
		if let Err(error) = tick(
			Arc::clone(&authority),
			Arc::clone(&stack),
			Arc::clone(&store),
			local_provider,
			local_key.clone(),
			Arc::clone(&transport),
			Arc::clone(&scheduler),
		)
		.await
		{
			eprintln!("checkpoint quorum tick failed: {error}");
		}
	}
}

async fn tick<A, T>(
	authority: Arc<A>,
	stack: Arc<CheckpointStack>,
	store: Arc<DiskStore>,
	local_provider: [u8; 32],
	local_key: ed25519::Pair,
	transport: Arc<T>,
	scheduler: Arc<CheckpointQuorumScheduler>,
) -> Result<(), String>
where
	A: ReplicationAuthority + 'static,
	T: CheckpointConfirmationTransport + 'static,
{
	let Some(inventory) = store.checkpoint_duty_inventory().map_err(|error| error.to_string())?
	else {
		return Ok(());
	};
	let resumes = stack
		.outstanding_checkpoint_quorums()
		.map_err(|error| error.to_string())?
		.into_iter()
		.filter_map(|proposal| {
			inventory
				.duties
				.iter()
				.find(|duty| current_duty_matches_proposal(duty, &proposal))
				.cloned()
				.map(|duty| (proposal, duty))
		})
		.collect::<Vec<_>>();
	let resume_ids = resumes
		.iter()
		.map(|(proposal, _)| proposal.duty_id.clone())
		.collect::<BTreeSet<_>>();
	let candidates = inventory
		.duties
		.iter()
		.filter(|duty| {
			duty.may_initiate
				&& duty.mode == crate::CheckpointDutyMode::Standard
				&& matches!(
					duty.phase,
					CheckpointDutyPhase::Primary | CheckpointDutyPhase::ReplicaFallback
				)
		})
		.filter(|duty| !resume_ids.contains(duty.duty_id.trim_start_matches("0x")))
		.cloned()
		.collect::<Vec<_>>();
	let (resumes, candidates) = scheduler
		.scan(&inventory, resumes, candidates)
		.map_err(|error| error.to_string())?;

	let mut actions = Vec::with_capacity(MAX_ACTIONS);
	for selection in fair_considerations(resumes, candidates) {
		if actions.len() == MAX_ACTIONS {
			break;
		}
		let admission = Admission::from(&selection);
		let prepared = match selection {
			Selection::Resume(proposal, duty) => stack
				.resume_checkpoint_quorum(&proposal, &local_key)
				.map(|snapshot| (proposal, duty, snapshot)),
			Selection::Candidate(duty) => stack
				.begin_checkpoint_quorum(&store, &duty, &local_key)
				.map(|(proposal, snapshot)| (proposal, duty, snapshot)),
		};
		match prepared {
			Ok((proposal, duty, snapshot)) => {
				if let Some(request) = snapshot.requests.into_iter().next() {
					scheduler.admit(&inventory, &admission).map_err(|error| error.to_string())?;
					actions.push((proposal, duty, request));
				}
			},
			Err(error) => eprintln!("checkpoint quorum selection skipped: {error}"),
		}
	}

	let mut tasks = JoinSet::new();
	let local_public = local_key.public().0;
	for (proposal, duty, request) in actions {
		let authority = Arc::clone(&authority);
		let stack = Arc::clone(&stack);
		let transport = Arc::clone(&transport);
		let inventory = inventory.clone();
		tasks.spawn(async move {
			let endpoint = resolve_endpoint(
				&*authority,
				&inventory,
				&duty,
				&proposal,
				&request,
				local_provider,
				local_public,
			)
			.await?;
			dispatch_confirmation(stack, transport, endpoint, proposal, request).await
		});
	}
	while let Some(result) = tasks.join_next().await {
		match result {
			Ok(Ok(())) => {},
			Ok(Err(error)) => eprintln!("checkpoint quorum action skipped: {error}"),
			Err(error) => eprintln!("checkpoint quorum action join failed: {error}"),
		}
	}
	Ok(())
}

async fn dispatch_confirmation<T: CheckpointConfirmationTransport>(
	stack: Arc<CheckpointStack>,
	transport: Arc<T>,
	endpoint: CheckpointConfirmationEndpoint,
	proposal: PreparedCheckpointProposalV2,
	request: Vec<u8>,
) -> Result<(), ContentError> {
	let response = transport
		.confirm(&endpoint, &request)
		.await
		.map_err(|_| ContentError::IntegrityFailed)?;
	stack.accept_checkpoint_confirmation(&proposal, &response)?;
	Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchedulerRecordV1 {
	version: u8,
	finalized_hash: String,
	finalized_number: u32,
	snapshot_checkpoint: u32,
	resume_after: Option<String>,
	candidate_after: Option<String>,
	record_hash: String,
}

struct CheckpointQuorumScheduler {
	root: PathBuf,
	record: Mutex<Option<SchedulerRecordV1>>,
}

impl CheckpointQuorumScheduler {
	fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let root = root.as_ref().join(SCHEDULER_ROOT);
		fs::create_dir_all(&root).map_err(io_error)?;
		let path = root.join("cursor.json");
		let temp = root.join("cursor.json.tmp");
		if temp.exists() {
			fs::remove_file(&temp).map_err(io_error)?;
		}
		for entry in fs::read_dir(&root).map_err(io_error)? {
			let entry = entry.map_err(io_error)?;
			if entry.path() != path || !entry.file_type().map_err(io_error)?.is_file() {
				return Err(ContentError::IntegrityFailed);
			}
		}
		let record = if path.exists() {
			let bytes = fs::read(&path).map_err(io_error)?;
			if bytes.len() > 4096 {
				return Err(ContentError::IntegrityFailed);
			}
			let record: SchedulerRecordV1 =
				serde_json::from_slice(&bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_scheduler_record(&record)?;
			Some(record)
		} else {
			None
		};
		Ok(Self { root, record: Mutex::new(record) })
	}

	fn scan(
		&self,
		inventory: &crate::storage::CheckpointDutyInventory,
		mut resumes: Vec<(PreparedCheckpointProposalV2, CheckpointDuty)>,
		mut candidates: Vec<CheckpointDuty>,
	) -> Result<
		(Vec<(PreparedCheckpointProposalV2, CheckpointDuty)>, Vec<CheckpointDuty>),
		ContentError,
	> {
		resumes.sort_by(|left, right| resume_key(&left.0).cmp(&resume_key(&right.0)));
		candidates.sort_by_key(duty_key);
		let guard = self.record.lock().map_err(|_| ContentError::IntegrityFailed)?;
		let canonical_finalized_hash = hex::encode(decode_hash(&inventory.finalized_hash)?);
		let same_inventory = guard.as_ref().is_some_and(|record| {
			record.finalized_hash == canonical_finalized_hash
				&& record.finalized_number == inventory.finalized_number
				&& record.snapshot_checkpoint == inventory.snapshot_checkpoint
		});
		let resume_after = same_inventory
			.then(|| guard.as_ref().and_then(|record| record.resume_after.as_deref()))
			.flatten();
		let candidate_after = same_inventory
			.then(|| guard.as_ref().and_then(|record| record.candidate_after.as_deref()))
			.flatten();
		let resumes = round_robin(resumes, resume_after, MAX_RESUMES, |item| resume_key(&item.0));
		let candidates = round_robin(candidates, candidate_after, MAX_CANDIDATES, duty_key);
		debug_assert!(resumes.len().saturating_add(candidates.len()) <= MAX_SELECTIONS);
		Ok((resumes, candidates))
	}

	fn admit(
		&self,
		inventory: &crate::storage::CheckpointDutyInventory,
		admission: &Admission,
	) -> Result<(), ContentError> {
		let mut guard = self.record.lock().map_err(|_| ContentError::IntegrityFailed)?;
		let canonical_finalized_hash = hex::encode(decode_hash(&inventory.finalized_hash)?);
		let same_inventory = guard.as_ref().is_some_and(|record| {
			record.finalized_hash == canonical_finalized_hash
				&& record.finalized_number == inventory.finalized_number
				&& record.snapshot_checkpoint == inventory.snapshot_checkpoint
		});
		let previous_resume = same_inventory
			.then(|| guard.as_ref().and_then(|record| record.resume_after.clone()))
			.flatten();
		let previous_candidate = same_inventory
			.then(|| guard.as_ref().and_then(|record| record.candidate_after.clone()))
			.flatten();
		let mut next = SchedulerRecordV1 {
			version: SCHEDULER_VERSION,
			finalized_hash: canonical_finalized_hash,
			finalized_number: inventory.finalized_number,
			snapshot_checkpoint: inventory.snapshot_checkpoint,
			resume_after: match admission.lane {
				Lane::Resume => Some(admission.key.clone()),
				Lane::Candidate => previous_resume,
			},
			candidate_after: match admission.lane {
				Lane::Resume => previous_candidate,
				Lane::Candidate => Some(admission.key.clone()),
			},
			record_hash: String::new(),
		};
		next.record_hash = scheduler_hash(&next)?;
		self.persist(&next)?;
		*guard = Some(next);
		Ok(())
	}

	fn persist(&self, record: &SchedulerRecordV1) -> Result<(), ContentError> {
		validate_scheduler_record(record)?;
		let bytes = serde_json::to_vec(record).map_err(io_error)?;
		let temp = self.root.join("cursor.json.tmp");
		let path = self.root.join("cursor.json");
		let mut file = File::create(&temp).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		file.sync_all().map_err(io_error)?;
		fs::rename(temp, path).map_err(io_error)?;
		File::open(&self.root).and_then(|file| file.sync_all()).map_err(io_error)
	}
}

fn round_robin<T, F>(mut items: Vec<T>, after: Option<&str>, limit: usize, key: F) -> Vec<T>
where
	T: Clone,
	F: Fn(&T) -> String,
{
	if items.is_empty() {
		return Vec::new();
	}
	items.sort_by_key(&key);
	let start = after
		.and_then(|after| items.iter().position(|item| key(item).as_str() > after))
		.unwrap_or(0);
	items.iter().cycle().skip(start).take(items.len().min(limit)).cloned().collect()
}

fn fair_considerations(
	resumes: Vec<(PreparedCheckpointProposalV2, CheckpointDuty)>,
	candidates: Vec<CheckpointDuty>,
) -> Vec<Selection> {
	let mut resumes = resumes.into_iter();
	let mut candidates = candidates.into_iter();
	let mut selected = Vec::with_capacity(MAX_SELECTIONS);
	loop {
		let resume = resumes.next();
		let candidate = candidates.next();
		if resume.is_none() && candidate.is_none() {
			break;
		}
		if let Some((proposal, duty)) = resume {
			selected.push(Selection::Resume(proposal, duty));
		}
		if let Some(duty) = candidate {
			selected.push(Selection::Candidate(duty));
		}
	}
	selected
}

fn duty_key(duty: &CheckpointDuty) -> String {
	format!("{}:{}", duty.bucket_id.trim_start_matches("0x"), duty.duty_id.trim_start_matches("0x"))
}

fn resume_key(proposal: &PreparedCheckpointProposalV2) -> String {
	format!("{}:{}", proposal.bucket_id, proposal.duty_id)
}

fn validate_scheduler_record(record: &SchedulerRecordV1) -> Result<(), ContentError> {
	if record.version != SCHEDULER_VERSION
		|| decode_hash(&record.finalized_hash)
			.is_ok_and(|hash| hex::encode(hash) != record.finalized_hash)
		|| decode_hash(&record.finalized_hash).is_err()
		|| record.resume_after.as_deref().is_some_and(|value| !valid_cursor_key(value))
		|| record.candidate_after.as_deref().is_some_and(|value| !valid_cursor_key(value))
		|| record.record_hash != scheduler_hash(record)?
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn valid_cursor_key(value: &str) -> bool {
	let Some((bucket, duty)) = value.split_once(':') else { return false };
	[bucket, duty].iter().all(|part| {
		part.len() == 64
			&& hex::decode(part).is_ok_and(|bytes| bytes.len() == 32 && hex::encode(bytes) == *part)
	})
}

fn scheduler_hash(record: &SchedulerRecordV1) -> Result<String, ContentError> {
	let mut canonical = record.clone();
	canonical.record_hash.clear();
	let mut input = SCHEDULER_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(sp_crypto_hashing::blake2_256(&input)))
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

enum Selection {
	Resume(PreparedCheckpointProposalV2, CheckpointDuty),
	Candidate(CheckpointDuty),
}

#[derive(Clone, Copy)]
enum Lane {
	Resume,
	Candidate,
}

struct Admission {
	lane: Lane,
	key: String,
}

impl From<&Selection> for Admission {
	fn from(selection: &Selection) -> Self {
		match selection {
			Selection::Resume(proposal, _) => {
				Self { lane: Lane::Resume, key: resume_key(proposal) }
			},
			Selection::Candidate(duty) => Self { lane: Lane::Candidate, key: duty_key(duty) },
		}
	}
}

fn current_duty_matches_proposal(
	duty: &CheckpointDuty,
	proposal: &PreparedCheckpointProposalV2,
) -> bool {
	duty.may_initiate
		&& matches!(duty.phase, CheckpointDutyPhase::Primary | CheckpointDutyPhase::ReplicaFallback)
		&& duty.duty_id.trim_start_matches("0x") == proposal.duty_id
		&& duty.duty_fingerprint.trim_start_matches("0x") == proposal.duty_fingerprint
}

async fn resolve_endpoint<A: ReplicationAuthority>(
	authority: &A,
	inventory: &crate::storage::CheckpointDutyInventory,
	duty: &CheckpointDuty,
	proposal: &PreparedCheckpointProposalV2,
	request_bytes: &[u8],
	local_provider: [u8; 32],
	local_key: [u8; 32],
) -> Result<CheckpointConfirmationEndpoint, ContentError> {
	if !current_duty_matches_proposal(duty, proposal) {
		return Err(ContentError::IntegrityFailed);
	}
	let request = ReplicaConfirmationRequestV1::decode_canonical(request_bytes)?;
	let encoded = hex::decode(duty.encoded_duty.trim_start_matches("0x"))
		.map_err(|_| ContentError::IntegrityFailed)?;
	let mut input = &encoded[..];
	let decoded = CheckpointDutyInfo::<AccountId32, H256, u32>::decode(&mut input)
		.map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || decoded.duty_id != request.duty_id {
		return Err(ContentError::IntegrityFailed);
	}
	let bucket = request.payload.bucket_id.0;
	let finalized_hash = decode_hash(&inventory.finalized_hash)?;
	let pinned = authority
		.replication_topology_at(bucket, finalized_hash, inventory.finalized_number)
		.await
		.map_err(|_| ContentError::IntegrityFailed)?;
	if pinned.finalized_hash != finalized_hash
		|| pinned.finalized_number != inventory.finalized_number
		|| pinned.governed_finalized_checkpoint != Some(inventory.snapshot_checkpoint)
	{
		return Err(ContentError::IntegrityFailed);
	}
	let current = authority
		.replication_topology(bucket)
		.await
		.map_err(|_| ContentError::IntegrityFailed)?;
	validate_topology_identity(&request, &decoded, &pinned, &current, local_provider, local_key)?;
	let target = account_bytes(&request.target_provider)?;
	let provider = pinned
		.providers
		.iter()
		.find(|provider| provider.provider == target)
		.ok_or(ContentError::IntegrityFailed)?;
	CheckpointConfirmationEndpoint::pinned(
		provider.endpoint.as_deref().ok_or(ContentError::IntegrityFailed)?,
		provider.endpoint_hash.ok_or(ContentError::IntegrityFailed)?,
	)
	.map_err(|_| ContentError::IntegrityFailed)
}

fn validate_topology_identity(
	request: &ReplicaConfirmationRequestV1,
	duty: &CheckpointDutyInfo<AccountId32, H256, u32>,
	pinned: &ReplicationTopologySnapshot,
	current: &ReplicationTopologySnapshot,
	local_provider: [u8; 32],
	local_key: [u8; 32],
) -> Result<(), ContentError> {
	let primary = account_bytes(&request.primary_provider)?;
	let target = account_bytes(&request.target_provider)?;
	if primary != local_provider
		|| request.primary_service_key.0 != local_key
		|| pinned.bucket_id != request.payload.bucket_id.0
		|| current.bucket_id != pinned.bucket_id
		|| current.finalized_number < pinned.finalized_number
		|| current.governed_finalized_checkpoint < pinned.governed_finalized_checkpoint
		|| current.bucket_version != pinned.bucket_version
	{
		return Err(ContentError::IntegrityFailed);
	}
	for (provider, key, version) in [
		(primary, local_key, None),
		(target, request.target_service_key.0, Some(request.target_service_key_version)),
	] as [([u8; 32], [u8; 32], Option<u64>); 2]
	{
		let authority = duty
			.authorities
			.iter()
			.find(|authority| account_bytes(&authority.provider).ok() == Some(provider))
			.ok_or(ContentError::IntegrityFailed)?;
		let pinned_provider = topology_provider(pinned, provider)?;
		let current_provider = topology_provider(current, provider)?;
		if authority.active_service_key != key
			|| version.is_some_and(|version| authority.active_service_key_version != version)
			|| pinned_provider.active_service_key != Some(key)
			|| version
				.is_some_and(|version| pinned_provider.active_service_key_version != Some(version))
			|| pinned_provider.endpoint_hash != Some(authority.endpoint_hash.0)
			|| !pinned_provider.usable
			|| !current_provider.usable
			|| current_provider.order != pinned_provider.order
			|| current_provider.active_service_key != pinned_provider.active_service_key
			|| current_provider.active_service_key_version
				!= pinned_provider.active_service_key_version
			|| current_provider.endpoint_hash != pinned_provider.endpoint_hash
		{
			return Err(ContentError::IntegrityFailed);
		}
	}
	Ok(())
}

fn topology_provider(
	topology: &ReplicationTopologySnapshot,
	provider: [u8; 32],
) -> Result<&crate::chain::ReplicationProviderSnapshot, ContentError> {
	topology
		.providers
		.iter()
		.find(|candidate| candidate.provider == provider)
		.ok_or(ContentError::IntegrityFailed)
}

fn decode_hash(value: &str) -> Result<[u8; 32], ContentError> {
	hex::decode(value.trim_start_matches("0x"))
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)
}

fn account_bytes(account: &AccountId32) -> Result<[u8; 32], ContentError> {
	let bytes: &[u8] = account.as_ref();
	bytes.try_into().map_err(|_| ContentError::IntegrityFailed)
}

#[cfg(test)]
mod tests {
	use async_trait::async_trait;
	use tempfile::TempDir;
	use tokio::sync::Notify;

	use super::*;
	use crate::{
		storage::CheckpointDutyInventory, CheckpointDutyMode, CheckpointDutyRole, NodeProfile,
	};

	fn duty(id: u8) -> CheckpointDuty {
		CheckpointDuty {
			duty_id: format!("0x{}", hex::encode([id; 32])),
			bucket_id: format!("0x{}", hex::encode([id; 32])),
			provider: format!("0x{}", hex::encode([1; 32])),
			role: CheckpointDutyRole::Primary,
			service_key_version: 1,
			service_key: format!("0x{}", hex::encode([2; 32])),
			snapshot_checkpoint: 7,
			snapshot_hash: format!("0x{}", hex::encode([3; 32])),
			due_at: 7,
			grace_until: 8,
			phase: CheckpointDutyPhase::Primary,
			mode: CheckpointDutyMode::Standard,
			may_sign: true,
			may_initiate: true,
			encoded_duty: "0x00".into(),
			duty_fingerprint: format!("0x{}", hex::encode([4; 32])),
		}
	}

	fn inventory(number: u32) -> CheckpointDutyInventory {
		CheckpointDutyInventory {
			finalized_hash: hex::encode([5; 32]),
			finalized_number: number,
			snapshot_checkpoint: 7,
			duties: (1..=120).map(duty).collect(),
		}
	}

	fn proposal() -> PreparedCheckpointProposalV2 {
		PreparedCheckpointProposalV2 {
			version: 2,
			duty_id: String::new(),
			duty_fingerprint: String::new(),
			duty_scale: String::new(),
			finalized_number: 0,
			finalized_hash: String::new(),
			snapshot_checkpoint: 0,
			snapshot_hash: String::new(),
			service_key_version: 0,
			service_key: String::new(),
			primary_provider: String::new(),
			bucket_id: String::new(),
			window_start: 0,
			window_end: 0,
			nonce: 0,
			start_seq: 0,
			leaf_count: 0,
			mmr_root: String::new(),
			payload_scale: String::new(),
			digest: String::new(),
			signature: String::new(),
			context_scale: String::new(),
			context_digest: String::new(),
			context_signature: String::new(),
			state: String::new(),
			record_hash: String::new(),
		}
	}

	fn resume(id: u8) -> (PreparedCheckpointProposalV2, CheckpointDuty) {
		let mut proposal = proposal();
		proposal.bucket_id = hex::encode([id; 32]);
		proposal.duty_id = hex::encode([id; 32]);
		(proposal, duty(id))
	}

	fn action_ids(actions: &[Selection]) -> (Vec<u8>, Vec<u8>) {
		let mut resumes = Vec::new();
		let mut candidates = Vec::new();
		for action in actions {
			match action {
				Selection::Resume(proposal, _) => {
					resumes.push(hex::decode(&proposal.bucket_id).unwrap()[0])
				},
				Selection::Candidate(duty) => candidates
					.push(hex::decode(duty.bucket_id.trim_start_matches("0x")).unwrap()[0]),
			}
		}
		(resumes, candidates)
	}

	fn admit_successes(
		scheduler: &CheckpointQuorumScheduler,
		current: &CheckpointDutyInventory,
		resumes: Vec<(PreparedCheckpointProposalV2, CheckpointDuty)>,
		candidates: Vec<CheckpointDuty>,
		failed_considerations: usize,
	) -> Vec<Selection> {
		let scanned = scheduler.scan(current, resumes, candidates).unwrap();
		let mut admitted = Vec::new();
		for (index, selection) in fair_considerations(scanned.0, scanned.1).into_iter().enumerate()
		{
			if index < failed_considerations {
				continue;
			}
			scheduler.admit(current, &Admission::from(&selection)).unwrap();
			admitted.push(selection);
			if admitted.len() == MAX_ACTIONS {
				break;
			}
		}
		admitted
	}

	#[test]
	fn ninth_persistent_resume_is_next_after_reopen_not_the_sixty_fifth() {
		let temp = TempDir::new().unwrap();
		let current = inventory(9);
		let resumes = (1..=70).map(resume).collect::<Vec<_>>();
		let scheduler = CheckpointQuorumScheduler::open(temp.path()).unwrap();
		let first = admit_successes(&scheduler, &current, resumes.clone(), Vec::new(), 0);
		assert_eq!(action_ids(&first).0, (1..=8).collect::<Vec<_>>());
		drop(scheduler);
		let reopened = CheckpointQuorumScheduler::open(temp.path()).unwrap();
		let second = admit_successes(&reopened, &current, resumes, Vec::new(), 0);
		assert_eq!(action_ids(&second).0.first(), Some(&9));
	}

	#[test]
	fn busy_resume_and_candidate_lanes_each_receive_four_actions_every_tick() {
		let temp = TempDir::new().unwrap();
		let current = inventory(9);
		let scheduler = CheckpointQuorumScheduler::open(temp.path()).unwrap();
		let resumes = (1..=20).map(resume).collect::<Vec<_>>();
		let candidates = (101..=120).map(duty).collect::<Vec<_>>();
		for expected_start in [1, 5] {
			let actions =
				admit_successes(&scheduler, &current, resumes.clone(), candidates.clone(), 0);
			let (resume_ids, candidate_ids) = action_ids(&actions);
			assert_eq!(resume_ids, (expected_start..expected_start + 4).collect::<Vec<_>>());
			assert_eq!(candidate_ids.len(), 4);
		}
	}

	#[test]
	fn uneven_or_empty_lane_fills_every_available_action_slot() {
		for (case, resumes, candidates, expected) in [
			(1, (1..=2).map(resume).collect(), (101..=110).map(duty).collect(), (2, 6)),
			(2, (1..=10).map(resume).collect(), Vec::new(), (8, 0)),
			(3, Vec::new(), (101..=103).map(duty).collect(), (0, 3)),
		] {
			let temp = TempDir::new().unwrap();
			let scheduler = CheckpointQuorumScheduler::open(temp.path()).unwrap();
			let current = inventory(case);
			let actions = admit_successes(&scheduler, &current, resumes, candidates, 0);
			let ids = action_ids(&actions);
			assert_eq!((ids.0.len(), ids.1.len()), expected);
		}
	}

	#[test]
	fn failed_or_requestless_selection_does_not_advance_but_cannot_starve_followers() {
		for candidates_only in [false, true] {
			let temp = TempDir::new().unwrap();
			let current = inventory(9);
			let scheduler = CheckpointQuorumScheduler::open(temp.path()).unwrap();
			let resumes = (!candidates_only)
				.then(|| (1..=12).map(resume).collect::<Vec<_>>())
				.unwrap_or_default();
			let candidates = candidates_only
				.then(|| (1..=12).map(duty).collect::<Vec<_>>())
				.unwrap_or_default();
			let scanned = scheduler.scan(&current, resumes.clone(), candidates.clone()).unwrap();
			let first_considered = fair_considerations(scanned.0, scanned.1);
			let expected_first =
				if candidates_only { (Vec::new(), vec![1]) } else { (vec![1], Vec::new()) };
			assert_eq!(action_ids(&first_considered[..1]), expected_first);
			drop(scheduler);
			let reopened = CheckpointQuorumScheduler::open(temp.path()).unwrap();
			let unchanged = reopened.scan(&current, resumes.clone(), candidates.clone()).unwrap();
			assert_eq!(
				action_ids(&fair_considerations(unchanged.0, unchanged.1)[..1]),
				expected_first
			);
			// The first selected preparation fails/no-requests; later prepared actions still progress.
			let admitted =
				admit_successes(&reopened, &current, resumes.clone(), candidates.clone(), 1);
			assert_eq!(admitted.len(), MAX_ACTIONS);
			drop(reopened);
			let reopened = CheckpointQuorumScheduler::open(temp.path()).unwrap();
			let next = admit_successes(&reopened, &current, resumes, candidates, 0);
			let ids = action_ids(&next);
			let first = if candidates_only { ids.1.first() } else { ids.0.first() };
			assert_eq!(first, Some(&10));
		}
	}

	struct GatedTransport {
		entered: Arc<Notify>,
		release: Arc<Notify>,
	}

	#[async_trait]
	impl CheckpointConfirmationTransport for GatedTransport {
		async fn confirm(
			&self,
			_endpoint: &CheckpointConfirmationEndpoint,
			_request: &[u8],
		) -> Result<Vec<u8>, crate::checkpoint_transport::CheckpointTransportError> {
			self.entered.notify_one();
			self.release.notified().await;
			Ok(Vec::new())
		}
	}

	#[tokio::test]
	async fn outbound_await_never_holds_the_checkpoint_stack_lock() {
		let temp = TempDir::new().unwrap();
		let profile = NodeProfile {
			provider: format!("0x{}", hex::encode([1; 32])),
			endpoint: "https://provider.invalid".into(),
			service_key: format!("0x{}", hex::encode([2; 32])),
			region: None,
		};
		DiskStore::open(temp.path(), profile, 1024).unwrap();
		let stack = Arc::new(CheckpointStack::open(temp.path()).unwrap());
		let entered = Arc::new(Notify::new());
		let release = Arc::new(Notify::new());
		let transport = Arc::new(GatedTransport {
			entered: Arc::clone(&entered),
			release: Arc::clone(&release),
		});
		let endpoint_bytes = b"http://127.0.0.1:1/";
		let endpoint = CheckpointConfirmationEndpoint::pinned(
			endpoint_bytes,
			sp_crypto_hashing::blake2_256(endpoint_bytes),
		)
		.unwrap();
		let task = tokio::spawn(dispatch_confirmation(
			Arc::clone(&stack),
			transport,
			endpoint,
			proposal(),
			vec![1],
		));
		entered.notified().await;
		assert!(stack.guard_is_available());
		task.abort();
	}
}
