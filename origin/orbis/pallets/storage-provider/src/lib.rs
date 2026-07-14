// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Native zero-stake storage-provider registry for Orbis.
//!
//! The pallet owns provider admission, capacity agreements and proof-accountability checkpoints.
//! It stores commitments and references only. Orbis Storage TransactionStorage remains the sole content
//! commitment/retention ledger and no content bytes are stored here.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

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
	Cancelled,
	Expired,
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
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ProviderRecord<Endpoint, Key, BlockNumber> {
	pub endpoint: Endpoint,
	pub service_key: Key,
	pub capacity_bytes: u64,
	pub allocated_bytes: u64,
	/// Capacity held by proposals awaiting provider acceptance.
	pub pending_bytes: u64,
	pub status: ProviderStatus,
	pub last_heartbeat: BlockNumber,
	pub reputation: i32,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct AgreementRecord<AccountId, Hash, BlockNumber> {
	pub owner: AccountId,
	pub provider: AccountId,
	pub container_ref: Hash,
	pub content_commitment: Hash,
	/// Exact Orbis Storage TransactionStorage reservation id for a resource-backed agreement.
	pub reservation_ref: Option<u64>,
	pub bytes: u64,
	pub created_at: BlockNumber,
	pub expires_at: BlockNumber,
	pub pending_expiry: Option<BlockNumber>,
	pub status: AgreementStatus,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ChallengeRecord<AccountId, Hash, BlockNumber> {
	pub provider: AccountId,
	pub agreement_id: Hash,
	pub expected_commitment: Hash,
	pub due_at: BlockNumber,
	pub proof_commitment: Option<Hash>,
	pub status: ChallengeStatus,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CheckpointRecord<Hash, BlockNumber> {
	pub challenge_id: Hash,
	pub proof_commitment: Hash,
	pub recorded_at: BlockNumber,
}

/// Latest provider-authenticated append-only Merkle root.
///
/// Both counters are monotonic. `sequence` orders signed submissions while `leaf_count` prevents a
/// provider from rolling its evidence log back to an older (or shorter) tree.
#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ProviderRootRecord<Hash, BlockNumber, Frontier> {
	pub sequence: u64,
	pub root: Hash,
	pub leaf_count: u64,
	pub frontier: Frontier,
	pub last_append_commitment: Hash,
	pub committed_at: BlockNumber,
}

/// Provider-signed evidence that content for a terminal agreement was deleted.
///
/// This fixed-size provider-bound audit tombstone survives agreement pruning. Retaining it makes
/// finalized durable-outbox replays exactly idempotent without retaining the larger agreement.
#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct DeletionAcknowledgementRecord<AccountId, Hash, BlockNumber> {
	pub provider: AccountId,
	pub content_commitment: Hash,
	pub tombstone_root: Hash,
	pub root_sequence: u64,
	pub leaf_index: u64,
	pub leaf_count: u64,
	pub proof_commitment: Hash,
	pub acknowledged_at: BlockNumber,
}

/// Validates agreement links against canonical Orbis Storage TransactionStorage reservations.
pub trait ReservationValidator<AccountId, Hash, BlockNumber> {
	fn valid(
		reservation_id: u64,
		owner: &AccountId,
		content_commitment: &Hash,
		bytes: u64,
		expires_at: BlockNumber,
		require_unattached: bool,
	) -> bool;
}

impl<AccountId, Hash, BlockNumber> ReservationValidator<AccountId, Hash, BlockNumber> for () {
	fn valid(_: u64, _: &AccountId, _: &Hash, _: u64, _: BlockNumber, _: bool) -> bool {
		false
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::EnsureOrigin, transactional};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Hash as HashT;

	// Origin and Orbis launch directly on this clean schema. There is no legacy migration path.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(5);

	pub type EndpointOf<T> = BoundedVec<u8, <T as Config>::MaxEndpointBytes>;
	pub type ServiceKeyOf<T> = BoundedVec<u8, <T as Config>::MaxServiceKeyBytes>;
	pub type ProviderRecordOf<T> =
		ProviderRecord<EndpointOf<T>, ServiceKeyOf<T>, BlockNumberFor<T>>;
	pub type AgreementRecordOf<T> = AgreementRecord<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
	>;
	pub type ChallengeRecordOf<T> = ChallengeRecord<
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::Hash,
		BlockNumberFor<T>,
	>;
	pub type ProviderFrontierOf<T> =
		BoundedVec<Option<<T as frame_system::Config>::Hash>, <T as Config>::MaxProviderRootDepth>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		#[pallet::constant]
		type MaxEndpointBytes: Get<u32>;
		#[pallet::constant]
		type MaxServiceKeyBytes: Get<u32>;
		#[pallet::constant]
		type MaxProviders: Get<u32>;
		#[pallet::constant]
		type MaxProviderAgreements: Get<u32>;
		#[pallet::constant]
		type MaxOwnerAgreements: Get<u32>;
		#[pallet::constant]
		type MaxContainerAgreements: Get<u32>;
		#[pallet::constant]
		type MaxChallengesPerBlock: Get<u32>;
		/// Maximum sibling hashes accepted in a deletion inclusion proof.
		#[pallet::constant]
		type MaxDeletionProofDepth: Get<u32>;
		/// Maximum binary frontier levels retained for each provider append-only log.
		#[pallet::constant]
		type MaxProviderRootDepth: Get<u32>;
		/// Maximum leaves whose exact values may be appended in one verified root update.
		#[pallet::constant]
		type MaxRootAppendBatch: Get<u32>;
		type ReservationValidator: ReservationValidator<
			Self::AccountId,
			Self::Hash,
			BlockNumberFor<Self>,
		>;
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
		StorageMap<_, Blake2_128Concat, ServiceKeyOf<T>, T::AccountId, OptionQuery>;

	#[pallet::storage]
	pub type Agreements<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, AgreementRecordOf<T>, OptionQuery>;

	#[pallet::storage]
	pub type ProviderAgreements<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<T::Hash, T::MaxProviderAgreements>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type OwnerAgreements<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<T::Hash, T::MaxOwnerAgreements>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type ContainerAgreements<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		BoundedVec<T::Hash, T::MaxContainerAgreements>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type AgreementNonce<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

	/// One Orbis Storage reservation can back at most one provider agreement.
	#[pallet::storage]
	pub type ReservationAgreement<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, T::Hash, OptionQuery>;

	/// Marks agreements which could have authorized off-chain content bytes.
	#[pallet::storage]
	pub type ActivatedAgreements<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, (), OptionQuery>;

	#[pallet::storage]
	pub type Challenges<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, ChallengeRecordOf<T>, OptionQuery>;

	#[pallet::storage]
	pub type ChallengesDue<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		BlockNumberFor<T>,
		BoundedVec<T::Hash, T::MaxChallengesPerBlock>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type OpenChallengeCount<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, u32, ValueQuery>;

	#[pallet::storage]
	pub type ProviderCheckpoint<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		CheckpointRecord<T::Hash, BlockNumberFor<T>>,
		OptionQuery,
	>;

	/// Current append-only evidence root authenticated by each provider account.
	#[pallet::storage]
	pub type ProviderRoots<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		ProviderRootRecord<T::Hash, BlockNumberFor<T>, ProviderFrontierOf<T>>,
		OptionQuery,
	>;

	/// Fixed-size provider-signed replay/audit tombstone retained after agreement pruning.
	#[pallet::storage]
	pub type DeletionAcknowledgements<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		DeletionAcknowledgementRecord<T::AccountId, T::Hash, BlockNumberFor<T>>,
		OptionQuery,
	>;

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
		ProviderStatusChanged {
			provider: T::AccountId,
			status: ProviderStatus,
		},
		ProviderRemoved {
			provider: T::AccountId,
		},
		Heartbeat {
			provider: T::AccountId,
			at: BlockNumberFor<T>,
		},
		AgreementProposed {
			agreement_id: T::Hash,
			owner: T::AccountId,
			provider: T::AccountId,
		},
		AgreementAccepted {
			agreement_id: T::Hash,
		},
		AgreementCancelled {
			agreement_id: T::Hash,
		},
		AgreementRenewalRequested {
			agreement_id: T::Hash,
			expires_at: BlockNumberFor<T>,
		},
		AgreementRenewed {
			agreement_id: T::Hash,
			expires_at: BlockNumberFor<T>,
		},
		AgreementExpired {
			agreement_id: T::Hash,
		},
		AgreementPruned {
			agreement_id: T::Hash,
		},
		ChallengeIssued {
			challenge_id: T::Hash,
			provider: T::AccountId,
			due_at: BlockNumberFor<T>,
		},
		CheckpointSubmitted {
			challenge_id: T::Hash,
			proof_commitment: T::Hash,
		},
		ProviderRootCommitted {
			provider: T::AccountId,
			sequence: u64,
			root: T::Hash,
			leaf_count: u64,
		},
		ChallengeTimedOut {
			challenge_id: T::Hash,
			provider: T::AccountId,
		},
		DeletionAcknowledged {
			agreement_id: T::Hash,
			provider: T::AccountId,
			content_commitment: T::Hash,
			tombstone_root: T::Hash,
			root_sequence: u64,
			leaf_index: u64,
			leaf_count: u64,
			proof_commitment: T::Hash,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		ProviderAlreadyExists,
		ProviderLimitReached,
		ProviderNotFound,
		ProviderNotActive,
		EndpointInUse,
		ServiceKeyInUse,
		CapacityBelowAllocation,
		CapacityExceeded,
		ProviderHasActiveAllocations,
		AgreementNotFound,
		AgreementNotProposed,
		AgreementNotActive,
		AgreementExpired,
		NotAgreementParty,
		InvalidExpiry,
		InvalidCapacity,
		AgreementAlreadyExists,
		AgreementIndexFull,
		RenewalNotRequested,
		OpenChallengesRemain,
		ChallengeNotFound,
		ChallengeNotOpen,
		ChallengeNotDue,
		ChallengeExpired,
		InvalidProofCommitment,
		ChallengeIndexFull,
		DeletionAlreadyAcknowledged,
		DeletionAcknowledgementRequired,
		ContentCommitmentMismatch,
		ReservationInvalid,
		ReservationAlreadyAllocated,
		InvalidRootSequence,
		InvalidLeafCount,
		ProviderRootNotFound,
		ProviderRootMismatch,
		InvalidProofDepth,
		InvalidInclusionProof,
		InvalidProviderFrontier,
		RootAppendBatchEmpty,
		RootLeafCountOverflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::register_provider())]
		pub fn register_provider(
			origin: OriginFor<T>,
			provider: T::AccountId,
			endpoint: EndpointOf<T>,
			service_key: ServiceKeyOf<T>,
			capacity_bytes: u64,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			ensure!(capacity_bytes > 0, Error::<T>::InvalidCapacity);
			ensure!(!Providers::<T>::contains_key(&provider), Error::<T>::ProviderAlreadyExists);
			ensure!(!EndpointOwner::<T>::contains_key(&endpoint), Error::<T>::EndpointInUse);
			ensure!(!ServiceKeyOwner::<T>::contains_key(&service_key), Error::<T>::ServiceKeyInUse);
			ProviderIds::<T>::try_mutate(|ids| ids.try_push(provider.clone()))
				.map_err(|_| Error::<T>::ProviderLimitReached)?;
			let now = frame_system::Pallet::<T>::block_number();
			Providers::<T>::insert(
				&provider,
				ProviderRecord {
					endpoint: endpoint.clone(),
					service_key: service_key.clone(),
					capacity_bytes,
					allocated_bytes: 0,
					pending_bytes: 0,
					status: ProviderStatus::Active,
					last_heartbeat: now,
					reputation: 0,
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
			service_key: ServiceKeyOf<T>,
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
				if service_key != record.service_key {
					ensure!(
						!ServiceKeyOwner::<T>::contains_key(&service_key),
						Error::<T>::ServiceKeyInUse
					);
					ServiceKeyOwner::<T>::remove(&record.service_key);
					ServiceKeyOwner::<T>::insert(&service_key, &provider);
					record.service_key = service_key;
				}
				record.capacity_bytes = capacity_bytes;
				Ok(())
			})?;
			Self::deposit_event(Event::ProviderUpdated { provider, capacity_bytes });
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::set_provider_status())]
		pub fn set_provider_status(
			origin: OriginFor<T>,
			provider: T::AccountId,
			status: ProviderStatus,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			Providers::<T>::try_mutate(&provider, |maybe| -> DispatchResult {
				maybe.as_mut().ok_or(Error::<T>::ProviderNotFound)?.status = status;
				Ok(())
			})?;
			Self::deposit_event(Event::ProviderStatusChanged { provider, status });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::remove_provider())]
		pub fn remove_provider(origin: OriginFor<T>, provider: T::AccountId) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let record = Providers::<T>::get(&provider).ok_or(Error::<T>::ProviderNotFound)?;
			ensure!(
				record.allocated_bytes == 0 && record.pending_bytes == 0,
				Error::<T>::ProviderHasActiveAllocations
			);
			Providers::<T>::remove(&provider);
			ProviderIds::<T>::mutate(|ids| {
				if let Some(index) = ids.iter().position(|id| id == &provider) {
					ids.swap_remove(index);
				}
			});
			EndpointOwner::<T>::remove(record.endpoint);
			ServiceKeyOwner::<T>::remove(record.service_key);
			Self::deposit_event(Event::ProviderRemoved { provider });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::heartbeat())]
		pub fn heartbeat(origin: OriginFor<T>) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			Providers::<T>::try_mutate(&provider, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::ProviderNotFound)?;
				ensure!(record.status == ProviderStatus::Active, Error::<T>::ProviderNotActive);
				record.last_heartbeat = now;
				Ok(())
			})?;
			Self::deposit_event(Event::Heartbeat { provider, at: now });
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::propose_agreement())]
		#[transactional]
		pub fn propose_agreement(
			origin: OriginFor<T>,
			provider: T::AccountId,
			container_ref: T::Hash,
			content_commitment: T::Hash,
			reservation_ref: Option<u64>,
			bytes: u64,
			expires_at: BlockNumberFor<T>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			ensure!(bytes > 0, Error::<T>::InvalidCapacity);
			ensure!(expires_at > now, Error::<T>::InvalidExpiry);
			let provider_record =
				Providers::<T>::get(&provider).ok_or(Error::<T>::ProviderNotFound)?;
			ensure!(
				provider_record.status == ProviderStatus::Active,
				Error::<T>::ProviderNotActive
			);
			ensure!(
				provider_record.capacity_bytes.saturating_sub(
					provider_record.allocated_bytes.saturating_add(provider_record.pending_bytes),
				) >= bytes,
				Error::<T>::CapacityExceeded
			);
			if let Some(reservation_id) = reservation_ref {
				ensure!(
					T::ReservationValidator::valid(
						reservation_id,
						&owner,
						&content_commitment,
						bytes,
						expires_at,
						true,
					),
					Error::<T>::ReservationInvalid
				);
				ensure!(
					!ReservationAgreement::<T>::contains_key(reservation_id),
					Error::<T>::ReservationAlreadyAllocated
				);
			}
			let nonce = AgreementNonce::<T>::get(&owner);
			let genesis = frame_system::Pallet::<T>::block_hash(BlockNumberFor::<T>::default());
			let agreement_id = T::Hashing::hash_of(&(
				b"orbis/storage-agreement/v1",
				genesis,
				&owner,
				&provider,
				container_ref,
				nonce,
				content_commitment,
				reservation_ref,
				bytes,
				expires_at,
			));
			ensure!(
				!Agreements::<T>::contains_key(agreement_id),
				Error::<T>::AgreementAlreadyExists
			);
			ProviderAgreements::<T>::try_mutate(&provider, |ids| ids.try_push(agreement_id))
				.map_err(|_| Error::<T>::AgreementIndexFull)?;
			OwnerAgreements::<T>::try_mutate(&owner, |ids| ids.try_push(agreement_id))
				.map_err(|_| Error::<T>::AgreementIndexFull)?;
			ContainerAgreements::<T>::try_mutate(container_ref, |ids| ids.try_push(agreement_id))
				.map_err(|_| Error::<T>::AgreementIndexFull)?;
			Agreements::<T>::insert(
				agreement_id,
				AgreementRecord {
					owner: owner.clone(),
					provider: provider.clone(),
					container_ref,
					content_commitment,
					reservation_ref,
					bytes,
					created_at: now,
					expires_at,
					pending_expiry: None,
					status: AgreementStatus::Proposed,
				},
			);
			Providers::<T>::mutate(&provider, |maybe| {
				if let Some(record) = maybe {
					record.pending_bytes = record.pending_bytes.saturating_add(bytes);
				}
			});
			if let Some(reservation_id) = reservation_ref {
				ReservationAgreement::<T>::insert(reservation_id, agreement_id);
			}
			AgreementNonce::<T>::insert(&owner, nonce.saturating_add(1));
			Self::deposit_event(Event::AgreementProposed { agreement_id, owner, provider });
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::accept_agreement())]
		#[transactional]
		pub fn accept_agreement(origin: OriginFor<T>, agreement_id: T::Hash) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			Agreements::<T>::try_mutate(agreement_id, |maybe| -> DispatchResult {
				let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;
				ensure!(agreement.provider == provider, Error::<T>::NotAgreementParty);
				ensure!(
					agreement.status == AgreementStatus::Proposed,
					Error::<T>::AgreementNotProposed
				);
				ensure!(agreement.expires_at > now, Error::<T>::AgreementExpired);
				if let Some(reservation_id) = agreement.reservation_ref {
					ensure!(
						T::ReservationValidator::valid(
							reservation_id,
							&agreement.owner,
							&agreement.content_commitment,
							agreement.bytes,
							agreement.expires_at,
							true,
						),
						Error::<T>::ReservationInvalid
					);
				}
				Providers::<T>::try_mutate(&provider, |maybe_provider| -> DispatchResult {
					let record = maybe_provider.as_mut().ok_or(Error::<T>::ProviderNotFound)?;
					ensure!(record.status == ProviderStatus::Active, Error::<T>::ProviderNotActive);
					ensure!(record.pending_bytes >= agreement.bytes, Error::<T>::CapacityExceeded);
					record.pending_bytes = record.pending_bytes.saturating_sub(agreement.bytes);
					record.allocated_bytes = record.allocated_bytes.saturating_add(agreement.bytes);
					Ok(())
				})?;
				agreement.status = AgreementStatus::Active;
				Ok(())
			})?;
			ActivatedAgreements::<T>::insert(agreement_id, ());
			Self::deposit_event(Event::AgreementAccepted { agreement_id });
			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::cancel_agreement())]
		#[transactional]
		pub fn cancel_agreement(origin: OriginFor<T>, agreement_id: T::Hash) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			Agreements::<T>::try_mutate(agreement_id, |maybe| -> DispatchResult {
				let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;
				ensure!(
					caller == agreement.owner || caller == agreement.provider,
					Error::<T>::NotAgreementParty
				);
				ensure!(
					matches!(agreement.status, AgreementStatus::Proposed | AgreementStatus::Active),
					Error::<T>::AgreementNotActive
				);
				if agreement.status == AgreementStatus::Active {
					Providers::<T>::mutate(&agreement.provider, |maybe_provider| {
						if let Some(record) = maybe_provider {
							record.allocated_bytes =
								record.allocated_bytes.saturating_sub(agreement.bytes);
						}
					});
				} else {
					Providers::<T>::mutate(&agreement.provider, |maybe_provider| {
						if let Some(record) = maybe_provider {
							record.pending_bytes =
								record.pending_bytes.saturating_sub(agreement.bytes);
						}
					});
				}
				agreement.status = AgreementStatus::Cancelled;
				agreement.pending_expiry = None;
				Ok(())
			})?;
			Self::deposit_event(Event::AgreementCancelled { agreement_id });
			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::issue_challenge())]
		#[transactional]
		pub fn issue_challenge(
			origin: OriginFor<T>,
			agreement_id: T::Hash,
			expected_commitment: T::Hash,
			due_at: BlockNumberFor<T>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			ensure!(due_at > now, Error::<T>::InvalidExpiry);
			let agreement =
				Agreements::<T>::get(agreement_id).ok_or(Error::<T>::AgreementNotFound)?;
			ensure!(agreement.status == AgreementStatus::Active, Error::<T>::AgreementNotActive);
			let committed = ProviderRoots::<T>::get(&agreement.provider)
				.ok_or(Error::<T>::ProviderRootNotFound)?;
			ensure!(committed.root == expected_commitment, Error::<T>::ProviderRootMismatch);
			let challenge_id = T::Hashing::hash_of(&(
				b"orbis/provider-challenge/v1",
				agreement_id,
				expected_commitment,
				due_at,
			));
			ensure!(!Challenges::<T>::contains_key(challenge_id), Error::<T>::ChallengeIndexFull);
			ChallengesDue::<T>::try_mutate(due_at, |ids| ids.try_push(challenge_id))
				.map_err(|_| Error::<T>::ChallengeIndexFull)?;
			Challenges::<T>::insert(
				challenge_id,
				ChallengeRecord {
					provider: agreement.provider.clone(),
					agreement_id,
					expected_commitment,
					due_at,
					proof_commitment: None,
					status: ChallengeStatus::Open,
				},
			);
			OpenChallengeCount::<T>::mutate(agreement_id, |count| *count = count.saturating_add(1));
			Self::deposit_event(Event::ChallengeIssued {
				challenge_id,
				provider: agreement.provider,
				due_at,
			});
			Ok(())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::submit_checkpoint())]
		pub fn submit_checkpoint(
			origin: OriginFor<T>,
			challenge_id: T::Hash,
			proof_commitment: T::Hash,
		) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			let newly_proved = Challenges::<T>::try_mutate(
				challenge_id,
				|maybe| -> Result<bool, DispatchError> {
					let challenge = maybe.as_mut().ok_or(Error::<T>::ChallengeNotFound)?;
					ensure!(challenge.provider == provider, Error::<T>::NotAgreementParty);
					if challenge.status == ChallengeStatus::Proved {
						ensure!(
							challenge.proof_commitment == Some(proof_commitment),
							Error::<T>::InvalidProofCommitment
						);
						return Ok(false);
					}
					ensure!(
						challenge.status == ChallengeStatus::Open,
						Error::<T>::ChallengeNotOpen
					);
					ensure!(now <= challenge.due_at, Error::<T>::ChallengeExpired);
					ensure!(
						Self::expected_challenge_proof(challenge_id, challenge)
							.is_some_and(|expected| proof_commitment == expected),
						Error::<T>::InvalidProofCommitment
					);
					challenge.proof_commitment = Some(proof_commitment);
					challenge.status = ChallengeStatus::Proved;
					Ok(true)
				},
			)?;
			if !newly_proved {
				return Ok(());
			}
			if let Some(challenge) = Challenges::<T>::get(challenge_id) {
				OpenChallengeCount::<T>::mutate(challenge.agreement_id, |count| {
					*count = count.saturating_sub(1)
				});
			}
			ProviderCheckpoint::<T>::insert(
				&provider,
				CheckpointRecord { challenge_id, proof_commitment, recorded_at: now },
			);
			Providers::<T>::mutate(&provider, |maybe| {
				if let Some(record) = maybe {
					record.reputation = record.reputation.saturating_add(1);
				}
			});
			Self::deposit_event(Event::CheckpointSubmitted { challenge_id, proof_commitment });
			Ok(())
		}

		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::timeout_challenge())]
		pub fn timeout_challenge(origin: OriginFor<T>, challenge_id: T::Hash) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			let provider = Challenges::<T>::try_mutate(
				challenge_id,
				|maybe| -> Result<T::AccountId, DispatchError> {
					let challenge = maybe.as_mut().ok_or(Error::<T>::ChallengeNotFound)?;
					ensure!(
						challenge.status == ChallengeStatus::Open,
						Error::<T>::ChallengeNotOpen
					);
					ensure!(now > challenge.due_at, Error::<T>::ChallengeNotDue);
					challenge.status = ChallengeStatus::TimedOut;
					Ok(challenge.provider.clone())
				},
			)?;
			if let Some(challenge) = Challenges::<T>::get(challenge_id) {
				OpenChallengeCount::<T>::mutate(challenge.agreement_id, |count| {
					*count = count.saturating_sub(1)
				});
			}
			Providers::<T>::mutate(&provider, |maybe| {
				if let Some(record) = maybe {
					record.status = ProviderStatus::Suspended;
					record.reputation = record.reputation.saturating_sub(1);
				}
			});
			Self::deposit_event(Event::ChallengeTimedOut { challenge_id, provider });
			Ok(())
		}

		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::request_renewal())]
		pub fn request_renewal(
			origin: OriginFor<T>,
			agreement_id: T::Hash,
			expires_at: BlockNumberFor<T>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Agreements::<T>::try_mutate(agreement_id, |maybe| -> DispatchResult {
				let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;
				ensure!(agreement.owner == owner, Error::<T>::NotAgreementParty);
				ensure!(
					agreement.status == AgreementStatus::Active,
					Error::<T>::AgreementNotActive
				);
				ensure!(expires_at > agreement.expires_at, Error::<T>::InvalidExpiry);
				if let Some(reservation_id) = agreement.reservation_ref {
					ensure!(
						T::ReservationValidator::valid(
							reservation_id,
							&agreement.owner,
							&agreement.content_commitment,
							agreement.bytes,
							expires_at,
							false,
						),
						Error::<T>::ReservationInvalid
					);
				}
				agreement.pending_expiry = Some(expires_at);
				Ok(())
			})?;
			Self::deposit_event(Event::AgreementRenewalRequested { agreement_id, expires_at });
			Ok(())
		}

		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::accept_renewal())]
		pub fn accept_renewal(origin: OriginFor<T>, agreement_id: T::Hash) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let expires_at = Agreements::<T>::try_mutate(
				agreement_id,
				|maybe| -> Result<BlockNumberFor<T>, DispatchError> {
					let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;
					ensure!(agreement.provider == provider, Error::<T>::NotAgreementParty);
					ensure!(
						agreement.status == AgreementStatus::Active,
						Error::<T>::AgreementNotActive
					);
					let expires_at =
						agreement.pending_expiry.take().ok_or(Error::<T>::RenewalNotRequested)?;
					if let Some(reservation_id) = agreement.reservation_ref {
						ensure!(
							T::ReservationValidator::valid(
								reservation_id,
								&agreement.owner,
								&agreement.content_commitment,
								agreement.bytes,
								expires_at,
								false,
							),
							Error::<T>::ReservationInvalid
						);
					}
					agreement.expires_at = expires_at;
					Ok(expires_at)
				},
			)?;
			Self::deposit_event(Event::AgreementRenewed { agreement_id, expires_at });
			Ok(())
		}

		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::expire_agreement())]
		#[transactional]
		pub fn expire_agreement(origin: OriginFor<T>, agreement_id: T::Hash) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			let now = frame_system::Pallet::<T>::block_number();
			Agreements::<T>::try_mutate(agreement_id, |maybe| -> DispatchResult {
				let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;
				ensure!(
					matches!(agreement.status, AgreementStatus::Proposed | AgreementStatus::Active),
					Error::<T>::AgreementNotActive
				);
				ensure!(now >= agreement.expires_at, Error::<T>::AgreementNotActive);
				if agreement.status == AgreementStatus::Active {
					Providers::<T>::mutate(&agreement.provider, |maybe_provider| {
						if let Some(record) = maybe_provider {
							record.allocated_bytes =
								record.allocated_bytes.saturating_sub(agreement.bytes);
						}
					});
				} else {
					Providers::<T>::mutate(&agreement.provider, |maybe_provider| {
						if let Some(record) = maybe_provider {
							record.pending_bytes =
								record.pending_bytes.saturating_sub(agreement.bytes);
						}
					});
				}
				agreement.pending_expiry = None;
				agreement.status = AgreementStatus::Expired;
				Ok(())
			})?;
			Self::deposit_event(Event::AgreementExpired { agreement_id });
			Ok(())
		}

		#[pallet::call_index(14)]
		#[pallet::weight(T::WeightInfo::prune_agreement())]
		pub fn prune_agreement(origin: OriginFor<T>, agreement_id: T::Hash) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			let agreement =
				Agreements::<T>::get(agreement_id).ok_or(Error::<T>::AgreementNotFound)?;
			ensure!(
				matches!(agreement.status, AgreementStatus::Cancelled | AgreementStatus::Expired),
				Error::<T>::AgreementNotActive
			);
			ensure!(
				OpenChallengeCount::<T>::get(agreement_id) == 0,
				Error::<T>::OpenChallengesRemain
			);
			ensure!(
				!ActivatedAgreements::<T>::contains_key(agreement_id)
					|| DeletionAcknowledgements::<T>::contains_key(agreement_id),
				Error::<T>::DeletionAcknowledgementRequired
			);
			ProviderAgreements::<T>::mutate(&agreement.provider, |ids| {
				if let Some(index) = ids.iter().position(|id| id == &agreement_id) {
					ids.swap_remove(index);
				}
			});
			OwnerAgreements::<T>::mutate(&agreement.owner, |ids| {
				if let Some(index) = ids.iter().position(|id| id == &agreement_id) {
					ids.swap_remove(index);
				}
			});
			ContainerAgreements::<T>::mutate(agreement.container_ref, |ids| {
				if let Some(index) = ids.iter().position(|id| id == &agreement_id) {
					ids.swap_remove(index);
				}
			});
			if let Some(reservation_id) = agreement.reservation_ref {
				ReservationAgreement::<T>::remove(reservation_id);
			}
			Agreements::<T>::remove(agreement_id);
			OpenChallengeCount::<T>::remove(agreement_id);
			// Retain the compact provider-bound acknowledgement as permanent replay/audit evidence.
			ActivatedAgreements::<T>::remove(agreement_id);
			Self::deposit_event(Event::AgreementPruned { agreement_id });
			Ok(())
		}

		/// Acknowledge off-chain content deletion for a terminal agreement.
		///
		/// The bounded proof must include the canonical deletion leaf in the provider's currently
		/// committed append-only root. The canonical leaf is derived entirely from on-chain fields:
		/// `(domain, agreement, content, provider, deletion_nonce)`. A clean network has one
		/// deletion transition per agreement, hence nonce zero.
		#[pallet::call_index(15)]
		#[pallet::weight(T::WeightInfo::acknowledge_deletion())]
		pub fn acknowledge_deletion(
			origin: OriginFor<T>,
			agreement_id: T::Hash,
			content_commitment: T::Hash,
			tombstone_root: T::Hash,
			root_sequence: u64,
			leaf_index: u64,
			leaf_count: u64,
			inclusion_proof: BoundedVec<T::Hash, T::MaxDeletionProofDepth>,
		) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let proof_commitment = T::Hashing::hash_of(&(
				b"orbis/provider-deletion-proof/v2",
				agreement_id,
				content_commitment,
				&provider,
				root_sequence,
				tombstone_root,
				leaf_index,
				leaf_count,
				&inclusion_proof,
			));
			if let Some(existing) = DeletionAcknowledgements::<T>::get(agreement_id) {
				ensure!(existing.provider == provider, Error::<T>::NotAgreementParty);
				ensure!(
					existing.content_commitment == content_commitment
						&& existing.tombstone_root == tombstone_root
						&& existing.root_sequence == root_sequence
						&& existing.leaf_index == leaf_index
						&& existing.leaf_count == leaf_count
						&& existing.proof_commitment == proof_commitment,
					Error::<T>::DeletionAlreadyAcknowledged
				);
				return Ok(());
			}
			let agreement =
				Agreements::<T>::get(agreement_id).ok_or(Error::<T>::AgreementNotFound)?;
			ensure!(agreement.provider == provider, Error::<T>::NotAgreementParty);
			ensure!(
				ActivatedAgreements::<T>::contains_key(agreement_id),
				Error::<T>::DeletionAcknowledgementRequired
			);
			ensure!(
				matches!(agreement.status, AgreementStatus::Cancelled | AgreementStatus::Expired),
				Error::<T>::AgreementNotActive
			);
			ensure!(
				agreement.content_commitment == content_commitment,
				Error::<T>::ContentCommitmentMismatch
			);
			let committed =
				ProviderRoots::<T>::get(&provider).ok_or(Error::<T>::ProviderRootNotFound)?;
			ensure!(
				committed.sequence == root_sequence
					&& committed.root == tombstone_root
					&& committed.leaf_count == leaf_count,
				Error::<T>::ProviderRootMismatch
			);
			ensure!(leaf_index < leaf_count, Error::<T>::InvalidLeafCount);
			ensure!(
				inclusion_proof.len() == Self::proof_depth(leaf_count),
				Error::<T>::InvalidProofDepth
			);
			let deletion_nonce = 0u64;
			let tombstone_leaf = T::Hashing::hash_of(&(
				b"orbis/provider-deletion-leaf/v1",
				agreement_id,
				content_commitment,
				&provider,
				deletion_nonce,
			));
			ensure!(
				Self::verify_inclusion(tombstone_leaf, leaf_index, leaf_count, &inclusion_proof,)
					== Some(tombstone_root),
				Error::<T>::InvalidInclusionProof
			);
			let acknowledged_at = frame_system::Pallet::<T>::block_number();
			DeletionAcknowledgements::<T>::insert(
				agreement_id,
				DeletionAcknowledgementRecord {
					provider: provider.clone(),
					content_commitment,
					tombstone_root,
					root_sequence,
					leaf_index,
					leaf_count,
					proof_commitment,
					acknowledged_at,
				},
			);
			Self::deposit_event(Event::DeletionAcknowledged {
				agreement_id,
				provider,
				content_commitment,
				tombstone_root,
				root_sequence,
				leaf_index,
				leaf_count,
				proof_commitment,
			});
			Ok(())
		}

		/// Commit the provider's latest append-only Merkle root before any acknowledgement may use
		/// it.
		#[pallet::call_index(16)]
		#[pallet::weight(T::WeightInfo::commit_provider_root(appended_leaves.len() as u32))]
		pub fn commit_provider_root(
			origin: OriginFor<T>,
			sequence: u64,
			appended_leaves: BoundedVec<T::Hash, T::MaxRootAppendBatch>,
		) -> DispatchResult {
			let provider = ensure_signed(origin)?;
			let record = Providers::<T>::get(&provider).ok_or(Error::<T>::ProviderNotFound)?;
			ensure!(record.status == ProviderStatus::Active, Error::<T>::ProviderNotActive);
			ensure!(!appended_leaves.is_empty(), Error::<T>::RootAppendBatchEmpty);
			let append_commitment = T::Hashing::hash_of(&(
				b"orbis/provider-root-append/v1",
				sequence,
				&appended_leaves,
			));
			let previous = ProviderRoots::<T>::get(&provider);
			if let Some(previous) = &previous {
				if previous.sequence == sequence
					&& previous.last_append_commitment == append_commitment
				{
					return Ok(());
				}
				let expected_sequence =
					previous.sequence.checked_add(1).ok_or(Error::<T>::RootLeafCountOverflow)?;
				ensure!(sequence == expected_sequence, Error::<T>::InvalidRootSequence);
			} else {
				ensure!(sequence == 1, Error::<T>::InvalidRootSequence);
			}
			let mut frontier =
				previous.as_ref().map(|root| root.frontier.clone()).unwrap_or_default();
			let mut leaf_count = previous.as_ref().map(|root| root.leaf_count).unwrap_or(0);
			for leaf in appended_leaves {
				Self::append_frontier(&mut frontier, leaf_count, leaf)?;
				leaf_count = leaf_count.checked_add(1).ok_or(Error::<T>::RootLeafCountOverflow)?;
			}
			let root = Self::frontier_root(&frontier, leaf_count)
				.ok_or(Error::<T>::InvalidProviderFrontier)?;
			let committed_at = frame_system::Pallet::<T>::block_number();
			ProviderRoots::<T>::insert(
				&provider,
				ProviderRootRecord {
					sequence,
					root,
					leaf_count,
					frontier,
					last_append_commitment: append_commitment,
					committed_at,
				},
			);
			Self::deposit_event(Event::ProviderRootCommitted {
				provider,
				sequence,
				root,
				leaf_count,
			});
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn append_frontier(
			frontier: &mut ProviderFrontierOf<T>,
			leaf_count: u64,
			mut node: T::Hash,
		) -> DispatchResult {
			let mut level = 0usize;
			let mut occupied = leaf_count;
			while occupied & 1 == 1 {
				let left = frontier
					.get_mut(level)
					.and_then(Option::take)
					.ok_or(Error::<T>::InvalidProviderFrontier)?;
				node = T::Hashing::hash_of(&(b"orbis/provider-node/v2", left, node));
				level = level.saturating_add(1);
				occupied >>= 1;
			}
			while frontier.len() <= level {
				frontier.try_push(None).map_err(|_| Error::<T>::RootLeafCountOverflow)?;
			}
			ensure!(frontier[level].is_none(), Error::<T>::InvalidProviderFrontier);
			frontier[level] = Some(node);
			Ok(())
		}

		fn frontier_root(frontier: &ProviderFrontierOf<T>, leaf_count: u64) -> Option<T::Hash> {
			if leaf_count == 0 {
				return None;
			}
			for (level, peak) in frontier.iter().enumerate() {
				let occupied = leaf_count.checked_shr(level as u32).unwrap_or(0) & 1 == 1;
				if peak.is_some() != occupied {
					return None;
				}
			}
			if leaf_count.checked_shr(frontier.len() as u32).unwrap_or(0) != 0 {
				return None;
			}
			let mut current: Option<(T::Hash, usize)> = None;
			for (level, peak) in frontier.iter().enumerate() {
				let Some(peak) = peak else { continue };
				current = Some(match current {
					None => (*peak, level),
					Some((mut right, mut right_level)) => {
						while right_level < level {
							right = T::Hashing::hash_of(&(b"orbis/provider-node/v2", right, right));
							right_level = right_level.saturating_add(1);
						}
						(
							T::Hashing::hash_of(&(b"orbis/provider-node/v2", peak, right)),
							level.saturating_add(1),
						)
					},
				});
			}
			current.map(|(root, _)| root)
		}

		fn proof_depth(mut leaf_count: u64) -> usize {
			let mut depth = 0usize;
			while leaf_count > 1 {
				leaf_count = leaf_count.saturating_add(1) / 2;
				depth = depth.saturating_add(1);
			}
			depth
		}

		/// Reconstruct a duplicate-last binary Merkle root from a leaf-to-root sibling path.
		fn verify_inclusion(
			mut node: T::Hash,
			mut index: u64,
			mut leaf_count: u64,
			proof: &[T::Hash],
		) -> Option<T::Hash> {
			for sibling in proof {
				if index % 2 == 0 && index.saturating_add(1) >= leaf_count && *sibling != node {
					return None;
				}
				node = if index % 2 == 0 {
					T::Hashing::hash_of(&(b"orbis/provider-node/v2", node, sibling))
				} else {
					T::Hashing::hash_of(&(b"orbis/provider-node/v2", sibling, node))
				};
				index /= 2;
				leaf_count = leaf_count.saturating_add(1) / 2;
			}
			Some(node)
		}

		/// Domain-separated challenge proof expected by the runtime. The challenge's
		/// `expected_commitment` is the provider root committed by the issuing authority.
		pub fn expected_challenge_proof(
			challenge_id: T::Hash,
			challenge: &ChallengeRecordOf<T>,
		) -> Option<T::Hash> {
			let agreement = Agreements::<T>::get(challenge.agreement_id)?;
			Some(T::Hashing::hash_of(&(
				b"orbis/provider-challenge-proof/v1",
				challenge_id,
				challenge.agreement_id,
				agreement.content_commitment,
				&challenge.provider,
				challenge.expected_commitment,
			)))
		}
	}
}
