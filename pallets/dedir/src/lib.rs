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

//! # Decentralized Directory Pallet - DeDir
//!
//!
//! ## Overview
//!
//! The Decentralized Directory Pallet (DeDir) aims to implement a decentralized
//! version of a registry. Enabling creation, updation of registries in a distributed
//! decentralized manner. Thereby enabling trust and transperency utilizing CORD blockchain.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `create_registry` - Create a registry, with states supported and entry types.
//! * `create_registry_entry` - Create a registry entry for the created registry.
//! * `registry_entry_status_change` - Change the status of the registry entry.

#![cfg_attr(not(feature = "std"), no_std)]

mod types;

use frame_support::{
	ensure,
	pallet_prelude::DispatchResult,
	traits::{Get, StorageVersion},
	BoundedVec,
};

pub use pallet::*;
use sp_std::{prelude::*, str};

use sp_runtime::traits::Hash;

pub use frame_system::WeightInfo;
pub use types::{
	Data, Entry, Registry, RegistryEntry, RegistrySupportedStateOf, RegistrySupportedTypeOf,
};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use identifier::{
		CordIdentifierType, IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier,
	};

	pub type RegistryHashOf<T> = <T as frame_system::Config>::Hash;
	pub type RegistryEntryHashOf<T> = <T as frame_system::Config>::Hash;

	pub type RegistryIdOf = Ss58Identifier;
	pub type RegistryEntryIdOf = Ss58Identifier;
	pub type RegistryStateOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedInputLength>;
	pub type RegistryKeyIdOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedInputLength>;

	// pub type EntryOf<T> = Entry<
	//     RegistryKeyIdOf<T>,
	//     RegistrySupportedTypeOf
	// >;
	// pub type RegistryOf<T> = BoundedVec<
	//     EntryOf<T>, <T as Config>::MaxRegistryEntries
	// >;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The maximum number of Registry Entries supported for a Registry.
		#[pallet::constant]
		type MaxRegistryEntries: Get<u32>;

		/// The maximum encoded length available for naming.
		/// Used by Identifiers, RegistryKeyIds, RegistryStates.
		#[pallet::constant]
		type MaxEncodedInputLength: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// Single Storage Map to hold Registry inforamtion
	/// Holds a bounded vector of Registry Key Ids and corresponding
	/// Registry Type of maximum `MaxRegistryEntries`.
	#[pallet::storage]
	pub type Registries<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		RegistryIdOf,
		Registry<RegistryKeyIdOf<T>, RegistrySupportedTypeOf, RegistryHashOf<T>>,
		OptionQuery,
	>;

	/// Double Storage Map to hold Registry Entry information
	/// for a Registry.
	#[pallet::storage]
	pub type RegistryEntries<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		RegistryIdOf,
		Blake2_128Concat,
		RegistryEntryIdOf,
		RegistryEntry<
			RegistryKeyIdOf<T>,
			Data,
			RegistryEntryHashOf<T>,
			RegistryIdOf,
			RegistrySupportedStateOf,
		>,
		OptionQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid Identifer Length
		InvalidIdentifierLength,
		/// Identifier Invalid or Not of DeDir Type
		InvalidDeDirIdentifier,
		/// Registry Identifier Already Exists
		RegistryIdAlreadyExists,
		/// Registry Identifier Does Not Exists
		RegistryIdDoesNotExist,
		/// Supported Registry Entries Not Found
		RegistryHasNoEntries,
		/// Registry Key Does Not Exist In Declared Registry
		RegistryKeyDoesNotExist,
		/// Registry Entry Identifier Already Exists
		RegistryEntryIdAlreadyExists,
		/// Registry Entry Identifier Does Not Exists
		RegistryEntryIdDoesNotExist,
		/// Registry Attributes Storage Upper Bound Breached
		MaxRegistryStorageOverflow,
		/// Regsitry Entry Attributes Storage Upper Bound Breached
		MaxRegistryEntryStorageOverflow,
		/// State Addition Failed
		AddingStateFailed,
		/// State Not Found In Declared Registry
		StateNotSupported,
		/// State Deemed Invalid While Converion From UTF8 To String
		InvalidState,
		/// State Deemed Inavlid While Conversion To MaxEncodedInputLength
		StateConversionError,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new registry has been created.
		/// \[creator, registry_identifier\]
		CreatedRegistry { creator: T::AccountId, registry_id: RegistryIdOf },
		/// A new registry entry has been created.
		/// \[creator, registry_identifier, registry_entry_identifier\]
		CreatedRegistryEntry {
			creator: T::AccountId,
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
		},
		/// State change has been made for existing registry entry.
		/// \[who, registry_identifier, registry_entry_identifier, new_state\]
		RegistryEntryStateChanged {
			who: T::AccountId,
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
			new_state: RegistrySupportedStateOf,
		},
	}

	#[pallet::call]
	/// DeDir pallet declaration.
	impl<T: Config> Pallet<T> {
		/// A new Registry has been created.
		///
		/// This function allows a user to submit a new create registry request.
		/// The Registry is created along with various metadata, including the
		/// attributes, creator and other data.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `registry_id` - The `registry_id` which is to be of SS58Identifier and must be of
		///   Ident type `DeDir`.
		/// * `digest` - The digest to be bound to the Registry.
		/// * `attrs` - (Optional) Bounded Vector of RegistryKeys and associated RegistryTypes which
		///   will be supported.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdAlreadyExists` if the registry identifier
		/// exists already.
		/// Returns `Error::<T>::MaxRegistryStorageOverflow` if the Registry Attributes
		/// storage upper bound has been breached.
		/// Returns `Error::<T>::AddingStateFailed` if the state addition fails to be added.
		///
		/// # Events
		/// Emits `CreatedRegistry` when a new Registry is created successfully.
		///
		/// # Example
		/// ```
		/// create_registry(origin, attrs, registry_id, digest, attrs)?;
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create_registry(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			digest: RegistryHashOf<T>,
			attrs: Option<Vec<(RegistryKeyIdOf<T>, RegistrySupportedTypeOf)>>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			/* TODO: Move the logic of creation of identifier to SDK.
			 * Only the validation of the identifier remains on chain.
			 */
			let id_digest =
				<T as frame_system::Config>::Hashing::hash(&[&creator.encode()[..]].concat()[..]);
			let registry_id =
				Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::DeDir)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			/* Ensure that registry_id is of valid ss58 format,
			 * and also the type matches to be of `DeDir`
			 */
			ensure!(Self::is_valid_ss58_format(&registry_id), Error::<T>::InvalidDeDirIdentifier);

			/* Ensure that the registry_id does not already exist */
			ensure!(
				!Registries::<T>::contains_key(&registry_id),
				Error::<T>::RegistryIdAlreadyExists
			);

			let mut registry = Registry { entries: BoundedVec::default(), digest };

			/* Ensure Registry attributes does not overflow */
			if let Some(attrs) = attrs {
				for (registry_key, registry_key_type) in attrs {
					let entry = Entry { registry_key, registry_key_type };
					if registry.entries.try_push(entry).is_err() {
						return Err(Error::<T>::MaxRegistryStorageOverflow.into());
					}
				}
			}

			Registries::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::CreatedRegistry { creator, registry_id });

			Ok(())
		}

		/// Registry Entry has been created for a Registry.
		///
		/// This function allows a user to submit a new create registry entry request
		/// for a existing Registry.
		/// The Registry Entry is created along with various metadata, including the
		/// attributes with the Registry Entry Key and its associated type-data,
		/// current-state of the registry-entry and other information.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `registry_id` - Registry Identifier associated with a existing Registry.
		/// * `registry_entry_id` - The `registry_entry_id` which is to be of SS58Identifier and
		///   must be of Ident type `DeDir`.
		/// * `digest` - The digest to be bound to the Registry Entry.
		/// * `attrs` - (Optional) Bounded Vector of RegistryKeys and associated RegistryTypes, and
		///   the data.
		/// * `state` - (Optional) Valid registry state for the registry entry to be associated
		///   with.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdDoesNotExists` if the registry identifier
		/// does not exist.
		/// Returns `Error::<T>::RegistryEntryIdAlreadyExists` if the registry entry
		/// identifier exists already.
		/// Returns `Error::<T>::MaxRegistryEntryStorageOverflow` if the Registry Entry
		/// Attributes storage upper bound has been breached.
		/// Returns `Error::<T>::RegistryHasNoEntries` if the Registry does not have attributes
		/// declared. And Registry Entry requests addition of attributes.
		/// Returns `Error::<T>::RegistryKeyDoesNotExist` if the Registry Entry
		/// Attributes Key addition does not have a associated key during Registry Creation.
		///
		/// # Events
		/// Emits `CreatedRegistryEntry` when a new Registry Entry is created successfully.
		///
		/// # Example
		/// ```
		/// create_registry_entry(origin, registry_id,
		///     registry_entry_id, digest, attrs, state)?;
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn create_registry_entry(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
			digest: RegistryEntryHashOf<T>,
			attrs: Option<Vec<(RegistryKeyIdOf<T>, Data)>>,
			state: Option<RegistrySupportedStateOf>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			/* Ensure RegistryId exists */
			let registry =
				Registries::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_id.encode()[..], &digest.encode()[..]].concat()[..] //, &uuid.as_bytes()[..]].concat()[..],
			);
			let registry_entry_id =
				Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::DeDir)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			/* Ensure that registry_entry_id is of valid ss58 format,
			 * and also the type matches to be of `DeDir`
			 */
			ensure!(
				Self::is_valid_ss58_format(&registry_entry_id),
				Error::<T>::InvalidDeDirIdentifier
			);

			/* Ensure RegistryEntryId does not already exists for given RegistryId */
			ensure!(
				RegistryEntries::<T>::get(&registry_id, &registry_entry_id).is_none(),
				Error::<T>::RegistryEntryIdAlreadyExists
			);

			/* Set default state to `DRAFT` */
			let current_state = if let Some(state) = state {
				ensure!(state.is_valid_state(), Error::<T>::StateNotSupported);
				state
			} else {
				RegistrySupportedStateOf::DRAFT
			};

			/* Set known and default values */
			let mut registry_entry = RegistryEntry {
				entries: BoundedVec::default(),
				registry_id: registry_id.clone(),
				digest,
				current_state: current_state.clone(),
			};

			/* Ensure RegistryEntry does not overflow */
			if let Some(attrs) = attrs {
				for (registry_entry_key, data) in attrs {
					if registry_entry.entries.try_push((registry_entry_key, data)).is_err() {
						return Err(Error::<T>::MaxRegistryEntryStorageOverflow.into());
					}
				}
			}

			/* Ensure Registry has entries declared before setting attributes */
			if (registry.entries.len() == 0) && (registry_entry.entries.len() > 0) {
				return Err(Error::<T>::RegistryHasNoEntries.into());
			}

			/* Ensure every Registry entry attribute has a corresponding declared key in Registry */
			// TOOD:
			// Ensure registry_key_type matches for correspondeing declared type.
			for (registry_key, _) in &registry_entry.entries {
				let mut registry_key_found = false;
				for entry in &registry.entries {
					if entry.registry_key == *registry_key {
						registry_key_found = true;
						break;
					}
				}
				if !registry_key_found {
					return Err(Error::<T>::RegistryKeyDoesNotExist.into());
				}
			}

			RegistryEntries::<T>::insert(&registry_id, &registry_entry_id, registry_entry);

			Self::deposit_event(Event::CreatedRegistryEntry {
				creator,
				registry_id,
				registry_entry_id,
			});

			Ok(())
		}

		/// Change the state of Registry Entry.
		///
		/// This function allows a user to submit a change of state request for an
		/// for a existing Registry Entry.
		/// The State of Registry Entry is updated with the new-state which is part of
		/// existing supported-states.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `registry_id` - Registry Identifier associated with a existing Registry.
		/// * `registry_entry_id` - Registry Entry Identifier to be associated with the Registry
		///   Entry.
		/// * `new_state` - The `new_state` which Registry Entry must be updated to.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdDoesNotExists` if the registry identifier
		/// does not exist.
		/// Returns `Error::<T>::RegistryEntryIdDoesNotExists` if the registry entry
		/// identifier does not exist.
		/// Returns `Error::<T>::InvalidState` State deemed invalid while converion
		/// from UTF8 to String.
		/// Returns `Error::<T>::StateConversionError` State deemed inavlid while conversion
		/// to MaxEncodedInputLength.
		/// Returns `Error::<T>::StateNotSupported` State not found in declared Registry.
		///
		/// # Events
		/// Emits `RegistryEntryStateChanged` when a new Registry Entry State has been updated.
		///
		/// # Example
		/// ```
		/// registry_entry_state_change(origin, registry_id, registry_entry_id,
		///     new_state)?;
		/// ```
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn registry_entry_state_change(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
			new_state: RegistrySupportedStateOf,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			/* Ensure RegistryId exists */
			let _registry =
				Registries::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure RegistryEntryId exists for the given RegistryId */
			let mut registry_entry = RegistryEntries::<T>::get(&registry_id, &registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdDoesNotExist)?;

			/* Ensure given `new_state` is part of supported states enum */
			ensure!(new_state.is_valid_state(), Error::<T>::StateNotSupported);

			registry_entry.current_state = new_state.clone();
			RegistryEntries::<T>::insert(&registry_id, &registry_entry_id, registry_entry);

			Self::deposit_event(Event::RegistryEntryStateChanged {
				who,
				registry_id,
				registry_entry_id,
				new_state,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Method to check if the input identifier calculated from sdk
	/// is actually a valid SS58 Identifier Format and of valid type `DeDir`.
	pub fn is_valid_ss58_format(identifier: &Ss58Identifier) -> bool {
		match identifier.get_type() {
			Ok(id_type) =>
				if id_type == IdentifierType::DeDir {
					log::debug!("The SS58 identifier is of type DeDir.");
					true
				} else {
					log::debug!("The SS58 identifier is not of type DeDir.");
					false
				},
			Err(e) => {
				log::debug!("Invalid SS58 identifier. Error: {:?}", e);
				false
			},
		}
	}
}
