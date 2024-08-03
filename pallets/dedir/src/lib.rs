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

pub use frame_system::WeightInfo;
pub use types::{Data, Entry, Registry, RegistryEntry, RegistrySupportedTypeOf};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	pub type EntryHashOf<T> = <T as frame_system::Config>::Hash;

	pub type RegistryIdOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedInputLength>;
	pub type RegistryEntryIdOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedInputLength>;
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

		/// The maximum number of supported optional states including '3' default.
		#[pallet::constant]
		type MaxSupportedOptionalStates: Get<u32>;

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
		RegistryIdOf<T>,
		Registry<RegistryKeyIdOf<T>, RegistrySupportedTypeOf>,
		OptionQuery,
	>;

	/// Double Storage Map to hold Registry Entry information
	/// for a Registry.
	#[pallet::storage]
	pub type RegistryEntries<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		RegistryIdOf<T>,
		Blake2_128Concat,
		RegistryEntryIdOf<T>,
		RegistryEntry<
			RegistryKeyIdOf<T>,
			Data,
			EntryHashOf<T>,
			RegistryIdOf<T>,
			RegistryStateOf<T>,
		>,
		OptionQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
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
		/// \[who, registry_identifier\]
		CreatedRegistry { who: T::AccountId, registry_id: RegistryIdOf<T> },
		/// A new registry entry has been created.
		/// \[who, registry_identifier, registry_entry_identifier\]
		CreatedRegistryEntry {
			who: T::AccountId,
			registry_id: RegistryIdOf<T>,
			registry_entry_id: RegistryEntryIdOf<T>,
		},
		/// State change has been made for existing registry entry.
		/// \[who, registry_identifier, registry_entry_identifier, new_state\]
		RegistryEntryStateChanged {
			who: T::AccountId,
			registry_id: RegistryIdOf<T>,
			registry_entry_id: RegistryEntryIdOf<T>,
			new_state: RegistryStateOf<T>,
		},
	}

	#[pallet::call]
	/// DeDir pallet declaration.
	impl<T: Config> Pallet<T> {
		/// A new Registry has been created.
		///
		/// This function allows a user to submit a new create registry request.
		/// The Registry is created along with various metadata, including the
		/// attributes, additional-states to be supported.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `registry_id` - Registry Identifier to be associated with the Registry.
		/// * `attrs` - (Optional) Bounded Vector of RegistryKeys and associated RegistryTypes which
		///   will be supported.
		/// * `additional_states` - (Optional) Bounded Vector of Additional States to be supported
		///   by the Registry.
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
		/// create_registry(origin, registry_id, attrs, additional_states)?;
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create_registry(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf<T>,
			attrs: Vec<(RegistryKeyIdOf<T>, RegistrySupportedTypeOf)>,
			// TODO: Use Runtime Constant for upper bound
			additional_states: Option<BoundedVec<BoundedVec<u8, ConstU32<32>>, ConstU32<10>>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			/* Ensure that the registry_id does not already exist */
			ensure!(
				!Registries::<T>::contains_key(&registry_id),
				Error::<T>::RegistryIdAlreadyExists
			);

			/* Set registry with default states */
			let mut registry = Registry::new();

			/* Ensure Registry attributes does not overflow */
			for (registry_key, registry_key_type) in attrs {
				let entry = Entry { registry_key, registry_key_type };
				if registry.entries.try_push(entry).is_err() {
					return Err(Error::<T>::MaxRegistryStorageOverflow.into());
				}
			}

			/* Ensure new-states does not overflow */
			if let Some(states) = additional_states {
				if registry.add_supported_states(states).is_err() {
					return Err(Error::<T>::AddingStateFailed.into());
				}
			}

			Registries::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::CreatedRegistry { who, registry_id });

			Ok(())
		}

		/// Registry Entry has been created for a Registry.
		///
		/// This function allows a user to submit a new create registry entry request
		/// for a existing Registry.
		/// The Registry Entry is created along with various metadata, including the
		/// attributes with the Registry Entry Key and its associated type and data,
		/// current-state to be supported.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `registry_id` - Registry Identifier associated with a existing Registry.
		/// * `registry_entry_id` - Registry Entry Identifier to be associated with the Registry
		///   Entry.
		/// * `digest` - The digest to be bound to the Registry Entry.
		/// * `attrs` - (Optional) Bounded Vector of RegistryKeys and associated RegistryTypes, and
		///   the data.
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
		/// create_registry_entry(origin, registry_id, registry_entry_id,
		///     digest, attrs)?;
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn create_registry_entry(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf<T>,
			registry_entry_id: RegistryEntryIdOf<T>,
			digest: EntryHashOf<T>,
			attrs: Option<Vec<(RegistryKeyIdOf<T>, Data)>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			/* Ensure RegistryId exists */
			let registry =
				Registries::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure RegistryEntryId does not already exists for given RegistryId */
			ensure!(
				RegistryEntries::<T>::get(&registry_id, &registry_entry_id).is_none(),
				Error::<T>::RegistryEntryIdAlreadyExists
			);

			/* Set known and default values */
			let mut registry_entry = RegistryEntry {
				entries: BoundedVec::default(),
				registry_id: registry_id.clone(),
				digest,
				current_state: RegistryStateOf::<T>::try_from(Vec::from(
					"ACTIVE".as_bytes().to_vec(),
				))
				.unwrap(),
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
				who,
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
		/// * `new_state` - The `new_state` which Registry Entry must be updated to. The `new_state`
		///   should be part of supported-states.
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
			registry_id: RegistryIdOf<T>,
			registry_entry_id: RegistryEntryIdOf<T>,
			new_state: RegistryStateOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			/* Ensure RegistryId exists */
			let registry =
				Registries::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure RegistryEntryId exists for the given RegistryId */
			let mut registry_entry = RegistryEntries::<T>::get(&registry_id, &registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdDoesNotExist)?;

			/* Uppercase the new_state from converting utf8 to str */
			let state_str = str::from_utf8(&new_state).map_err(|_| Error::<T>::InvalidState)?;
			let upper_state_str = state_str.to_ascii_uppercase();
			let upper_state = upper_state_str.into_bytes();

			/* Ensure str to BoundedVec conversion respects max-length found in supported_states */
			let bounded_state = RegistryStateOf::<T>::try_from(upper_state)
				.map_err(|_| Error::<T>::StateConversionError)?;

			/* Ensure `new_state` is part of `supported_states` of Registry */
			let mut state_found = false;
			for state in &registry.supported_states {
				if bounded_state == *state {
					state_found = true;
					break;
				}
			}
			ensure!(state_found, Error::<T>::StateNotSupported);

			registry_entry.current_state = bounded_state.clone();
			RegistryEntries::<T>::insert(&registry_id, &registry_entry_id, registry_entry);

			Self::deposit_event(Event::RegistryEntryStateChanged {
				who,
				registry_id,
				registry_entry_id,
				new_state: bounded_state,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	// TODO:
	//
}
