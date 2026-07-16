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

//! Finalized-state runtime API views for the native Commons storage control plane.
//!
//! Clients MUST invoke these APIs at a finalized block hash. Responses contain only bounded
//! control-plane records and commitments: never object bytes, private data, contract addresses,
//! legacy reservation references or a second storage source of truth.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Codec, Decode, Encode};
use scale_decode::DecodeAsType;
use scale_info::TypeInfo;

pub const RESPONSE_VERSION: u16 = 9;
pub const MAX_PAGE_SIZE: u32 = 100;
pub const MAX_CHECKPOINT_DUTY_PAGE_SIZE: u32 = 128;
/// Maximum manifest-deletion duties returned by one finalized runtime-API page.
pub const MAX_DELETION_DUTY_PAGE_SIZE: u32 = 128;
/// Maximum concurrently indexed agreements for one control-plane bucket.
pub const MAX_BUCKET_AGREEMENTS: u32 = 32;

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum ProviderStatus {
	Active,
	Suspended,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum AgreementStatus {
	Proposed,
	Active,
	Suspended,
	Cancelled,
	Expired,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum ChallengeStatus {
	Open,
	Proved,
	TimedOut,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum ManifestState {
	Publishable,
	Pending,
	Tombstoned,
	Missing,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum BucketRole {
	Reader,
	Writer,
	Admin,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum ContainerStatus {
	Active,
	Archived,
	Deleted,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct Versioned<T> {
	pub version: u16,
	pub value: Option<T>,
}

impl<T> Versioned<T> {
	pub const fn new(value: Option<T>) -> Self {
		Self { version: RESPONSE_VERSION, value }
	}
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct Page<T> {
	pub version: u16,
	pub items: Vec<T>,
	pub next_cursor: Option<u32>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct SnapshotCursor {
	pub snapshot_version: u64,
	pub last_key: Vec<u8>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct SnapshotPage<T> {
	pub version: u16,
	pub items: Vec<T>,
	pub next_cursor: Option<SnapshotCursor>,
	pub snapshot_version: u64,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct CheckpointDutyCursor<BlockNumber> {
	pub snapshot_checkpoint: BlockNumber,
	pub last_key: Vec<u8>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct CheckpointDutyPage<T, BlockNumber> {
	pub version: u16,
	pub items: Vec<T>,
	pub next_cursor: Option<CheckpointDutyCursor<BlockNumber>>,
	pub snapshot_checkpoint: BlockNumber,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum CheckpointDutyPageError {
	FinalizedCheckpointUnavailable,
	PageLimitInvalid,
	CursorSnapshotStale,
	CursorKeyInvalid,
}

/// Cursor for one deletion-duty scan fixed to a governed finalized checkpoint.
///
/// `last_manifest` is the exact last key returned by the preceding page. A cursor is valid only
/// while that key remains in the provider's unacknowledged duty set at `snapshot_checkpoint`.
#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct DeletionDutyCursor<BlockNumber> {
	pub snapshot_checkpoint: BlockNumber,
	pub last_manifest: [u8; 32],
}

/// One canonical manifest deletion addressed to one exact provider.
#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct DeletionDutyInfo<AccountId, Hash, BlockNumber> {
	pub provider: AccountId,
	pub manifest: [u8; 32],
	pub bucket_id: Hash,
	pub provider_commitment: [u8; 32],
	pub tombstoned_at: BlockNumber,
}

/// Bounded deletion-duty page from one fixed governed finalized snapshot.
#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct DeletionDutyPage<T, BlockNumber> {
	pub version: u16,
	pub items: Vec<T>,
	pub next_cursor: Option<DeletionDutyCursor<BlockNumber>>,
	pub snapshot_checkpoint: BlockNumber,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum DeletionDutyPageError {
	FinalizedCheckpointUnavailable,
	PageLimitInvalid,
	CursorSnapshotStale,
	CursorKeyInvalid,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum S3ListError {
	BucketNotFound,
	BucketDeleted,
	CursorStale,
	PageLimitInvalid,
	CursorKeyInvalid,
}

impl<T> Page<T> {
	pub const fn new(items: Vec<T>, next_cursor: Option<u32>) -> Self {
		Self { version: RESPONSE_VERSION, items, next_cursor }
	}
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct OrganizationInfo<Hash, BlockNumber> {
	pub entity_id: Vec<u8>,
	pub attestation_id: Hash,
	pub schema_id: Hash,
	pub sla_commitment: Hash,
	pub sla_version: u16,
	pub valid_from: BlockNumber,
	pub valid_until: BlockNumber,
	pub rotation_predecessor: Option<Hash>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ServiceKeyInfo<BlockNumber> {
	pub active: [u8; 32],
	pub active_version: u64,
	pub previous: Option<[u8; 32]>,
	pub pending: Option<[u8; 32]>,
	pub pending_version: Option<u64>,
	pub pending_effective_at: Option<BlockNumber>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ProviderInfo<Hash, BlockNumber> {
	pub endpoint: Vec<u8>,
	pub organization: OrganizationInfo<Hash, BlockNumber>,
	pub service_key: ServiceKeyInfo<BlockNumber>,
	pub capacity_bytes: u64,
	pub allocated_bytes: u64,
	pub pending_bytes: u64,
	pub status: ProviderStatus,
	pub last_heartbeat: BlockNumber,
	pub overdue_challenges: u32,
	pub authority_validated_at: Option<BlockNumber>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct BucketGrantInfo<AccountId> {
	/// Bucket ACL account. This is never a provider capability authority.
	pub account: AccountId,
	/// Coarse ACL role with no fallback into host-delegation verification.
	pub role: BucketRole,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct HostDelegationInfo<AccountId, Hash, BlockNumber> {
	pub grant_id: Hash,
	pub bucket_id: Hash,
	pub owner: AccountId,
	pub issuance_nonce: u64,
	pub issuer_key_id: Hash,
	pub issuer_public_key: [u8; 32],
	pub key_version: u64,
	pub state_version: u64,
	pub key_activated_at: BlockNumber,
	pub product_id: Vec<u8>,
	pub methods: Vec<u16>,
	pub cid: Option<Vec<u8>>,
	pub max_bytes: u64,
	pub issued_at: BlockNumber,
	pub expires_at: BlockNumber,
	pub revoked_at: Option<BlockNumber>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ControlBucketInfo<AccountId, Hash, BlockNumber> {
	pub bucket_id: Hash,
	pub owner: AccountId,
	pub version: u64,
	pub policy: Hash,
	pub primary: AccountId,
	pub replicas: Vec<AccountId>,
	pub grants: Vec<BucketGrantInfo<AccountId>>,
	pub created_at: BlockNumber,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct AgreementInfo<AccountId, Hash, BlockNumber> {
	pub agreement_id: Hash,
	pub owner: AccountId,
	pub bucket_id: Hash,
	pub primary: AccountId,
	pub replicas: Vec<AccountId>,
	pub bytes: u64,
	pub created_at: BlockNumber,
	pub expires_at: BlockNumber,
	pub release_at: Option<BlockNumber>,
	pub state_version: u64,
	pub status: AgreementStatus,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct CommitmentInfo<Hash> {
	pub mmr_root: Hash,
	pub start_seq: u64,
	pub leaf_count: u64,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ChunkLocationInfo {
	pub leaf_index: u64,
	pub chunk_index: u32,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct CheckpointInfo<AccountId, Hash, BlockNumber> {
	pub bucket_id: Hash,
	pub commitment: CommitmentInfo<Hash>,
	pub checkpoint_block: BlockNumber,
	pub primary_signers: u8,
	pub commitment_nonce: BlockNumber,
	pub replica_confirmations: Vec<AccountId>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct CheckpointDutyInfo<AccountId, Hash, BlockNumber> {
	pub response_version: u16,
	pub commons_genesis_hash: Hash,
	pub commons_spec_version: u32,
	pub commons_transaction_version: u32,
	pub commons_metadata_hash: Hash,
	pub duty_id: Hash,
	pub bucket_id: Hash,
	pub primary: AccountId,
	pub replicas: Vec<AccountId>,
	pub authorities: Vec<ProviderDutyAuthority<AccountId, Hash, BlockNumber>>,
	pub initiator: Option<AccountId>,
	pub phase: CheckpointDutyPhase,
	pub mode: CheckpointDutyMode,
	pub snapshot_checkpoint: BlockNumber,
	pub snapshot_hash: Hash,
	pub due_at: BlockNumber,
	pub grace_until: BlockNumber,
	pub expected_nonce: BlockNumber,
	pub scheduled_at: BlockNumber,
	pub previous_commitment: Option<CommitmentInfo<Hash>>,
	pub previous_checkpoint: Option<BlockNumber>,
	pub expected_next_start_seq: u64,
	pub required_primary_confirmations: u8,
	pub required_replica_confirmations: u8,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum ProviderDutyRole {
	Primary,
	Replica,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum ProviderDutyExclusion {
	MissingProvider,
	Inactive,
	OrganizationUnknown,
	AttestationInvalid,
	AttestationExpired,
	SlaInvalid,
	ServiceKeyInvalid,
	OverdueChallenge,
	ReplicaCheckpointMissingOrStale,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum CheckpointDutyPhase {
	NotDue,
	Primary,
	ReplicaFallback,
	ReplicaFallbackPromotion,
	BlockedInsufficientFallbackQuorum,
	Unavailable,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum CheckpointDutyMode {
	Standard,
	PromotionPending,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ProviderDutyAuthority<AccountId, Hash, BlockNumber> {
	pub provider: AccountId,
	pub role: ProviderDutyRole,
	pub order: u8,
	pub active_service_key_version: u64,
	pub active_service_key: [u8; 32],
	pub endpoint_hash: Hash,
	pub organization_sla_eligible: bool,
	pub overdue_challenge: bool,
	pub eligible: bool,
	pub may_sign: bool,
	pub may_initiate: bool,
	pub exclusion: Option<ProviderDutyExclusion>,
	pub initiation_exclusion: Option<ProviderDutyExclusion>,
	pub confirmed_checkpoint: Option<BlockNumber>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ChallengeInfo<AccountId, Hash, BlockNumber> {
	pub challenge_id: Hash,
	pub bucket_id: Hash,
	pub provider: AccountId,
	pub expected_commitment: CommitmentInfo<Hash>,
	pub location: ChunkLocationInfo,
	pub due_at: BlockNumber,
	pub status: ChallengeStatus,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct EvidenceInfo<AccountId, Hash, BlockNumber> {
	pub provider: AccountId,
	pub bucket_id: Hash,
	pub evidence_hash: Hash,
	pub recorded_at: BlockNumber,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ManifestInfo<AccountId, Hash, BlockNumber> {
	pub manifest: [u8; 32],
	pub bucket_id: Hash,
	pub provider_commitment: Option<[u8; 32]>,
	pub state: ManifestState,
	pub checkpoint: Option<BlockNumber>,
	pub tombstoned_at: Option<BlockNumber>,
	pub deletion_required: Vec<AccountId>,
	pub deletion_acknowledged: Vec<AccountId>,
	pub deletion_evidence_satisfied: bool,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct DriveInfo<AccountId, Hash, BlockNumber> {
	pub drive_id: Hash,
	pub owner: AccountId,
	pub name: Vec<u8>,
	pub root_manifest: Option<[u8; 32]>,
	pub root_provider_commitment: Option<[u8; 32]>,
	pub version: u64,
	pub status: ContainerStatus,
	pub created_at: BlockNumber,
	pub updated_at: BlockNumber,
	pub controllers: Vec<AccountId>,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct BucketInfo<AccountId, Hash, BlockNumber> {
	pub bucket_id: Hash,
	pub name: Vec<u8>,
	pub owner: AccountId,
	pub controllers: Vec<AccountId>,
	pub status: ContainerStatus,
	pub versioning_enabled: bool,
	pub version: u64,
	pub live_objects: u32,
	pub created_at: BlockNumber,
	pub updated_at: BlockNumber,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ObjectInfo<AccountId, Hash, BlockNumber> {
	pub object_id: Hash,
	pub bucket_id: Hash,
	pub key: Vec<u8>,
	pub content_hash: Option<[u8; 32]>,
	pub provider_commitment: Option<[u8; 32]>,
	pub version: u64,
	pub deleted: bool,
	pub updated_by: AccountId,
	pub updated_at: BlockNumber,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ObjectVersionInfo<AccountId, BlockNumber> {
	pub content_hash: Option<[u8; 32]>,
	pub provider_commitment: Option<[u8; 32]>,
	pub version: u64,
	pub deleted: bool,
	pub updated_by: AccountId,
	pub updated_at: BlockNumber,
}

sp_api::decl_runtime_apis! {
		#[api_version(11)]
	pub trait StorageProviderApi<AccountId, Hash, BlockNumber>
	where
		AccountId: Codec,
		Hash: Codec,
		BlockNumber: Codec,
	{
		fn provider(provider: AccountId) -> Versioned<ProviderInfo<Hash, BlockNumber>>;
		fn providers(cursor: Option<u32>, limit: u32) -> Page<(AccountId, ProviderInfo<Hash, BlockNumber>)>;
		fn control_bucket(bucket_id: Hash) -> Versioned<ControlBucketInfo<AccountId, Hash, BlockNumber>>;
			fn control_buckets(cursor: Option<u32>, limit: u32) -> Page<ControlBucketInfo<AccountId, Hash, BlockNumber>>;
			fn capability_authority(grant_id: Hash) -> Versioned<HostDelegationInfo<AccountId, Hash, BlockNumber>>;
		fn agreement(agreement_id: Hash) -> Versioned<AgreementInfo<AccountId, Hash, BlockNumber>>;
		fn provider_agreements(provider: AccountId, cursor: Option<u32>, limit: u32) -> Page<AgreementInfo<AccountId, Hash, BlockNumber>>;
		fn agreement_nonce(owner: AccountId) -> u64;
		fn challenge(challenge_id: Hash) -> Versioned<ChallengeInfo<AccountId, Hash, BlockNumber>>;
		fn challenges_at(block: BlockNumber, cursor: Option<u32>, limit: u32) -> Page<ChallengeInfo<AccountId, Hash, BlockNumber>>;
		fn checkpoint(bucket_id: Hash) -> Versioned<CheckpointInfo<AccountId, Hash, BlockNumber>>;
		fn checkpoint_duties(provider: AccountId, cursor: Option<CheckpointDutyCursor<BlockNumber>>, limit: u32) -> Result<CheckpointDutyPage<CheckpointDutyInfo<AccountId, Hash, BlockNumber>, BlockNumber>, CheckpointDutyPageError>;
		fn deletion_duties(provider: AccountId, cursor: Option<DeletionDutyCursor<BlockNumber>>, limit: u32) -> Result<DeletionDutyPage<DeletionDutyInfo<AccountId, Hash, BlockNumber>, BlockNumber>, DeletionDutyPageError>;
		fn replica_checkpoint(bucket_id: Hash, provider: AccountId) -> Option<BlockNumber>;
		fn canonical_manifest(manifest: [u8; 32]) -> Versioned<ManifestInfo<AccountId, Hash, BlockNumber>>;
		fn provider_evidence(provider: AccountId, cursor: Option<u32>, limit: u32) -> Page<EvidenceInfo<AccountId, Hash, BlockNumber>>;
		fn can_accept_capacity(provider: AccountId, additional_bytes: u64) -> bool;
		fn provider_is_eligible(provider: AccountId) -> bool;
		fn governed_finalized_checkpoint() -> Option<BlockNumber>;
	}

	#[api_version(2)]
	pub trait DriveRegistryApi<AccountId, Hash, BlockNumber>
	where
		AccountId: Codec,
		Hash: Codec,
		BlockNumber: Codec,
	{
		fn drive(drive_id: Hash) -> Versioned<DriveInfo<AccountId, Hash, BlockNumber>>;
		fn drives(owner: AccountId, cursor: Option<u32>, limit: u32) -> Page<DriveInfo<AccountId, Hash, BlockNumber>>;
		fn controllers(drive_id: Hash, cursor: Option<u32>, limit: u32) -> Page<AccountId>;
		fn next_drive_nonce(owner: AccountId) -> u64;
		fn is_drive_owner(owner: AccountId, drive_id: Hash) -> bool;
	}

	#[api_version(3)]
	pub trait S3RegistryApi<AccountId, Hash, BlockNumber>
	where
		AccountId: Codec,
		Hash: Codec,
		BlockNumber: Codec,
	{
		fn bucket(bucket_id: Hash) -> Versioned<BucketInfo<AccountId, Hash, BlockNumber>>;
		fn bucket_by_name(name: Vec<u8>) -> Versioned<BucketInfo<AccountId, Hash, BlockNumber>>;
		fn buckets(owner: AccountId, cursor: Option<u32>, limit: u32) -> Page<BucketInfo<AccountId, Hash, BlockNumber>>;
		fn is_bucket_owner(owner: AccountId, bucket_id: Hash) -> bool;
		fn object(bucket_id: Hash, key: Vec<u8>) -> Versioned<ObjectInfo<AccountId, Hash, BlockNumber>>;
		fn object_keys(bucket_id: Hash, prefix: Option<Vec<u8>>, cursor: Option<SnapshotCursor>, limit: u32) -> Result<SnapshotPage<Vec<u8>>, S3ListError>;
		fn object_history(bucket_id: Hash, key: Vec<u8>, cursor: Option<u32>, limit: u32) -> Page<ObjectVersionInfo<AccountId, BlockNumber>>;
		fn object_id(bucket_id: Hash, key: Vec<u8>) -> Versioned<Hash>;
	}
}
