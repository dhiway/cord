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

//! Native, bounded Orbis Names registry for the Commons runtime.
//!
//! This pallet is the sole native name authority. It intentionally contains no contract caller,
//! H160 dispatcher, ABI encoding, tokenized ownership, escrow, pricing, legacy import, or
//! compatibility facade.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

/// Consensus label policy. Version 1 accepts lowercase ASCII letters, digits and internal hyphens.
pub const LABEL_POLICY_VERSION: u16 = 1;
const NAME_ID_DOMAIN: &[u8] = b"cord:orbis:names:name:v1";
const COMMITMENT_DOMAIN: &[u8] = b"cord:orbis:names:commitment:v1";

pub trait ContentReferenceValidator<Commitment> {
	fn contains(commitment: &Commitment) -> bool;
}

impl<Commitment> ContentReferenceValidator<Commitment> for () {
	fn contains(_: &Commitment) -> bool {
		true
	}
}

/// O(1) validation boundary for subject commitments owned by the native identity/attestation
/// domain. Orbis Names stores only the canonical identifier and does not duplicate identity state.
pub trait SubjectReferenceValidator<Subject> {
	fn contains(subject: &Subject) -> bool;
}

impl<Subject> SubjectReferenceValidator<Subject> for () {
	fn contains(_: &Subject) -> bool {
		true
	}
}

/// O(1) validation boundary for attestations owned by the native attestation domain.
/// Implementations must reject records which exist but are no longer live.
pub trait AttestationReferenceValidator<Attestation> {
	fn is_live(attestation: &Attestation) -> bool;
}

impl<Attestation> AttestationReferenceValidator<Attestation> for () {
	fn is_live(_: &Attestation) -> bool {
		true
	}
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct NameRecord<
	AccountId,
	BlockNumber,
	NameId,
	Label,
	Address,
	SubjectId,
	AttestationId,
	ContentCommitment,
> {
	pub parent: Option<NameId>,
	pub label: Label,
	pub owner: AccountId,
	pub expires_at: BlockNumber,
	pub depth: u32,
	pub address: Option<Address>,
	pub subject: Option<SubjectId>,
	pub attestation: Option<AttestationId>,
	pub content: Option<ContentCommitment>,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct Reservation<AccountId, BlockNumber> {
	/// `None` reserves the name for administration only.
	pub beneficiary: Option<AccountId>,
	/// `None` is an indefinite reservation.
	pub expires_at: Option<BlockNumber>,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ContentOperationReceipt<Hash, ContentCommitment> {
	pub request_hash: Hash,
	pub name: Hash,
	pub content: Option<ContentCommitment>,
	pub revision: u64,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use alloc::vec::Vec;
	use frame_support::{
		ensure,
		pallet_prelude::*,
		traits::{EnsureOrigin, Get},
		transactional,
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{Hash as HashT, Saturating, Zero};

	pub type LabelOf<T> = BoundedVec<u8, <T as Config>::MaxLabelLength>;
	pub type SaltOf<T> = BoundedVec<u8, <T as Config>::MaxSaltLength>;
	pub type AddressOf<T> = BoundedVec<u8, <T as Config>::MaxAddressLength>;
	pub type TextKeyOf<T> = BoundedVec<u8, <T as Config>::MaxTextKeyLength>;
	pub type TextValueOf<T> = BoundedVec<u8, <T as Config>::MaxTextValueLength>;
	pub type NameRecordOf<T> = NameRecord<
		<T as frame_system::Config>::AccountId,
		BlockNumberFor<T>,
		<T as frame_system::Config>::Hash,
		LabelOf<T>,
		AddressOf<T>,
		<T as Config>::SubjectId,
		<T as Config>::AttestationId,
		<T as Config>::ContentCommitment,
	>;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
		/// Root or an equivalent emergency administrator.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Canonical subject identifier owned by the identity/personhood domain.
		type SubjectId: Parameter + MaxEncodedLen;
		type SubjectReferenceValidator: SubjectReferenceValidator<Self::SubjectId>;
		/// Canonical attestation identifier owned by the attestation domain.
		type AttestationId: Parameter + MaxEncodedLen;
		type AttestationReferenceValidator: AttestationReferenceValidator<Self::AttestationId>;
		/// Canonical content commitment owned by the storage domain.
		type ContentCommitment: Parameter + MaxEncodedLen;
		type ContentReferenceValidator: ContentReferenceValidator<Self::ContentCommitment>;
		#[pallet::constant]
		type MaxLabelLength: Get<u32>;
		#[pallet::constant]
		type MaxSaltLength: Get<u32>;
		#[pallet::constant]
		type MaxAddressLength: Get<u32>;
		#[pallet::constant]
		type MaxTextKeyLength: Get<u32>;
		#[pallet::constant]
		type MaxTextValueLength: Get<u32>;
		#[pallet::constant]
		type MaxTextRecords: Get<u32>;
		#[pallet::constant]
		type MaxControllers: Get<u32>;
		#[pallet::constant]
		type MaxRegistrars: Get<u32>;
		#[pallet::constant]
		type MaxBootstrapReservations: Get<u32>;
		#[pallet::constant]
		type MaxNamesPerOwner: Get<u32>;
		#[pallet::constant]
		type MaxChildrenPerName: Get<u32>;
		#[pallet::constant]
		type MaxRootNames: Get<u32>;
		#[pallet::constant]
		type MaxNameDepth: Get<u32>;
		#[pallet::constant]
		type MaxCommitmentsPerAccount: Get<u32>;
		/// Retained idempotency receipts per publisher. Oldest receipts are evicted deterministically.
		#[pallet::constant]
		type MaxContentOperationReceipts: Get<u32>;
		#[pallet::constant]
		type MinCommitmentAge: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type MaxCommitmentAge: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type RegistrationPeriod: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type MaxRenewalPeriod: Get<BlockNumberFor<Self>>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// Commitments are account-bound and cannot be revealed by a front runner.
	#[pallet::storage]
	pub type Commitments<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::Hash,
		BlockNumberFor<T>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type CommitmentCount<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn name_record)]
	pub type Names<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, NameRecordOf<T>, OptionQuery>;

	#[pallet::storage]
	pub type ContentRevisions<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, u64, ValueQuery>;

	#[pallet::storage]
	pub type ContentOperationReceipts<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		[u8; 16],
		ContentOperationReceipt<T::Hash, T::ContentCommitment>,
		OptionQuery,
	>;

	/// Oldest-first bounded replay window for content publication operation identifiers.
	#[pallet::storage]
	pub type ContentOperationReceiptIds<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<[u8; 16], T::MaxContentOperationReceipts>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn controllers)]
	pub type Controllers<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		BoundedVec<T::AccountId, T::MaxControllers>,
		ValueQuery,
	>;

