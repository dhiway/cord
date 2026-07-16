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
	CheckpointDutyInfo, CheckpointDutyMode as RuntimeMode, CheckpointDutyPhase as RuntimePhase,
	ProviderDutyAuthority, ProviderDutyRole,
};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
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
			let cursor =
				serde_json::from_slice(&bytes).map_err(|_| ContentError::IntegrityFailed)?;
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
			cursor.finalized_hash == finalized_hash &&
				cursor.finalized_number == inventory.finalized_number &&
				cursor.snapshot_checkpoint == inventory.snapshot_checkpoint
		});
		let start = if same_inventory {
			let cursor = guard.as_ref().ok_or(ContentError::IntegrityFailed)?;
			let index =
				usize::try_from(cursor.last_index).map_err(|_| ContentError::IntegrityFailed)?;
			let duty = inventory.duties.get(index).ok_or(ContentError::IntegrityFailed)?;
			if duty.duty_id != cursor.last_duty_id ||
				duty.duty_fingerprint != cursor.last_duty_fingerprint ||
				duty.bucket_id != cursor.last_bucket_id
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
	if let Some(inventory) =
		store.checkpoint_duty_inventory().map_err(|_| ContentError::IntegrityFailed)?
	{
		let candidates = scheduler.reserve(&inventory, local_provider, local_key.public().0)?;
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
	if !duty.may_initiate ||
		duty.role != CheckpointDutyRole::Replica ||
		duty.mode != crate::CheckpointDutyMode::Standard ||
		duty.phase != CheckpointDutyPhase::ReplicaFallbackPromotion
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
	if projected != *duty ||
		decoded.mode != RuntimeMode::Standard ||
		decoded.phase != RuntimePhase::ReplicaFallbackPromotion
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
	pinned
		.validate(local_provider, local_key)
		.map_err(|_| ContentError::IntegrityFailed)?;
	current
		.validate(local_provider, local_key)
		.map_err(|_| ContentError::IntegrityFailed)?;
	if pinned.finalized_hash != expected_hash ||
		pinned.finalized_number != inventory.finalized_number ||
		pinned.governed_finalized_checkpoint != Some(inventory.snapshot_checkpoint) ||
		pinned.genesis_hash != duty.commons_genesis_hash.0 ||
		pinned.bucket_id != duty.bucket_id.0 ||
		pinned.primary != primary ||
		pinned.replicas != replicas ||
		current.genesis_hash != pinned.genesis_hash ||
		current.bucket_id != pinned.bucket_id ||
		current.governed_finalized_checkpoint != pinned.governed_finalized_checkpoint ||
		current.bucket_version != pinned.bucket_version ||
		current.primary != pinned.primary ||
		current.replicas != pinned.replicas ||
		current.current_checkpoint != pinned.current_checkpoint
	{
		return Err(ContentError::IntegrityFailed);
	}
	if pinned.providers.len() != duty.authorities.len() ||
		current.providers.len() != pinned.providers.len()
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
	if provider.provider != account_bytes(&authority.provider)? ||
		provider.order != authority.order ||
		provider.primary != (authority.role == ProviderDutyRole::Primary) ||
		provider.active_service_key != Some(authority.active_service_key) ||
		provider.active_service_key_version != Some(authority.active_service_key_version) ||
		provider.endpoint_hash != Some(authority.endpoint_hash.0) ||
		provider.organization_valid != authority.organization_sla_eligible ||
		(provider.overdue_challenges > 0) != authority.overdue_challenge ||
		provider.eligible != authority.eligible ||
		provider.confirmed_checkpoint != authority.confirmed_checkpoint ||
		provider.usable != selection_eligible(authority)
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn selection_eligible(authority: &ProviderDutyAuthority<AccountId32, H256, u32>) -> bool {
	authority.eligible &&
		authority.organization_sla_eligible &&
		!authority.overdue_challenge &&
		authority.exclusion.is_none() &&
		authority.initiation_exclusion.is_none() &&
		authority.active_service_key_version > 0 &&
		authority.active_service_key != [0; 32]
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
	let bytes: &[u8] = account.as_ref();
	bytes.try_into().map_err(|_| ContentError::IntegrityFailed)
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
	if cursor.version != VERSION ||
		cursor.finalized_hash.len() != 64 ||
		cursor.record_hash != cursor_hash(cursor)?
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

#[cfg(test)]
mod tests {
	use std::sync::{
		atomic::{AtomicUsize, Ordering},
		Arc,
	};

	use async_trait::async_trait;
	use frame_metadata::v15::{
		CustomMetadata, ExtrinsicMetadata, OuterEnums, PalletCallMetadata, PalletMetadata,
		RuntimeMetadataV15,
	};
	use orbis_storage_runtime_api::{
		CheckpointInfo, CommitmentInfo, ProviderDutyExclusion, RESPONSE_VERSION,
	};
	use pallet_orbis_storage_provider::CheckpointFallbackPromotionV1;
	use scale_info::{meta_type, TypeInfo};
	use sp_core::Pair as _;
	use tempfile::TempDir;

	use super::*;
	use crate::{
		chain::{ChainError, ReplicationProviderExclusion},
		checkpoint::{
			checkpoint_promotion::{
				promotion_finality_attestation_digest, CheckpointPromotionStoreV2,
			},
			checkpoint_submitter::FinalizedEvidence,
		},
		CheckpointDutyBatch, NodeProfile,
	};

	const FINALITY_ATTESTATION_VERSION: u8 = 1;
	const FINALIZED_STATE: &str = "finalized";

	#[allow(non_camel_case_types, dead_code)]
	#[derive(TypeInfo)]
	enum PromotionCall {
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
				calls: Some(PalletCallMetadata { ty: meta_type::<PromotionCall>() }),
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
		ed25519::Pair::from_seed(&[seed.saturating_add(10); 32])
	}

	fn account(seed: u8) -> AccountId32 {
		AccountId32::new([seed; 32])
	}

	fn invalid_duty(seed: u8) -> CheckpointDuty {
		CheckpointDuty {
			duty_id: format!("0x{}", hex::encode([seed; 32])),
			bucket_id: format!("0x{}", hex::encode([seed; 32])),
			provider: format!("0x{}", hex::encode([2; 32])),
			role: CheckpointDutyRole::Replica,
			service_key_version: 1,
			service_key: format!("0x{}", hex::encode(pair(2).public().0)),
			snapshot_checkpoint: 120,
			snapshot_hash: format!("0x{}", hex::encode([15; 32])),
			due_at: 100,
			grace_until: 110,
			phase: CheckpointDutyPhase::ReplicaFallbackPromotion,
			mode: crate::CheckpointDutyMode::Standard,
			may_sign: true,
			may_initiate: true,
			encoded_duty: "0x00".into(),
			duty_fingerprint: format!("0x{}", hex::encode([seed.wrapping_add(1); 32])),
		}
	}

	fn authority(
		seed: u8,
		role: ProviderDutyRole,
		order: u8,
		eligible: bool,
		may_initiate: bool,
	) -> ProviderDutyAuthority<AccountId32, H256, u32> {
		ProviderDutyAuthority {
			provider: account(seed),
			role,
			order,
			active_service_key_version: 5,
			active_service_key: pair(seed).public().0,
			endpoint_hash: H256::from(blake2_256(&endpoint(seed))),
			organization_sla_eligible: true,
			overdue_challenge: false,
			eligible,
			may_sign: eligible,
			may_initiate,
			exclusion: (!eligible).then_some(ProviderDutyExclusion::Inactive),
			initiation_exclusion: None,
			confirmed_checkpoint: Some(100),
		}
	}

	fn runtime_duty() -> RuntimeDuty {
		RuntimeDuty {
			response_version: RESPONSE_VERSION,
			commons_genesis_hash: H256::repeat_byte(10),
			commons_spec_version: 11,
			commons_transaction_version: 12,
			commons_metadata_hash: H256::repeat_byte(13),
			duty_id: H256::repeat_byte(14),
			bucket_id: H256::repeat_byte(4),
			primary: account(1),
			replicas: vec![account(2), account(3)],
			authorities: vec![
				authority(1, ProviderDutyRole::Primary, 0, false, false),
				authority(2, ProviderDutyRole::Replica, 1, true, true),
				authority(3, ProviderDutyRole::Replica, 2, true, false),
			],
			initiator: Some(account(2)),
			phase: RuntimePhase::ReplicaFallbackPromotion,
			mode: RuntimeMode::Standard,
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

	fn inventory_with(duties: Vec<CheckpointDuty>) -> CheckpointDutyInventory {
		CheckpointDutyInventory {
			finalized_hash: hex::encode([5; 32]),
			finalized_number: 130,
			snapshot_checkpoint: 120,
			duties,
		}
	}

	fn endpoint(seed: u8) -> Vec<u8> {
		format!("https://provider-{seed}.invalid/storage").into_bytes()
	}

	fn provider(seed: u8, order: u8, primary: bool, eligible: bool) -> ReplicationProviderSnapshot {
		let endpoint = endpoint(seed);
		ReplicationProviderSnapshot {
			provider: [seed; 32],
			order,
			primary,
			record_present: true,
			endpoint_hash: Some(blake2_256(&endpoint)),
			endpoint: Some(endpoint),
			active_service_key: Some(pair(seed).public().0),
			active_service_key_version: Some(5),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(120),
			overdue_challenges: 0,
			eligible,
			usable: eligible,
			exclusions: (!eligible)
				.then_some(ReplicationProviderExclusion::RuntimeIneligible)
				.into_iter()
				.collect(),
			confirmed_checkpoint: Some(100),
		}
	}

	fn seal(mut topology: ReplicationTopologySnapshot) -> ReplicationTopologySnapshot {
		topology.snapshot_hash = [0; 32];
		let mut input = b"cord/provider/replication-topology/v1".to_vec();
		topology.encode_to(&mut input);
		topology.snapshot_hash = blake2_256(&input);
		topology
	}

	fn topology() -> ReplicationTopologySnapshot {
		seal(ReplicationTopologySnapshot {
			genesis_hash: [10; 32],
			finalized_hash: [5; 32],
			finalized_number: 130,
			governed_finalized_checkpoint: Some(120),
			bucket_id: [4; 32],
			bucket_version: 7,
			primary: [1; 32],
			replicas: vec![[2; 32], [3; 32]],
			providers: vec![
				provider(1, 0, true, false),
				provider(2, 1, false, true),
				provider(3, 2, false, true),
			],
			current_checkpoint: Some(CheckpointInfo {
				bucket_id: H256::repeat_byte(4),
				commitment: CommitmentInfo {
					mmr_root: H256::repeat_byte(5),
					start_seq: 0,
					leaf_count: 5,
				},
				checkpoint_block: 100,
				primary_signers: 1,
				commitment_nonce: 100,
				replica_confirmations: vec![account(2), account(3)],
			}),
			snapshot_hash: [0; 32],
		})
	}

	struct TopologyAuthority {
		pinned: ReplicationTopologySnapshot,
		current: ReplicationTopologySnapshot,
		stack: Arc<CheckpointStack>,
		guard_available: Arc<std::sync::atomic::AtomicBool>,
	}

	#[async_trait]
	impl ReplicationAuthority for TopologyAuthority {
		async fn replication_topology(
			&self,
			_bucket_id: [u8; 32],
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			self.guard_available.store(self.stack.guard_is_available(), Ordering::SeqCst);
			Ok(self.current.clone())
		}

		async fn replication_topology_at(
			&self,
			_bucket_id: [u8; 32],
			_finalized_hash: [u8; 32],
			_finalized_number: u32,
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			self.guard_available.store(self.stack.guard_is_available(), Ordering::SeqCst);
			Ok(self.pinned.clone())
		}
	}

	struct FinalizingLane {
		metadata: subxt::Metadata,
		calls: Arc<AtomicUsize>,
	}

	#[async_trait]
	impl PromotionFinalityLane for FinalizingLane {
		fn metadata(&self) -> &subxt::Metadata {
			&self.metadata
		}

		fn signer_account(&self) -> [u8; 32] {
			[2; 32]
		}

		fn service_key(&self) -> [u8; 32] {
			pair(2).public().0
		}

		async fn submit_and_finalize(
			&self,
			_intent_id: &str,
			intent: &crate::checkpoint::checkpoint_promotion::FallbackPromotionIntentV2,
			_payload: subxt::tx::DynamicPayload,
		) -> Result<FinalizedEvidence, ContentError> {
			self.calls.fetch_add(1, Ordering::SeqCst);
			let block_hash = [21; 32];
			let block_number = 131;
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
				finality_signature: pair(2).sign(&digest).0,
			})
		}
	}

	#[test]
	fn more_than_sixty_four_failures_advance_durably_across_restart() {
		let temp = TempDir::new().unwrap();
		let inventory = inventory_with((0..130).map(invalid_duty).collect());
		let scheduler = PromotionDiscoveryScheduler::open(temp.path()).unwrap();
		assert!(scheduler.reserve(&inventory, [2; 32], pair(2).public().0).unwrap().is_empty());
		assert_eq!(scheduler.cursor.lock().unwrap().as_ref().unwrap().last_index, 63);
		drop(scheduler);

		let reopened = PromotionDiscoveryScheduler::open(temp.path()).unwrap();
		assert!(reopened.reserve(&inventory, [2; 32], pair(2).public().0).unwrap().is_empty());
		assert_eq!(reopened.cursor.lock().unwrap().as_ref().unwrap().last_index, 127);
	}

	#[test]
	fn reopen_rejects_tampered_cursor_and_unknown_scheduler_entries() {
		for unknown_entry in [false, true] {
			let temp = TempDir::new().unwrap();
			let inventory = inventory_with(vec![invalid_duty(1)]);
			let scheduler = PromotionDiscoveryScheduler::open(temp.path()).unwrap();
			scheduler.reserve(&inventory, [2; 32], pair(2).public().0).unwrap();
			drop(scheduler);
			let root = temp.path().join(ROOT);
			if unknown_entry {
				fs::write(root.join("unexpected"), b"unexpected").unwrap();
			} else {
				let path = root.join("cursor.json");
				let mut record: serde_json::Value =
					serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
				record["last_index"] = serde_json::json!(9);
				fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
			}
			assert!(matches!(
				PromotionDiscoveryScheduler::open(temp.path()),
				Err(ContentError::IntegrityFailed)
			));
		}
	}

	#[test]
	fn deterministic_tie_admits_only_the_lowest_replica() {
		let temp = TempDir::new().unwrap();
		let duty = runtime_duty();
		let store = CheckpointPromotionStoreV2::open(temp.path()).unwrap();
		assert!(store.authorize(&account(2), &duty.encode(), &pair(2)).is_ok());

		let mut nonselected = duty;
		nonselected.initiator = Some(account(3));
		nonselected.authorities[1].may_initiate = false;
		nonselected.authorities[2].may_initiate = true;
		assert!(matches!(
			store.authorize(&account(3), &nonselected.encode(), &pair(3)),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[test]
	fn exact_topology_accepts_then_rejects_current_provider_and_bucket_drift() {
		let duty = runtime_duty();
		let inventory = inventory_with(vec![validate_checkpoint_duty(
			duty.clone(),
			&account(2),
			pair(2).public().0,
			120,
		)
		.unwrap()]);
		let pinned = topology();
		let mut current = pinned.clone();
		current.finalized_hash = [6; 32];
		current.finalized_number = 131;
		current = seal(current);
		validate_topologies(&inventory, &duty, &pinned, &current, [2; 32], pair(2).public().0)
			.unwrap();

		let mut version_drift = current.clone();
		version_drift.bucket_version += 1;
		version_drift = seal(version_drift);
		assert!(matches!(
			validate_topologies(
				&inventory,
				&duty,
				&pinned,
				&version_drift,
				[2; 32],
				pair(2).public().0,
			),
			Err(ContentError::IntegrityFailed)
		));

		let mut provider_drift = current;
		provider_drift.providers[2].active_service_key = Some(pair(9).public().0);
		provider_drift = seal(provider_drift);
		assert!(matches!(
			validate_topologies(
				&inventory,
				&duty,
				&pinned,
				&provider_drift,
				[2; 32],
				pair(2).public().0,
			),
			Err(ContentError::IntegrityFailed)
		));
	}

	#[tokio::test]
	async fn discovery_then_next_tick_finality_is_exact_once_across_reopen() {
		let temp = TempDir::new().unwrap();
		let local_key = pair(2);
		let duty = runtime_duty();
		let public =
			validate_checkpoint_duty(duty, &account(2), local_key.public().0, 120).unwrap();
		let profile = NodeProfile {
			provider: hex::encode([2; 32]),
			endpoint: "https://provider-2.invalid/storage".into(),
			service_key: hex::encode(local_key.public().0),
			region: None,
		};
		let store = DiskStore::open(temp.path(), profile.clone(), 1024).unwrap();
		store
			.stage_checkpoint_duty_page(CheckpointDutyBatch {
				finalized_hash: hex::encode([5; 32]),
				finalized_number: 130,
				provider: profile.provider,
				snapshot_checkpoint: 120,
				requested_cursor: None,
				next_cursor: None,
				duties: vec![public],
			})
			.unwrap();
		let stack = Arc::new(CheckpointStack::open(temp.path()).unwrap());
		let scheduler = PromotionDiscoveryScheduler::open(temp.path()).unwrap();
		let pinned = topology();
		let mut current = pinned.clone();
		current.finalized_hash = [6; 32];
		current.finalized_number = 131;
		let guard_available = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let authority = TopologyAuthority {
			pinned,
			current: seal(current),
			stack: Arc::clone(&stack),
			guard_available: Arc::clone(&guard_available),
		};
		let calls = Arc::new(AtomicUsize::new(0));
		let lane = FinalizingLane { metadata: metadata(), calls: Arc::clone(&calls) };

		assert!(tick(
			&authority,
			&stack,
			&store,
			&lane,
			&scheduler,
			[2; 32],
			&local_key,
			Duration::from_secs(1),
			Duration::from_secs(1),
		)
		.await
		.unwrap()
		.is_none());
		assert_eq!(calls.load(Ordering::SeqCst), 0);
		assert!(guard_available.load(Ordering::SeqCst));
		assert!(stack.pending_checkpoint_publications(8).unwrap().is_empty());
		assert_eq!(fs::read_dir(temp.path().join("checkpoint-submissions-v2")).unwrap().count(), 0);
		assert!(tick(
			&authority,
			&stack,
			&store,
			&lane,
			&scheduler,
			[2; 32],
			&local_key,
			Duration::from_secs(1),
			Duration::from_secs(1),
		)
		.await
		.unwrap()
		.is_some());
		assert_eq!(calls.load(Ordering::SeqCst), 1);
		assert!(stack.pending_checkpoint_publications(8).unwrap().is_empty());
		assert_eq!(fs::read_dir(temp.path().join("checkpoint-submissions-v2")).unwrap().count(), 0);
		drop(authority);
		drop(stack);
		drop(scheduler);

		let reopened = Arc::new(CheckpointStack::open(temp.path()).unwrap());
		let reopened_scheduler = PromotionDiscoveryScheduler::open(temp.path()).unwrap();
		let pinned = topology();
		let mut current = pinned.clone();
		current.finalized_hash = [6; 32];
		current.finalized_number = 131;
		let authority = TopologyAuthority {
			pinned,
			current: seal(current),
			stack: Arc::clone(&reopened),
			guard_available,
		};
		assert!(tick(
			&authority,
			&reopened,
			&store,
			&lane,
			&reopened_scheduler,
			[2; 32],
			&local_key,
			Duration::from_secs(1),
			Duration::from_secs(1),
		)
		.await
		.unwrap()
		.is_none());
		assert_eq!(calls.load(Ordering::SeqCst), 1);
		assert!(reopened.pending_checkpoint_publications(8).unwrap().is_empty());
	}
}
