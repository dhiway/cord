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

//! Governed, zero-stake storage control plane for Commons.
//!
//! The pallet is the canonical authority for provider admission, buckets, agreements, checkpoint
//! evidence and deterministic replica failover. It never stores application bytes and deliberately
//! contains no price, stake, reward, slash or public-market mechanism.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

extern crate alloc;

pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::BoundedVec, traits::Get, weights::Weight};
use scale_info::TypeInfo;
use sp_core::ed25519;
use sp_runtime::traits::{SaturatedConversion, Saturating};

use pallet_orbis_storage_control_primitives::{Commitment as CanonicalCommitment, CommitmentState};

/// Checkpoint submission wire errors. Codes 220--241 are the frozen P0 vocabulary; code 242 is
/// the additive context-v1 extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum CheckpointErrorCode {
	StorageCheckpointWrongDomain = 220,
	StorageCheckpointWrongVersion = 221,
	StorageCheckpointWrongBucket = 222,
	StorageCheckpointWrongKey = 223,
	StorageCheckpointStaleNonce = 224,
	StorageCheckpointWrongWindow = 225,
	StorageCheckpointInsufficientQuorum = 239,
	StorageCheckpointSequenceInvalid = 240,
	StorageCheckpointEquivocation = 241,
	StorageCheckpointWrongContext = 242,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum CheckpointFallbackPromotionErrorCode {
	WrongVersion = 243,
	WrongDuty = 244,
	WrongKey = 245,
	NotAllowed = 246,
}

impl<T: pallet::Config> pallet_orbis_storage_control_primitives::CanonicalStorageControl
	for pallet::Pallet<T>
{
	fn manifest_state(manifest: &CanonicalCommitment) -> CommitmentState {
		pallet::CanonicalManifests::<T>::get(manifest)
			.map(|record| record.state)
			.unwrap_or(CommitmentState::Missing)
	}

	fn provider_commitment_matches(
		manifest: &CanonicalCommitment,
		provider_commitment: &CanonicalCommitment,
	) -> bool {
		pallet::CanonicalManifests::<T>::get(manifest).is_some_and(|record| {
			record.state == CommitmentState::Publishable
				&& record.provider_commitment.as_ref() == Some(provider_commitment)
		})
	}

	fn deletion_evidence_satisfied(manifest: &CanonicalCommitment) -> bool {
		pallet::CanonicalManifests::<T>::get(manifest).is_some_and(|record| {
			record.state == CommitmentState::Tombstoned
				&& record.tombstoned_at.is_some_and(|at| {
					let window_elapsed = pallet::GovernedFinalizedCheckpoint::<T>::get()
						.is_some_and(|finalized| {
							finalized >= at.saturating_add(T::EvidenceWindow::get())
						});
					let required = pallet::ManifestDeletionRequirements::<T>::get(manifest);
					window_elapsed
						&& (record.provider_commitment.is_none()
							|| (!required.is_empty()
								&& required.iter().all(|provider| {
									pallet::ManifestDeletionAcknowledgements::<T>::contains_key(
										manifest, provider,
									)
								})))
				})
		})
	}
}

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum ProviderStatus {
	Active,
	Suspended,
}

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum AgreementStatus {
	Proposed,
	Active,
	Suspended,
	Cancelled,
	Expired,
}

/// The provider-capacity ledger entry held by an agreement.
///
/// This is deliberately independent from the agreement version and lifecycle status: provider
/// replacement can advance a proposed agreement's version without changing which capacity counter
/// must eventually be released.
#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum AgreementCapacityState {
	Pending,
	Allocated,
	Released,
}

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum ReconciliationDeferReason {
	PendingDuty,
	NoEligibleReplica,
	InvariantFault,
}

impl AgreementStatus {
	pub fn is_terminal(self) -> bool {
		matches!(self, Self::Cancelled | Self::Expired)
	}
}

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum BucketRole {
	Reader,
	Writer,
	Admin,
}

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum ChallengeStatus {
	Open,
	Proved,
	TimedOut,
}

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum ProviderAuthorityError {
	OrganizationUnknown,
	AttestationInvalid,
	AttestationExpired,
	SlaInvalid,
	ServiceKeyInvalid,
}

/// Finalized provider authority adapter supplied by the Commons runtime.
///
/// Implementations must jointly check Entity liveness, final/non-revoked attestation state,
/// allowlisted issuer, validity interval, admitted SLA schema/version and the organization/key
/// bind.
pub trait ProviderAuthority<AccountId, OrganizationRef, ServiceKey, BlockNumber> {
	fn validate(
		provider: &AccountId,
		organization: &OrganizationRef,
		service_key: &ServiceKey,
		finalized_at: BlockNumber,
	) -> Result<(), ProviderAuthorityError>;
}

pub trait CheckpointContextProvider<Hash, BlockNumber> {
	fn genesis_hash() -> Hash;
	fn spec_version() -> u32;
	fn transaction_version() -> u32;
	fn metadata_hash() -> Hash;
	fn block_hash(block: BlockNumber) -> Hash;
}

impl<AccountId, OrganizationRef, ServiceKey, BlockNumber>
	ProviderAuthority<AccountId, OrganizationRef, ServiceKey, BlockNumber> for ()
{
	fn validate(
		_: &AccountId,
		_: &OrganizationRef,
		_: &ServiceKey,
		_: BlockNumber,
	) -> Result<(), ProviderAuthorityError> {
		Err(ProviderAuthorityError::OrganizationUnknown)
	}
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ProviderOrganizationRefV1<EntityId, Hash, BlockNumber> {
	pub entity_id: EntityId,
	pub attestation_id: Hash,
	pub schema_id: Hash,
	pub sla_commitment: Hash,
	pub sla_version: u16,
	pub valid_from: BlockNumber,
	pub valid_until: BlockNumber,
	pub rotation_predecessor: Option<Hash>,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ServiceKeyRecord<BlockNumber> {
	pub active: ed25519::Public,
	pub active_version: u64,
	pub previous: Option<ed25519::Public>,
	pub pending: Option<ed25519::Public>,
	pub pending_version: Option<u64>,
	pub pending_effective_at: Option<BlockNumber>,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ProviderRecord<Endpoint, OrganizationRef, BlockNumber> {
	pub endpoint: Endpoint,
	pub organization: OrganizationRef,
	pub service_key: ServiceKeyRecord<BlockNumber>,
	pub capacity_bytes: u64,
	pub allocated_bytes: u64,
	pub pending_bytes: u64,
	pub status: ProviderStatus,
	pub last_heartbeat: BlockNumber,
	pub authority_validated_at: Option<BlockNumber>,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct BucketGrant<AccountId> {
	/// Account admitted by the bucket ACL. This is not a provider capability authority.
	pub account: AccountId,
	/// Coarse bucket ACL role. Provider capabilities never fall back to this role.
	pub role: BucketRole,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct BucketRecord<AccountId, Hash, BlockNumber, Replicas, Grants> {
	pub owner: AccountId,
	pub version: u64,
	pub policy: Hash,
	pub primary: AccountId,
	pub replicas: Replicas,
	pub grants: Grants,
	pub created_at: BlockNumber,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct BucketOperationReceipt<Hash, AccountId, Replicas> {
	pub request_hash: Hash,
	pub bucket_id: Hash,
	pub primary: AccountId,
	pub replicas: Replicas,
	pub version: u64,
}

/// Canonical finalized host-delegation authority for one provider capability grant.
#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct HostDelegationRecord<AccountId, Hash, BlockNumber, ProductId, Methods, Cid> {
	pub bucket_id: Hash,
	pub owner: AccountId,
	pub issuance_nonce: u64,
	pub issuer_key_id: Hash,
	pub issuer_public_key: ed25519::Public,
	pub key_version: u64,
	pub state_version: u64,
	pub key_activated_at: BlockNumber,
	pub product_id: ProductId,
	pub methods: Methods,
	pub cid: Option<Cid>,
	pub max_bytes: u64,
	pub issued_at: BlockNumber,
	pub expires_at: BlockNumber,
	pub revoked_at: Option<BlockNumber>,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct AgreementRecord<AccountId, Hash, BlockNumber, Replicas> {
	pub owner: AccountId,
	pub bucket_id: Hash,
	pub primary: AccountId,
	pub replicas: Replicas,
	pub bytes: u64,
	pub created_at: BlockNumber,
	pub expires_at: BlockNumber,
	pub release_at: Option<BlockNumber>,
	pub version: u64,
	pub status: AgreementStatus,
	pub capacity_state: AgreementCapacityState,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CheckpointDutyRecord<AccountId, Hash, BlockNumber, Replicas> {
	pub bucket_id: Hash,
	pub primary: AccountId,
	pub replicas: Replicas,
	pub previous_checkpoint: BlockNumber,
	pub previous_commitment: Option<CommitmentV1<Hash>>,
	pub expected_next_start_seq: u64,
	pub due_at: BlockNumber,
	pub grace_until: BlockNumber,
	pub scheduled_at: BlockNumber,
	pub mode: CheckpointDutyMode,
	pub promotion_predecessor: Option<AccountId>,
}

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum CheckpointDutyMode {
	Standard,
	PromotionPending,
}

/// Stable logical SCALE types published under the `cord::storage` metadata namespace.
pub mod storage {
	use super::*;

	#[derive(
		Clone,
		Copy,
		Debug,
		Decode,
		DecodeWithMemTracking,
		Encode,
		Eq,
		MaxEncodedLen,
		PartialEq,
		TypeInfo,
	)]
	#[scale_info(replace_segment("pallet_orbis_storage_provider", "cord"))]
	pub struct MmrLeafV1<Hash> {
		pub data_root: Hash,
		pub data_size: u64,
		pub total_size: u64,
	}

	#[derive(
		Clone,
		Copy,
		Debug,
		Decode,
		DecodeWithMemTracking,
		Encode,
		Eq,
		MaxEncodedLen,
		PartialEq,
		TypeInfo,
	)]
	#[scale_info(replace_segment("pallet_orbis_storage_provider", "cord"))]
	pub struct CommitmentV1<Hash> {
		pub mmr_root: Hash,
		pub start_seq: u64,
		pub leaf_count: u64,
	}

	impl<Hash> CommitmentV1<Hash> {
		pub fn range_end(&self) -> Option<u64> {
			self.start_seq.checked_add(self.leaf_count)
		}
	}

	#[derive(
		Clone,
		Copy,
		Debug,
		Decode,
		DecodeWithMemTracking,
		Encode,
		Eq,
		MaxEncodedLen,
		PartialEq,
		TypeInfo,
	)]
	#[scale_info(replace_segment("pallet_orbis_storage_provider", "cord"))]
	pub struct ChunkLocationV1 {
		pub leaf_index: u64,
		pub chunk_index: u32,
	}

	#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
	#[scale_info(replace_segment("pallet_orbis_storage_provider", "cord"))]
	pub struct MmrProofV1<Hash> {
		pub peaks: alloc::vec::Vec<Hash>,
		pub leaf: MmrLeafV1<Hash>,
		pub leaf_proof: alloc::vec::Vec<Hash>,
	}

	#[derive(
		Clone,
		Copy,
		Debug,
		Decode,
		DecodeWithMemTracking,
		Encode,
		Eq,
		MaxEncodedLen,
		PartialEq,
		TypeInfo,
	)]
	#[scale_info(replace_segment("pallet_orbis_storage_provider", "cord"))]
	pub struct CommitmentPayloadV2<Hash, BlockNumber> {
		pub version: u8,
		pub bucket_id: Hash,
		pub commitment: CommitmentV1<Hash>,
		pub nonce: BlockNumber,
	}

	#[derive(
		Clone,
		Copy,
		Debug,
		Decode,
		DecodeWithMemTracking,
		Encode,
		Eq,
		MaxEncodedLen,
		PartialEq,
		TypeInfo,
	)]
	#[scale_info(replace_segment("pallet_orbis_storage_provider", "cord"))]
	pub struct CheckpointFallbackPromotionV1<Hash, BlockNumber> {
		pub version: u8,
		pub bucket_id: Hash,
		pub snapshot_nonce: BlockNumber,
		pub duty_id: Hash,
	}

	#[derive(
		Clone,
		Copy,
		Debug,
		Decode,
		DecodeWithMemTracking,
		Encode,
		Eq,
		MaxEncodedLen,
		PartialEq,
		TypeInfo,
	)]
	#[scale_info(replace_segment("pallet_orbis_storage_provider", "cord"))]
	pub struct CheckpointContextV1<Hash> {
		pub version: u8,
		pub genesis_hash: Hash,
		pub spec_version: u32,
		pub transaction_version: u32,
		pub metadata_hash: Hash,
		pub finalized_hash: Hash,
		pub duty_id: Hash,
		pub v2_digest: [u8; 32],
	}
}

