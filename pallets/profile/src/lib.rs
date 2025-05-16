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
//

//! # Pallet Profile
//!
//! The Profle pallet provides a framework for creating and managing
//! identities within the CORD blockchain that can be persisted and not tied
//! for a specific account.
//! Pallet Profile allows to create identities tied to a unique CORD URI which
//! can be watched/subscribed on its activities.
//! Pallet Profile allows for rotation of keys without losing the data tied with
//! account created.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub mod types;

extern crate alloc;
use alloc::{str, string::String};

pub use crate::{pallet::*, types::*};
use codec::Encode;
use frame_support::{
	ensure, pallet_prelude::DispatchResult, storage::types::StorageMap, BoundedVec,
};
use pallet_identifier::{EventBlock, EventTypeOf, Identifier};
use sp_runtime::traits::Hash;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{IsPermissioned, Ss58Identifier, StatusOf};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use frame_system::WeightInfo;

	use sp_runtime::Vec;

	/// DataKeyOf is the type of Profile Data Key.
	pub type DataKeyOf<T> = BoundedVec<u8, <T as Config>::MaxDataKeyLength>;

	/// DataValueOf is the type of the Profile Data Value.
	pub type DataValueOf<T> = BoundedVec<u8, <T as Config>::MaxDataValueLength>;

	/// Type of the Profile Identifier is a Ss58.
	pub type ProfileIdOf = Ss58Identifier;

	/// CreatorOf is the creator/holder of the Profile/
	pub type CreatorOf<T> = <T as frame_system::Config>::AccountId;

	/// Profile Metadata Of is the type wrapper for Profile Metadata struct.
	/// Currently only holds latest-key.
	pub type ProfileMetadataOf<T> = ProfileMetadata<CreatorOf<T>>;

	#[pallet::config]
	// TODO: Check workaround of not having TypeInfo here
	pub trait Config:
		frame_system::Config + scale_info::TypeInfo + pallet_identifier::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type MaxDataKeyLength: Get<u32>;

		#[pallet::constant]
		type MaxDataValueLength: Get<u32>;

		type WeightInfo: frame_system::WeightInfo;
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Storage to map Profile Identifier and
	/// its associated metadata.
	#[pallet::storage]
	pub type Profiles<T: Config> =
		StorageMap<_, Blake2_128Concat, ProfileIdOf, ProfileMetadataOf<T>, OptionQuery>;

	/// Storage to map the latest CORD account and
	/// its associated Profile Identifier.
	#[pallet::storage]
	pub type AccountProfiles<T> =
		StorageMap<_, Blake2_128Concat, CreatorOf<T>, ProfileIdOf, OptionQuery>;

	/// Storage to map Profile Identifier and
	/// the actual Profile Data HashMap.
	#[pallet::storage]
	pub type ProfileData<T> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		ProfileIdOf,
		Blake2_128Concat,
		DataKeyOf<T>,
		DataValueOf<T>,
		OptionQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// The length of the identifier exceeds capacity limit.
		InvalidIdentifierLength,
		/// The Profile data key must start with 'pub_'
		InvalidKeyPrefix,
		/// The Profile already exists with a identifier.
		ProfileAlreadyExists,
		/// The Profile Identifier does not exist.
		ProfileNotFound,
		/// The event activity update has failed.
		EventUpdateFailed,
		/// The provided event type is invalid.
		InvalidEventType,
		/// Rotation must be to new key, not to existing tied account.
		CannotRotateToSameAccount,
		/// Storage transaction has failed abrubtly.
		TransactionFailed,
		// State Update Failed
		StateUpdateFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new profile has been created.
		/// \[Creator, Profile Identifier\]
		ProfileSet { who: CreatorOf<T>, identifier: ProfileIdOf },

		/// Existing Profile's Key has been rotated.
		/// \[Old Profile Holder, Profile Identifier, New public of the Holder]
		KeyRotated { who: CreatorOf<T>, identifier: ProfileIdOf, new_key: CreatorOf<T> },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Creates a new Profile with associated details.
		///
		/// This extrinsic creates a new Profile to be mapped to a CORD account.
		/// Also it expects a vector of Data Keys and its values.
		/// This HashMap is tied to the generated Profile Identifier.
		/// But currently expects the user given Data Keys to start with `pub_` prefix only.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by the creator of the profile.
		/// - `data`: The data is a vector of a HashMap of the Data Key and associated Data Values.
		///
		/// # Returns
		/// Returns `Ok(())` if the Profile is succesfully been created.
		/// or an `Err` if the operation fails due to a same profile already existing.
		///
		/// # Errors
		/// - `InvalidIdentifierLength`: If the newly creted Profile Identifier exceeds limit.
		/// - `ProfileAlreadyExists`: If the newly created Profile already exists. This happens if a
		///   user creates a duplicate profile with the same account key-pair.
		/// - `InvalidKeyPrefix`: If the Profile Data Key starts with anything other than `pub_`.
		#[pallet::call_index(0)]
		#[pallet::weight({10_000})]
		pub fn set_profile(
			origin: OriginFor<T>,
			data: Vec<(DataKeyOf<T>, DataValueOf<T>)>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let digest = T::Hashing::hash(&who.encode());
			let pallet_name = <Self as frame_support::traits::PalletInfoAccess>::name();
			let profile_id = <pallet_identifier::Pallet<T> as Identifier<T>>::build(
				&(digest).encode()[..],
				pallet_name,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!Profiles::<T>::contains_key(&profile_id), Error::<T>::ProfileAlreadyExists);

			for (key, _) in &data {
				let key_str = str::from_utf8(key.as_slice())
					.map_err(|_| Error::<T>::InvalidKeyPrefix)
					.map(String::from)?;
				ensure!(key_str.starts_with("pub_"), Error::<T>::InvalidKeyPrefix);
			}

			let profile = ProfileMetadataOf::<T> { latest_key: who.clone() };
			Profiles::<T>::insert(&profile_id, profile);
			AccountProfiles::<T>::insert(&who, &profile_id);

			for (key, value) in data {
				ProfileData::<T>::insert(&profile_id, &key, value);
			}

			Self::record_activity(&profile_id, digest, b"ProfileSet")?;
			Self::deposit_event(Event::ProfileSet { who: who.clone(), identifier: profile_id });

			Ok(())
		}

		/// Rotates the key of an existing Profile.
		///
		/// This extrinsic rotates the key of an existing Profile, identified through its
		/// Profile Identifier.
		/// This must be signed by the account tied to the Profile only.
		/// The `new-key` will be the public-key of the CORD/Substarte account tied to the Profile
		/// from here on. So all new operations will require `new-key` based acconut only.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by the owner of the profile.
		/// - `new_key`: The new_key represnts the public key of the CORD/Substrate account for
		///   which The existing Profile Identifier will be tied to from here on. Make Sure this
		///   public key exists and accessible before making this call. This is irreversible and
		///   will tie all existing Profile Data to new-key based account.
		///
		/// # Returns
		/// Returns `Ok(())` if the Profile is succesfully rotated to new-key.
		/// or an `Err` if the operation fails due to issues regarding Profile existence.
		///
		/// # Errors
		/// - `ProfileNotFound`: If the Profile does not exist for the signed origin account.
		#[pallet::call_index(1)]
		#[pallet::weight({10_000})]
		pub fn rotate_key(origin: OriginFor<T>, new_key: CreatorOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let profile_id = AccountProfiles::<T>::get(&who).ok_or(Error::<T>::ProfileNotFound)?;

			let mut profile = Profiles::<T>::get(&profile_id).ok_or(Error::<T>::ProfileNotFound)?;
			let old_key = profile.latest_key;

			ensure!(old_key != new_key, Error::<T>::CannotRotateToSameAccount);
			let digest = T::Hashing::hash(&new_key.encode());

			profile.latest_key = new_key.clone();

			Profiles::<T>::insert(&profile_id, profile);

			AccountProfiles::<T>::insert(&new_key, &profile_id);
			AccountProfiles::<T>::remove(&who);

			Self::record_activity(&profile_id, digest, b"KeyRotated")?;

			Self::deposit_event(Event::KeyRotated { who, identifier: profile_id, new_key });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Retrieves the Profile Identifier associated with the account if exists,
	/// Else returns appropriate errors.
	pub fn get_profile_id(who: &CreatorOf<T>) -> Result<ProfileIdOf, Error<T>> {
		let profile_id = AccountProfiles::<T>::get(&who).ok_or(Error::<T>::ProfileNotFound)?;
		Profiles::<T>::get(&profile_id).ok_or(Error::<T>::ProfileNotFound)?;

		Ok(profile_id)
	}

	/// Checks if the given profile exists, else returns appropriate error of `ProfileNotFound`.
	pub fn does_profile_exists(profile_id: &ProfileIdOf) -> Result<ProfileIdOf, Error<T>> {
		Profiles::<T>::get(&profile_id).ok_or(Error::<T>::ProfileNotFound)?;

		Ok(profile_id.clone())
	}

	/// Records an activity using a provided event message.
	pub fn record_activity(
		identifier: &Ss58Identifier,
		digest: T::Hash,
		msg: &[u8],
	) -> DispatchResult {
		let action: EventTypeOf =
			msg.to_vec().try_into().map_err(|_| Error::<T>::InvalidEventType)?;
		let stamp = EventBlock::current::<T>();
		<pallet_identifier::Pallet<T> as Identifier<T>>::state_event(
			identifier, digest, action, stamp,
		)
		.map_err(|_| Error::<T>::StateUpdateFailed)?;
		Ok(())
	}
}

// TODO:
// 1. Have a way through runtime constants where we would be able to configure deposits for ops.
