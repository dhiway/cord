// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

//! # Pallet storing unique nickname <-> DID links for user-friendly DID
//! nicknames.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod did_name;

pub mod weights;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(test)]
pub mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub use crate::{pallet::*, weights::WeightInfo};

#[frame_support::pallet]
pub mod pallet {
	use codec::FullCodec;
	use frame_support::{
		pallet_prelude::*, sp_runtime::SaturatedConversion, traits::StorageVersion,
		Blake2_128Concat,
	};
	use frame_system::pallet_prelude::*;
	use sp_std::{fmt::Debug, vec::Vec};

	use cord_utilities::traits::CallSources;

	use super::WeightInfo;
	use crate::did_name::DidNameOwnership;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	pub type DidNameOwnerOf<T> = <T as Config>::DidNameOwner;
	pub type DidNameInput<T> = BoundedVec<u8, <T as Config>::MaxNameLength>;
	pub type DidNameOf<T> = <T as Config>::DidName;
	pub type DidNameOwnershipOf<T> = DidNameOwnership<DidNameOwnerOf<T>, BlockNumberFor<T>>;

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// Map of name -> ownership details.
	#[pallet::storage]
	pub type Owner<T> = StorageMap<_, Blake2_128Concat, DidNameOf<T>, DidNameOwnershipOf<T>>;

	/// Map of owner -> name.
	#[pallet::storage]
	pub type Names<T> = StorageMap<_, Blake2_128Concat, DidNameOwnerOf<T>, DidNameOf<T>>;

	/// Map of name -> ().
	///
	/// If a name key is present, the name is currently banned.
	#[pallet::storage]
	pub type Banned<T> = StorageMap<_, Blake2_128Concat, DidNameOf<T>, ()>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type BanOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, DidNameOwnerOf<Self>>;
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The type of a name.
		type DidName: FullCodec
			+ Debug
			+ PartialEq
			+ Clone
			+ TypeInfo
			+ TryFrom<Vec<u8>, Error = Error<Self>>
			+ MaxEncodedLen;
		/// The type of a name owner.
		type DidNameOwner: Parameter + MaxEncodedLen;
		/// The min encoded length of a name.
		#[pallet::constant]
		type MinNameLength: Get<u32>;
		/// The max encoded length of a name.
		#[pallet::constant]
		type MaxNameLength: Get<u32>;
		/// The max encoded length of a prefix.
		#[pallet::constant]
		type MaxPrefixLength: Get<u32>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new name has been claimed.
		DidNameRegistered { owner: DidNameOwnerOf<T>, name: DidNameOf<T> },
		/// A name has been released.
		DidNameReleased { owner: DidNameOwnerOf<T>, name: DidNameOf<T> },
		/// A name has been banned.
		DidNameBanned { name: DidNameOf<T> },
		/// A name has been unbanned.
		DidNameUnbanned { name: DidNameOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The tx submitter does not have enough funds to pay for the deposit.
		InsufficientFunds,
		/// The specified name has already been previously claimed.
		AlreadyExists,
		/// The specified name does not exist.
		NotFound,
		/// The specified owner already owns a name.
		OwnerAlreadyExists,
		/// The specified owner does not own any names.
		OwnerNotFound,
		/// The specified name has been banned and cannot be interacted
		/// with.
		Banned,
		/// The specified name is not currently banned.
		NotBanned,
		/// The specified name has already been previously banned.
		AlreadyBanned,
		/// The actor cannot performed the specified operation.
		NotAuthorized,
		/// A name that is too short is being claimed.
		NameTooShort,
		/// A name that is too long is being claimed.
		NameExceedsMaxLength,
		/// A prefix that is too short is being claimed.
		NamePrefixTooShort,
		/// A prefix that is too long is being claimed.
		NamePrefixTooLong,
		/// A suffix that is too short is being claimed.
		InvalidSuffix,
		/// A suffix that is too long is being claimed.
		SuffixTooLong,
		/// A name that contains not allowed characters is being claimed.
		InvalidFormat,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Assign the specified name to the owner as specified in the
		/// origin.
		///
		/// The name must not have already been registered by someone else and
		/// the owner must not already own another name.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::register(name.len().saturated_into()))]
		pub fn register(origin: OriginFor<T>, name: DidNameInput<T>) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let decoded_name = Self::check_claiming_preconditions(name, &owner)?;

			Self::register_name(decoded_name.clone(), owner.clone());
			Self::deposit_event(Event::<T>::DidNameRegistered { owner, name: decoded_name });

			Ok(())
		}

		/// Release the provided name from its owner.
		///
		/// The origin must be the owner of the specified name.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::release())]
		pub fn release(origin: OriginFor<T>) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let owned_name = Self::check_releasing_preconditions(&owner)?;

			Self::unregister_name(&owned_name);
			Self::deposit_event(Event::<T>::DidNameReleased { owner, name: owned_name });

