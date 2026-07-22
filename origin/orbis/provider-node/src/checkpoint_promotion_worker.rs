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

//! Bounded discovery and finality lifecycle for checkpoint fallback promotion.

use std::{
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::Mutex,
	time::Duration,
};

use codec::{Decode, Encode};
use orbis_storage_runtime_api::{
	CheckpointDutyInfo, CheckpointDutyMode as RuntimeMode,
	CheckpointDutyPhase as RuntimePhase, ProviderDutyAuthority, ProviderDutyRole,
};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, ed25519, H256};
use sp_crypto_hashing::blake2_256;
use tokio::time::timeout;

use crate::{
	chain::{
		validate_checkpoint_duty, ReplicationAuthority, ReplicationProviderSnapshot,
		ReplicationTopologySnapshot,
	},
	checkpoint::{
		checkpoint_promotion::FallbackPromotionFinalizedReceiptV2,
		checkpoint_promotion_submitter::PromotionFinalityLane,
	},
	checkpoint_stack::CheckpointStack,
	storage::CheckpointDutyInventory,
	CheckpointDuty, CheckpointDutyPhase, CheckpointDutyRole, ContentError, DiskStore,
};

const ROOT: &str = "checkpoint-promotion-discovery-v1";
const VERSION: u8 = 1;
const DOMAIN: &[u8] = b"cord/provider/checkpoint-promotion-discovery/v1";
const MAX_SCAN: usize = 64;
const MAX_ACTIONS: usize = 8;
const MAX_RECORD_BYTES: usize = 4 * 1024;

type RuntimeDuty = CheckpointDutyInfo<AccountId32, H256, u32>;

#[derive(Clone, Debug)]
struct Candidate {
	public: CheckpointDuty,
	decoded: RuntimeDuty,
	duty_scale: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscoveryCursorV1 {
	version: u8,
	finalized_hash: String,
	finalized_number: u32,
	snapshot_checkpoint: u32,
	last_index: u32,
	last_duty_id: String,
	last_duty_fingerprint: String,
	last_bucket_id: String,
	record_hash: String,
}

pub(crate) struct PromotionDiscoveryScheduler {
	root: PathBuf,
	cursor: Mutex<Option<DiscoveryCursorV1>>,
}

impl PromotionDiscoveryScheduler {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let root = root.as_ref().join(ROOT);
		fs::create_dir_all(&root).map_err(io_error)?;
		let path = root.join("cursor.json");
		let temp = root.join("cursor.json.tmp");
		if temp.exists() {
			fs::remove_file(&temp).map_err(io_error)?;
			File::open(&root).and_then(|directory| directory.sync_all()).map_err(io_error)?;
		}
		for entry in fs::read_dir(&root).map_err(io_error)? {
			let entry = entry.map_err(io_error)?;
			if entry.path() != path || !entry.file_type().map_err(io_error)?.is_file() {
				return Err(ContentError::IntegrityFailed);
			}
		}
		let cursor = if path.exists() {
			let bytes = fs::read(path).map_err(io_error)?;
			if bytes.len() > MAX_RECORD_BYTES {
				return Err(ContentError::IntegrityFailed);
			}
			let cursor = serde_json::from_slice(&bytes).map_err(|_| ContentError::IntegrityFailed)?;
			validate_cursor(&cursor)?;
			Some(cursor)
		} else {
			None
		};
		Ok(Self { root, cursor: Mutex::new(cursor) })
	}

