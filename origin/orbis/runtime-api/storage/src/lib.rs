// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stable runtime API views for native Orbis storage metadata.
//!
//! Clients invoke these APIs at a finalized hash. Responses contain references and commitments,
//! never content bytes, private data, CIDs, contract addresses or compatibility fields.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Codec, Decode, Encode};
use scale_decode::DecodeAsType;
use scale_info::TypeInfo;

pub const RESPONSE_VERSION: u16 = 4;
pub const MAX_PAGE_SIZE: u32 = 100;

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum ProviderStatus {
	Active,
	Suspended,
}

#[derive(Clone, Copy, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub enum AgreementStatus {
	Proposed,
	Active,
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

impl<T> Page<T> {
	pub const fn new(items: Vec<T>, next_cursor: Option<u32>) -> Self {
		Self { version: RESPONSE_VERSION, items, next_cursor }
	}
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ProviderInfo<BlockNumber> {
	pub endpoint: Vec<u8>,
	pub service_key: Vec<u8>,
	pub capacity_bytes: u64,
	pub allocated_bytes: u64,
	pub pending_bytes: u64,
	pub status: ProviderStatus,
	pub last_heartbeat: BlockNumber,
	pub reputation: i32,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct AgreementInfo<AccountId, Hash, BlockNumber> {
	pub agreement_id: Hash,
	pub owner: AccountId,
	pub provider: AccountId,
	pub container_ref: Hash,
	pub content_commitment: Hash,
	pub reservation_ref: Option<u64>,
	pub bytes: u64,
	pub created_at: BlockNumber,
	pub expires_at: BlockNumber,
	pub pending_expiry: Option<BlockNumber>,
	pub status: AgreementStatus,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ChallengeInfo<AccountId, Hash, BlockNumber> {
	pub challenge_id: Hash,
	pub provider: AccountId,
	pub agreement_id: Hash,
	pub expected_commitment: Hash,
	pub due_at: BlockNumber,
	pub proof_commitment: Option<Hash>,
	pub status: ChallengeStatus,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct CheckpointInfo<Hash, BlockNumber> {
	pub challenge_id: Hash,
	pub proof_commitment: Hash,
	pub recorded_at: BlockNumber,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ProviderRootInfo<Hash, BlockNumber> {
	pub sequence: u64,
	pub root: Hash,
	pub leaf_count: u64,
	pub committed_at: BlockNumber,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct DeletionAcknowledgementInfo<AccountId, Hash, BlockNumber> {
	pub provider: AccountId,
	pub content_commitment: Hash,
	pub tombstone_root: Hash,
	pub root_sequence: u64,
	pub leaf_index: u64,
	pub leaf_count: u64,
	pub proof_commitment: Hash,
	pub acknowledged_at: BlockNumber,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct DriveInfo<AccountId, Hash, BlockNumber> {
	pub drive_id: Hash,
	pub owner: AccountId,
	pub name: Vec<u8>,
	pub root_storage_ref: Option<[u8; 32]>,
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
	pub version: u64,
	pub deleted: bool,
	pub updated_by: AccountId,
	pub updated_at: BlockNumber,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, PartialEq, TypeInfo)]
pub struct ObjectVersionInfo<AccountId, BlockNumber> {
	pub content_hash: Option<[u8; 32]>,
	pub version: u64,
	pub deleted: bool,
	pub updated_by: AccountId,
	pub updated_at: BlockNumber,
}

sp_api::decl_runtime_apis! {
	#[api_version(4)]
	pub trait StorageProviderApi<AccountId, Hash, BlockNumber>
	where
		AccountId: Codec,
		Hash: Codec,
		BlockNumber: Codec,
	{
		fn provider(provider: AccountId) -> Versioned<ProviderInfo<BlockNumber>>;
		fn providers(cursor: Option<u32>, limit: u32) -> Page<(AccountId, ProviderInfo<BlockNumber>)>;
		fn agreement(agreement_id: Hash) -> Versioned<AgreementInfo<AccountId, Hash, BlockNumber>>;
		fn provider_agreements(provider: AccountId, cursor: Option<u32>, limit: u32) -> Page<AgreementInfo<AccountId, Hash, BlockNumber>>;
		fn owner_agreements(owner: AccountId, cursor: Option<u32>, limit: u32) -> Page<AgreementInfo<AccountId, Hash, BlockNumber>>;
		fn container_agreements(container: Hash, cursor: Option<u32>, limit: u32) -> Page<AgreementInfo<AccountId, Hash, BlockNumber>>;
		fn agreement_nonce(owner: AccountId) -> u64;
		fn challenge(challenge_id: Hash) -> Versioned<ChallengeInfo<AccountId, Hash, BlockNumber>>;
		fn challenges_at(block: BlockNumber, cursor: Option<u32>, limit: u32) -> Page<ChallengeInfo<AccountId, Hash, BlockNumber>>;
		fn open_challenge_count(agreement_id: Hash) -> u32;
		fn can_accept_capacity(provider: AccountId, additional_bytes: u64) -> bool;
		fn checkpoint(provider: AccountId) -> Versioned<CheckpointInfo<Hash, BlockNumber>>;
		fn provider_root(provider: AccountId) -> Versioned<ProviderRootInfo<Hash, BlockNumber>>;
		fn deletion_acknowledgement(agreement_id: Hash) -> Versioned<DeletionAcknowledgementInfo<AccountId, Hash, BlockNumber>>;
	}

	#[api_version(1)]
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

	#[api_version(1)]
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
		fn object_keys(bucket_id: Hash, cursor: Option<u32>, limit: u32) -> Page<Vec<u8>>;
		fn object_history(bucket_id: Hash, key: Vec<u8>, cursor: Option<u32>, limit: u32) -> Page<ObjectVersionInfo<AccountId, BlockNumber>>;
		fn object_id(bucket_id: Hash, key: Vec<u8>) -> Versioned<Hash>;
	}
}
