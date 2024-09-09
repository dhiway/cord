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

//! # Entries Pallet - Part of `DeDir (Decentralized Directory)`.
//!
//!
//! ## Overview
//!
//! The Entries pallet which is part of the `DeDir (Decentralized Directory)` aims to implement a
//! decentralized version of a Registry Entry (record). Enabling creation, updation of entries in a
//! decentralized manner. Thereby enabling trust and transperency of Registries utilizing CORD
//! blockchain.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `create` - Creates a new Registry Entry.
//! * `update` - Updates a existing Registry Entry.
//! * `update_state` - Updates the state of a existing Registry Entry.

#![cfg_attr(not(feature = "std"), no_std)]

mod types;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(test)]
mod tests;

use frame_support::{
	ensure,
	pallet_prelude::DispatchResult,
	traits::{Get, StorageVersion},
	BoundedVec,
};

pub use pallet::*;
use sp_std::{prelude::*, str};

//use sp_runtime::traits::Hash;

pub use frame_system::WeightInfo;
pub use types::{RegistryEntryDetails, RegistryEntrySupportedStateOf};

//use codec::Encode;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use identifier::{
		CordIdentifierType, IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier,
	};

	pub type RegistryEntryHashOf<T> = <T as frame_system::Config>::Hash;

	pub type RegistryIdOf = Ss58Identifier;
	pub type RegistryEntryIdOf = Ss58Identifier;

	pub type MaxRegistryEntryBlobSizeOf<T> = <T as crate::Config>::MaxRegistryEntryBlobSize;

	pub type CreatorOf<T> = <T as frame_system::Config>::AccountId;

	pub type RegistryEntryBlobOf<T> = BoundedVec<u8, MaxRegistryEntryBlobSizeOf<T>>;

	pub type RegistryEntryDetailsOf<T> = RegistryEntryDetails<
		RegistryEntryHashOf<T>,
		RegistryEntrySupportedStateOf,
		CreatorOf<T>,
		RegistryIdOf,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_registries::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The maximum number of bytes in size a Registry Entry Blob can hold.
		#[pallet::constant]
		type MaxRegistryEntryBlobSize: Get<u32>;

		/// The maximum encoded length available for naming.
		#[pallet::constant]
		type MaxEncodedInputLength: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// Storage for Registry Entries.
	/// It maps Registry Entry Identifier to Registry Entry Details.
	#[pallet::storage]
	pub type RegistryEntries<T: Config> =
		StorageMap<_, Blake2_128Concat, RegistryEntryIdOf, RegistryEntryDetailsOf<T>, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid Identifer Length
		InvalidIdentifierLength,
		/// Identifier Invalid or Not of DeDir Type
		InvalidRegistryEntryIdentifier,
		/// Account has no valid authorization
		UnauthorizedOperation,
		/// Registry Entry Identifier Already Exists
		RegistryEntryIdAlreadyExists,
		/// Registry Entry Identifier Does Not Exists
		RegistryEntryIdDoesNotExist,
		/// State Not Found In Declared Registry
		StateNotSupported,
		/// Blob and Digest Does not match.
		BlobDoesNotMatchDigest,
		/// Registry Entry Already exists in same state.
		RegistryEntryAlreadyInSameState,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new registry entry has been created.
		/// \[creator, registry_identifier, registry_entry_identifier\]
		RegistryEntryCreated {
			creator: T::AccountId,
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
		},

		/// A existing registry entry state has been updated.
		/// \[who, registry_entry_identifier, new_state\]
		RegistryEntryStateChanged {
			who: T::AccountId,
			registry_entry_id: RegistryEntryIdOf,
			new_state: RegistryEntrySupportedStateOf,
		},

		/// A existing registry entry has been updated.
		/// \[updater, registry_entry_identifier\]
		RegistryEntryUpdated { updater: T::AccountId, registry_entry_id: RegistryEntryIdOf },
	}

	#[pallet::call]
	/// Entries pallet declaration.
	impl<T: Config> Pallet<T> {
		/// Creates a new Registry Entry within a specified Registry.
		///
		/// This function allows a user to create a new entry within an existing Registry. The entry
		/// is identified by the provided `registry_entry_id`, which is derived and managed
		/// externally. The function verifies that the caller is authorized to add an entry to the
		/// Registry and ensures that the entry does not already exist. If valid, the entry is
		/// stored along with the associated data (`digest`, optional `blob`, and optional
		/// `state`).
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be a signed account (creator of the
		///   entry).
		/// * `registry_id` - The unique identifier for the Registry in which the new entry will be
		///   created.
		/// * `registry_entry_id` - The unique identifier for the new Registry entry. Must not
		///   already exist.
		/// * `digest` - The hash value or digest of the content associated with the Registry entry.
		/// * `blob` - (Optional) Additional data associated with the Registry entry.
		/// * `state` - (Optional) The initial state of the Registry entry. Defaults to `ACTIVE` if
		///   not provided.
		///
		/// # Errors
		/// This function returns an error in the following cases:
		/// * `UnauthorizedOperation` - If the caller does not have permission to create entries
		///   within the Registry.
		/// * `RegistryEntryIdAlreadyExists` - If the `registry_entry_id` already exists in the
		///   storage.
		/// * `InvalidRegistryEntryIdentifier` - If the `registry_entry_id` is not a valid SS58
		///   identifier or not of type `RegistryEntry`.
		///
		/// # Events
		/// Emits the `Event::RegistryEntryCreated` event upon successful creation of a new Registry
		/// entry. This event includes the `creator`, `registry_id`, and the `registry_entry_id`
		/// of the new entry.
		///
		/// # Example
		/// ```
		/// create(origin, registry_id, registry_entry_id, digest, Some(blob), Some(state))?;
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
			digest: RegistryEntryHashOf<T>,
			_blob: Option<RegistryEntryBlobOf<T>>,
			state: Option<RegistryEntrySupportedStateOf>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let registry = pallet_registries::Pallet::<T>::ensure_valid_registry_id(&registry_id)
				.map_err(<pallet_registries::Error<T>>::from)?;

			/* Ensure `creator` is either a OWNER/ADMIN/DELEGATE of the Registry */
			let is_delegate_authorized =
				pallet_registries::Pallet::<T>::ensure_delegate_authorization(&registry, &creator);
			ensure!(is_delegate_authorized, Error::<T>::UnauthorizedOperation,);

			/* Identifier Management will happen at SDK.
			 * It is to be constructed as below.
			 */
			// let mut id_digest = <T as frame_system::Config>::Hashing::hash(
			// 		&[&registry_id.encode()[..], &digest.encode()[..]].concat()[..],
			// 	);
			// if blob.is_some() {
			// 	id_digest = <T as frame_system::Config>::Hashing::hash(
			// 		&[&registry_id.encode()[..], &digest.encode()[..],
			// &blob.encode()[..]].concat()[..], 	);
			// }
			// let registry_entry_id =
			// 	Ss58Identifier::create_identifier(&(id_digest).encode()[..],
			// IdentifierType::Registries) 		.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			/* Ensure that the registry_entry_id does not already exist */
			ensure!(
				!RegistryEntries::<T>::contains_key(&registry_entry_id),
				Error::<T>::RegistryEntryIdAlreadyExists
			);

			/* Ensure that registry_id is of valid ss58 format,
			 * and also the type matches to be of `Registry`
			 */
			ensure!(
				Self::is_valid_ss58_format(&registry_entry_id),
				Error::<T>::InvalidRegistryEntryIdentifier
			);

			let registry_entry = RegistryEntryDetails {
				digest,
				state: state.unwrap_or(RegistryEntrySupportedStateOf::ACTIVE),
				creator: creator.clone(),
				registry_id: registry_id.clone(),
			};

			RegistryEntries::<T>::insert(&registry_entry_id, registry_entry);

			Self::deposit_event(Event::RegistryEntryCreated {
				creator,
				registry_id,
				registry_entry_id,
			});

			Ok(())
		}

		/// Updates an existing Registry Entry with new metadata or state.
		///
		/// This function allows an authorized user to update the metadata (such as the `digest` or
		/// `blob`) or the `state` of an existing Registry Entry. The user must have the necessary
		/// permissions to perform this operation, which includes being either the creator of the
		/// entry or a delegate with OWNER or ADMIN permissions in the Registry.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be a signed account (updater).
		/// * `registry_entry_id` - The unique identifier of the Registry Entry to be updated.
		/// * `digest` - The new hash value or digest to be associated with the Registry Entry.
		/// * `blob` - (Optional) New additional data to be associated with the Registry Entry.
		/// * `state` - (Optional) The new state of the Registry Entry.
		///
		/// # Errors
		/// This function returns an error in the following cases:
		/// * `UnauthorizedOperation` - If the caller does not have permission to update the
		///   Registry Entry.
		/// * `RegistryEntryIdDoesNotExist` - If the specified `registry_entry_id` does not exist.
		/// * `StateNotSupported` - If the provided `state` is invalid.
		///
		/// # Events
		/// Emits the `Event::RegistryEntryUpdated` event upon successful update of the Registry
		/// Entry. This event includes the `updater` and the `registry_entry_id`.
		///
		/// # Example
		/// ```
		/// update(origin, registry_entry_id, digest, Some(blob), Some(state))?;
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn update(
			origin: OriginFor<T>,
			registry_entry_id: RegistryEntryIdOf,
			digest: RegistryEntryHashOf<T>,
			blob: Option<RegistryEntryBlobOf<T>>,
			state: Option<RegistryEntrySupportedStateOf>,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;

			/* Ensure RegistryEntry exists */
			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdDoesNotExist)?;

			let registry =
				pallet_registries::Pallet::<T>::ensure_valid_registry_id(&entry.registry_id)
					.map_err(<pallet_registries::Error<T>>::from)?;

			let is_owner_or_admin =
				pallet_registries::Pallet::<T>::ensure_owner_or_admin_authorization(
					&registry, &updater,
				);

			let is_authorized = is_owner_or_admin || entry.creator == updater;

			/* Ensure the account is either a OWNER/ADMIN or
			 * the creator of the Registry Entry.
			 */
			ensure!(is_authorized, Error::<T>::UnauthorizedOperation,);

			// TODO: Should the updater mandatorily part of the `delegates` list?
			// /* Ensure the `updater` is part of the delegate list */
			// let is_part_of_delegate_list = registry.delegates.iter().any(|delegate_info|
			// delegate_info.delegate == updater); /* Check if the updater is either:
			// * 1. An OWNER/ADMIN and part of the delegate list, OR
			// * 2. The creator of the registry entry and part of the delegate list.
			// */
			// let is_authorized = (is_owner_or_admin && is_part_of_delegate_list) ||
			//     (entry.creator == updater && is_part_of_delegate_list);

			entry.digest = digest;

			if let Some(_new_blob) = blob {
				/* TODO:
				 * Handle blob updates..
				 */
			}

			if let Some(new_state) = state {
				// TODO:
				// Is there a need to check for state already exists in same state?
				ensure!(new_state.is_valid_state(), Error::<T>::StateNotSupported);
				entry.state = new_state;
			}

			RegistryEntries::<T>::insert(&registry_entry_id, entry);

			/* Emit an event for successful entry state update */
			Self::deposit_event(Event::RegistryEntryUpdated { updater, registry_entry_id });

			Ok(())
		}

		/// Updates the state of an existing Registry Entry.
		///
		/// This function allows an authorized user to update the state of an existing
		/// Registry Entry. The user must either be the creator of the entry or a account
		/// with OWNER or ADMIN permissions in the associated Registry.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be a signed account (the updater).
		/// * `registry_entry_id` - The unique identifier of the Registry Entry to update.
		/// * `new_state` - The new state to be assigned to the Registry Entry. Must be one of the
		///   supported states.
		///
		/// # Errors
		/// This function will return an error in the following cases:
		/// * `RegistryEntryIdDoesNotExist` - If the specified `registry_entry_id` does not exist.
		/// * `StateNotSupported` - If the `new_state` is not part of the supported states.
		/// * `UnauthorizedOperation` - If the caller is not authorized to update the Registry
		///   Entry.
		/// * `RegistryEntryAlreadyInSameState` - If the new state is the same as the current state
		///   of the entry.
		///
		/// # Events
		/// Emits the `Event::RegistryEntryStateChanged` event upon successful update of the
		/// Registry Entry's state. This event includes the `who` (updater), the
		/// `registry_entry_id`, and the `new_state`.
		///
		/// # Example
		/// ```
		/// update_state(origin, registry_entry_id, new_state)?;
		/// ```
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn update_state(
			origin: OriginFor<T>,
			registry_entry_id: RegistryEntryIdOf,
			new_state: RegistryEntrySupportedStateOf,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			/* Ensure RegistryEntry exists */
			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdDoesNotExist)?;

			/* Ensure given `new_state` is part of supported states enum */
			ensure!(new_state.is_valid_state(), Error::<T>::StateNotSupported);

			if entry.state == new_state {
				return Err(Error::<T>::RegistryEntryAlreadyInSameState.into());
			}

			let registry =
				pallet_registries::Pallet::<T>::ensure_valid_registry_id(&entry.registry_id)
					.map_err(<pallet_registries::Error<T>>::from)?;

			let is_owner_or_admin =
				pallet_registries::Pallet::<T>::ensure_owner_or_admin_authorization(
					&registry, &who,
				);

			let is_authorized = is_owner_or_admin || entry.creator == who;

			/* Ensure the account is either a OWNER/ADMIN or
			 * the creator of the Registry Entry.
			 */
			ensure!(is_authorized, Error::<T>::UnauthorizedOperation,);

			// TODO: Should the updater mandatorily part of the `delegates` list?
			// /* Ensure the `updater` is part of the delegate list */
			// let is_part_of_delegate_list = registry.delegates.iter().any(|delegate_info|
			// delegate_info.delegate == updater); /* Check if the updater is either:
			// * 1. An OWNER/ADMIN and part of the delegate list, OR
			// * 2. The creator of the registry entry and part of the delegate list.
			// */
			// let is_authorized = (is_owner_or_admin && is_part_of_delegate_list) ||
			//     (entry.creator == updater && is_part_of_delegate_list);

			/* Update the state */
			entry.state = new_state.clone();
			RegistryEntries::<T>::insert(&registry_entry_id, entry);

			/* Emit an event for successful entry state update */
			Self::deposit_event(Event::RegistryEntryStateChanged {
				who,
				registry_entry_id,
				new_state,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Method to check if the input identifier calculated from sdk
	/// is actually a valid SS58 Identifier Format and of valid type `Entries`.
	pub fn is_valid_ss58_format(identifier: &Ss58Identifier) -> bool {
		match identifier.get_type() {
			Ok(id_type) =>
				if id_type == IdentifierType::Entries {
					log::debug!("The SS58 identifier is of type Entries.");
					true
				} else {
					log::debug!("The SS58 identifier is not of type Entries.");
					false
				},
			Err(e) => {
				log::debug!("Invalid SS58 identifier. Error: {:?}", e);
				false
			},
		}
	}

	// /// Method to check if blob matches the given digest
	// pub fn does_blob_matches_digest(blob: &RegistryEntryBlobOf<T>, digest:
	// &RegistryEntryHashOf<T>) -> bool { 	let blob_digest = <T as
	// frame_system::Config>::Hashing::hash(&blob.encode()[..]);

	// 	log::info!("digest: {:?}, blob_digest: {:?}, blob: {:?}", *digest, blob_digest, blob);

	// 	blob_digest == *digest
	// }
}