	fn reserve(
		&self,
		inventory: &CheckpointDutyInventory,
		local_provider: [u8; 32],
		local_key: [u8; 32],
	) -> Result<Vec<Candidate>, ContentError> {
		if inventory.duties.is_empty() {
			return Ok(Vec::new());
		}
		let finalized_hash = canonical_hash(&inventory.finalized_hash)?;
		let mut guard = self.cursor.lock().map_err(|_| ContentError::IntegrityFailed)?;
		let same_inventory = guard.as_ref().is_some_and(|cursor| {
			cursor.finalized_hash == finalized_hash
				&& cursor.finalized_number == inventory.finalized_number
				&& cursor.snapshot_checkpoint == inventory.snapshot_checkpoint
		});
		let start = if same_inventory {
			let cursor = guard.as_ref().ok_or(ContentError::IntegrityFailed)?;
			let index = usize::try_from(cursor.last_index).map_err(|_| ContentError::IntegrityFailed)?;
			let duty = inventory.duties.get(index).ok_or(ContentError::IntegrityFailed)?;
			if duty.duty_id != cursor.last_duty_id
				|| duty.duty_fingerprint != cursor.last_duty_fingerprint
				|| duty.bucket_id != cursor.last_bucket_id
			{
				return Err(ContentError::IntegrityFailed);
			}
			(index + 1) % inventory.duties.len()
		} else {
			0
		};
		let mut candidates = Vec::with_capacity(MAX_ACTIONS);
		let mut last_considered = None;
		for offset in 0..inventory.duties.len().min(MAX_SCAN) {
			let index = (start + offset) % inventory.duties.len();
			let duty = &inventory.duties[index];
			last_considered = Some((index, duty));
			if let Ok(candidate) = decode_candidate(inventory, duty, local_provider, local_key) {
				candidates.push(candidate);
				if candidates.len() == MAX_ACTIONS {
					break;
				}
			}
		}
		if let Some((index, duty)) = last_considered {
			let mut cursor = DiscoveryCursorV1 {
				version: VERSION,
				finalized_hash,
				finalized_number: inventory.finalized_number,
				snapshot_checkpoint: inventory.snapshot_checkpoint,
				last_index: u32::try_from(index).map_err(|_| ContentError::IntegrityFailed)?,
				last_duty_id: duty.duty_id.clone(),
				last_duty_fingerprint: duty.duty_fingerprint.clone(),
				last_bucket_id: duty.bucket_id.clone(),
				record_hash: String::new(),
			};
			cursor.record_hash = cursor_hash(&cursor)?;
			persist_cursor(&self.root, &cursor)?;
			*guard = Some(cursor);
		}
		Ok(candidates)
	}
}

pub(crate) async fn tick<A, L>(
	authority: &A,
	stack: &CheckpointStack,
	store: &DiskStore,
	lane: &L,
	scheduler: &PromotionDiscoveryScheduler,
	local_provider: [u8; 32],
	local_key: &ed25519::Pair,
	finality_timeout: Duration,
	topology_timeout: Duration,
) -> Result<Option<FallbackPromotionFinalizedReceiptV2>, ContentError>
where
	A: ReplicationAuthority,
	L: PromotionFinalityLane,
{
	let finalized = match timeout(
		finality_timeout,
		stack.consume_checkpoint_promotion_with_lane_bounded(lane, MAX_ACTIONS),
	)
	.await
	{
		Ok(result) => result,
		Err(_) => Err(ContentError::Io("checkpoint promotion finality timed out".into())),
	};
	let mut first_error = finalized.as_ref().err().cloned();
	if let Some(inventory) = store.checkpoint_duty_inventory().map_err(|_| ContentError::IntegrityFailed)? {
		let candidates = scheduler.reserve(
			&inventory,
			local_provider,
			local_key.public().0,
		)?;
		for candidate in candidates {
			match validate_topology_and_authorize(
				authority,
				stack,
				&inventory,
				candidate,
				local_provider,
				local_key,
				topology_timeout,
			)
			.await
			{
				Ok(()) => {},
				Err(error) => {
					first_error.get_or_insert(error);
				},
			}
		}
	}
	if let Some(error) = first_error {
		return Err(error);
	}
	finalized
}