pub use storage::{
	CheckpointContextV1, CheckpointFallbackPromotionV1, ChunkLocationV1, CommitmentPayloadV2,
	CommitmentV1, MmrLeafV1, MmrProofV1,
};

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CheckpointFallbackPromotionReceipt<AccountId, Hash, BlockNumber> {
	pub payload: CheckpointFallbackPromotionV1<Hash, BlockNumber>,
	pub provider: AccountId,
	pub service_key: ed25519::Public,
	pub signature: ed25519::Signature,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ReplicaSignature<AccountId> {
	pub provider: AccountId,
	pub service_key: ed25519::Public,
	pub signature: ed25519::Signature,
	pub context_signature: ed25519::Signature,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct BucketSnapshot<Hash, BlockNumber, Confirmations> {
	pub commitment: CommitmentV1<Hash>,
	pub checkpoint_block: BlockNumber,
	pub primary_signers: u8,
	pub commitment_nonce: BlockNumber,
	pub replica_confirmations: Confirmations,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CheckpointClaim<AccountId, Hash, BlockNumber, Confirmations> {
	pub primary: AccountId,
	pub payload: CommitmentPayloadV2<Hash, BlockNumber>,
	pub call_digest: [u8; 32],
	pub primary_signature: ed25519::Signature,
	pub primary_context_signature: ed25519::Signature,
	pub replica_confirmations: Confirmations,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ChallengeRecord<AccountId, Hash, BlockNumber> {
	pub bucket_id: Hash,
	pub provider: AccountId,
	pub expected_commitment: CommitmentV1<Hash>,
	pub location: ChunkLocationV1,
	pub due_at: BlockNumber,
	pub status: ChallengeStatus,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct EvidenceRecord<AccountId, Hash, BlockNumber> {
	pub provider: AccountId,
	pub bucket_id: Hash,
	pub evidence_hash: Hash,
	pub recorded_at: BlockNumber,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CanonicalManifestRecord<Hash, BlockNumber> {
	pub bucket_id: Hash,
	pub provider_commitment: Option<CanonicalCommitment>,
	pub state: CommitmentState,
	pub checkpoint: Option<BlockNumber>,
	pub tombstoned_at: Option<BlockNumber>,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct DeletionAcknowledgement<AccountId, Hash, BlockNumber> {
	pub provider: AccountId,
	pub bucket_id: Hash,
	pub manifest: CanonicalCommitment,
	pub evidence_hash: Hash,
	pub service_key: ed25519::Public,
	pub signature: ed25519::Signature,
	pub acknowledged_at: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		traits::{EnsureOrigin, Hooks},
		transactional,
	};
	use frame_system::pallet_prelude::*;
	use scale_info::prelude::vec::Vec;
	use sp_runtime::traits::{Hash as HashT, Saturating};

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);
	const CHECKPOINT_DOMAIN: &[u8] = b"cord/storage/checkpoint/v2";
	const CHECKPOINT_CONTEXT_DOMAIN: &[u8] = b"cord/storage/checkpoint-context/v1";
	const CHECKPOINT_PROMOTION_DOMAIN: &[u8] = b"cord/storage/checkpoint-promotion/v1";

	pub type EndpointOf<T> = BoundedVec<u8, <T as Config>::MaxEndpointBytes>;
	pub type EntityIdOf<T> = BoundedVec<u8, <T as Config>::MaxEntityIdBytes>;
	pub type OrganizationRefOf<T> = ProviderOrganizationRefV1<
		EntityIdOf<T>,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
	>;
	pub type ProviderRecordOf<T> =
		ProviderRecord<EndpointOf<T>, OrganizationRefOf<T>, BlockNumberFor<T>>;
	pub type ReplicasOf<T> =
		BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::MaxReplicas>;
	pub type AssignedProvidersOf<T> =
		BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::MaxAssignedProviders>;
	pub type GrantsOf<T> = BoundedVec<
		BucketGrant<<T as frame_system::Config>::AccountId>,
		<T as Config>::MaxBucketGrants,
	>;
	pub type BucketRecordOf<T> = BucketRecord<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
		ReplicasOf<T>,
		GrantsOf<T>,
	>;
	pub type CapabilityProductIdOf<T> = BoundedVec<u8, <T as Config>::MaxCapabilityProductIdBytes>;
	pub type CapabilityMethodsOf<T> = BoundedVec<u16, <T as Config>::MaxCapabilityMethods>;
	pub type CapabilityCidOf<T> = BoundedVec<u8, <T as Config>::MaxCapabilityCidBytes>;
	pub type HostDelegationRecordOf<T> = HostDelegationRecord<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
		CapabilityProductIdOf<T>,
		CapabilityMethodsOf<T>,
		CapabilityCidOf<T>,
	>;
	pub type AgreementRecordOf<T> = AgreementRecord<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
		ReplicasOf<T>,
	>;
	pub type CheckpointDutyRecordOf<T> = CheckpointDutyRecord<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
		ReplicasOf<T>,
	>;
	pub type ConfirmationsOf<T> = BoundedVec<
		ReplicaSignature<<T as frame_system::Config>::AccountId>,
		<T as Config>::MaxReplicas,
	>;
	pub type SnapshotOf<T> =
		BucketSnapshot<<T as frame_system::Config>::Hash, BlockNumberFor<T>, ReplicasOf<T>>;
	pub type ClaimOf<T> = CheckpointClaim<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
		ConfirmationsOf<T>,
	>;
	pub type PromotionReceiptOf<T> = CheckpointFallbackPromotionReceipt<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
	>;
	pub type MmrProofOf<T> = MmrProofV1<<T as frame_system::Config>::Hash>;
	pub type ChallengeRecordOf<T> = ChallengeRecord<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
	>;
	pub type EvidenceRecordOf<T> = EvidenceRecord<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
	>;
	pub type DeletionAcknowledgementOf<T> = DeletionAcknowledgement<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		#[pallet::constant]
		type MaxEndpointBytes: Get<u32>;
		#[pallet::constant]
		type MaxEntityIdBytes: Get<u32>;
		#[pallet::constant]
		type MaxProviders: Get<u32>;
		#[pallet::constant]
		type MaxBuckets: Get<u32>;
		#[pallet::constant]
		type MaxBucketGrants: Get<u32>;
		#[pallet::constant]
		type MaxHostDelegationsPerBucket: Get<u32>;
		#[pallet::constant]
		type MaxCapabilityProductIdBytes: Get<u32>;
		#[pallet::constant]
		type MaxCapabilityMethods: Get<u32>;
		#[pallet::constant]
		type MaxCapabilityCidBytes: Get<u32>;
		#[pallet::constant]
		type MaxHostDelegationLifetime: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type MaxReplicas: Get<u32>;
		#[pallet::constant]
		type MaxAssignedProviders: Get<u32>;
		#[pallet::constant]
		type MaxProviderAgreements: Get<u32>;
		#[pallet::constant]
		type MaxBucketAgreements: Get<u32>;
		#[pallet::constant]
		type MaxDutiesPerBlock: Get<u32>;
		type MaxChallengeBacklog: Get<u32>;
		type MaxCapacityReleasesPerBlock: Get<u32>;
		#[pallet::constant]
		type MaxChallengesPerBlock: Get<u32>;
		#[pallet::constant]
		type MaxReconciliationRecords: Get<u32>;
		#[pallet::constant]
		type MaxProofNodes: Get<u32>;
		#[pallet::constant]
		type MaxEvidencePerProvider: Get<u32>;
		#[pallet::constant]
		type MaxOrganizationHistory: Get<u32>;
		#[pallet::constant]
		type CheckpointCadence: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type CheckpointGrace: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type MaxCheckpointAge: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type EvidenceWindow: Get<BlockNumberFor<Self>>;
		type ProviderAuthority: ProviderAuthority<
			Self::AccountId,
			OrganizationRefOf<Self>,
			ed25519::Public,
			BlockNumberFor<Self>,
		>;
		type CheckpointContext: CheckpointContextProvider<Self::Hash, BlockNumberFor<Self>>;
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type Providers<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, ProviderRecordOf<T>, OptionQuery>;
	#[pallet::storage]
	pub type ProviderIds<T: Config> =
		StorageValue<_, BoundedVec<T::AccountId, T::MaxProviders>, ValueQuery>;
	#[pallet::storage]
	pub type EndpointOwner<T: Config> =
		StorageMap<_, Blake2_128Concat, EndpointOf<T>, T::AccountId, OptionQuery>;
	#[pallet::storage]
	pub type ServiceKeyOwner<T: Config> =
		StorageMap<_, Blake2_128Concat, ed25519::Public, T::AccountId, OptionQuery>;
	#[pallet::storage]
	pub type ProviderBucketAssignmentCount<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;
	#[pallet::storage]
	pub type OrganizationHistory<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<OrganizationRefOf<T>, T::MaxOrganizationHistory>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type Buckets<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, BucketRecordOf<T>, OptionQuery>;
	#[pallet::storage]
	pub type BucketIds<T: Config> = StorageValue<_, BoundedVec<T::Hash, T::MaxBuckets>, ValueQuery>;

	#[pallet::storage]
	pub type BucketOperationReceipts<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		[u8; 16],
		BucketOperationReceipt<T::Hash, T::AccountId, ReplicasOf<T>>,
		OptionQuery,
	>;
	#[pallet::storage]
	pub type BucketNonce<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;
	/// Owner-scoped checked nonce used to derive unique host-delegation grant identifiers.
	#[pallet::storage]
	pub type GrantNonce<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;
	/// Sole provider-capability authority. Revoked entries remain as fail-closed tombstones.
	#[pallet::storage]
	pub type HostDelegations<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, HostDelegationRecordOf<T>, OptionQuery>;
	/// Bounded active host-delegation identifiers for a bucket.
	#[pallet::storage]
	pub type BucketHostDelegations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		BoundedVec<T::Hash, T::MaxHostDelegationsPerBucket>,
		ValueQuery,
	>;
	#[pallet::storage]
	pub type BucketSnapshots<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, SnapshotOf<T>, OptionQuery>;
	/// Last finalized duty version per bucket.
	#[pallet::storage]
	pub type CheckpointDutyCurrent<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, CheckpointDutyRecordOf<T>, OptionQuery>;
	/// Latest staged duty version per bucket. It is selected only after its scheduling block is at
	/// or before the governed finalized pointer.
	#[pallet::storage]
	pub type CheckpointDutyPending<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, CheckpointDutyRecordOf<T>, OptionQuery>;
	#[pallet::storage]
	pub type DutyAdmissionBlock<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;
	#[pallet::storage]
	pub type DutyAdmissionCount<T: Config> = StorageValue<_, u32, ValueQuery>;
	#[pallet::storage]
	pub type CheckpointFallbackPromotionReceiptByBucket<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, PromotionReceiptOf<T>, OptionQuery>;
	/// Singular manifest-to-checkpoint authority consumed by Drive and S3.
	#[pallet::storage]
	pub type CanonicalManifests<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CanonicalCommitment,
		CanonicalManifestRecord<T::Hash, BlockNumberFor<T>>,
		OptionQuery,
	>;
	#[pallet::storage]
	pub type ManifestDeletionRequirements<T: Config> =
		StorageMap<_, Blake2_128Concat, CanonicalCommitment, AssignedProvidersOf<T>, ValueQuery>;
	/// Provider-first bounded-page index of unacknowledged canonical manifest deletion duties.
	#[pallet::storage]
	pub type ManifestDeletionDuties<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		CanonicalCommitment,
		(),
		OptionQuery,
	>;
	#[pallet::storage]
	pub type GovernedFinalizedCheckpoint<T: Config> =
		StorageValue<_, BlockNumberFor<T>, OptionQuery>;
	#[pallet::storage]
	pub type ManifestDeletionAcknowledgements<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CanonicalCommitment,
		Blake2_128Concat,
		T::AccountId,
		DeletionAcknowledgementOf<T>,
		OptionQuery,
	>;
	#[pallet::storage]
	pub type ReplicaCheckpoint<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Blake2_128Concat,
		T::AccountId,
		BlockNumberFor<T>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type Agreements<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, AgreementRecordOf<T>, OptionQuery>;
	#[pallet::storage]
	pub type AgreementNonce<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;
	#[pallet::storage]
	pub type ProviderAgreements<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<T::Hash, T::MaxProviderAgreements>,
		ValueQuery,
	>;
	#[pallet::storage]
	pub type BucketAgreements<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		BoundedVec<T::Hash, T::MaxBucketAgreements>,
		ValueQuery,
	>;
	#[pallet::storage]
	pub type CapacityReleases<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		BlockNumberFor<T>,
		BoundedVec<T::Hash, T::MaxCapacityReleasesPerBlock>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type CheckpointClaims<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Blake2_128Concat,
		T::Hash,
		ClaimOf<T>,
		OptionQuery,
	>;
	#[pallet::storage]
	pub type EquivocationEvidence<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<ClaimOf<T>, T::MaxEvidencePerProvider>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type Challenges<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, ChallengeRecordOf<T>, OptionQuery>;

	/// Canonical bounded work queue for open challenges.
	#[pallet::storage]
	pub type ChallengeBacklog<T: Config> =
		StorageValue<_, BoundedVec<T::Hash, T::MaxChallengeBacklog>, ValueQuery>;
	#[pallet::storage]
	pub type ChallengeBacklogCursor<T: Config> = StorageValue<_, u32, ValueQuery>;
	#[pallet::storage]
	pub type OverdueChallenges<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;
	#[pallet::storage]
	pub type ProviderEvidence<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<EvidenceRecordOf<T>, T::MaxEvidencePerProvider>,
		ValueQuery,
	>;
	/// Number of timeout evidence records omitted because the bounded evidence journal was full.
	/// The corresponding provider is still suspended and the challenge is still timed out.
	#[pallet::storage]
	pub type ProviderEvidenceOverflow<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;
	#[pallet::storage]
	pub type ProviderReconciliationCursor<T: Config> = StorageValue<_, u32, ValueQuery>;
	#[pallet::storage]
	pub type BucketReconciliationCursor<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ProviderRegistered {
			provider: T::AccountId,
			capacity_bytes: u64,
		},
		ProviderUpdated {
			provider: T::AccountId,
			capacity_bytes: u64,
		},
		ServiceKeyRotationScheduled {
			provider: T::AccountId,
			old_key: ed25519::Public,
			new_key: ed25519::Public,
			effective_at: BlockNumberFor<T>,
		},
		ServiceKeyRotated {
			provider: T::AccountId,
			old_key: ed25519::Public,
			new_key: ed25519::Public,
			effective_at: BlockNumberFor<T>,
		},
		ProviderOrganizationRotated {
			provider: T::AccountId,
			predecessor: T::Hash,
		},
		ProviderStatusChanged {
			provider: T::AccountId,
			status: ProviderStatus,
		},
		ProviderAuthorityRefreshed {
			provider: T::AccountId,
			bucket_id: T::Hash,
			checkpoint: BlockNumberFor<T>,
			valid: bool,
		},
		BucketAuthorityRefreshed {
			bucket_id: T::Hash,
			checkpoint: BlockNumberFor<T>,
			validated: u32,
			suspended: u32,
		},
		BucketReconciliationDeferred {
			bucket_id: T::Hash,
			checkpoint: BlockNumberFor<T>,
			reason: ReconciliationDeferReason,
		},
		FinalizedCheckpointAdvanced {
			previous: Option<BlockNumberFor<T>>,
			current: BlockNumberFor<T>,
		},
		ProviderRemoved {
			provider: T::AccountId,
		},
		Heartbeat {
			provider: T::AccountId,
			at: BlockNumberFor<T>,
		},
		BucketCreated {
			bucket_id: T::Hash,
			owner: T::AccountId,
			primary: T::AccountId,
			replicas: ReplicasOf<T>,
			version: u64,
			operation_id: [u8; 16],
			replayed: bool,
		},
		BucketGrantChanged {
			bucket_id: T::Hash,
			account: T::AccountId,
			role: Option<BucketRole>,
			previous_version: u64,
			new_version: u64,
		},
		HostDelegationCreated {
			grant_id: T::Hash,
			bucket_id: T::Hash,
			owner: T::AccountId,
			issuance_nonce: u64,
			state_version: u64,
		},
		HostDelegationKeyRotated {
			grant_id: T::Hash,
			old_key_id: T::Hash,
			new_key_id: T::Hash,
			key_version: u64,
			state_version: u64,
			key_activated_at: BlockNumberFor<T>,
		},
		HostDelegationRevoked {
			grant_id: T::Hash,
			state_version: u64,
			revoked_at: BlockNumberFor<T>,
		},
		AgreementTransitioned {
			agreement_id: T::Hash,
			previous: Option<AgreementStatus>,
			current: AgreementStatus,
			previous_version: u64,
			new_version: u64,
		},
		AgreementCapacityReleased {
			agreement_id: T::Hash,
		},
		ChallengeEvidenceOverflowed {
			challenge_id: T::Hash,
			provider: T::AccountId,
			evidence_hash: T::Hash,
		},
		CheckpointAccepted {
			bucket_id: T::Hash,
			commitment: CommitmentV1<T::Hash>,
			checkpoint: BlockNumberFor<T>,
			replica_confirmations: ReplicasOf<T>,
		},
		CheckpointEquivocation {
			code: u16,
			bucket_id: T::Hash,
			provider: T::AccountId,
			accepted_root: T::Hash,
			conflicting_root: T::Hash,
			nonce: BlockNumberFor<T>,
		},
		ChallengeIssued {
			challenge_id: T::Hash,
			bucket_id: T::Hash,
			provider: T::AccountId,
			due_at: BlockNumberFor<T>,
		},
		ChallengeProved {
			challenge_id: T::Hash,
			provider: T::AccountId,
		},
		ChallengeTimedOut {
			challenge_id: T::Hash,
			provider: T::AccountId,
			checkpoint: BlockNumberFor<T>,
		},
		EvidenceRecorded {
			provider: T::AccountId,
			bucket_id: T::Hash,
			evidence_hash: T::Hash,
		},
		ProviderIneligible {
			provider: T::AccountId,
			bucket_id: T::Hash,
			checkpoint: BlockNumberFor<T>,
		},
		ReplicaSelected {
			bucket_id: T::Hash,
			provider: T::AccountId,
			checkpoint: BlockNumberFor<T>,
		},
		PrimaryPromoted {
			bucket_id: T::Hash,
			old_provider: T::AccountId,
			new_provider: T::AccountId,
			checkpoint: BlockNumberFor<T>,
		},
		CheckpointFallbackPromotionPendingQuorum {
			bucket_id: T::Hash,
			provider: T::AccountId,
			snapshot_nonce: BlockNumberFor<T>,
			duty_id: T::Hash,
		},
		ManifestCommitmentChanged {
			manifest: CanonicalCommitment,
			bucket_id: T::Hash,
			state: CommitmentState,
			checkpoint: Option<BlockNumberFor<T>>,
		},
		BucketReplicaReplaced {
			bucket_id: T::Hash,
			old_provider: T::AccountId,
			new_provider: T::AccountId,
			previous_version: u64,
			new_version: u64,
		},
		AgreementProviderRebound {
			agreement_id: T::Hash,
			old_provider: T::AccountId,
			new_provider: T::AccountId,
			status: AgreementStatus,
			bytes: u64,
		},
		ManifestDeletionAcknowledged {
			manifest: CanonicalCommitment,
			bucket_id: T::Hash,
			provider: T::AccountId,
			evidence_hash: T::Hash,
			acknowledged_at: BlockNumberFor<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		ProviderAlreadyExists,
		ProviderLimitReached,
		ProviderNotFound,
		ProviderIneligible,
		EndpointInUse,
		ServiceKeyInUse,
		ServiceKeyRotationPending,
		CapacityBelowAllocation,
		CapacityExceeded,
		ProviderHasActiveAllocations,
		ProviderHasBucketAssignments,
		OrganizationHistoryFull,
		ProviderOrgUnknown,
		ProviderAttestationInvalid,
		ProviderAttestationExpired,
		ProviderSlaInvalid,
		ProviderServiceKeyInvalid,
		ProviderNotBucketPrimary,
		FinalizedCheckpointUninitialized,
		FinalizedCheckpointNotMonotonic,
		FinalizedCheckpointInFuture,
		BucketNotFound,
		BucketAlreadyExists,
		BucketLimitReached,
		OperationIdConflict,
		BucketVersionConflict,
		BucketMemberLimit,
		HostDelegationLimit,
		HostDelegationNotFound,
		HostDelegationAlreadyExists,
		HostDelegationRevoked,
		HostDelegationVersionConflict,
		GrantNonceOverflow,
		InvalidCapabilityScope,
		InvalidCapabilityLifetime,
		InvalidReplicaCount,
		DuplicateProviderAssignment,
		NotBucketOwner,
		AgreementNotFound,
		AgreementInvalidState,
		AgreementCapacityExceeded,
		AgreementIndexFull,
		AgreementAlreadyExists,
		InvalidExpiry,
		InvalidCapacity,
		NotAgreementParty,
		CapacityReleaseQueueFull,
		ChallengeNotFound,
		ChallengeNotOpen,
		ChallengeNotDue,
		ChallengeExpired,
		ChallengeDutyLimit,
		ChallengeAlreadyExists,
		ProofInvalid,
		ProofNodeLimit,
		EvidenceLimit,
		StorageCheckpointWrongDomain,
		StorageCheckpointWrongVersion,
		StorageCheckpointWrongBucket,
		StorageCheckpointWrongKey,
		StorageCheckpointStaleNonce,
		StorageCheckpointWrongWindow,
		StorageCheckpointInsufficientQuorum,
		InsufficientFallbackQuorum,
		CheckpointFallbackPromotionWrongVersion,
		CheckpointFallbackPromotionWrongDuty,
		CheckpointFallbackPromotionWrongKey,
		CheckpointFallbackPromotionNotAllowed,
		StorageCheckpointSequenceInvalid,
		StorageCheckpointEquivocation,
		StorageCheckpointWrongContext,
		CheckpointDutyLimit,
		CheckpointDutyPending,
		ManifestAlreadyExists,
		ManifestNotFound,
		ManifestInvalidState,
		ManifestCommitmentMismatch,
		DeletionProviderNotRequired,
		DeletionAlreadyAcknowledged,
		DeletionEvidenceInvalid,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(now: BlockNumberFor<T>) -> Weight {
			match now.saturated_into::<u64>() % 3 {
				0 => {
					let released = Self::release_due_capacity(now);
					T::WeightInfo::on_initialize_release(released)
				},
				1 => {
					let (records, agreements) = Self::reconcile_finalized();
					T::WeightInfo::on_initialize_reconcile(records, agreements)
				},
				_ => {
					let processed = Self::process_due_challenges(now);
					T::WeightInfo::on_initialize_challenges(processed)
				},
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::register_provider())]
		pub fn register_provider(
			origin: OriginFor<T>,
			provider: T::AccountId,
			endpoint: EndpointOf<T>,
			service_key: ed25519::Public,
			organization: OrganizationRefOf<T>,
			capacity_bytes: u64,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			ensure!(capacity_bytes > 0, Error::<T>::InvalidCapacity);
			ensure!(!Providers::<T>::contains_key(&provider), Error::<T>::ProviderAlreadyExists);
			ensure!(!EndpointOwner::<T>::contains_key(&endpoint), Error::<T>::EndpointInUse);
			ensure!(!ServiceKeyOwner::<T>::contains_key(service_key), Error::<T>::ServiceKeyInUse);
			let finalized = Self::finalized_checkpoint()?;
			Self::ensure_authority(&provider, &organization, &service_key, finalized)?;
			ProviderIds::<T>::try_mutate(|ids| ids.try_push(provider.clone()))
				.map_err(|_| Error::<T>::ProviderLimitReached)?;
			OrganizationHistory::<T>::try_mutate(&provider, |history| {
				history.try_push(organization.clone())
			})
			.map_err(|_| Error::<T>::OrganizationHistoryFull)?;
			let now = frame_system::Pallet::<T>::block_number();
			Providers::<T>::insert(
				&provider,
				ProviderRecord {
					endpoint: endpoint.clone(),
					organization,
					service_key: ServiceKeyRecord {
						active: service_key,
						active_version: 1,
						previous: None,
						pending: None,
						pending_version: None,
						pending_effective_at: None,
					},
					capacity_bytes,
					allocated_bytes: 0,
					pending_bytes: 0,
					status: ProviderStatus::Active,
					last_heartbeat: now,
					authority_validated_at: Some(finalized),
				},
			);
			EndpointOwner::<T>::insert(endpoint, &provider);
			ServiceKeyOwner::<T>::insert(service_key, &provider);
			Self::deposit_event(Event::ProviderRegistered { provider, capacity_bytes });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::update_provider())]
		pub fn update_provider(
			origin: OriginFor<T>,
			provider: T::AccountId,
			endpoint: EndpointOf<T>,
			capacity_bytes: u64,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			ensure!(capacity_bytes > 0, Error::<T>::InvalidCapacity);
			Providers::<T>::try_mutate(&provider, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::ProviderNotFound)?;
				ensure!(
					capacity_bytes >= record.allocated_bytes.saturating_add(record.pending_bytes),
					Error::<T>::CapacityBelowAllocation
				);
				if endpoint != record.endpoint {
					ensure!(
						!EndpointOwner::<T>::contains_key(&endpoint),
						Error::<T>::EndpointInUse
					);
					EndpointOwner::<T>::remove(&record.endpoint);
					EndpointOwner::<T>::insert(&endpoint, &provider);
					record.endpoint = endpoint;
				}
				record.capacity_bytes = capacity_bytes;
				Ok(())
			})?;
			Self::deposit_event(Event::ProviderUpdated { provider, capacity_bytes });
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::rotate_service_key())]
		#[transactional]
		pub fn rotate_service_key(
			origin: OriginFor<T>,
			provider: T::AccountId,
			new_key: ed25519::Public,
			effective_at: BlockNumberFor<T>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			ensure!(!ServiceKeyOwner::<T>::contains_key(new_key), Error::<T>::ServiceKeyInUse);
			let finalized = Self::finalized_checkpoint()?;
			Providers::<T>::try_mutate(&provider, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::ProviderNotFound)?;
				ensure!(
					record.service_key.pending.is_none(),
					Error::<T>::ServiceKeyRotationPending
				);
				let old_key = record.service_key.active;
				ServiceKeyOwner::<T>::insert(new_key, &provider);
				if effective_at <= finalized {
					Self::ensure_authority(&provider, &record.organization, &new_key, finalized)?;
					ServiceKeyOwner::<T>::remove(old_key);
					record.service_key.previous = Some(old_key);
					record.service_key.active = new_key;
					record.service_key.active_version =
						record.service_key.active_version.saturating_add(1);
					record.authority_validated_at = Some(finalized);
					Self::deposit_event(Event::ServiceKeyRotated {
						provider: provider.clone(),
						old_key,
						new_key,
						effective_at,
					});
				} else {
					Self::ensure_authority(
						&provider,
						&record.organization,
						&new_key,
						effective_at,
					)?;
					record.service_key.pending = Some(new_key);
					record.service_key.pending_version =
						Some(record.service_key.active_version.saturating_add(1));
					record.service_key.pending_effective_at = Some(effective_at);
					Self::deposit_event(Event::ServiceKeyRotationScheduled {
						provider: provider.clone(),
						old_key,
						new_key,
						effective_at,
					});
				}
				Ok(())
			})
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::rotate_provider_organization())]
		#[transactional]
		pub fn rotate_provider_organization(
			origin: OriginFor<T>,
			provider: T::AccountId,
			organization: OrganizationRefOf<T>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let finalized = Self::finalized_checkpoint()?;
			let mut record = Providers::<T>::get(&provider).ok_or(Error::<T>::ProviderNotFound)?;
			Self::activate_pending_key(&provider, &mut record, finalized);
			Self::ensure_authority(
				&provider,
				&organization,
				&record.service_key.active,
				finalized,
			)?;
			let predecessor = T::Hashing::hash_of(&record.organization);
			ensure!(
				organization.rotation_predecessor == Some(predecessor),
				Error::<T>::ProviderAttestationInvalid
			);
			OrganizationHistory::<T>::try_mutate(&provider, |history| {
				history.try_push(organization.clone())
			})
			.map_err(|_| Error::<T>::OrganizationHistoryFull)?;
			record.organization = organization;
			record.authority_validated_at = Some(finalized);
			Providers::<T>::insert(&provider, record);
			Self::deposit_event(Event::ProviderOrganizationRotated { provider, predecessor });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::set_provider_status())]
		#[transactional]
		pub fn set_provider_status(
			origin: OriginFor<T>,
			provider: T::AccountId,
			status: ProviderStatus,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			Providers::<T>::try_mutate(&provider, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::ProviderNotFound)?;
				if status == ProviderStatus::Active {
					let finalized = Self::finalized_checkpoint()?;
					Self::activate_pending_key(&provider, record, finalized);
					Self::ensure_authority(
						&provider,
						&record.organization,
						&record.service_key.active,
						finalized,
					)?;
					record.authority_validated_at = Some(finalized);
				}
				record.status = status;
				Ok(())
			})?;
			Self::deposit_event(Event::ProviderStatusChanged { provider, status });
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::remove_provider())]
		pub fn remove_provider(origin: OriginFor<T>, provider: T::AccountId) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let record = Providers::<T>::get(&provider).ok_or(Error::<T>::ProviderNotFound)?;
			ensure!(
				record.allocated_bytes == 0 && record.pending_bytes == 0,
				Error::<T>::ProviderHasActiveAllocations
			);
			ensure!(
				ProviderBucketAssignmentCount::<T>::get(&provider) == 0,
				Error::<T>::ProviderHasBucketAssignments
			);
			Providers::<T>::remove(&provider);
			ProviderIds::<T>::mutate(|ids| {
				if let Some(index) = ids.iter().position(|id| id == &provider) {
					ids.remove(index);
				}
			});
			EndpointOwner::<T>::remove(record.endpoint);
			ServiceKeyOwner::<T>::remove(record.service_key.active);
			if let Some(pending) = record.service_key.pending {
				ServiceKeyOwner::<T>::remove(pending);
			}
			Self::deposit_event(Event::ProviderRemoved { provider });
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::heartbeat())]
		pub fn heartbeat(origin: OriginFor<T>) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let at = frame_system::Pallet::<T>::block_number();
			Providers::<T>::try_mutate(&provider, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::ProviderNotFound)?;
				ensure!(record.status == ProviderStatus::Active, Error::<T>::ProviderIneligible);
				record.last_heartbeat = at;
				Ok(())
			})?;
			Self::deposit_event(Event::Heartbeat { provider, at });
			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::create_bucket(replicas.len() as u32))]
		#[transactional]
		pub fn create_bucket(
			origin: OriginFor<T>,
			policy: T::Hash,
			primary: T::AccountId,
			replicas: ReplicasOf<T>,
			operation_id: [u8; 16],
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let request_hash = T::Hashing::hash_of(&(policy, &primary, &replicas));
			if let Some(receipt) = BucketOperationReceipts::<T>::get(&owner, operation_id) {
				ensure!(receipt.request_hash == request_hash, Error::<T>::OperationIdConflict);
				Self::deposit_event(Event::BucketCreated {
					bucket_id: receipt.bucket_id,
					owner,
					primary: receipt.primary,
					replicas: receipt.replicas,
					version: receipt.version,
					operation_id,
					replayed: true,
				});
				return Ok(());
			}
			Self::ensure_assignments(&primary, &replicas)?;
			let finalized = Self::finalized_checkpoint()?;
			Self::ensure_provider_eligible(&primary, finalized)?;
			for replica in &replicas {
				Self::ensure_provider_eligible(replica, finalized)?;
			}
			let nonce = BucketNonce::<T>::get(&owner);
			let bucket_id =
				T::Hashing::hash_of(&(b"cord/storage/bucket/v1", &owner, nonce, policy));
			ensure!(!Buckets::<T>::contains_key(bucket_id), Error::<T>::BucketAlreadyExists);
			BucketIds::<T>::try_mutate(|ids| ids.try_push(bucket_id))
				.map_err(|_| Error::<T>::BucketLimitReached)?;
			BucketNonce::<T>::insert(&owner, nonce.saturating_add(1));
			let created_at = frame_system::Pallet::<T>::block_number();
			Buckets::<T>::insert(
				bucket_id,
				BucketRecord {
					owner: owner.clone(),
					version: 1,
					policy,
					primary: primary.clone(),
					replicas: replicas.clone(),
					grants: Default::default(),
					created_at,
				},
			);
			Self::stage_checkpoint_duty(
				bucket_id,
				&primary,
				&replicas,
				created_at,
				created_at.saturating_add(T::CheckpointCadence::get()),
				CheckpointDutyMode::Standard,
				None,
			)?;
			ProviderBucketAssignmentCount::<T>::mutate(&primary, |count| {
				*count = count.saturating_add(1)
			});
			for replica in replicas.iter() {
				ProviderBucketAssignmentCount::<T>::mutate(replica, |count| {
					*count = count.saturating_add(1)
				});
			}
			BucketOperationReceipts::<T>::insert(
				&owner,
				operation_id,
				BucketOperationReceipt {
					request_hash,
					bucket_id,
					primary: primary.clone(),
					replicas: replicas.clone(),
					version: 1,
				},
			);
			Self::deposit_event(Event::BucketCreated {
				bucket_id,
				owner,
				primary,
				replicas,
				version: 1,
				operation_id,
				replayed: false,
			});
			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::change_bucket_grant())]
		pub fn change_bucket_grant(
			origin: OriginFor<T>,
			bucket_id: T::Hash,
			expected_version: u64,
			account: T::AccountId,
			role: Option<BucketRole>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let (previous_version, new_version) = Buckets::<T>::try_mutate(
				bucket_id,
				|maybe| -> Result<(u64, u64), DispatchError> {
					let bucket = maybe.as_mut().ok_or(Error::<T>::BucketNotFound)?;
					ensure!(bucket.owner == owner, Error::<T>::NotBucketOwner);
					ensure!(bucket.version == expected_version, Error::<T>::BucketVersionConflict);
					let existing = bucket.grants.iter().position(|grant| grant.account == account);
					match (existing, role) {
						(Some(index), Some(next)) => bucket.grants[index].role = next,
						(Some(index), None) => {
							bucket.grants.remove(index);
						},
						(None, Some(next)) => bucket
							.grants
							.try_push(BucketGrant { account: account.clone(), role: next })
							.map_err(|_| Error::<T>::BucketMemberLimit)?,
						(None, None) => {},
					}
					let previous = bucket.version;
					bucket.version =
						bucket.version.checked_add(1).ok_or(Error::<T>::BucketVersionConflict)?;
					Ok((previous, bucket.version))
				},
			)?;
			Self::deposit_event(Event::BucketGrantChanged {
				bucket_id,
				account,
				role,
				previous_version,
				new_version,
			});
			Ok(())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::propose_agreement(replicas_bound::<T>()))]
		#[transactional]
		pub fn propose_agreement(
			origin: OriginFor<T>,
			bucket_id: T::Hash,
			expected_bucket_version: u64,
			bytes: u64,
			expires_at: BlockNumberFor<T>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			ensure!(bytes > 0, Error::<T>::InvalidCapacity);
			let now = frame_system::Pallet::<T>::block_number();
			ensure!(expires_at > now, Error::<T>::InvalidExpiry);
			let bucket = Buckets::<T>::get(bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			ensure!(bucket.owner == owner, Error::<T>::NotBucketOwner);
			ensure!(bucket.version == expected_bucket_version, Error::<T>::BucketVersionConflict);
			let finalized = Self::finalized_checkpoint()?;
			Self::ensure_provider_eligible(&bucket.primary, finalized)?;
			for provider in bucket.replicas.iter() {
				Self::ensure_provider_eligible(provider, finalized)?;
			}
			let nonce = AgreementNonce::<T>::get(&owner);
			let agreement_id = T::Hashing::hash_of(&(
				b"cord/storage/agreement/v1",
				&owner,
				bucket_id,
				nonce,
				bytes,
				expires_at,
			));
			ensure!(
				!Agreements::<T>::contains_key(agreement_id),
				Error::<T>::AgreementAlreadyExists
			);
			Self::reserve_capacity(&bucket.primary, bytes, agreement_id)?;
			for provider in bucket.replicas.iter() {
				Self::reserve_capacity(provider, bytes, agreement_id)?;
			}
			BucketAgreements::<T>::try_mutate(bucket_id, |ids| {
				ensure!(!ids.contains(&agreement_id), Error::<T>::AgreementAlreadyExists);
				ids.try_push(agreement_id).map_err(|_| Error::<T>::AgreementIndexFull)
			})?;
			AgreementNonce::<T>::insert(&owner, nonce.saturating_add(1));
			Agreements::<T>::insert(
				agreement_id,
				AgreementRecord {
					owner,
					bucket_id,
					primary: bucket.primary,
					replicas: bucket.replicas,
					bytes,
					created_at: now,
					expires_at,
					release_at: None,
					version: 1,
					status: AgreementStatus::Proposed,
					capacity_state: AgreementCapacityState::Pending,
				},
			);
			Self::deposit_event(Event::AgreementTransitioned {
				agreement_id,
				previous: None,
				current: AgreementStatus::Proposed,
				previous_version: 0,
				new_version: 1,
			});
			Ok(())
		}

		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::accept_agreement(replicas_bound::<T>()))]
		#[transactional]
		pub fn accept_agreement(
			origin: OriginFor<T>,
			agreement_id: T::Hash,
			expected_version: u64,
		) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let finalized = Self::finalized_checkpoint()?;
			let mut agreement =
				Agreements::<T>::get(agreement_id).ok_or(Error::<T>::AgreementNotFound)?;
			ensure!(agreement.primary == provider, Error::<T>::NotAgreementParty);
			ensure!(
				agreement.version == expected_version
					&& agreement.status == AgreementStatus::Proposed,
				Error::<T>::AgreementInvalidState
			);
			ensure!(
				agreement.expires_at > frame_system::Pallet::<T>::block_number(),
				Error::<T>::InvalidExpiry
			);
			Self::ensure_provider_eligible(&agreement.primary, finalized)?;
			for replica in agreement.replicas.iter() {
				Self::ensure_provider_eligible(replica, finalized)?;
			}
			Self::allocate_capacity(&agreement.primary, agreement.bytes)?;
			for replica in agreement.replicas.iter() {
				Self::allocate_capacity(replica, agreement.bytes)?;
			}
			let previous_version = agreement.version;
			agreement.version = agreement.version.saturating_add(1);
			agreement.status = AgreementStatus::Active;
			agreement.capacity_state = AgreementCapacityState::Allocated;
			Agreements::<T>::insert(agreement_id, &agreement);
			Self::deposit_event(Event::AgreementTransitioned {
				agreement_id,
				previous: Some(AgreementStatus::Proposed),
				current: AgreementStatus::Active,
				previous_version,
				new_version: agreement.version,
			});
			Ok(())
		}

		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::set_agreement_suspension())]
		pub fn set_agreement_suspension(
			origin: OriginFor<T>,
			agreement_id: T::Hash,
			expected_version: u64,
			suspended: bool,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let (previous, previous_version, new_version) =
				Agreements::<T>::try_mutate(agreement_id, |maybe| -> Result<_, DispatchError> {
					let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;
					ensure!(
						agreement.version == expected_version,
						Error::<T>::AgreementInvalidState
					);
					let expected = if suspended {
						AgreementStatus::Active
					} else {
						AgreementStatus::Suspended
					};
					let next = if suspended {
						AgreementStatus::Suspended
					} else {
						AgreementStatus::Active
					};
					ensure!(agreement.status == expected, Error::<T>::AgreementInvalidState);
					if !suspended {
						let finalized = Self::finalized_checkpoint()?;
						Self::ensure_provider_eligible(&agreement.primary, finalized)?;
						for replica in agreement.replicas.iter() {
							Self::ensure_provider_eligible(replica, finalized)?;
						}
					}
					let previous_version = agreement.version;
					agreement.status = next;
					agreement.version = agreement.version.saturating_add(1);
					Ok((expected, previous_version, agreement.version))
				})?;
			let current =
				if suspended { AgreementStatus::Suspended } else { AgreementStatus::Active };
			Self::deposit_event(Event::AgreementTransitioned {
				agreement_id,
				previous: Some(previous),
				current,
				previous_version,
				new_version,
			});
			Ok(())
		}

		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::terminate_agreement())]
		#[transactional]
		pub fn terminate_agreement(
			origin: OriginFor<T>,
			agreement_id: T::Hash,
			expected_version: u64,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			let agreement =
				Agreements::<T>::get(agreement_id).ok_or(Error::<T>::AgreementNotFound)?;
			ensure!(
				caller == agreement.owner || caller == agreement.primary,
				Error::<T>::NotAgreementParty
			);
			Self::make_terminal(agreement_id, expected_version, AgreementStatus::Cancelled)
		}

		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::expire_agreement())]
		#[transactional]
		pub fn expire_agreement(
			origin: OriginFor<T>,
			agreement_id: T::Hash,
			expected_version: u64,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			let agreement =
				Agreements::<T>::get(agreement_id).ok_or(Error::<T>::AgreementNotFound)?;
			ensure!(
				frame_system::Pallet::<T>::block_number() >= agreement.expires_at,
				Error::<T>::InvalidExpiry
			);
			Self::make_terminal(agreement_id, expected_version, AgreementStatus::Expired)
		}

		#[pallet::call_index(14)]
		#[pallet::weight(
			T::WeightInfo::submit_checkpoint(confirmations.len() as u32).saturating_add(
				T::WeightInfo::promote_checkpoint_fallback(
					BucketAgreements::<T>::decode_len(payload.bucket_id).unwrap_or(0) as u32,
				)
			).saturating_add(Weight::from_parts(5_000_000, 64))
		)]
		#[transactional]
		pub fn submit_checkpoint(
			origin: OriginFor<T>,
			domain: BoundedVec<u8, frame_support::traits::ConstU32<64>>,
			payload: CommitmentPayloadV2<T::Hash, BlockNumberFor<T>>,
			window_start: BlockNumberFor<T>,
			window_end: BlockNumberFor<T>,
			service_key: ed25519::Public,
			primary_signature: ed25519::Signature,
			primary_context_signature: ed25519::Signature,
			confirmations: ConfirmationsOf<T>,
		) -> DispatchResult {
			let primary = ensure_signed(origin)?;
			ensure!(
				domain.as_slice() == CHECKPOINT_DOMAIN,
				Error::<T>::StorageCheckpointWrongDomain
			);
			ensure!(payload.version == 2, Error::<T>::StorageCheckpointWrongVersion);
			let claim_key = T::Hashing::hash_of(&(payload.nonce, payload.commitment.start_seq));
			let call_digest = sp_io::hashing::blake2_256(
				&(
					b"cord/storage/checkpoint-call/v2",
					&primary,
					&domain,
					payload,
					window_start,
					window_end,
					service_key,
					primary_signature,
					primary_context_signature,
					&confirmations,
				)
					.encode(),
			);
			let submitted_claim = CheckpointClaim {
				primary: primary.clone(),
				payload,
				call_digest,
				primary_signature,
				primary_context_signature,
				replica_confirmations: confirmations.clone(),
			};
			if CheckpointClaims::<T>::get(payload.bucket_id, claim_key).as_ref()
				== Some(&submitted_claim)
			{
				return Ok(());
			}
			let mut bucket = Buckets::<T>::get(payload.bucket_id)
				.ok_or(Error::<T>::StorageCheckpointWrongBucket)?;
			let finalized = Self::finalized_checkpoint()?;
			ensure!(
				payload.nonce <= finalized
					&& finalized.saturating_sub(payload.nonce) <= T::MaxCheckpointAge::get(),
				Error::<T>::StorageCheckpointStaleNonce
			);
			let duty = Self::checkpoint_duty_at(payload.bucket_id, payload.nonce)
				.ok_or(Error::<T>::StorageCheckpointWrongWindow)?;
			ensure!(
				bucket.primary == duty.primary && bucket.replicas == duty.replicas,
				Error::<T>::StorageCheckpointWrongWindow
			);
			ensure!(
				window_start == duty.due_at
					&& window_end == duty.grace_until
					&& finalized >= window_start,
				Error::<T>::StorageCheckpointWrongWindow
			);
			let fallback =
				duty.mode == CheckpointDutyMode::Standard && finalized >= duty.grace_until;
			let mut post_promotion_replicas = duty.replicas.clone();
			if fallback {
				let mut candidates = duty
					.replicas
					.iter()
					.filter(|candidate| Self::checkpoint_signer_eligible(candidate, finalized))
					.filter_map(|candidate| {
						let confirmed = ReplicaCheckpoint::<T>::get(payload.bucket_id, candidate);
						if duty.previous_commitment.is_some()
							&& confirmed != Some(duty.previous_checkpoint)
						{
							return None;
						}
						Some((confirmed, candidate.encode(), candidate.clone()))
					})
					.collect::<Vec<_>>();
				candidates
					.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
				let promoted = candidates
					.first()
					.map(|(_, _, provider)| provider.clone())
					.ok_or(Error::<T>::InsufficientFallbackQuorum)?;
				ensure!(primary == promoted, Error::<T>::StorageCheckpointWrongKey);
				let promoted_index = post_promotion_replicas
					.iter()
					.position(|provider| provider == &promoted)
					.ok_or(Error::<T>::StorageCheckpointWrongKey)?;
				post_promotion_replicas[promoted_index] = duty.primary.clone();
				ensure!(
					post_promotion_replicas
						.iter()
						.filter(|provider| Self::checkpoint_signer_eligible(provider, finalized))
						.count() >= 2,
					Error::<T>::StorageCheckpointInsufficientQuorum
				);
			} else {
				ensure!(duty.primary == primary, Error::<T>::StorageCheckpointWrongKey);
			}
			let mut provider =
				Providers::<T>::get(&primary).ok_or(Error::<T>::StorageCheckpointWrongKey)?;
			Self::activate_pending_key(&primary, &mut provider, finalized);
			Providers::<T>::insert(&primary, &provider);
			ensure!(
				provider.status == ProviderStatus::Active
					&& provider.service_key.active == service_key,
				Error::<T>::StorageCheckpointWrongKey
			);
			Self::ensure_authority(&primary, &provider.organization, &service_key, finalized)
				.map_err(|_| Error::<T>::StorageCheckpointWrongKey)?;
			ensure!(
				OverdueChallenges::<T>::get(&primary) == 0,
				Error::<T>::StorageCheckpointWrongKey
			);
			let digest = Self::checkpoint_digest(&domain, &payload);
			let context = Self::checkpoint_context(&duty, payload.nonce, digest);
			let context_digest = Self::checkpoint_context_digest(&context);
			ensure!(
				sp_io::crypto::ed25519_verify(&primary_signature, &digest, &service_key),
				Error::<T>::StorageCheckpointWrongKey
			);
			ensure!(
				sp_io::crypto::ed25519_verify(
					&primary_context_signature,
					&context_digest,
					&service_key
				),
				Error::<T>::StorageCheckpointWrongContext
			);
			if let Some(accepted) = CheckpointClaims::<T>::get(payload.bucket_id, claim_key) {
				if accepted.payload.commitment.mmr_root != payload.commitment.mmr_root {
					let conflicting = submitted_claim.clone();
					EquivocationEvidence::<T>::try_mutate(&primary, |items| -> DispatchResult {
						if !items.contains(&accepted) {
							items
								.try_push(accepted.clone())
								.map_err(|_| Error::<T>::EvidenceLimit)?;
						}
						items
							.try_push(conflicting.clone())
							.map_err(|_| Error::<T>::EvidenceLimit)?;
						Ok(())
					})?;
					Providers::<T>::mutate(&primary, |maybe| {
						if let Some(record) = maybe {
							record.status = ProviderStatus::Suspended;
						}
					});
					Self::deposit_event(Event::CheckpointEquivocation {
						code: CheckpointErrorCode::StorageCheckpointEquivocation as u16,
						bucket_id: accepted.payload.bucket_id,
						provider: primary,
						accepted_root: accepted.payload.commitment.mmr_root,
						conflicting_root: conflicting.payload.commitment.mmr_root,
						nonce: conflicting.payload.nonce,
					});
					// FRAME rolls all state and events back when a dispatchable returns `Err`. The
					// durable consensus result is therefore the code-bearing evidence event;
					// host/SDK adapters map that event to closed wire error 241.
					return Ok(());
				}
				return Err(Error::<T>::StorageCheckpointSequenceInvalid.into());
			}
			let expected_start = BucketSnapshots::<T>::get(payload.bucket_id)
				.and_then(|snapshot| snapshot.commitment.range_end())
				.unwrap_or(0);
			ensure!(
				payload.commitment.leaf_count > 0
					&& payload.commitment.start_seq == expected_start
					&& payload.commitment.start_seq == duty.expected_next_start_seq
					&& payload.commitment.range_end().is_some(),
				Error::<T>::StorageCheckpointSequenceInvalid
			);
			ensure!(confirmations.len() == 2, Error::<T>::StorageCheckpointInsufficientQuorum);
			let mut confirmed: ReplicasOf<T> = Default::default();
			let mut previous_provider: Option<Vec<u8>> = None;
			for confirmation in confirmations.iter() {
				let encoded_provider = confirmation.provider.encode();
				ensure!(
					post_promotion_replicas.contains(&confirmation.provider)
						&& confirmation.provider != primary
						&& previous_provider
							.as_ref()
							.is_none_or(|previous| previous < &encoded_provider)
						&& !confirmed.contains(&confirmation.provider),
					Error::<T>::StorageCheckpointInsufficientQuorum
				);
				previous_provider = Some(encoded_provider);
				let mut replica = Providers::<T>::get(&confirmation.provider)
					.ok_or(Error::<T>::StorageCheckpointInsufficientQuorum)?;
				Self::activate_pending_key(&confirmation.provider, &mut replica, finalized);
				Providers::<T>::insert(&confirmation.provider, &replica);
				ensure!(
					replica.status == ProviderStatus::Active
						&& replica.service_key.active == confirmation.service_key
						&& OverdueChallenges::<T>::get(&confirmation.provider) == 0,
					Error::<T>::StorageCheckpointInsufficientQuorum
				);
				Self::ensure_authority(
					&confirmation.provider,
					&replica.organization,
					&confirmation.service_key,
					finalized,
				)
				.map_err(|_| Error::<T>::StorageCheckpointInsufficientQuorum)?;
				ensure!(
					sp_io::crypto::ed25519_verify(
						&confirmation.signature,
						&digest,
						&confirmation.service_key
					),
					Error::<T>::StorageCheckpointInsufficientQuorum
				);
				ensure!(
					sp_io::crypto::ed25519_verify(
						&confirmation.context_signature,
						&context_digest,
						&confirmation.service_key
					),
					Error::<T>::StorageCheckpointWrongContext
				);
				confirmed
					.try_push(confirmation.provider.clone())
					.map_err(|_| Error::<T>::StorageCheckpointInsufficientQuorum)?;
			}
			if fallback {
				let old_primary = bucket.primary.clone();
				bucket.primary = primary.clone();
				bucket.replicas = post_promotion_replicas.clone();
				bucket.version = bucket.version.saturating_add(1);
				Self::rebind_failover_agreements(payload.bucket_id, &old_primary, &primary)?;
				Buckets::<T>::insert(payload.bucket_id, &bucket);
				Self::deposit_event(Event::ReplicaSelected {
					bucket_id: payload.bucket_id,
					provider: primary.clone(),
					checkpoint: duty.previous_checkpoint,
				});
				Self::deposit_event(Event::PrimaryPromoted {
					bucket_id: payload.bucket_id,
					old_provider: old_primary,
					new_provider: primary.clone(),
					checkpoint: duty.previous_checkpoint,
				});
			}
			CheckpointClaims::<T>::insert(payload.bucket_id, claim_key, submitted_claim);
			for replica in confirmed.iter() {
				ReplicaCheckpoint::<T>::insert(payload.bucket_id, replica, finalized);
			}
			BucketSnapshots::<T>::insert(
				payload.bucket_id,
				BucketSnapshot {
					commitment: payload.commitment,
					checkpoint_block: finalized,
					primary_signers: 1,
					commitment_nonce: payload.nonce,
					replica_confirmations: confirmed.clone(),
				},
			);
			let due_at = finalized.saturating_add(T::CheckpointCadence::get());
			Self::stage_checkpoint_duty(
				payload.bucket_id,
				&bucket.primary,
				&bucket.replicas,
				finalized,
				due_at,
				CheckpointDutyMode::Standard,
				None,
			)?;
			Self::deposit_event(Event::CheckpointAccepted {
				bucket_id: payload.bucket_id,
				commitment: payload.commitment,
				checkpoint: finalized,
				replica_confirmations: confirmed,
			});
			Ok(())
		}

		#[pallet::call_index(15)]
		#[pallet::weight(T::WeightInfo::issue_challenge())]
		#[transactional]
		pub fn issue_challenge(
			origin: OriginFor<T>,
			bucket_id: T::Hash,
			provider: T::AccountId,
			location: ChunkLocationV1,
			due_at: BlockNumberFor<T>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let snapshot =
				BucketSnapshots::<T>::get(bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			let bucket = Buckets::<T>::get(bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			ensure!(
				provider == bucket.primary || bucket.replicas.contains(&provider),
				Error::<T>::ProviderIneligible
			);
			let now = frame_system::Pallet::<T>::block_number();
			ensure!(due_at > now, Error::<T>::InvalidExpiry);
			let challenge_id = T::Hashing::hash_of(&(
				b"cord/storage/challenge/v1",
				bucket_id,
				&provider,
				location,
				due_at,
			));
			ensure!(
				!Challenges::<T>::contains_key(challenge_id),
				Error::<T>::ChallengeAlreadyExists
			);

			Self::reserve_duty_admission(Error::<T>::ChallengeDutyLimit)?;
			ChallengeBacklog::<T>::try_mutate(|ids| {
				ids.try_push(challenge_id).map_err(|_| Error::<T>::ChallengeDutyLimit)
			})?;
			Challenges::<T>::insert(
				challenge_id,
				ChallengeRecord {
					bucket_id,
					provider: provider.clone(),
					expected_commitment: snapshot.commitment,
					location,
					due_at,
					status: ChallengeStatus::Open,
				},
			);
			Self::deposit_event(Event::ChallengeIssued {
				challenge_id,
				bucket_id,
				provider,
				due_at,
			});
			Ok(())
		}

		#[pallet::call_index(16)]
		#[pallet::weight(T::WeightInfo::submit_challenge_proof(proof.peaks.len() as u32 + proof.leaf_proof.len() as u32))]
		#[transactional]
		pub fn submit_challenge_proof(
			origin: OriginFor<T>,
			challenge_id: T::Hash,
			proof: MmrProofOf<T>,
		) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let challenge =
				Challenges::<T>::get(challenge_id).ok_or(Error::<T>::ChallengeNotFound)?;
			ensure!(challenge.status == ChallengeStatus::Open, Error::<T>::ChallengeNotOpen);
			ensure!(challenge.provider == provider, Error::<T>::NotAgreementParty);
			ensure!(
				frame_system::Pallet::<T>::block_number() <= challenge.due_at,
				Error::<T>::ChallengeExpired
			);
			ensure!(
				Self::verify_mmr_proof(
					&proof,
					challenge.location.leaf_index,
					challenge.expected_commitment.mmr_root
				),
				Error::<T>::ProofInvalid
			);
			Challenges::<T>::mutate(challenge_id, |maybe| {
				if let Some(record) = maybe {
					record.status = ChallengeStatus::Proved;
				}
			});
			Self::remove_challenge_backlog(challenge_id);
			Self::deposit_event(Event::ChallengeProved { challenge_id, provider });
			Ok(())
		}

		#[pallet::call_index(18)]
		#[pallet::weight(T::WeightInfo::reconcile_bucket(
			replicas_bound::<T>(),
			BucketAgreements::<T>::decode_len(bucket_id).unwrap_or(0) as u32,
		))]
		pub fn reconcile_bucket(origin: OriginFor<T>, bucket_id: T::Hash) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			Self::reconcile_one_bucket(bucket_id, Self::finalized_checkpoint()?)
		}

		/// Admit a manifest into the pending control plane. This records no bytes and does not make
		/// the manifest publishable.
		#[pallet::call_index(19)]
		#[pallet::weight(T::WeightInfo::register_manifest())]
		pub fn register_manifest(
			origin: OriginFor<T>,
			bucket_id: T::Hash,
			expected_bucket_version: u64,
			manifest: CanonicalCommitment,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let bucket = Buckets::<T>::get(bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			ensure!(bucket.owner == owner, Error::<T>::NotBucketOwner);
			ensure!(bucket.version == expected_bucket_version, Error::<T>::BucketVersionConflict);
			ensure!(
				!CanonicalManifests::<T>::contains_key(manifest),
				Error::<T>::ManifestAlreadyExists
			);
			CanonicalManifests::<T>::insert(
				manifest,
				CanonicalManifestRecord {
					bucket_id,
					provider_commitment: None,
					state: CommitmentState::Pending,
					checkpoint: None,
					tombstoned_at: None,
				},
			);
			Self::deposit_event(Event::ManifestCommitmentChanged {
				manifest,
				bucket_id,
				state: CommitmentState::Pending,
				checkpoint: None,
			});
			Ok(())
		}

		/// Bind a pending manifest to a leaf in the latest finalized quorum checkpoint.
		#[pallet::call_index(20)]
		#[pallet::weight(T::WeightInfo::publish_manifest(proof.peaks.len() as u32 + proof.leaf_proof.len() as u32))]
		pub fn publish_manifest(
			origin: OriginFor<T>,
			manifest: CanonicalCommitment,
			leaf_index: u64,
			proof: MmrProofOf<T>,
		) -> DispatchResult {
			let primary = ensure_signed(origin)?;
			let mut record =
				CanonicalManifests::<T>::get(manifest).ok_or(Error::<T>::ManifestNotFound)?;
			ensure!(record.state == CommitmentState::Pending, Error::<T>::ManifestInvalidState);
			let bucket = Buckets::<T>::get(record.bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			ensure!(bucket.primary == primary, Error::<T>::NotAgreementParty);
			let snapshot = BucketSnapshots::<T>::get(record.bucket_id)
				.ok_or(Error::<T>::ManifestInvalidState)?;
			ensure!(
				snapshot.replica_confirmations.len() >= 2,
				Error::<T>::StorageCheckpointInsufficientQuorum
			);
			let end = snapshot
				.commitment
				.range_end()
				.ok_or(Error::<T>::StorageCheckpointSequenceInvalid)?;
			ensure!(
				leaf_index >= snapshot.commitment.start_seq && leaf_index < end,
				Error::<T>::StorageCheckpointSequenceInvalid
			);
			ensure!(
				Self::verify_mmr_proof(&proof, leaf_index, snapshot.commitment.mmr_root),
				Error::<T>::ProofInvalid
			);
			let provider_commitment = Self::hash_commitment(&proof.leaf.data_root)
				.ok_or(Error::<T>::ManifestCommitmentMismatch)?;
			record.provider_commitment = Some(provider_commitment);
			record.state = CommitmentState::Publishable;
			record.checkpoint = Some(snapshot.checkpoint_block);
			CanonicalManifests::<T>::insert(manifest, &record);
			Self::deposit_event(Event::ManifestCommitmentChanged {
				manifest,
				bucket_id: record.bucket_id,
				state: record.state,
				checkpoint: record.checkpoint,
			});
			Ok(())
		}

		#[pallet::call_index(21)]
		#[pallet::weight(T::WeightInfo::tombstone_manifest(replicas_bound::<T>()))]
		pub fn tombstone_manifest(
			origin: OriginFor<T>,
			manifest: CanonicalCommitment,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let mut record =
				CanonicalManifests::<T>::get(manifest).ok_or(Error::<T>::ManifestNotFound)?;
			let bucket = Buckets::<T>::get(record.bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			ensure!(bucket.owner == owner, Error::<T>::NotBucketOwner);
			ensure!(
				matches!(record.state, CommitmentState::Pending | CommitmentState::Publishable),
				Error::<T>::ManifestInvalidState
			);
			record.state = CommitmentState::Tombstoned;
			record.tombstoned_at = Some(Self::finalized_checkpoint()?);
			let mut required: AssignedProvidersOf<T> = Default::default();
			if record.provider_commitment.is_some() {
				required
					.try_push(bucket.primary.clone())
					.map_err(|_| Error::<T>::InvalidReplicaCount)?;
				for provider in bucket.replicas.iter() {
					if !required.contains(provider) {
						required
							.try_push(provider.clone())
							.map_err(|_| Error::<T>::InvalidReplicaCount)?;
					}
				}
			}
			for provider in required.iter() {
				ManifestDeletionDuties::<T>::insert(provider, manifest, ());
			}
			ManifestDeletionRequirements::<T>::insert(manifest, required);
			CanonicalManifests::<T>::insert(manifest, &record);
			Self::deposit_event(Event::ManifestCommitmentChanged {
				manifest,
				bucket_id: record.bucket_id,
				state: record.state,
				checkpoint: record.checkpoint,
			});
			Ok(())
		}

		/// Record signed evidence that an assigned replica no longer serves a tombstoned manifest.
		#[pallet::call_index(23)]
		#[pallet::weight(T::WeightInfo::acknowledge_manifest_deletion())]
		pub fn acknowledge_manifest_deletion(
			origin: OriginFor<T>,
			manifest: CanonicalCommitment,
			evidence_hash: T::Hash,
			service_key: ed25519::Public,
			signature: ed25519::Signature,
		) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let record =
				CanonicalManifests::<T>::get(manifest).ok_or(Error::<T>::ManifestNotFound)?;
			ensure!(record.state == CommitmentState::Tombstoned, Error::<T>::ManifestInvalidState);
			ensure!(
				ManifestDeletionRequirements::<T>::get(manifest).contains(&provider),
				Error::<T>::DeletionProviderNotRequired
			);
			if let Some(existing) =
				ManifestDeletionAcknowledgements::<T>::get(manifest, &provider)
			{
				if existing.provider == provider &&
					existing.bucket_id == record.bucket_id &&
					existing.manifest == manifest &&
					existing.evidence_hash == evidence_hash &&
					existing.service_key == service_key &&
					existing.signature == signature
				{
					return Ok(())
				}
				return Err(Error::<T>::DeletionAlreadyAcknowledged.into())
			}
			let finalized = Self::finalized_checkpoint()?;
			let mut provider_record =
				Providers::<T>::get(&provider).ok_or(Error::<T>::ProviderNotFound)?;
			Self::activate_pending_key(&provider, &mut provider_record, finalized);
			Providers::<T>::insert(&provider, &provider_record);
			ensure!(
				provider_record.service_key.active == service_key,
				Error::<T>::DeletionEvidenceInvalid
			);
			let tombstoned_at = record.tombstoned_at.ok_or(Error::<T>::ManifestInvalidState)?;
			let digest = T::Hashing::hash_of(&(
				b"cord/storage/deletion-ack/v1",
				record.bucket_id,
				manifest,
				evidence_hash,
				tombstoned_at,
			));
			let digest_bytes = digest.encode();
			ensure!(digest_bytes.len() == 32, Error::<T>::DeletionEvidenceInvalid);
			let mut digest_array = [0u8; 32];
			digest_array.copy_from_slice(&digest_bytes);
			ensure!(
				sp_io::crypto::ed25519_verify(&signature, &digest_array, &service_key),
				Error::<T>::DeletionEvidenceInvalid
			);
			let acknowledged_at = finalized;
			ManifestDeletionAcknowledgements::<T>::insert(
				manifest,
				&provider,
				DeletionAcknowledgement {
					provider: provider.clone(),
					bucket_id: record.bucket_id,
					manifest,
					evidence_hash,
					service_key,
					signature,
					acknowledged_at,
				},
			);
			ManifestDeletionDuties::<T>::remove(&provider, manifest);
			Self::deposit_event(Event::ManifestDeletionAcknowledged {
				manifest,
				bucket_id: record.bucket_id,
				provider,
				evidence_hash,
				acknowledged_at,
			});
			Ok(())
		}

		/// Replace an assigned replica after suspension/removal while preserving the bucket's exact
		/// one-primary and two-to-four-replica invariant.
		#[pallet::call_index(22)]
		#[pallet::weight(T::WeightInfo::replace_bucket_replica(
			BucketAgreements::<T>::decode_len(bucket_id).unwrap_or(0) as u32,
		))]
		#[transactional]
		pub fn replace_bucket_replica(
			origin: OriginFor<T>,
			bucket_id: T::Hash,
			expected_version: u64,
			old_provider: T::AccountId,
			new_provider: T::AccountId,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let finalized = Self::finalized_checkpoint()?;
			Self::ensure_provider_eligible(&new_provider, finalized)?;
			let mut bucket = Buckets::<T>::get(bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			ensure!(bucket.owner == owner, Error::<T>::NotBucketOwner);
			ensure!(bucket.version == expected_version, Error::<T>::BucketVersionConflict);
			ensure!(
				bucket.primary != new_provider && !bucket.replicas.contains(&new_provider),
				Error::<T>::DuplicateProviderAssignment
			);
			let replica_index = bucket
				.replicas
				.iter()
				.position(|provider| provider == &old_provider)
				.ok_or(Error::<T>::ProviderIneligible)?;
			let old_index = BucketAgreements::<T>::get(bucket_id);
			let mut rebound = Vec::new();
			let mut pending = 0u64;
			let mut allocated = 0u64;
			for agreement_id in old_index.iter() {
				let agreement =
					Agreements::<T>::get(agreement_id).ok_or(Error::<T>::AgreementNotFound)?;
				ensure!(agreement.bucket_id == bucket_id, Error::<T>::AgreementInvalidState);
				if agreement.capacity_state == AgreementCapacityState::Released {
					continue;
				}
				ensure!(agreement.replicas.contains(&old_provider), Error::<T>::ProviderIneligible);
				if agreement.capacity_state == AgreementCapacityState::Pending {
					pending = pending
						.checked_add(agreement.bytes)
						.ok_or(Error::<T>::AgreementCapacityExceeded)?;
				} else if agreement.capacity_state == AgreementCapacityState::Allocated {
					allocated = allocated
						.checked_add(agreement.bytes)
						.ok_or(Error::<T>::AgreementCapacityExceeded)?;
				}
				rebound.push((*agreement_id, agreement));
			}
			let mut old_record =
				Providers::<T>::get(&old_provider).ok_or(Error::<T>::ProviderNotFound)?;
			ensure!(
				old_record.pending_bytes >= pending && old_record.allocated_bytes >= allocated,
				Error::<T>::AgreementCapacityExceeded
			);
			let mut new_record =
				Providers::<T>::get(&new_provider).ok_or(Error::<T>::ProviderNotFound)?;
			let next_pending = new_record
				.pending_bytes
				.checked_add(pending)
				.ok_or(Error::<T>::AgreementCapacityExceeded)?;
			let next_allocated = new_record
				.allocated_bytes
				.checked_add(allocated)
				.ok_or(Error::<T>::AgreementCapacityExceeded)?;
			ensure!(
				next_pending
					.checked_add(next_allocated)
					.is_some_and(|used| used <= new_record.capacity_bytes),
				Error::<T>::AgreementCapacityExceeded
			);
			ProviderAgreements::<T>::try_mutate(&new_provider, |ids| -> DispatchResult {
				for (agreement_id, _) in rebound.iter() {
					if !ids.contains(agreement_id) {
						ids.try_push(*agreement_id).map_err(|_| Error::<T>::AgreementIndexFull)?;
					}
				}
				Ok(())
			})?;
			for (agreement_id, mut agreement) in rebound.iter().cloned() {
				let index = agreement
					.replicas
					.iter()
					.position(|provider| provider == &old_provider)
					.ok_or(Error::<T>::ProviderIneligible)?;
				agreement.replicas[index] = new_provider.clone();
				agreement.version = agreement.version.saturating_add(1);
				Agreements::<T>::insert(agreement_id, &agreement);
				Self::deposit_event(Event::AgreementProviderRebound {
					agreement_id,
					old_provider: old_provider.clone(),
					new_provider: new_provider.clone(),
					status: agreement.status,
					bytes: agreement.bytes,
				});
			}
			ProviderAgreements::<T>::mutate(&old_provider, |ids| {
				ids.retain(|agreement_id| !rebound.iter().any(|(id, _)| id == agreement_id));
			});
			old_record.pending_bytes -= pending;
			old_record.allocated_bytes -= allocated;
			new_record.pending_bytes = next_pending;
			new_record.allocated_bytes = next_allocated;
			Providers::<T>::insert(&old_provider, old_record);
			Providers::<T>::insert(&new_provider, new_record);
			bucket.replicas[replica_index] = new_provider.clone();
			let previous_version = bucket.version;
			bucket.version = bucket.version.saturating_add(1);
			let new_version = bucket.version;
			Self::stage_bucket_authority_duty(bucket_id, &bucket.primary, &bucket.replicas)?;
			Buckets::<T>::insert(bucket_id, bucket);
			ProviderBucketAssignmentCount::<T>::mutate(&old_provider, |count| {
				*count = count.saturating_sub(1)
			});
			ProviderBucketAssignmentCount::<T>::mutate(&new_provider, |count| {
				*count = count.saturating_add(1)
			});
			Self::deposit_event(Event::BucketReplicaReplaced {
				bucket_id,
				old_provider,
				new_provider,
				previous_version,
				new_version,
			});
			Ok(())
		}

		/// Refresh every provider assigned to one bucket at the same governed finalized checkpoint.
		/// Invalid providers are suspended and cannot be promoted; failover is bounded to this
		/// bucket.
		#[pallet::call_index(24)]
		#[pallet::weight(Pallet::<T>::refresh_bucket_authority_weight(
			*bucket_id,
			replicas_bound::<T>(),
		))]
		#[transactional]
		pub fn refresh_bucket_authority(
			origin: OriginFor<T>,
			bucket_id: T::Hash,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			let bucket = Buckets::<T>::get(bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			let finalized = Self::finalized_checkpoint()?;
			let mut validated = 0u32;
			let mut suspended = 0u32;
			for provider in core::iter::once(&bucket.primary).chain(bucket.replicas.iter()) {
				let mut record =
					Providers::<T>::get(provider).ok_or(Error::<T>::ProviderNotFound)?;
				Self::activate_pending_key(provider, &mut record, finalized);
				let valid = Self::ensure_authority(
					provider,
					&record.organization,
					&record.service_key.active,
					finalized,
				)
				.is_ok();
				record.authority_validated_at = Some(finalized);
				if valid {
					validated = validated.saturating_add(1);
				} else {
					record.status = ProviderStatus::Suspended;
					suspended = suspended.saturating_add(1);
					Self::deposit_event(Event::ProviderStatusChanged {
						provider: provider.clone(),
						status: ProviderStatus::Suspended,
					});
				}
				Providers::<T>::insert(provider, record);
				Self::deposit_event(Event::ProviderAuthorityRefreshed {
					provider: provider.clone(),
					bucket_id,
					checkpoint: finalized,
					valid,
				});
			}
			Self::reconcile_one_bucket(bucket_id, finalized)?;
			Self::deposit_event(Event::BucketAuthorityRefreshed {
				bucket_id,
				checkpoint: finalized,
				validated,
				suspended,
			});
			Ok(())
		}

		/// Advance the storage control plane's governed finalized checkpoint.
		#[pallet::call_index(25)]
		#[pallet::weight(T::WeightInfo::advance_finalized_checkpoint())]
		#[transactional]
		pub fn advance_finalized_checkpoint(
			origin: OriginFor<T>,
			checkpoint: BlockNumberFor<T>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			ensure!(
				checkpoint <= frame_system::Pallet::<T>::block_number(),
				Error::<T>::FinalizedCheckpointInFuture
			);
			let previous = GovernedFinalizedCheckpoint::<T>::get();
			ensure!(
				previous.is_none_or(|current| checkpoint > current),
				Error::<T>::FinalizedCheckpointNotMonotonic
			);
			GovernedFinalizedCheckpoint::<T>::put(checkpoint);
			Self::deposit_event(Event::FinalizedCheckpointAdvanced {
				previous,
				current: checkpoint,
			});
			Ok(())
		}

		/// Promote the deterministic grace fallback without publishing a checkpoint. The resulting
		/// recovery duty is visible only at the next governed finalized snapshot. Its predecessor
		/// is retained as audit data and remains eligible to confirm if it becomes valid again.
		#[pallet::call_index(26)]
		#[pallet::weight(T::WeightInfo::promote_checkpoint_fallback(
			BucketAgreements::<T>::decode_len(payload.bucket_id).unwrap_or(0) as u32,
		))]
		#[transactional]
		pub fn promote_checkpoint_fallback(
			origin: OriginFor<T>,
			payload: CheckpointFallbackPromotionV1<T::Hash, BlockNumberFor<T>>,
			service_key: ed25519::Public,
			signature: ed25519::Signature,
		) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			if CheckpointFallbackPromotionReceiptByBucket::<T>::get(payload.bucket_id).is_some_and(
				|receipt| {
					receipt.payload == payload
						&& receipt.provider == provider
						&& receipt.service_key == service_key
						&& receipt.signature == signature
				},
			) {
				return Ok(());
			}
			ensure!(payload.version == 1, Error::<T>::CheckpointFallbackPromotionWrongVersion);
			let finalized = Self::finalized_checkpoint()?;
			ensure!(
				payload.snapshot_nonce == finalized,
				Error::<T>::CheckpointFallbackPromotionWrongDuty
			);
			let duty = Self::checkpoint_duty_at(payload.bucket_id, payload.snapshot_nonce)
				.ok_or(Error::<T>::CheckpointFallbackPromotionWrongDuty)?;
			ensure!(
				duty.mode == CheckpointDutyMode::Standard
					&& payload.snapshot_nonce >= duty.grace_until,
				Error::<T>::CheckpointFallbackPromotionNotAllowed
			);
			ensure!(
				Self::checkpoint_duty_id(&duty, payload.snapshot_nonce) == payload.duty_id,
				Error::<T>::CheckpointFallbackPromotionWrongDuty
			);
			let mut bucket = Buckets::<T>::get(payload.bucket_id)
				.ok_or(Error::<T>::CheckpointFallbackPromotionWrongDuty)?;
			ensure!(
				bucket.primary == duty.primary && bucket.replicas == duty.replicas,
				Error::<T>::CheckpointFallbackPromotionWrongDuty
			);
			let mut candidates = duty
				.replicas
				.iter()
				.filter(|candidate| Self::checkpoint_signer_eligible(candidate, finalized))
				.filter_map(|candidate| {
					let confirmed = ReplicaCheckpoint::<T>::get(payload.bucket_id, candidate);
					if duty.previous_commitment.is_some()
						&& confirmed != Some(duty.previous_checkpoint)
					{
						return None;
					}
					Some((confirmed, candidate.encode(), candidate.clone()))
				})
				.collect::<Vec<_>>();
			candidates
				.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
			let promoted = candidates
				.first()
				.map(|(_, _, provider)| provider.clone())
				.ok_or(Error::<T>::CheckpointFallbackPromotionNotAllowed)?;
			ensure!(provider == promoted, Error::<T>::CheckpointFallbackPromotionWrongKey);
			let mut provider_record = Providers::<T>::get(&provider)
				.ok_or(Error::<T>::CheckpointFallbackPromotionWrongKey)?;
			Self::activate_pending_key(&provider, &mut provider_record, finalized);
			Providers::<T>::insert(&provider, &provider_record);
			ensure!(
				provider_record.status == ProviderStatus::Active
					&& provider_record.service_key.active == service_key
					&& OverdueChallenges::<T>::get(&provider) == 0,
				Error::<T>::CheckpointFallbackPromotionWrongKey
			);
			Self::ensure_authority(
				&provider,
				&provider_record.organization,
				&service_key,
				finalized,
			)
			.map_err(|_| Error::<T>::CheckpointFallbackPromotionWrongKey)?;
			let digest = Self::checkpoint_promotion_digest(&payload);
			ensure!(
				sp_io::crypto::ed25519_verify(&signature, &digest, &service_key),
				Error::<T>::CheckpointFallbackPromotionWrongKey
			);
			let old_primary = bucket.primary.clone();
			let promoted_index = bucket
				.replicas
				.iter()
				.position(|candidate| candidate == &promoted)
				.ok_or(Error::<T>::CheckpointFallbackPromotionWrongDuty)?;
			bucket.replicas[promoted_index] = old_primary.clone();
			bucket.primary = promoted.clone();
			bucket.version = bucket.version.saturating_add(1);
			Self::rebind_failover_agreements(payload.bucket_id, &old_primary, &promoted)?;
			Self::stage_checkpoint_duty(
				payload.bucket_id,
				&bucket.primary,
				&bucket.replicas,
				duty.previous_checkpoint,
				finalized,
				CheckpointDutyMode::PromotionPending,
				Some(old_primary.clone()),
			)?;
			Buckets::<T>::insert(payload.bucket_id, bucket);
			CheckpointFallbackPromotionReceiptByBucket::<T>::insert(
				payload.bucket_id,
				CheckpointFallbackPromotionReceipt {
					payload,
					provider: promoted.clone(),
					service_key,
					signature,
				},
			);
			Self::deposit_event(Event::ReplicaSelected {
				bucket_id: payload.bucket_id,
				provider: promoted.clone(),
				checkpoint: duty.previous_checkpoint,
			});
			Self::deposit_event(Event::PrimaryPromoted {
				bucket_id: payload.bucket_id,
				old_provider: old_primary,
				new_provider: promoted.clone(),
				checkpoint: duty.previous_checkpoint,
			});
			Self::deposit_event(Event::CheckpointFallbackPromotionPendingQuorum {
				bucket_id: payload.bucket_id,
				provider: promoted,
				snapshot_nonce: payload.snapshot_nonce,
				duty_id: payload.duty_id,
			});
			Ok(())
		}

		/// Create the sole host-delegation authority for a provider capability grant.
		#[pallet::call_index(27)]
		#[pallet::weight(T::WeightInfo::create_host_delegation())]
		#[transactional]
		pub fn create_host_delegation(
			origin: OriginFor<T>,
			bucket_id: T::Hash,
			issuer_key_id: T::Hash,
			issuer_public_key: ed25519::Public,
			product_id: CapabilityProductIdOf<T>,
			methods: CapabilityMethodsOf<T>,
			cid: Option<CapabilityCidOf<T>>,
			max_bytes: u64,
			expires_at: BlockNumberFor<T>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let bucket = Buckets::<T>::get(bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			ensure!(bucket.owner == owner, Error::<T>::NotBucketOwner);
			Self::validate_capability_scope(&product_id, &methods, cid.as_ref(), max_bytes)?;
			let now = frame_system::Pallet::<T>::block_number();
			Self::validate_capability_lifetime(now, expires_at)?;
			let issuance_nonce = GrantNonce::<T>::get(&owner);
			let next_nonce = issuance_nonce.checked_add(1).ok_or(Error::<T>::GrantNonceOverflow)?;
			let grant_id = Self::host_delegation_id(&owner, bucket_id, issuance_nonce);
			ensure!(
				!HostDelegations::<T>::contains_key(grant_id),
				Error::<T>::HostDelegationAlreadyExists
			);
			BucketHostDelegations::<T>::try_mutate(bucket_id, |ids| {
				ids.try_push(grant_id).map_err(|_| Error::<T>::HostDelegationLimit)
			})?;
			HostDelegations::<T>::insert(
				grant_id,
				HostDelegationRecord {
					bucket_id,
					owner: owner.clone(),
					issuance_nonce,
					issuer_key_id,
					issuer_public_key,
					key_version: 1,
					state_version: 1,
					key_activated_at: now,
					product_id,
					methods,
					cid,
					max_bytes,
					issued_at: now,
					expires_at,
					revoked_at: None,
				},
			);
			GrantNonce::<T>::insert(&owner, next_nonce);
			Self::deposit_event(Event::HostDelegationCreated {
				grant_id,
				bucket_id,
				owner,
				issuance_nonce,
				state_version: 1,
			});
			Ok(())
		}

		/// Rotate only the host delegation key; its scope and expiry remain immutable.
		#[pallet::call_index(28)]
		#[pallet::weight(T::WeightInfo::rotate_host_delegation())]
		pub fn rotate_host_delegation(
			origin: OriginFor<T>,
			grant_id: T::Hash,
			expected_version: u64,
			new_key_id: T::Hash,
			new_public_key: ed25519::Public,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			HostDelegations::<T>::try_mutate(grant_id, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::HostDelegationNotFound)?;
				ensure!(record.owner == owner, Error::<T>::NotBucketOwner);
				ensure!(record.revoked_at.is_none(), Error::<T>::HostDelegationRevoked);
				ensure!(
					record.state_version == expected_version,
					Error::<T>::HostDelegationVersionConflict
				);
				let old_key_id = record.issuer_key_id;
				record.issuer_key_id = new_key_id;
				record.issuer_public_key = new_public_key;
				record.key_version = record
					.key_version
					.checked_add(1)
					.ok_or(Error::<T>::HostDelegationVersionConflict)?;
				record.state_version = record
					.state_version
					.checked_add(1)
					.ok_or(Error::<T>::HostDelegationVersionConflict)?;
				record.key_activated_at = now;
				Self::deposit_event(Event::HostDelegationKeyRotated {
					grant_id,
					old_key_id,
					new_key_id,
					key_version: record.key_version,
					state_version: record.state_version,
					key_activated_at: now,
				});
				Ok(())
			})
		}

		/// Revoke a host delegation permanently while retaining its tombstone.
		#[pallet::call_index(29)]
		#[pallet::weight(T::WeightInfo::revoke_host_delegation())]
		#[transactional]
		pub fn revoke_host_delegation(
			origin: OriginFor<T>,
			grant_id: T::Hash,
			expected_version: u64,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			let mut record =
				HostDelegations::<T>::get(grant_id).ok_or(Error::<T>::HostDelegationNotFound)?;
			ensure!(record.owner == owner, Error::<T>::NotBucketOwner);
			ensure!(record.revoked_at.is_none(), Error::<T>::HostDelegationRevoked);
			ensure!(
				record.state_version == expected_version,
				Error::<T>::HostDelegationVersionConflict
			);
			record.state_version = record
				.state_version
				.checked_add(1)
				.ok_or(Error::<T>::HostDelegationVersionConflict)?;
			record.revoked_at = Some(now);
			BucketHostDelegations::<T>::try_mutate(record.bucket_id, |ids| -> DispatchResult {
				let position = ids
					.iter()
					.position(|candidate| candidate == &grant_id)
					.ok_or(Error::<T>::HostDelegationNotFound)?;
				ids.remove(position);
				Ok(())
			})?;
			HostDelegations::<T>::insert(grant_id, &record);
			Self::deposit_event(Event::HostDelegationRevoked {
				grant_id,
				state_version: record.state_version,
				revoked_at: now,
			});
			Ok(())
		}
	}

	fn replicas_bound<T: Config>() -> u32 {
		T::MaxReplicas::get()
	}

	impl<T: Config> Pallet<T> {
		pub fn governed_finalized_checkpoint() -> Option<BlockNumberFor<T>> {
			GovernedFinalizedCheckpoint::<T>::get()
		}

		pub fn host_delegation_id(
			owner: &T::AccountId,
			bucket_id: T::Hash,
			issuance_nonce: u64,
		) -> T::Hash {
			T::Hashing::hash_of(&(
				b"cord/storage/host-delegation/v1",
				owner,
				bucket_id,
				issuance_nonce,
			))
		}

		fn validate_capability_scope(
			product_id: &CapabilityProductIdOf<T>,
			methods: &CapabilityMethodsOf<T>,
			cid: Option<&CapabilityCidOf<T>>,
			max_bytes: u64,
		) -> DispatchResult {
			ensure!(
				!product_id.is_empty() && core::str::from_utf8(product_id).is_ok(),
				Error::<T>::InvalidCapabilityScope
			);
			ensure!(
				!methods.is_empty() && methods.windows(2).all(|pair| pair[0] < pair[1]),
				Error::<T>::InvalidCapabilityScope
			);
			ensure!(
				cid.is_none_or(|value| !value.is_empty() && core::str::from_utf8(value).is_ok()),
				Error::<T>::InvalidCapabilityScope
			);
			ensure!(max_bytes > 0, Error::<T>::InvalidCapabilityScope);
			Ok(())
		}

		fn validate_capability_lifetime(
			issued_at: BlockNumberFor<T>,
			expires_at: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure!(expires_at > issued_at, Error::<T>::InvalidCapabilityLifetime);
			ensure!(
				expires_at <= issued_at.saturating_add(T::MaxHostDelegationLifetime::get()),
				Error::<T>::InvalidCapabilityLifetime
			);
			Ok(())
		}

		fn finalized_checkpoint() -> Result<BlockNumberFor<T>, DispatchError> {
			GovernedFinalizedCheckpoint::<T>::get()
				.ok_or(Error::<T>::FinalizedCheckpointUninitialized.into())
		}

		pub(crate) fn stage_checkpoint_duty(
			bucket_id: T::Hash,
			primary: &T::AccountId,
			replicas: &ReplicasOf<T>,
			previous_checkpoint: BlockNumberFor<T>,
			due_at: BlockNumberFor<T>,
			mode: CheckpointDutyMode,
			promotion_predecessor: Option<T::AccountId>,
		) -> DispatchResult {
			let scheduled_at = Self::finalized_checkpoint()?;
			Self::reserve_duty_admission(Error::<T>::CheckpointDutyLimit)?;
			let duty = CheckpointDutyRecord {
				bucket_id,
				primary: primary.clone(),
				replicas: replicas.clone(),
				previous_checkpoint,
				previous_commitment: BucketSnapshots::<T>::get(bucket_id)
					.map(|snapshot| snapshot.commitment),
				expected_next_start_seq: BucketSnapshots::<T>::get(bucket_id)
					.and_then(|snapshot| snapshot.commitment.range_end())
					.unwrap_or(0),
				due_at,
				grace_until: due_at.saturating_add(T::CheckpointGrace::get()),
				scheduled_at,
				mode,
				promotion_predecessor,
			};
			if let Some(previous) = CheckpointDutyPending::<T>::get(bucket_id) {
				let finalized = GovernedFinalizedCheckpoint::<T>::get()
					.ok_or(Error::<T>::FinalizedCheckpointUninitialized)?;
				ensure!(previous.scheduled_at < finalized, Error::<T>::CheckpointDutyPending);
				CheckpointDutyCurrent::<T>::insert(bucket_id, previous);
			}
			CheckpointDutyPending::<T>::insert(bucket_id, duty);
			Ok(())
		}

		fn reserve_duty_admission(error: Error<T>) -> DispatchResult {
			let admission_at = frame_system::Pallet::<T>::block_number();
			if DutyAdmissionBlock::<T>::get() != Some(admission_at) {
				DutyAdmissionBlock::<T>::put(admission_at);
				DutyAdmissionCount::<T>::put(0);
			}
			ensure!(DutyAdmissionCount::<T>::get() < T::MaxDutiesPerBlock::get(), error);
			DutyAdmissionCount::<T>::mutate(|count| *count = count.saturating_add(1));
			Ok(())
		}

		pub fn checkpoint_duty_at(
			bucket_id: T::Hash,
			snapshot_checkpoint: BlockNumberFor<T>,
		) -> Option<CheckpointDutyRecordOf<T>> {
			CheckpointDutyPending::<T>::get(bucket_id)
				.filter(|duty| duty.scheduled_at < snapshot_checkpoint)
				.or_else(|| CheckpointDutyCurrent::<T>::get(bucket_id))
		}

		pub fn checkpoint_duty_id(
			duty: &CheckpointDutyRecordOf<T>,
			snapshot_checkpoint: BlockNumberFor<T>,
		) -> T::Hash {
			T::Hashing::hash(&Self::checkpoint_duty_preimage(duty, snapshot_checkpoint))
		}

		pub fn checkpoint_duty_preimage(
			duty: &CheckpointDutyRecordOf<T>,
			snapshot_checkpoint: BlockNumberFor<T>,
		) -> Vec<u8> {
			(
				b"cord/storage/checkpoint-duty/v2",
				T::CheckpointContext::genesis_hash(),
				T::CheckpointContext::spec_version(),
				T::CheckpointContext::transaction_version(),
				T::CheckpointContext::metadata_hash(),
				snapshot_checkpoint,
				T::CheckpointContext::block_hash(snapshot_checkpoint),
				duty.bucket_id,
				&duty.primary,
				&duty.replicas,
				duty.previous_checkpoint,
				duty.previous_commitment,
				duty.expected_next_start_seq,
				duty.due_at,
				duty.grace_until,
				duty.mode,
				&duty.promotion_predecessor,
			)
				.encode()
		}

		fn checkpoint_context(
			duty: &CheckpointDutyRecordOf<T>,
			snapshot_checkpoint: BlockNumberFor<T>,
			v2_digest: [u8; 32],
		) -> CheckpointContextV1<T::Hash> {
			CheckpointContextV1 {
				version: 1,
				genesis_hash: T::CheckpointContext::genesis_hash(),
				spec_version: T::CheckpointContext::spec_version(),
				transaction_version: T::CheckpointContext::transaction_version(),
				metadata_hash: T::CheckpointContext::metadata_hash(),
				finalized_hash: T::CheckpointContext::block_hash(snapshot_checkpoint),
				duty_id: Self::checkpoint_duty_id(duty, snapshot_checkpoint),
				v2_digest,
			}
		}

		pub fn checkpoint_context_for(
			payload: &CommitmentPayloadV2<T::Hash, BlockNumberFor<T>>,
		) -> Option<CheckpointContextV1<T::Hash>> {
			let duty = Self::checkpoint_duty_at(payload.bucket_id, payload.nonce)?;
			let digest = Self::checkpoint_digest(CHECKPOINT_DOMAIN, payload);
			Some(Self::checkpoint_context(&duty, payload.nonce, digest))
		}

		pub fn checkpoint_context_digest(context: &CheckpointContextV1<T::Hash>) -> [u8; 32] {
			let mut message = CHECKPOINT_CONTEXT_DOMAIN.to_vec();
			context.encode_to(&mut message);
			sp_io::hashing::blake2_256(&message)
		}

		pub(crate) fn refresh_bucket_authority_weight(
			bucket_id: T::Hash,
			replicas: u32,
		) -> frame_support::weights::Weight {
			let agreements = BucketAgreements::<T>::decode_len(bucket_id).unwrap_or(0) as u32;
			T::WeightInfo::refresh_bucket_authority_valid(replicas)
				.max(T::WeightInfo::refresh_bucket_authority_failover(replicas, agreements))
		}

		pub fn checkpoint_error_code(error: &Error<T>) -> Option<u16> {
			Some(match error {
				Error::StorageCheckpointWrongDomain => {
					CheckpointErrorCode::StorageCheckpointWrongDomain as u16
				},
				Error::StorageCheckpointWrongVersion => {
					CheckpointErrorCode::StorageCheckpointWrongVersion as u16
				},
				Error::StorageCheckpointWrongBucket => {
					CheckpointErrorCode::StorageCheckpointWrongBucket as u16
				},
				Error::StorageCheckpointWrongKey => {
					CheckpointErrorCode::StorageCheckpointWrongKey as u16
				},
				Error::StorageCheckpointStaleNonce => {
					CheckpointErrorCode::StorageCheckpointStaleNonce as u16
				},
				Error::StorageCheckpointWrongWindow => {
					CheckpointErrorCode::StorageCheckpointWrongWindow as u16
				},
				Error::StorageCheckpointInsufficientQuorum => {
					CheckpointErrorCode::StorageCheckpointInsufficientQuorum as u16
				},
				Error::StorageCheckpointSequenceInvalid => {
					CheckpointErrorCode::StorageCheckpointSequenceInvalid as u16
				},
				Error::StorageCheckpointEquivocation => {
					CheckpointErrorCode::StorageCheckpointEquivocation as u16
				},
				Error::StorageCheckpointWrongContext => {
					CheckpointErrorCode::StorageCheckpointWrongContext as u16
				},
				_ => return None,
			})
		}

		pub fn checkpoint_promotion_error_code(error: &Error<T>) -> Option<u16> {
			Some(match error {
				Error::CheckpointFallbackPromotionWrongVersion => {
					CheckpointFallbackPromotionErrorCode::WrongVersion as u16
				},
				Error::CheckpointFallbackPromotionWrongDuty => {
					CheckpointFallbackPromotionErrorCode::WrongDuty as u16
				},
				Error::CheckpointFallbackPromotionWrongKey => {
					CheckpointFallbackPromotionErrorCode::WrongKey as u16
				},
				Error::CheckpointFallbackPromotionNotAllowed => {
					CheckpointFallbackPromotionErrorCode::NotAllowed as u16
				},
				_ => return None,
			})
		}

		fn ensure_authority(
			provider: &T::AccountId,
			organization: &OrganizationRefOf<T>,
			service_key: &ed25519::Public,
			finalized_at: BlockNumberFor<T>,
		) -> DispatchResult {
			T::ProviderAuthority::validate(provider, organization, service_key, finalized_at)
				.map_err(|error| match error {
					ProviderAuthorityError::OrganizationUnknown => Error::<T>::ProviderOrgUnknown,
					ProviderAuthorityError::AttestationInvalid => {
						Error::<T>::ProviderAttestationInvalid
					},
					ProviderAuthorityError::AttestationExpired => {
						Error::<T>::ProviderAttestationExpired
					},
					ProviderAuthorityError::SlaInvalid => Error::<T>::ProviderSlaInvalid,
					ProviderAuthorityError::ServiceKeyInvalid => {
						Error::<T>::ProviderServiceKeyInvalid
					},
				})?;
			Ok(())
		}

		fn ensure_provider_eligible(
			provider: &T::AccountId,
			finalized_at: BlockNumberFor<T>,
		) -> DispatchResult {
			let record = Providers::<T>::get(provider).ok_or(Error::<T>::ProviderIneligible)?;
			ensure!(
				record.status == ProviderStatus::Active
					&& record.authority_validated_at == Some(finalized_at)
					&& OverdueChallenges::<T>::get(provider) == 0
					&& finalized_at >= record.organization.valid_from
					&& finalized_at < record.organization.valid_until,
				Error::<T>::ProviderIneligible
			);
			Ok(())
		}

		fn activate_pending_key(
			provider: &T::AccountId,
			record: &mut ProviderRecordOf<T>,
			finalized: BlockNumberFor<T>,
		) {
			if record.service_key.pending_effective_at.is_some_and(|at| at <= finalized) {
				if let Some(new_key) = record.service_key.pending.take() {
					let old_key = record.service_key.active;
					ServiceKeyOwner::<T>::remove(old_key);
					record.service_key.previous = Some(old_key);
					record.service_key.active = new_key;
					record.service_key.active_version = record
						.service_key
						.pending_version
						.take()
						.unwrap_or_else(|| record.service_key.active_version.saturating_add(1));
					record.service_key.pending_effective_at = None;
					record.authority_validated_at = None;
					Self::deposit_event(Event::ServiceKeyRotated {
						provider: provider.clone(),
						old_key,
						new_key,
						effective_at: finalized,
					});
				}
			}
		}

		fn ensure_assignments(primary: &T::AccountId, replicas: &ReplicasOf<T>) -> DispatchResult {
			ensure!(replicas.len() >= 2 && replicas.len() <= 4, Error::<T>::InvalidReplicaCount);
			ensure!(!replicas.contains(primary), Error::<T>::DuplicateProviderAssignment);
			for (index, replica) in replicas.iter().enumerate() {
				ensure!(
					!replicas[..index].contains(replica),
					Error::<T>::DuplicateProviderAssignment
				);
			}
			Ok(())
		}

		fn reserve_capacity(
			provider: &T::AccountId,
			bytes: u64,
			agreement_id: T::Hash,
		) -> DispatchResult {
			ProviderAgreements::<T>::try_mutate(provider, |ids| {
				ensure!(!ids.contains(&agreement_id), Error::<T>::AgreementAlreadyExists);
				ids.try_push(agreement_id).map_err(|_| Error::<T>::AgreementIndexFull)
			})?;
			Providers::<T>::try_mutate(provider, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::ProviderIneligible)?;
				let used = record
					.allocated_bytes
					.checked_add(record.pending_bytes)
					.and_then(|value| value.checked_add(bytes))
					.ok_or(Error::<T>::AgreementCapacityExceeded)?;
				ensure!(used <= record.capacity_bytes, Error::<T>::AgreementCapacityExceeded);
				record.pending_bytes = record.pending_bytes.saturating_add(bytes);
				Ok(())
			})
		}

		fn allocate_capacity(provider: &T::AccountId, bytes: u64) -> DispatchResult {
			Providers::<T>::try_mutate(provider, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::ProviderIneligible)?;
				ensure!(record.pending_bytes >= bytes, Error::<T>::AgreementCapacityExceeded);
				record.pending_bytes -= bytes;
				record.allocated_bytes = record
					.allocated_bytes
					.checked_add(bytes)
					.ok_or(Error::<T>::AgreementCapacityExceeded)?;
				Ok(())
			})
		}

		fn make_terminal(
			agreement_id: T::Hash,
			expected_version: u64,
			status: AgreementStatus,
		) -> DispatchResult {
			let now = frame_system::Pallet::<T>::block_number();
			let release_at = Self::capacity_release_block(now);
			let (previous, previous_version, new_version) =
				Agreements::<T>::try_mutate(agreement_id, |maybe| -> Result<_, DispatchError> {
					let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;
					ensure!(
						agreement.version == expected_version
							&& matches!(
								agreement.status,
								AgreementStatus::Proposed
									| AgreementStatus::Active | AgreementStatus::Suspended
							),
						Error::<T>::AgreementInvalidState
					);
					let previous = agreement.status;
					let previous_version = agreement.version;
					agreement.status = status;
					agreement.version = agreement.version.saturating_add(1);
					agreement.release_at = Some(release_at);
					Ok((previous, previous_version, agreement.version))
				})?;
			CapacityReleases::<T>::try_mutate(release_at, |ids| ids.try_push(agreement_id))
				.map_err(|_| Error::<T>::CapacityReleaseQueueFull)?;
			Self::deposit_event(Event::AgreementTransitioned {
				agreement_id,
				previous: Some(previous),
				current: status,
				previous_version,
				new_version,
			});
			Ok(())
		}

		fn capacity_release_block(now: BlockNumberFor<T>) -> BlockNumberFor<T> {
			let earliest = now.saturating_add(T::EvidenceWindow::get());
			let remainder = earliest.saturated_into::<u64>() % 3;
			let offset: BlockNumberFor<T> = (((3 - remainder) % 3) as u32).into();
			earliest.saturating_add(offset)
		}

		fn remove_challenge_backlog(challenge_id: T::Hash) {
			ChallengeBacklog::<T>::mutate(|ids| {
				if let Some(index) = ids.iter().position(|id| id == &challenge_id) {
					ids.swap_remove(index);
				}
			});
		}

		fn release_due_capacity(now: BlockNumberFor<T>) -> u32 {
			let ids = CapacityReleases::<T>::take(now);
			let mut released = 0u32;
			for agreement_id in ids {
				let Some(mut agreement) = Agreements::<T>::get(agreement_id) else { continue };
				if !agreement.status.is_terminal() || agreement.release_at != Some(now) {
					continue;
				}
				let capacity_state = agreement.capacity_state;
				for provider in
					core::iter::once(&agreement.primary).chain(agreement.replicas.iter())
				{
					Providers::<T>::mutate(provider, |maybe| {
						if let Some(record) = maybe {
							match capacity_state {
								AgreementCapacityState::Pending => {
									record.pending_bytes =
										record.pending_bytes.saturating_sub(agreement.bytes);
								},
								AgreementCapacityState::Allocated => {
									record.allocated_bytes =
										record.allocated_bytes.saturating_sub(agreement.bytes);
								},
								AgreementCapacityState::Released => {},
							}
						}
					});
					ProviderAgreements::<T>::mutate(provider, |provider_ids| {
						if let Some(index) = provider_ids.iter().position(|id| id == &agreement_id)
						{
							provider_ids.remove(index);
						}
					});
				}
				BucketAgreements::<T>::mutate(agreement.bucket_id, |bucket_ids| {
					if let Some(index) = bucket_ids.iter().position(|id| id == &agreement_id) {
						bucket_ids.remove(index);
					}
				});
				agreement.release_at = None;
				agreement.capacity_state = AgreementCapacityState::Released;
				Agreements::<T>::insert(agreement_id, agreement);
				Self::deposit_event(Event::AgreementCapacityReleased { agreement_id });
				released = released.saturating_add(1);
			}
			released
		}

		fn checkpoint_digest(
			domain: &[u8],
			payload: &CommitmentPayloadV2<T::Hash, BlockNumberFor<T>>,
		) -> [u8; 32] {
			let mut message = domain.to_vec();
			payload.encode_to(&mut message);
			sp_io::hashing::blake2_256(&message)
		}

		pub fn checkpoint_promotion_digest(
			payload: &CheckpointFallbackPromotionV1<T::Hash, BlockNumberFor<T>>,
		) -> [u8; 32] {
			let mut message = CHECKPOINT_PROMOTION_DOMAIN.to_vec();
			payload.encode_to(&mut message);
			sp_io::hashing::blake2_256(&message)
		}

		fn checkpoint_signer_eligible(
			provider: &T::AccountId,
			finalized: BlockNumberFor<T>,
		) -> bool {
			Providers::<T>::get(provider).is_some_and(|record| {
				let service_key = Self::effective_service_key(&record, finalized);
				record.status == ProviderStatus::Active
					&& OverdueChallenges::<T>::get(provider) == 0
					&& Self::ensure_authority(
						provider,
						&record.organization,
						&service_key,
						finalized,
					)
					.is_ok()
			})
		}

		fn effective_service_key(
			record: &ProviderRecordOf<T>,
			finalized: BlockNumberFor<T>,
		) -> ed25519::Public {
			if record.service_key.pending_effective_at.is_some_and(|at| at <= finalized) {
				record.service_key.pending.unwrap_or(record.service_key.active)
			} else {
				record.service_key.active
			}
		}

		fn verify_mmr_proof(proof: &MmrProofOf<T>, mut index: u64, expected_root: T::Hash) -> bool {
			if proof.peaks.len().saturating_add(proof.leaf_proof.len())
				> T::MaxProofNodes::get() as usize
			{
				return false;
			}
			let mut current = T::Hashing::hash_of(&proof.leaf);
			for sibling in proof.leaf_proof.iter() {
				current = if index & 1 == 1 {
					T::Hashing::hash_of(&(*sibling, current))
				} else {
					T::Hashing::hash_of(&(current, *sibling))
				};
				index >>= 1;
			}
			if !proof.peaks.contains(&current) {
				return false;
			}
			proof
				.peaks
				.iter()
				.rev()
				.fold(None, |right: Option<T::Hash>, peak| {
					Some(match right {
						None => *peak,
						Some(value) => T::Hashing::hash_of(&(*peak, value)),
					})
				})
				.is_some_and(|root| root == expected_root)
		}

		fn hash_commitment(hash: &T::Hash) -> Option<CanonicalCommitment> {
			let encoded = hash.encode();
			if encoded.len() != 32 {
				return None;
			}
			let mut commitment = [0u8; 32];
			commitment.copy_from_slice(&encoded);
			Some(commitment)
		}

		fn timeout_one_challenge(challenge_id: T::Hash, now: BlockNumberFor<T>) -> DispatchResult {
			let mut challenge =
				Challenges::<T>::get(challenge_id).ok_or(Error::<T>::ChallengeNotFound)?;
			ensure!(challenge.status == ChallengeStatus::Open, Error::<T>::ChallengeNotOpen);
			ensure!(now > challenge.due_at, Error::<T>::ChallengeNotDue);
			let evidence_hash = T::Hashing::hash_of(&(
				b"cord/storage/challenge-timeout/v1",
				challenge_id,
				&challenge,
			));
			let evidence_recorded =
				ProviderEvidence::<T>::try_mutate(&challenge.provider, |items| {
					items.try_push(EvidenceRecord {
						provider: challenge.provider.clone(),
						bucket_id: challenge.bucket_id,
						evidence_hash,
						recorded_at: now,
					})
				})
				.is_ok();
			if !evidence_recorded {
				ProviderEvidenceOverflow::<T>::mutate(&challenge.provider, |count| {
					*count = count.saturating_add(1)
				});
				Self::deposit_event(Event::ChallengeEvidenceOverflowed {
					challenge_id,
					provider: challenge.provider.clone(),
					evidence_hash,
				});
			}
			challenge.status = ChallengeStatus::TimedOut;
			Challenges::<T>::insert(challenge_id, &challenge);
			OverdueChallenges::<T>::mutate(&challenge.provider, |count| {
				*count = count.saturating_add(1)
			});
			if evidence_recorded {
				Self::deposit_event(Event::EvidenceRecorded {
					provider: challenge.provider.clone(),
					bucket_id: challenge.bucket_id,
					evidence_hash,
				});
			}
			Providers::<T>::mutate(&challenge.provider, |maybe| {
				if let Some(record) = maybe {
					record.status = ProviderStatus::Suspended;
				}
			});
			Self::deposit_event(Event::ProviderIneligible {
				provider: challenge.provider.clone(),
				bucket_id: challenge.bucket_id,
				checkpoint: now,
			});
			Self::deposit_event(Event::ChallengeTimedOut {
				challenge_id,
				provider: challenge.provider,
				checkpoint: now,
			});
			Ok(())
		}

		fn process_due_challenges(_now: BlockNumberFor<T>) -> u32 {
			Self::process_due_challenges_with_limit(T::MaxChallengesPerBlock::get())
		}

		pub(crate) fn process_due_challenges_with_limit(processing_limit: u32) -> u32 {
			let finalized = GovernedFinalizedCheckpoint::<T>::get();
			let limit = processing_limit.min(T::MaxChallengesPerBlock::get()) as usize;
			let mut attempted = 0u32;
			ChallengeBacklog::<T>::mutate(|duties| {
				let attempts = core::cmp::min(limit, duties.len());
				let mut cursor = (ChallengeBacklogCursor::<T>::get() as usize)
					.min(duties.len().saturating_sub(1));
				for _ in 0..attempts {
					if duties.is_empty() {
						break;
					}
					cursor %= duties.len();
					let challenge_id = duties[cursor];
					attempted = attempted.saturating_add(1);
					let Some(challenge) = Challenges::<T>::get(challenge_id) else {
						duties.swap_remove(cursor);
						continue;
					};
					if challenge.status != ChallengeStatus::Open {
						duties.swap_remove(cursor);
						continue;
					}
					let ready = finalized.is_some_and(|checkpoint| checkpoint > challenge.due_at);
					if !ready {
						cursor = cursor.saturating_add(1);
						continue;
					}
					if let Some(checkpoint) = finalized {
						if Self::timeout_one_challenge(challenge_id, checkpoint).is_ok() {
							duties.swap_remove(cursor);
						} else {
							cursor = cursor.saturating_add(1);
						}
					}
				}
				ChallengeBacklogCursor::<T>::put(if duties.is_empty() {
					0
				} else {
					(cursor % duties.len()) as u32
				});
			});
			attempted
		}

		fn reconcile_finalized() -> (u32, u32) {
			let Some(finalized) = GovernedFinalizedCheckpoint::<T>::get() else { return (0, 0) };
			let provider_ids = ProviderIds::<T>::get();
			let bucket_ids = BucketIds::<T>::get();
			let limit = T::MaxReconciliationRecords::get();
			let provider_budget = limit.saturating_add(1) / 2;
			let mut processed = 0u32;
			let mut provider_cursor = ProviderReconciliationCursor::<T>::get() as usize;
			while processed < provider_budget && provider_cursor < provider_ids.len() {
				let provider = &provider_ids[provider_cursor];
				if let Some(mut record) = Providers::<T>::get(provider) {
					Self::activate_pending_key(provider, &mut record, finalized);
					if record.status == ProviderStatus::Active
						&& (finalized < record.organization.valid_from
							|| finalized >= record.organization.valid_until)
					{
						record.status = ProviderStatus::Suspended;
						Self::deposit_event(Event::ProviderStatusChanged {
							provider: provider.clone(),
							status: ProviderStatus::Suspended,
						});
					}
					Providers::<T>::insert(provider, record);
				}
				provider_cursor += 1;
				processed += 1;
			}
			ProviderReconciliationCursor::<T>::put(if provider_cursor >= provider_ids.len() {
				0
			} else {
				provider_cursor as u32
			});
			let mut bucket_cursor = BucketReconciliationCursor::<T>::get() as usize;
			let mut agreements = 0u32;
			while processed < limit && bucket_cursor < bucket_ids.len() {
				processed += 1;
				let bucket_id = bucket_ids[bucket_cursor];
				let primary_eligible = Buckets::<T>::get(bucket_id).is_some_and(|bucket| {
					Self::ensure_provider_eligible(&bucket.primary, finalized).is_ok()
				});
				if primary_eligible {
					bucket_cursor += 1;
					continue;
				}
				agreements = BucketAgreements::<T>::decode_len(bucket_id).unwrap_or(0) as u32;
				if let Err(error) = Self::reconcile_one_bucket(bucket_id, finalized) {
					let reason = if error == Error::<T>::CheckpointDutyPending.into() {
						ReconciliationDeferReason::PendingDuty
					} else if error == Error::<T>::ProviderIneligible.into() {
						ReconciliationDeferReason::NoEligibleReplica
					} else {
						ReconciliationDeferReason::InvariantFault
					};
					Self::deposit_event(Event::BucketReconciliationDeferred {
						bucket_id,
						checkpoint: finalized,
						reason,
					});
					bucket_cursor += 1;
				} else {
					bucket_cursor += 1;
				}
				break;
			}
			BucketReconciliationCursor::<T>::put(if bucket_cursor >= bucket_ids.len() {
				0
			} else {
				bucket_cursor as u32
			});
			(processed, agreements)
		}

		#[transactional]
		fn reconcile_one_bucket(
			bucket_id: T::Hash,
			finalized: BlockNumberFor<T>,
		) -> DispatchResult {
			let mut bucket = Buckets::<T>::get(bucket_id).ok_or(Error::<T>::BucketNotFound)?;
			if Self::ensure_provider_eligible(&bucket.primary, finalized).is_ok() {
				return Ok(());
			}
			let snapshot =
				BucketSnapshots::<T>::get(bucket_id).ok_or(Error::<T>::ProviderIneligible)?;
			Self::deposit_event(Event::ProviderIneligible {
				provider: bucket.primary.clone(),
				bucket_id,
				checkpoint: snapshot.checkpoint_block,
			});
			let mut candidates: Vec<(BlockNumberFor<T>, Vec<u8>, T::AccountId)> = bucket
				.replicas
				.iter()
				.filter(|provider| Self::ensure_provider_eligible(provider, finalized).is_ok())
				.filter_map(|provider| {
					ReplicaCheckpoint::<T>::get(bucket_id, provider)
						.map(|checkpoint| (checkpoint, provider.encode(), provider.clone()))
				})
				.filter(|(checkpoint, _, _)| *checkpoint == snapshot.checkpoint_block)
				.collect();
			candidates
				.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
			let (_, _, promoted) =
				candidates.into_iter().next().ok_or(Error::<T>::ProviderIneligible)?;
			let old = bucket.primary.clone();
			let index = bucket
				.replicas
				.iter()
				.position(|provider| provider == &promoted)
				.ok_or(Error::<T>::ProviderIneligible)?;
			bucket.replicas[index] = old.clone();
			bucket.primary = promoted.clone();
			bucket.version = bucket.version.saturating_add(1);
			Self::stage_bucket_authority_duty(bucket_id, &bucket.primary, &bucket.replicas)?;
			Self::rebind_failover_agreements(bucket_id, &old, &promoted)?;
			Buckets::<T>::insert(bucket_id, bucket);
			Self::deposit_event(Event::ReplicaSelected {
				bucket_id,
				provider: promoted.clone(),
				checkpoint: snapshot.checkpoint_block,
			});
			Self::deposit_event(Event::PrimaryPromoted {
				bucket_id,
				old_provider: old,
				new_provider: promoted,
				checkpoint: snapshot.checkpoint_block,
			});
			Ok(())
		}

		fn stage_bucket_authority_duty(
			bucket_id: T::Hash,
			primary: &T::AccountId,
			replicas: &ReplicasOf<T>,
		) -> DispatchResult {
			let finalized = Self::finalized_checkpoint()?;
			let existing = Self::checkpoint_duty_at(bucket_id, finalized)
				.or_else(|| CheckpointDutyPending::<T>::get(bucket_id));
			let (previous_checkpoint, due_at, mode, promotion_predecessor) =
				if let Some(existing) = existing {
					(
						existing.previous_checkpoint,
						existing.due_at,
						existing.mode,
						existing.promotion_predecessor,
					)
				} else {
					let created_at =
						Buckets::<T>::get(bucket_id).ok_or(Error::<T>::BucketNotFound)?.created_at;
					(
						created_at,
						created_at.saturating_add(T::CheckpointCadence::get()),
						CheckpointDutyMode::Standard,
						None,
					)
				};
			Self::stage_checkpoint_duty(
				bucket_id,
				primary,
				replicas,
				previous_checkpoint,
				due_at,
				mode,
				promotion_predecessor,
			)
		}

		/// Keep agreement authority aligned with a bucket failover. Both providers were already
		/// parties to every matching agreement, so capacity and provider indexes do not move; only
		/// their primary/replica roles change. Terminal agreements remain bound until their
		/// capacity is actually released. All fallible validation is completed before writes.
		fn rebind_failover_agreements(
			bucket_id: T::Hash,
			old_primary: &T::AccountId,
			promoted: &T::AccountId,
		) -> DispatchResult {
			let mut rebound = Vec::new();
			for agreement_id in BucketAgreements::<T>::get(bucket_id) {
				let mut agreement =
					Agreements::<T>::get(agreement_id).ok_or(Error::<T>::AgreementNotFound)?;
				ensure!(agreement.bucket_id == bucket_id, Error::<T>::AgreementInvalidState);
				if agreement.capacity_state == AgreementCapacityState::Released {
					continue;
				}
				ensure!(agreement.primary == *old_primary, Error::<T>::AgreementInvalidState);
				let replica_index = agreement
					.replicas
					.iter()
					.position(|provider| provider == promoted)
					.ok_or(Error::<T>::ProviderIneligible)?;
				agreement.primary = promoted.clone();
				agreement.replicas[replica_index] = old_primary.clone();
				agreement.version = agreement.version.saturating_add(1);
				rebound.push((agreement_id, agreement));
			}
			for (agreement_id, agreement) in rebound {
				Agreements::<T>::insert(agreement_id, &agreement);
				Self::deposit_event(Event::AgreementProviderRebound {
					agreement_id,
					old_provider: old_primary.clone(),
					new_provider: promoted.clone(),
					status: agreement.status,
					bytes: agreement.bytes,
				});
			}
			Ok(())
		}
	}
}