			Ok(())
		}

		/// Ban a name.
		///
		/// A banned name cannot be registered by anyone.
		///
		/// The origin must be the ban origin.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::ban(name.len().saturated_into()))]
		pub fn ban(origin: OriginFor<T>, name: DidNameInput<T>) -> DispatchResult {
			T::BanOrigin::ensure_origin(origin)?;

			let (decoded_name, is_claimed) = Self::check_banning_preconditions(name)?;

			if is_claimed {
				Self::unregister_name(&decoded_name);
			}

			Self::ban_name(&decoded_name);
			Self::deposit_event(Event::<T>::DidNameBanned { name: decoded_name });

			Ok(())
		}

		/// Unban a name.
		///
		/// Make a name available again.
		///
		/// The origin must be the ban origin.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::unban(name.len().saturated_into()))]
		pub fn unban(origin: OriginFor<T>, name: DidNameInput<T>) -> DispatchResult {
			T::BanOrigin::ensure_origin(origin)?;

			let decoded_name = Self::check_unbanning_preconditions(name)?;

			Self::unban_name(&decoded_name);
			Self::deposit_event(Event::<T>::DidNameUnbanned { name: decoded_name });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Verify that the claiming preconditions are verified. Specifically:
		/// - The name input data can be decoded as a valid name
		/// - The name does not already exist
		/// - The owner does not already own a name
		/// - The name has not been banned
		fn check_claiming_preconditions(
			name_input: DidNameInput<T>,
			owner: &DidNameOwnerOf<T>,
		) -> Result<DidNameOf<T>, DispatchError> {
			let name =
				DidNameOf::<T>::try_from(name_input.into_inner()).map_err(DispatchError::from)?;

			ensure!(!Names::<T>::contains_key(owner), Error::<T>::OwnerAlreadyExists);
			ensure!(!Owner::<T>::contains_key(&name), Error::<T>::AlreadyExists);
			ensure!(!Banned::<T>::contains_key(&name), Error::<T>::Banned);

			Ok(name)
		}

		/// Assign a name to the provided owner reserving the deposit from
		/// the provided account. This function must be called after
		/// `check_claiming_preconditions` as it does not verify all the
		/// preconditions again.
		pub(crate) fn register_name(name: DidNameOf<T>, owner: DidNameOwnerOf<T>) {
			let block_number = frame_system::Pallet::<T>::block_number();

			Names::<T>::insert(&owner, name.clone());
			Owner::<T>::insert(
				&name,
				DidNameOwnershipOf::<T> { owner, registered_at: block_number },
			);
		}

		/// Verify that the releasing preconditions for an owner are verified.
		/// Specifically:
		/// - The owner has a previously claimed name
		fn check_releasing_preconditions(
			owner: &DidNameOwnerOf<T>,
		) -> Result<DidNameOf<T>, DispatchError> {
			let name = Names::<T>::get(owner).ok_or(Error::<T>::OwnerNotFound)?;

			Ok(name)
		}

		/// Release the provided name and returns the deposit to the
		/// original payer. This function must be called after
		/// `check_releasing_preconditions` as it does not verify all the
		/// preconditions again.
		fn unregister_name(name: &DidNameOf<T>) -> DidNameOwnershipOf<T> {
			let name_ownership = Owner::<T>::take(name).unwrap();
			Names::<T>::remove(&name_ownership.owner);

			name_ownership
		}

		/// Verify that the banning preconditions are verified.
		/// Specifically:
		/// - The name input data can be decoded as a valid name
		/// - The name must not be already banned
		///
		/// If the preconditions are verified, return
		/// a tuple containing the parsed name value and whether the name
		/// being banned is currently assigned to someone or not.
		fn check_banning_preconditions(
			name_input: DidNameInput<T>,
		) -> Result<(DidNameOf<T>, bool), DispatchError> {
			let name =
				DidNameOf::<T>::try_from(name_input.into_inner()).map_err(DispatchError::from)?;

			ensure!(!Banned::<T>::contains_key(&name), Error::<T>::AlreadyBanned);

			let is_claimed = Owner::<T>::contains_key(&name);

			Ok((name, is_claimed))
		}

		/// Ban the provided name. This function must be called after
		/// `check_banning_preconditions` as it does not verify all the
		/// preconditions again.
		pub(crate) fn ban_name(name: &DidNameOf<T>) {
			Banned::<T>::insert(name, ());
		}

		/// Verify that the unbanning preconditions are verified.
		/// Specifically:
		/// - The name input data can be decoded as a valid name
		/// - The name must have already been banned
		fn check_unbanning_preconditions(
			name_input: DidNameInput<T>,
		) -> Result<DidNameOf<T>, DispatchError> {
			let name =
				DidNameOf::<T>::try_from(name_input.into_inner()).map_err(DispatchError::from)?;

			ensure!(Banned::<T>::contains_key(&name), Error::<T>::NotBanned);

			Ok(name)
		}

		/// Unban the provided name. This function must be called after
		/// `check_unbanning_preconditions` as it does not verify all the
		/// preconditions again.
		fn unban_name(name: &DidNameOf<T>) {
			Banned::<T>::remove(name);
		}
	}
}