async fn validate_topology_and_authorize<A: ReplicationAuthority>(
	authority: &A,
	stack: &CheckpointStack,
	inventory: &CheckpointDutyInventory,
	candidate: Candidate,
	local_provider: [u8; 32],
	local_key: &ed25519::Pair,
	topology_timeout: Duration,
) -> Result<(), ContentError> {
	let bucket = hash_bytes(&candidate.public.bucket_id)?;
	let finalized_hash = hash_bytes(&inventory.finalized_hash)?;
	let pinned = timeout(
		topology_timeout,
		authority.replication_topology_at(bucket, finalized_hash, inventory.finalized_number),
	)
	.await
	.map_err(|_| ContentError::Io("pinned promotion topology timed out".into()))?
	.map_err(|_| ContentError::IntegrityFailed)?;
	let current = timeout(topology_timeout, authority.replication_topology(bucket))
		.await
		.map_err(|_| ContentError::Io("current promotion topology timed out".into()))?
		.map_err(|_| ContentError::IntegrityFailed)?;
	validate_topologies(
		inventory,
		&candidate.decoded,
		&pinned,
		&current,
		local_provider,
		local_key.public().0,
	)?;
	stack.authorize_checkpoint_promotion(
		&AccountId32::new(local_provider),
		&candidate.duty_scale,
		local_key,
	)?;
	Ok(())
}

fn decode_candidate(
	inventory: &CheckpointDutyInventory,
	duty: &CheckpointDuty,
	local_provider: [u8; 32],
	local_key: [u8; 32],
) -> Result<Candidate, ContentError> {
	if !duty.may_initiate
		|| duty.role != CheckpointDutyRole::Replica
		|| duty.mode != crate::CheckpointDutyMode::Standard
		|| duty.phase != CheckpointDutyPhase::ReplicaFallbackPromotion
	{
		return Err(ContentError::IntegrityFailed);
	}
	let duty_scale = canonical_hex(&duty.encoded_duty)?;
	let mut input = &duty_scale[..];
	let decoded = RuntimeDuty::decode(&mut input).map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || decoded.encode() != duty_scale {
		return Err(ContentError::IntegrityFailed);
	}
	let projected = validate_checkpoint_duty(
		decoded.clone(),
		&AccountId32::new(local_provider),
		local_key,
		inventory.snapshot_checkpoint,
	)
	.map_err(|_| ContentError::IntegrityFailed)?;
	if projected != *duty
		|| decoded.mode != RuntimeMode::Standard
		|| decoded.phase != RuntimePhase::ReplicaFallbackPromotion
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(Candidate { public: duty.clone(), decoded, duty_scale })
}