	/// Accounts delegated only the bounded reservation/protection administration surface.
	#[pallet::storage]
	#[pallet::getter(fn registrars)]
	pub type Registrars<T: Config> =
		StorageValue<_, BoundedVec<T::AccountId, T::MaxRegistrars>, ValueQuery>;

	/// Genesis-only root-label reservations keyed by exact label-policy-v1 bytes.
	#[pallet::storage]
	#[pallet::getter(fn bootstrap_reservation)]
	pub type BootstrapReservations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		LabelOf<T>,
		Reservation<T::AccountId, BlockNumberFor<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type BootstrapReservationLabels<T: Config> =
		StorageValue<_, BoundedVec<LabelOf<T>, T::MaxBootstrapReservations>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn names_by_owner)]
	pub type OwnerNames<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<T::Hash, T::MaxNamesPerOwner>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn root_names)]
	pub type RootNames<T: Config> =
		StorageValue<_, BoundedVec<T::Hash, T::MaxRootNames>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn children)]
	pub type Children<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		BoundedVec<T::Hash, T::MaxChildrenPerName>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn primary_name_stored)]
	pub type PrimaryName<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, T::Hash, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn text_record)]
	pub type TextRecords<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Blake2_128Concat,
		TextKeyOf<T>,
		TextValueOf<T>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn text_keys)]
	pub type TextKeys<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		BoundedVec<TextKeyOf<T>, T::MaxTextRecords>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn reservation)]
	pub type Reservations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Reservation<T::AccountId, BlockNumberFor<T>>,
		OptionQuery,
	>;

	/// Root labels which cannot be registered through the public reveal path.
	#[pallet::storage]
	#[pallet::getter(fn protected_label)]
	pub type ProtectedLabels<T: Config> =
		StorageMap<_, Blake2_128Concat, LabelOf<T>, (), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn paused)]
	pub type Paused<T: Config> = StorageValue<_, bool, ValueQuery>;

	#[derive(frame_support::DefaultNoBound)]
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub registrars: Vec<T::AccountId>,
		pub root_reservations: Vec<(LabelOf<T>, Option<T::AccountId>)>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			let mut registrars = BoundedVec::<T::AccountId, T::MaxRegistrars>::default();
			for registrar in &self.registrars {
				assert!(!registrars.contains(registrar), "duplicate Orbis Names genesis registrar");
				registrars
					.try_push(registrar.clone())
					.expect("Orbis Names genesis registrars exceed MaxRegistrars");
			}
			Registrars::<T>::put(registrars);

			let mut labels = BoundedVec::<LabelOf<T>, T::MaxBootstrapReservations>::default();
			for (label, beneficiary) in &self.root_reservations {
				assert!(Pallet::<T>::ensure_valid_label(label).is_ok(), "invalid Orbis Names genesis label");
				assert!(!labels.contains(label), "duplicate Orbis Names genesis reservation");
				labels
					.try_push(label.clone())
					.expect("Orbis Names genesis reservations exceed MaxBootstrapReservations");
				BootstrapReservations::<T>::insert(
					label,
					Reservation { beneficiary: beneficiary.clone(), expires_at: None },
				);
			}
			BootstrapReservationLabels::<T>::put(labels);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CommitmentStored {
			owner: T::AccountId,
			commitment: T::Hash,
			at: BlockNumberFor<T>,
		},
		CommitmentRemoved {
			owner: T::AccountId,
			commitment: T::Hash,
		},
		NameRegistered {
			name: T::Hash,
			parent: Option<T::Hash>,
			label: LabelOf<T>,
			owner: T::AccountId,
			expires_at: BlockNumberFor<T>,
		},
		NameRenewed {
			name: T::Hash,
			expires_at: BlockNumberFor<T>,
		},
		NameTransferred {
			name: T::Hash,
			from: T::AccountId,
			to: T::AccountId,
		},
		NameReleased {
			name: T::Hash,
			owner: T::AccountId,
		},
		ExpiredNameRemoved {
			name: T::Hash,
		},
		ControllerAdded {
			name: T::Hash,
			controller: T::AccountId,
		},
		ControllerRemoved {
			name: T::Hash,
			controller: T::AccountId,
		},
		AddressSet {
			name: T::Hash,
			present: bool,
		},
		SubjectSet {
			name: T::Hash,
			present: bool,
		},
		AttestationSet {
			name: T::Hash,
			present: bool,
		},
		ContentSet {
			name: T::Hash,
			present: bool,
			revision: u64,
			operation_id: [u8; 16],
			replayed: bool,
		},
		ContentOperationReceiptPruned {
			owner: T::AccountId,
			operation_id: [u8; 16],
		},
		TextSet {
			name: T::Hash,
			key: TextKeyOf<T>,
			present: bool,
		},
		PrimaryNameSet {
			owner: T::AccountId,
			name: Option<T::Hash>,
		},
		NameReserved {
			name: T::Hash,
			beneficiary: Option<T::AccountId>,
			expires_at: Option<BlockNumberFor<T>>,
		},
		ReservationCleared {
			name: T::Hash,
		},
		LabelProtectionSet {
			label: LabelOf<T>,
			protected: bool,
		},
		PauseSet {
			paused: bool,
		},
		EmergencyNameRevoked {
			name: T::Hash,
		},
		RegistrarSet {
			registrar: T::AccountId,
			enabled: bool,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		Paused,
		InvalidLabel,
		InvalidCommitmentWindow,
		CommitmentExists,
		TooManyCommitments,
		CommitmentNotFound,
		CommitmentTooYoung,
		CommitmentTooOld,
		NameNotFound,
		NameTaken,
		NameExpired,
		NotOwner,
		NotAuthorized,
		ProtectedLabel,
		ReservedName,
		InvalidReservationExpiry,
		ParentNotFound,
		ParentExpired,
		MaximumDepth,
		TooManyRootNames,
		TooManyChildren,
		TooManyNames,
		TooManyControllers,
		ControllerExists,
		ControllerNotFound,
		InvalidRenewal,
		ChildrenRemain,
		EmptyRecord,
		InvalidSubjectReference,
		InvalidAttestationReference,
		InvalidContentReference,
		ContentRevisionConflict,
		OperationIdConflict,
		ContentOperationReceiptLimitDisabled,
		TooManyTextRecords,
		PrimaryNameInvalid,
		InvalidSalt,
		NotRegistrar,
		TooManyRegistrars,
		RegistrarNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::commit())]
		pub fn commit(origin: OriginFor<T>, commitment: T::Hash) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::ensure_running()?;
			let now = frame_system::Pallet::<T>::block_number();
			ensure!(
				T::MinCommitmentAge::get() <= T::MaxCommitmentAge::get(),
				Error::<T>::InvalidCommitmentWindow
			);

			if let Some(created_at) = Commitments::<T>::get(&owner, commitment) {
				ensure!(
					now > created_at.saturating_add(T::MaxCommitmentAge::get()),
					Error::<T>::CommitmentExists
				);
			} else {
				CommitmentCount::<T>::try_mutate(&owner, |count| -> DispatchResult {
					ensure!(
						*count < T::MaxCommitmentsPerAccount::get(),
						Error::<T>::TooManyCommitments
					);
					*count = count.saturating_add(1);
					Ok(())
				})?;
			}

			Commitments::<T>::insert(&owner, commitment, now);
			Self::deposit_event(Event::CommitmentStored { owner, commitment, at: now });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::cancel_commitment())]
		pub fn cancel_commitment(origin: OriginFor<T>, commitment: T::Hash) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			ensure!(
				Commitments::<T>::take(&owner, commitment).is_some(),
				Error::<T>::CommitmentNotFound
			);
			Self::decrement_commitments(&owner);
			Self::deposit_event(Event::CommitmentRemoved { owner, commitment });
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::prune_commitment())]
		pub fn prune_expired_commitment(
			origin: OriginFor<T>,
			owner: T::AccountId,
			commitment: T::Hash,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			let created_at =
				Commitments::<T>::get(&owner, commitment).ok_or(Error::<T>::CommitmentNotFound)?;
			let now = frame_system::Pallet::<T>::block_number();
			ensure!(
				now > created_at.saturating_add(T::MaxCommitmentAge::get()),
				Error::<T>::CommitmentTooYoung
			);
			Commitments::<T>::remove(&owner, commitment);
			Self::decrement_commitments(&owner);
			Self::deposit_event(Event::CommitmentRemoved { owner, commitment });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::register())]
		#[transactional]
		pub fn register(
			origin: OriginFor<T>,
			parent: Option<T::Hash>,
			label: LabelOf<T>,
			salt: SaltOf<T>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::ensure_running()?;
			Self::ensure_valid_label(&label)?;
			ensure!(!salt.is_empty(), Error::<T>::InvalidSalt);
			let now = frame_system::Pallet::<T>::block_number();
			let commitment = Self::registration_commitment(&owner, parent, &label, &salt);
			let created_at =
				Commitments::<T>::get(&owner, commitment).ok_or(Error::<T>::CommitmentNotFound)?;
			ensure!(
				now >= created_at.saturating_add(T::MinCommitmentAge::get()),
				Error::<T>::CommitmentTooYoung
			);
			ensure!(
				now <= created_at.saturating_add(T::MaxCommitmentAge::get()),
				Error::<T>::CommitmentTooOld
			);
			let name = Self::derive_name_id(parent, &label);
			let replacing = Names::<T>::contains_key(name);

			let (depth, parent_expiry) = if let Some(parent_id) = parent {
				let parent_record = Names::<T>::get(parent_id).ok_or(Error::<T>::ParentNotFound)?;
				ensure!(now < parent_record.expires_at, Error::<T>::ParentExpired);
				Self::ensure_authorized_record(&owner, parent_id, &parent_record, now)?;
				let depth = parent_record.depth.saturating_add(1);
				ensure!(depth <= T::MaxNameDepth::get(), Error::<T>::MaximumDepth);
				ensure!(
					replacing
						|| Children::<T>::get(parent_id).len()
							< T::MaxChildrenPerName::get() as usize,
					Error::<T>::TooManyChildren
				);
				(depth, Some(parent_record.expires_at))
			} else {
				ensure!(!ProtectedLabels::<T>::contains_key(&label), Error::<T>::ProtectedLabel);
				Self::ensure_bootstrap_reservation_allows(&label, &owner)?;
				ensure!(
					replacing || RootNames::<T>::get().len() < T::MaxRootNames::get() as usize,
					Error::<T>::TooManyRootNames
				);
				(0, None)
			};

			Self::ensure_reservation_allows(name, &owner, now)?;
			let mut replaces_same_owner = false;
			if let Some(existing) = Names::<T>::get(name) {
				ensure!(now >= existing.expires_at, Error::<T>::NameTaken);
				ensure!(Children::<T>::get(name).is_empty(), Error::<T>::ChildrenRemain);
				replaces_same_owner = existing.owner == owner;
				Self::remove_name_state(name, existing)?;
			}
			ensure!(
				replaces_same_owner
					|| OwnerNames::<T>::get(&owner).len() < T::MaxNamesPerOwner::get() as usize,
				Error::<T>::TooManyNames
			);

			let mut expires_at = now.saturating_add(T::RegistrationPeriod::get());
			if let Some(parent_expires) = parent_expiry {
				if parent_expires < expires_at {
					expires_at = parent_expires;
				}
			}
			ensure!(expires_at > now, Error::<T>::InvalidRenewal);

			let record = NameRecord {
				parent,
				label: label.clone(),
				owner: owner.clone(),
				expires_at,
				depth,
				address: None,
				subject: None,
				attestation: None,
				content: None,
			};
			Names::<T>::insert(name, record);
			OwnerNames::<T>::try_mutate(&owner, |names| {
				names.try_push(name).map_err(|_| Error::<T>::TooManyNames)
			})?;
			if let Some(parent_id) = parent {
				Children::<T>::try_mutate(parent_id, |children| {
					children.try_push(name).map_err(|_| Error::<T>::TooManyChildren)
				})?;
			} else {
				RootNames::<T>::try_mutate(|names| {
					names.try_push(name).map_err(|_| Error::<T>::TooManyRootNames)
				})?;
			}
			Commitments::<T>::remove(&owner, commitment);
			Self::decrement_commitments(&owner);
			Reservations::<T>::remove(name);
			Self::deposit_event(Event::NameRegistered { name, parent, label, owner, expires_at });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::renew())]
		pub fn renew(
			origin: OriginFor<T>,
			name: T::Hash,
			additional_period: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_running()?;
			ensure!(
				!additional_period.is_zero() && additional_period <= T::MaxRenewalPeriod::get(),
				Error::<T>::InvalidRenewal
			);
			let now = frame_system::Pallet::<T>::block_number();
			Names::<T>::try_mutate(name, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::NameNotFound)?;
				Self::ensure_authorized_record(&who, name, record, now)?;
				let base = if record.expires_at > now { record.expires_at } else { now };
				let mut next = base.saturating_add(additional_period);
				if let Some(parent) = record.parent {
					let parent_expiry =
						Names::<T>::get(parent).ok_or(Error::<T>::ParentNotFound)?.expires_at;
					ensure!(now < parent_expiry, Error::<T>::ParentExpired);
					if parent_expiry < next {
						next = parent_expiry;
					}
				}
				ensure!(next > record.expires_at, Error::<T>::InvalidRenewal);
				record.expires_at = next;
				Self::deposit_event(Event::NameRenewed { name, expires_at: next });
				Ok(())
			})
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::transfer())]
		#[transactional]
		pub fn transfer(
			origin: OriginFor<T>,
			name: T::Hash,
			new_owner: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_running()?;
			let now = frame_system::Pallet::<T>::block_number();
			let mut record = Names::<T>::get(name).ok_or(Error::<T>::NameNotFound)?;
			ensure!(record.owner == who, Error::<T>::NotOwner);
			ensure!(now < record.expires_at, Error::<T>::NameExpired);
			ensure!(record.owner != new_owner, Error::<T>::NotOwner);
			ensure!(
				OwnerNames::<T>::get(&new_owner).len() < T::MaxNamesPerOwner::get() as usize,
				Error::<T>::TooManyNames
			);
			OwnerNames::<T>::mutate(&who, |names| {
				Self::remove_index_value::<T::MaxNamesPerOwner>(names, name)
			});
			OwnerNames::<T>::try_mutate(&new_owner, |names| {
				names.try_push(name).map_err(|_| Error::<T>::TooManyNames)
			})?;
			if PrimaryName::<T>::get(&who) == Some(name) {
				PrimaryName::<T>::remove(&who);
				Self::deposit_event(Event::PrimaryNameSet { owner: who.clone(), name: None });
			}
			Controllers::<T>::remove(name);
			record.owner = new_owner.clone();
			Names::<T>::insert(name, record);
			Self::deposit_event(Event::NameTransferred { name, from: who, to: new_owner });
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::controller())]
		pub fn add_controller(
			origin: OriginFor<T>,
			name: T::Hash,
			controller: T::AccountId,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::ensure_running()?;
			Self::ensure_owner_active(&owner, name)?;
			Controllers::<T>::try_mutate(name, |controllers| -> DispatchResult {
				ensure!(!controllers.contains(&controller), Error::<T>::ControllerExists);
				controllers
					.try_push(controller.clone())
					.map_err(|_| Error::<T>::TooManyControllers)?;
				Ok(())
			})?;
			Self::deposit_event(Event::ControllerAdded { name, controller });
			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::controller())]
		pub fn remove_controller(
			origin: OriginFor<T>,
			name: T::Hash,
			controller: T::AccountId,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::ensure_running()?;
			Self::ensure_owner_active(&owner, name)?;
			Controllers::<T>::try_mutate(name, |controllers| -> DispatchResult {
				let position = controllers
					.iter()
					.position(|candidate| candidate == &controller)
					.ok_or(Error::<T>::ControllerNotFound)?;
				controllers.swap_remove(position);
				Ok(())
			})?;
			Self::deposit_event(Event::ControllerRemoved { name, controller });
			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::resolver_write())]
		pub fn set_address(
			origin: OriginFor<T>,
			name: T::Hash,
			address: Option<AddressOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_running()?;
			if let Some(ref value) = address {
				ensure!(!value.is_empty(), Error::<T>::EmptyRecord);
			}
			let present = address.is_some();
			Self::mutate_authorized_record(&who, name, |record| record.address = address)?;
			Self::deposit_event(Event::AddressSet { name, present });
			Ok(())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::resolver_write())]
		pub fn set_subject(
			origin: OriginFor<T>,
			name: T::Hash,
			subject: Option<T::SubjectId>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_running()?;
			if let Some(reference) = subject.as_ref() {
				ensure!(
					T::SubjectReferenceValidator::contains(reference),
					Error::<T>::InvalidSubjectReference
				);
			}
			let present = subject.is_some();
			Self::mutate_authorized_record(&who, name, |record| record.subject = subject)?;
			Self::deposit_event(Event::SubjectSet { name, present });
			Ok(())
		}

		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::resolver_write())]
		pub fn set_attestation(
			origin: OriginFor<T>,
			name: T::Hash,
			attestation: Option<T::AttestationId>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_running()?;
			if let Some(reference) = attestation.as_ref() {
				ensure!(
					T::AttestationReferenceValidator::is_live(reference),
					Error::<T>::InvalidAttestationReference
				);
			}
			let present = attestation.is_some();
			Self::mutate_authorized_record(&who, name, |record| record.attestation = attestation)?;
			Self::deposit_event(Event::AttestationSet { name, present });
			Ok(())
		}

		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::publish_content())]
		#[transactional]
		pub fn publish_content(
			origin: OriginFor<T>,
			name: T::Hash,
			content: Option<T::ContentCommitment>,
			expected_revision: Option<u64>,
			operation_id: [u8; 16],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_running()?;
			let request_hash = T::Hashing::hash_of(&(name, &content, expected_revision));
			if let Some(receipt) = ContentOperationReceipts::<T>::get(&who, operation_id) {
				ensure!(receipt.request_hash == request_hash, Error::<T>::OperationIdConflict);
				Self::deposit_event(Event::ContentSet {
					name: receipt.name,
					present: receipt.content.is_some(),
					revision: receipt.revision,
					operation_id,
					replayed: true,
				});
				return Ok(());
			}
			if let Some(reference) = content.as_ref() {
				ensure!(
					T::ContentReferenceValidator::contains(reference),
					Error::<T>::InvalidContentReference
				);
			}
			let current = ContentRevisions::<T>::get(name);
			if let Some(expected) = expected_revision {
				ensure!(expected == current, Error::<T>::ContentRevisionConflict);
			}
			let revision = current.checked_add(1).ok_or(Error::<T>::ContentRevisionConflict)?;
			let present = content.is_some();
			Self::mutate_authorized_record(&who, name, |record| record.content = content.clone())?;
			ContentRevisions::<T>::insert(name, revision);
			ContentOperationReceiptIds::<T>::try_mutate(
				&who,
				|ids| -> DispatchResult {
					ensure!(
						T::MaxContentOperationReceipts::get() > 0,
						Error::<T>::ContentOperationReceiptLimitDisabled
					);
					if ids.len() as u32 == T::MaxContentOperationReceipts::get() {
						let evicted = ids.remove(0);
						ContentOperationReceipts::<T>::remove(&who, evicted);
						Self::deposit_event(Event::ContentOperationReceiptPruned {
							owner: who.clone(),
							operation_id: evicted,
						});
					}
					ids.try_push(operation_id)
						.map_err(|_| Error::<T>::ContentOperationReceiptLimitDisabled)?;
					Ok(())
				},
			)?;
			ContentOperationReceipts::<T>::insert(
				&who,
				operation_id,
				ContentOperationReceipt { request_hash, name, content, revision },
			);
			Self::deposit_event(Event::ContentSet {
				name,
				present,
				revision,
				operation_id,
				replayed: false,
			});
			Ok(())
		}

		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::set_text())]
		#[transactional]
		pub fn set_text(
			origin: OriginFor<T>,
			name: T::Hash,
			key: TextKeyOf<T>,
			value: Option<TextValueOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_running()?;
			ensure!(!key.is_empty(), Error::<T>::EmptyRecord);
			if let Some(ref value) = value {
				ensure!(!value.is_empty(), Error::<T>::EmptyRecord);
			}
			Self::ensure_authorized(&who, name)?;
			let existed = TextRecords::<T>::contains_key(name, &key);
			match value {
				Some(value) => {
					if !existed {
						TextKeys::<T>::try_mutate(name, |keys| {
							keys.try_push(key.clone()).map_err(|_| Error::<T>::TooManyTextRecords)
						})?;
					}
					TextRecords::<T>::insert(name, &key, value);
				},
				None => {
					ensure!(existed, Error::<T>::EmptyRecord);
					TextRecords::<T>::remove(name, &key);
					TextKeys::<T>::mutate(name, |keys| {
						if let Some(position) = keys.iter().position(|candidate| candidate == &key)
						{
							keys.swap_remove(position);
						}
					});
				},
			}
			let present = TextRecords::<T>::contains_key(name, &key);
			Self::deposit_event(Event::TextSet { name, key, present });
			Ok(())
		}

		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::set_primary())]
		pub fn set_primary_name(origin: OriginFor<T>, name: Option<T::Hash>) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::ensure_running()?;
			if let Some(name_id) = name {
				let record = Names::<T>::get(name_id).ok_or(Error::<T>::NameNotFound)?;
				ensure!(record.owner == owner, Error::<T>::PrimaryNameInvalid);
				ensure!(Self::record_is_active(&record), Error::<T>::PrimaryNameInvalid);
				PrimaryName::<T>::insert(&owner, name_id);
			} else {
				PrimaryName::<T>::remove(&owner);
			}
			Self::deposit_event(Event::PrimaryNameSet { owner, name });
			Ok(())
		}

		#[pallet::call_index(14)]
		#[pallet::weight(T::WeightInfo::remove_name())]
		#[transactional]
		pub fn release(origin: OriginFor<T>, name: T::Hash) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::ensure_running()?;
			let record = Names::<T>::get(name).ok_or(Error::<T>::NameNotFound)?;
			ensure!(record.owner == owner, Error::<T>::NotOwner);
			ensure!(Children::<T>::get(name).is_empty(), Error::<T>::ChildrenRemain);
			Self::remove_name_state(name, record)?;
			Self::deposit_event(Event::NameReleased { name, owner });
			Ok(())
		}

		#[pallet::call_index(15)]
		#[pallet::weight(T::WeightInfo::remove_name())]
		#[transactional]
		pub fn remove_expired_name(origin: OriginFor<T>, name: T::Hash) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			let record = Names::<T>::get(name).ok_or(Error::<T>::NameNotFound)?;
			ensure!(!Self::record_is_active(&record), Error::<T>::NameTaken);
			ensure!(Children::<T>::get(name).is_empty(), Error::<T>::ChildrenRemain);
			Self::remove_name_state(name, record)?;
			Self::deposit_event(Event::ExpiredNameRemoved { name });
			Ok(())
		}

		#[pallet::call_index(16)]
		#[pallet::weight(T::WeightInfo::reservation())]
		pub fn reserve_name(
			origin: OriginFor<T>,
			parent: Option<T::Hash>,
			label: LabelOf<T>,
			beneficiary: Option<T::AccountId>,
			expires_at: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			Self::ensure_registrar_or_admin(origin)?;
			Self::ensure_valid_label(&label)?;
			let now = frame_system::Pallet::<T>::block_number();
			if let Some(expiry) = expires_at {
				ensure!(expiry > now, Error::<T>::InvalidReservationExpiry);
			}
			if let Some(parent_id) = parent {
				ensure!(Names::<T>::contains_key(parent_id), Error::<T>::ParentNotFound);
			}
			let name = Self::derive_name_id(parent, &label);
			if let Some(record) = Names::<T>::get(name) {
				ensure!(!Self::record_is_active(&record), Error::<T>::NameTaken);
			}
			Reservations::<T>::insert(
				name,
				Reservation { beneficiary: beneficiary.clone(), expires_at },
			);
			Self::deposit_event(Event::NameReserved { name, beneficiary, expires_at });
			Ok(())
		}

		#[pallet::call_index(17)]
		#[pallet::weight(T::WeightInfo::reservation())]
		pub fn clear_reservation(origin: OriginFor<T>, name: T::Hash) -> DispatchResult {
			Self::ensure_registrar_or_admin(origin)?;
			Reservations::<T>::remove(name);
			Self::deposit_event(Event::ReservationCleared { name });
			Ok(())
		}

		#[pallet::call_index(18)]
		#[pallet::weight(T::WeightInfo::reservation())]
		pub fn set_label_protection(
			origin: OriginFor<T>,
			label: LabelOf<T>,
			protected: bool,
		) -> DispatchResult {
			Self::ensure_registrar_or_admin(origin)?;
			Self::ensure_valid_label(&label)?;
			if protected {
				ProtectedLabels::<T>::insert(&label, ());
			} else {
				ProtectedLabels::<T>::remove(&label);
			}
			Self::deposit_event(Event::LabelProtectionSet { label, protected });
			Ok(())
		}

		#[pallet::call_index(19)]
		#[pallet::weight(T::WeightInfo::emergency())]
		pub fn set_paused(origin: OriginFor<T>, paused: bool) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			Paused::<T>::put(paused);
			Self::deposit_event(Event::PauseSet { paused });
			Ok(())
		}

		#[pallet::call_index(20)]
		#[pallet::weight(T::WeightInfo::emergency())]
		#[transactional]
		pub fn force_transfer(
			origin: OriginFor<T>,
			name: T::Hash,
			new_owner: T::AccountId,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let mut record = Names::<T>::get(name).ok_or(Error::<T>::NameNotFound)?;
			ensure!(record.owner != new_owner, Error::<T>::NotOwner);
			ensure!(
				OwnerNames::<T>::get(&new_owner).len() < T::MaxNamesPerOwner::get() as usize,
				Error::<T>::TooManyNames
			);
			let old_owner = record.owner.clone();
			OwnerNames::<T>::mutate(&old_owner, |names| {
				Self::remove_index_value::<T::MaxNamesPerOwner>(names, name)
			});
			OwnerNames::<T>::try_mutate(&new_owner, |names| {
				names.try_push(name).map_err(|_| Error::<T>::TooManyNames)
			})?;
			if PrimaryName::<T>::get(&old_owner) == Some(name) {
				PrimaryName::<T>::remove(&old_owner);
			}
			Controllers::<T>::remove(name);
			record.owner = new_owner.clone();
			Names::<T>::insert(name, record);
			Self::deposit_event(Event::NameTransferred { name, from: old_owner, to: new_owner });
			Ok(())
		}

		#[pallet::call_index(21)]
		#[pallet::weight(T::WeightInfo::emergency())]
		#[transactional]
		pub fn force_revoke(origin: OriginFor<T>, name: T::Hash) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let record = Names::<T>::get(name).ok_or(Error::<T>::NameNotFound)?;
			ensure!(Children::<T>::get(name).is_empty(), Error::<T>::ChildrenRemain);
			Self::remove_name_state(name, record)?;
			Self::deposit_event(Event::EmergencyNameRevoked { name });
			Ok(())
		}

		/// Governed delegation for the reservation/protection surface only.
		#[pallet::call_index(22)]
		#[pallet::weight(T::WeightInfo::reservation())]
		pub fn set_registrar(
			origin: OriginFor<T>,
			registrar: T::AccountId,
			enabled: bool,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			Registrars::<T>::try_mutate(|registrars| -> DispatchResult {
				match (registrars.iter().position(|candidate| candidate == &registrar), enabled) {
					(None, true) => registrars
						.try_push(registrar.clone())
						.map_err(|_| Error::<T>::TooManyRegistrars)?,
					(Some(index), false) => {
						registrars.swap_remove(index);
					},
					(Some(_), true) => return Ok(()),
					(None, false) => return Err(Error::<T>::RegistrarNotFound.into()),
				}
				Ok(())
			})?;
			Self::deposit_event(Event::RegistrarSet { registrar, enabled });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn ensure_registrar_or_admin(origin: OriginFor<T>) -> DispatchResult {
			match T::AdminOrigin::try_origin(origin) {
				Ok(_) => Ok(()),
				Err(origin) => {
					let who = ensure_signed(origin)?;
					ensure!(Registrars::<T>::get().contains(&who), Error::<T>::NotRegistrar);
					Ok(())
				},
			}
		}

		fn ensure_bootstrap_reservation_allows(
			label: &LabelOf<T>,
			owner: &T::AccountId,
		) -> DispatchResult {
			if let Some(reservation) = BootstrapReservations::<T>::get(label) {
				ensure!(reservation.beneficiary.as_ref() == Some(owner), Error::<T>::ReservedName);
			}
			Ok(())
		}

		pub fn label_policy_version() -> u16 {
			LABEL_POLICY_VERSION
		}

		/// Rejects rather than silently rewriting uppercase, Unicode, controls or edge hyphens.
		pub fn validate_label(raw: Vec<u8>) -> Result<LabelOf<T>, Error<T>> {
			let label: LabelOf<T> = raw.try_into().map_err(|_| Error::<T>::InvalidLabel)?;
			Self::ensure_valid_label(&label)?;
			Ok(label)
		}

		pub fn derive_name_id(parent: Option<T::Hash>, label: &LabelOf<T>) -> T::Hash {
			T::Hashing::hash_of(&(NAME_ID_DOMAIN, Self::genesis_hash(), parent, label))
		}

		pub fn registration_commitment(
			owner: &T::AccountId,
			parent: Option<T::Hash>,
			label: &LabelOf<T>,
			salt: &SaltOf<T>,
		) -> T::Hash {
			let name = Self::derive_name_id(parent, label);
			T::Hashing::hash_of(&(COMMITMENT_DOMAIN, Self::genesis_hash(), owner, name, salt))
		}

		pub fn is_name_active(name: T::Hash) -> bool {
			Names::<T>::get(name).is_some_and(|record| Self::record_is_active(&record))
		}

		/// Returns the stored primary name only if it is still active and owned by `owner`.
		pub fn primary_name(owner: &T::AccountId) -> Option<T::Hash> {
			let name = PrimaryName::<T>::get(owner)?;
			let record = Names::<T>::get(name)?;
			(record.owner == *owner && Self::record_is_active(&record)).then_some(name)
		}

		fn genesis_hash() -> T::Hash {
			frame_system::Pallet::<T>::block_hash(BlockNumberFor::<T>::zero())
		}

		fn ensure_running() -> DispatchResult {
			ensure!(!Paused::<T>::get(), Error::<T>::Paused);
			Ok(())
		}

		fn ensure_valid_label(label: &LabelOf<T>) -> Result<(), Error<T>> {
			let bytes = label.as_slice();
			if bytes.is_empty() || bytes.first() == Some(&b'-') || bytes.last() == Some(&b'-') {
				return Err(Error::<T>::InvalidLabel);
			}
			if bytes.iter().any(|byte| !matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-')) {
				return Err(Error::<T>::InvalidLabel);
			}
			Ok(())
		}

		fn record_is_active(record: &NameRecordOf<T>) -> bool {
			frame_system::Pallet::<T>::block_number() < record.expires_at
		}

		fn ensure_owner_active(owner: &T::AccountId, name: T::Hash) -> DispatchResult {
			let record = Names::<T>::get(name).ok_or(Error::<T>::NameNotFound)?;
			ensure!(record.owner == *owner, Error::<T>::NotOwner);
			ensure!(Self::record_is_active(&record), Error::<T>::NameExpired);
			Ok(())
		}

		fn ensure_authorized(who: &T::AccountId, name: T::Hash) -> DispatchResult {
			let record = Names::<T>::get(name).ok_or(Error::<T>::NameNotFound)?;
			let now = frame_system::Pallet::<T>::block_number();
			Self::ensure_authorized_record(who, name, &record, now)
		}

		fn ensure_authorized_record(
			who: &T::AccountId,
			name: T::Hash,
			record: &NameRecordOf<T>,
			now: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure!(now < record.expires_at, Error::<T>::NameExpired);
			ensure!(
				record.owner == *who || Controllers::<T>::get(name).contains(who),
				Error::<T>::NotAuthorized
			);
			Ok(())
		}

		fn mutate_authorized_record(
			who: &T::AccountId,
			name: T::Hash,
			mutator: impl FnOnce(&mut NameRecordOf<T>),
		) -> DispatchResult {
			let now = frame_system::Pallet::<T>::block_number();
			Names::<T>::try_mutate(name, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::NameNotFound)?;
				Self::ensure_authorized_record(who, name, record, now)?;
				mutator(record);
				Ok(())
			})
		}

		fn ensure_reservation_allows(
			name: T::Hash,
			owner: &T::AccountId,
			now: BlockNumberFor<T>,
		) -> DispatchResult {
			if let Some(reservation) = Reservations::<T>::get(name) {
				if reservation.expires_at.is_some_and(|expiry| now >= expiry) {
					Reservations::<T>::remove(name);
					return Ok(());
				}
				ensure!(reservation.beneficiary.as_ref() == Some(owner), Error::<T>::ReservedName);
			}
			Ok(())
		}

		fn decrement_commitments(owner: &T::AccountId) {
			CommitmentCount::<T>::mutate(owner, |count| *count = count.saturating_sub(1));
		}

		fn remove_name_state(name: T::Hash, record: NameRecordOf<T>) -> DispatchResult {
			ensure!(Children::<T>::get(name).is_empty(), Error::<T>::ChildrenRemain);
			OwnerNames::<T>::mutate(&record.owner, |names| {
				Self::remove_index_value::<T::MaxNamesPerOwner>(names, name)
			});
			if let Some(parent) = record.parent {
				Children::<T>::mutate(parent, |children| {
					Self::remove_index_value::<T::MaxChildrenPerName>(children, name)
				});
			} else {
				RootNames::<T>::mutate(|names| {
					Self::remove_index_value::<T::MaxRootNames>(names, name)
				});
			}
			if PrimaryName::<T>::get(&record.owner) == Some(name) {
				PrimaryName::<T>::remove(&record.owner);
			}
			for key in TextKeys::<T>::take(name) {
				TextRecords::<T>::remove(name, key);
			}
			Controllers::<T>::remove(name);
			Children::<T>::remove(name);
			ContentRevisions::<T>::remove(name);
			Names::<T>::remove(name);
			Ok(())
		}

		fn remove_index_value<Bound: Get<u32>>(
			values: &mut BoundedVec<T::Hash, Bound>,
			value: T::Hash,
		) {
			if let Some(position) = values.iter().position(|candidate| candidate == &value) {
				values.swap_remove(position);
			}
		}
	}
}