fn validate_topologies(
	inventory: &CheckpointDutyInventory,
	duty: &RuntimeDuty,
	pinned: &ReplicationTopologySnapshot,
	current: &ReplicationTopologySnapshot,
	local_provider: [u8; 32],
	local_key: [u8; 32],
) -> Result<(), ContentError> {
	let expected_hash = hash_bytes(&inventory.finalized_hash)?;
	let primary = account_bytes(&duty.primary)?;
	let replicas = duty.replicas.iter().map(account_bytes).collect::<Result<Vec<_>, _>>()?;
	if pinned.finalized_hash != expected_hash
		|| pinned.finalized_number != inventory.finalized_number
		|| pinned.governed_finalized_checkpoint != Some(inventory.snapshot_checkpoint)
		|| pinned.genesis_hash != duty.commons_genesis_hash.0
		|| pinned.bucket_id != duty.bucket_id.0
		|| pinned.primary != primary
		|| pinned.replicas != replicas
		|| current.genesis_hash != pinned.genesis_hash
		|| current.bucket_id != pinned.bucket_id
		|| current.governed_finalized_checkpoint != pinned.governed_finalized_checkpoint
		|| current.bucket_version != pinned.bucket_version
		|| current.primary != pinned.primary
		|| current.replicas != pinned.replicas
	{
		return Err(ContentError::IntegrityFailed);
	}
	if pinned.providers.len() != duty.authorities.len()
		|| current.providers.len() != pinned.providers.len()
	{
		return Err(ContentError::IntegrityFailed);
	}
	for authority in &duty.authorities {
		let provider = account_bytes(&authority.provider)?;
		let pinned_provider = topology_provider(pinned, provider)?;
		let current_provider = topology_provider(current, provider)?;
		validate_provider(authority, pinned_provider)?;
		if current_provider != pinned_provider {
			return Err(ContentError::IntegrityFailed);
		}
	}
	let local = topology_provider(pinned, local_provider)?;
	if local.active_service_key != Some(local_key) || !local.usable {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn validate_provider(
	authority: &ProviderDutyAuthority<AccountId32, H256, u32>,
	provider: &ReplicationProviderSnapshot,
) -> Result<(), ContentError> {
	if provider.provider != account_bytes(&authority.provider)?
		|| provider.order != authority.order
		|| provider.primary != (authority.role == ProviderDutyRole::Primary)
		|| provider.active_service_key != Some(authority.active_service_key)
		|| provider.active_service_key_version != Some(authority.active_service_key_version)
		|| provider.endpoint_hash != Some(authority.endpoint_hash.0)
		|| provider.organization_valid != authority.organization_sla_eligible
		|| (provider.overdue_challenges > 0) != authority.overdue_challenge
		|| provider.eligible != authority.eligible
		|| provider.confirmed_checkpoint != authority.confirmed_checkpoint
		|| provider.usable != selection_eligible(authority)
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn selection_eligible(authority: &ProviderDutyAuthority<AccountId32, H256, u32>) -> bool {
	authority.eligible
		&& authority.organization_sla_eligible
		&& !authority.overdue_challenge
		&& authority.exclusion.is_none()
		&& authority.initiation_exclusion.is_none()
		&& authority.active_service_key_version > 0
		&& authority.active_service_key != [0; 32]
}

fn topology_provider(
	topology: &ReplicationTopologySnapshot,
	provider: [u8; 32],
) -> Result<&ReplicationProviderSnapshot, ContentError> {
	topology
		.providers
		.iter()
		.find(|candidate| candidate.provider == provider)
		.ok_or(ContentError::IntegrityFailed)
}

fn account_bytes(account: &AccountId32) -> Result<[u8; 32], ContentError> {
	account.as_ref().try_into().map_err(|_| ContentError::IntegrityFailed)
}

fn canonical_hex(value: &str) -> Result<Vec<u8>, ContentError> {
	let value = value.strip_prefix("0x").unwrap_or(value);
	if value.bytes().any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase()) {
		return Err(ContentError::IntegrityFailed);
	}
	hex::decode(value).map_err(|_| ContentError::IntegrityFailed)
}

fn hash_bytes(value: &str) -> Result<[u8; 32], ContentError> {
	canonical_hex(value)?.try_into().map_err(|_| ContentError::IntegrityFailed)
}

fn canonical_hash(value: &str) -> Result<String, ContentError> {
	Ok(hex::encode(hash_bytes(value)?))
}

fn cursor_hash(cursor: &DiscoveryCursorV1) -> Result<String, ContentError> {
	let mut canonical = cursor.clone();
	canonical.record_hash.clear();
	let mut input = DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn validate_cursor(cursor: &DiscoveryCursorV1) -> Result<(), ContentError> {
	if cursor.version != VERSION
		|| cursor.finalized_hash.len() != 64
		|| cursor.record_hash != cursor_hash(cursor)?
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn persist_cursor(root: &Path, cursor: &DiscoveryCursorV1) -> Result<(), ContentError> {
	validate_cursor(cursor)?;
	let bytes = serde_json::to_vec(cursor).map_err(io_error)?;
	if bytes.len() > MAX_RECORD_BYTES {
		return Err(ContentError::IntegrityFailed);
	}
	let temp = root.join("cursor.json.tmp");
	let mut file = File::create(&temp).map_err(io_error)?;
	file.write_all(&bytes).map_err(io_error)?;
	file.sync_all().map_err(io_error)?;
	fs::rename(temp, root.join("cursor.json")).map_err(io_error)?;
	File::open(root).and_then(|directory| directory.sync_all()).map_err(io_error)
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}
