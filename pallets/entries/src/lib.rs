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
//! blockchain. Registry & Delegation management is handled by the Registries Pallet.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `create` - Creates a new Registry Entry.
//! * `update` - Updates a existing Registry Entry.
//! * `revoke` - Revokes a existing Registry Entry.
//! * `reinstate` - Reinstates a existing Registry Entry.
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
use sp_runtime::traits::{Hash, UniqueSaturatedInto};

use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};

pub use pallet::*;
use sp_std::{prelude::*, str};

pub use frame_system::WeightInfo;
pub use types::RegistryEntryDetails;

pub use cord_primitives::StatusOf;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use identifier::{
		CordIdentifierType, IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier,
	};

	/// Type of the Authorization
	pub type AuthorizationIdOf = Ss58Identifier;
	/// Type of the Registry Entry Digest
	pub type RegistryEntryHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the Registry Identifier
	pub type RegistryIdOf = Ss58Identifier;
	/// Type of the Resgistry Entry Identifier
	pub type RegistryEntryIdOf = Ss58Identifier;
	/// Type of the Maximum size of Registry Entry Blob
	pub type MaxRegistryEntryBlobSizeOf<T> = <T as crate::Config>::MaxRegistryEntryBlobSize;
	/// Type of the Registry Entry Creator
	pub type CreatorOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of the Registry Entry Blob
	pub type RegistryEntryBlobOf<T> = BoundedVec<u8, MaxRegistryEntryBlobSizeOf<T>>;

	/// Type of the Registry Entry Details.
	/// Consists of Entry status, creator, registry id.
	pub type RegistryEntryDetailsOf<T> =
		RegistryEntryDetails<RegistryEntryHashOf<T>, StatusOf, CreatorOf<T>, RegistryIdOf>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_registries::Config + identifier::Config
	{
		//pub trait Config: frame_system::Config + pallet_registries::Config {
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

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

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
		RegistryEntryIdentifierAlreadyExists,
		/// Registry Entry Identifier Does Not Exists
		RegistryEntryIdentifierDoesNotExist,
		/// Registry Entry has not been revoked.
		RegistryEntryNotRevoked,
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

		/// A existing registry entry has been updated.
		/// \[updater, registry_entry_identifier\]
		RegistryEntryUpdated { updater: T::AccountId, registry_entry_id: RegistryEntryIdOf },

		/// A existing registry entry has been revoked.
		/// \[updater, registry_entry_identifier\]
		RegistryEntryRevoked { updater: T::AccountId, registry_entry_id: RegistryEntryIdOf },

		/// A existing registry entry has been reinstated.
		/// \[updater, registry_enrtry_identifier\]
		RegistryEntryReinstated { updater: T::AccountId, registry_entry_id: RegistryEntryIdOf },
	}

	#[pallet::call]
	/// Entries pallet declaration.
	impl<T: Config> Pallet<T> {
		/// Creates a new Registry Entry within a specified Registry.
		///
		/// This function allows a user to create a new entry within an existing Registry.
		/// The function verifies that the caller is authorized to create an entry within the
		/// specified Registry, ensures that the entry does not already exist.
		///
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be a signed account (creator of the
		///   entry).
		/// * `registry_entry_id` - A unique id as registry entry identifier.
		/// * `authorization` - The authorization identifier that links the creator to the Registry.
		/// * `digest` - The hash value or digest of the content associated with the Registry entry.
		/// * `blob` - (Optional) Additional data associated with the Registry entry, provided as an
		///   optional field.
		///
		/// # Errors
		/// This function returns an error in the following cases:
		/// * `UnauthorizedOperation` - If the caller does not have permission to create entries
		///   within the Registry.
		/// * `RegistryEntryIdentifierAlreadyExists` - If the `registry_entry_id` already exists in
		///   the storage.
		/// * `InvalidIdentifierLength` - If the `registry_entry_id` generated from the hash exceeds
		///   the expected length for identifiers.
		///
		/// # Events
		/// Emits the `Event::RegistryEntryCreated` event upon successful creation of a new Registry
		/// entry. This event includes the `creator`, `registry_id`, and the `registry_entry_id`
		/// of the new entry.
		///
		/// # Example
		/// ```rust
		/// create(origin, registry_entry_id, authorization, digest, Some(blob))?;
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			_registry_entry_id: RegistryEntryIdOf,
			authorization: AuthorizationIdOf,
			digest: RegistryEntryHashOf<T>,
			_blob: Option<RegistryEntryBlobOf<T>>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let registry_id = pallet_registries::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator,
			)
			.map_err(<pallet_registries::Error<T>>::from)?;

			// TODO:
			/* Identifier Management will happen at SDK.
			 * It is to be constructed as below.
			 */
			// Id Digest = concat (H(<scale_encoded_statement_digest>,
			// <scale_encoded_space_identifier>, <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]].concat()
					[..],
			);

			let registry_entry_id = Ss58Identifier::create_identifier(
				&(id_digest).encode()[..],
				IdentifierType::Entries,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			/* Ensure that the registry_entry_id does not already exist */
			ensure!(
				!RegistryEntries::<T>::contains_key(&registry_entry_id),
				Error::<T>::RegistryEntryIdentifierAlreadyExists
			);

			// TODO: Validate incoming `registry_entry_id` from SDK.
			// /* Ensure that registry_id is of valid ss58 format,
			//  * and also the type matches to be of `Registry`
			//  */
			// ensure!(
			// 	Self::is_valid_ss58_format(&registry_entry_id),
			// 	Error::<T>::InvalidRegistryEntryIdentifier
			// );
			let registry_entry = RegistryEntryDetails {
				digest,
				revoked: false,
				creator: creator.clone(),
				registry_id: registry_id.clone(),
			};

			RegistryEntries::<T>::insert(&registry_entry_id, registry_entry);

			Self::update_activity(&registry_entry_id, CallTypeOf::Genesis)
				.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::RegistryEntryCreated {
				creator,
				registry_id,
				registry_entry_id,
			});

			Ok(())
		}

		/// Updates an existing Registry Entry with new metadata.
		///
		/// This function allows an authorized user to update the metadata (such as the `digest` or
		/// optional `blob`) of an existing Registry Entry. The user must have the necessary
		/// permissions to perform this operation.
		///
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be a signed account (updater).
		/// * `registry_entry_id` - The unique identifier of the Registry Entry to be updated.
		/// * `authorization` - The authorization identifier that links the updater to the Registry.
		/// * `digest` - The new hash value or digest to be associated with the Registry Entry.
		/// * `blob` - (Optional) New additional data to be associated with the Registry Entry.
		///
		/// # Errors
		/// This function returns an error in the following cases:
		/// * `UnauthorizedOperation` - If the caller does not have permission to update the
		///   Registry Entry.
		/// * `RegistryEntryIdentifierDoesNotExist` - If the specified `registry_entry_id` does not
		///   exist.
		/// * `StateNotSupported` - If an unsupported state is provided.
		///
		/// # Events
		/// Emits the `Event::RegistryEntryUpdated` event upon successful update of the Registry
		/// Entry. This event includes the `updater` and the `registry_entry_id`.
		///
		/// # Example
		/// ```rust
		/// update(origin, registry_entry_id, authorization, digest, Some(blob))?;
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn update(
			origin: OriginFor<T>,
			registry_entry_id: RegistryEntryIdOf,
			authorization: AuthorizationIdOf,
			digest: RegistryEntryHashOf<T>,
			_blob: Option<RegistryEntryBlobOf<T>>,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;
			let registry_id = pallet_registries::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&updater,
			)
			.map_err(<pallet_registries::Error<T>>::from)?;

			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(entry.registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			entry.digest = digest;

			RegistryEntries::<T>::insert(&registry_entry_id, entry);

			Self::update_activity(&registry_entry_id, CallTypeOf::Update)
				.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::RegistryEntryUpdated { updater, registry_entry_id });

			Ok(())
		}

		/// Revokes an existing Registry Entry.
		///
		/// This function allows an authorized user to revoke an existing Registry Entry, marking it
		/// as no longer valid. The revocation can only be performed by the account with
		/// appropriate permissions.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be a signed account (updater).
		/// * `registry_entry_id` - The unique identifier of the Registry Entry to be revoked.
		/// * `authorization` - The authorization identifier that links the updater to the Registry.
		///
		/// # Errors
		/// This function returns an error in the following cases:
		/// * `UnauthorizedOperation` - If the caller does not have permission to revoke the
		///   Registry Entry.
		/// * `RegistryEntryIdentifierDoesNotExist` - If the specified `registry_entry_id` does not
		///   exist.
		///
		/// # Events
		/// Emits the `Event::RegistryEntryRevoked` event upon successful revocation of the Registry
		/// Entry. This event includes the `updater` and the `registry_entry_id`.
		///
		/// # Example
		/// ```rust
		/// revoke(origin, registry_entry_id, authorization)?;
		/// ```
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn revoke(
			origin: OriginFor<T>,
			registry_entry_id: RegistryEntryIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;
			let registry_id = pallet_registries::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&updater,
			)
			.map_err(<pallet_registries::Error<T>>::from)?;

			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(entry.registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			entry.revoked = true;

			RegistryEntries::<T>::insert(&registry_entry_id, entry);

			Self::update_activity(&registry_entry_id, CallTypeOf::Revoke)
				.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::RegistryEntryRevoked { updater, registry_entry_id });

			Ok(())
		}

		/// Reinstates an revoked existing Registry Entry.
		///
		/// This function allows an authorized user to reinstates revoked an existing Registry
		/// Entry, marking it active again. The revocation can only be performed by the account
		/// with appropriate permissions
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be a signed account (updater).
		/// * `registry_entry_id` - The unique identifier of the Registry Entry to be reinstated.
		/// * `authorization` - The authorization identifier that links the updater to the Registry.
		///
		/// # Errors
		/// This function returns an error in the following cases:
		/// * `UnauthorizedOperation` - If the caller does not have permission to revoke the
		///   Registry Entry.
		/// * `RegistryEntryIdentifierDoesNotExist` - If the specified `registry_entry_id` does not
		///   exist.
		///
		/// # Events
		/// Emits the `Event::RegistryEntryReinstated` event upon Registry Entry successfully
		/// reinstated. This event includes the `updater` and the `registry_entry_id`.
		///
		/// # Example
		/// ```rust
		/// reinstate(origin, registry_entry_id, authorization)?;
		/// ```
		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn reinstate(
			origin: OriginFor<T>,
			registry_entry_id: RegistryEntryIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;
			let registry_id = pallet_registries::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&updater,
			)
			.map_err(<pallet_registries::Error<T>>::from)?;

			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(entry.registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			ensure!(entry.revoked, Error::<T>::RegistryEntryNotRevoked);

			entry.revoked = false;

			RegistryEntries::<T>::insert(&registry_entry_id, entry);

			Self::update_activity(&registry_entry_id, CallTypeOf::Reinstate)
				.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::RegistryEntryReinstated { updater, registry_entry_id });

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

	/// Updates the global timeline with a new activity event for a registry entry.
	/// This function is called whenever a significant action is performed on a
	/// registry entry, ensuring that all such activities are logged with a timestamp
	/// for future reference and accountability.
	///
	/// An `EventEntryOf` struct is created, encapsulating the type of action
	/// (`tx_action`) and the `Timepoint` of the event, which is obtained by
	/// calling the `timepoint` function. This entry is then passed to the
	/// `update_timeline` function of the `identifier` pallet, which integrates
	/// it into the global timeline.
	///
	/// # Parameters
	/// - `tx_id`: The identifier of the registry entry that the activity pertains to.
	/// - `tx_action`: The type of action taken on the registry entry, encapsulated within
	///   `CallTypeOf`.
	///
	/// # Returns
	/// Returns `Ok(())` after successfully updating the timeline. If any errors
	/// occur within the `update_timeline` function, they are not captured here
	/// and the function will still return `Ok(())`.
	///
	/// # Usage
	/// This function is not intended to be called directly by external entities
	/// but is invoked internally within the pallet's logic whenever a
	/// statement's status is altered.
	pub fn update_activity(
		tx_id: &RegistryEntryIdOf,
		tx_action: CallTypeOf,
	) -> Result<(), Error<T>> {
		let tx_moment = Self::timepoint();

		let tx_entry = EventEntryOf { action: tx_action, location: tx_moment };
		let _ =
			IdentifierTimeline::update_timeline::<T>(tx_id, IdentifierTypeOf::Entries, tx_entry);
		Ok(())
	}

	/// Retrieves the current timepoint.
	///
	/// This function returns a `Timepoint` structure containing the current
	/// block number and extrinsic index. It is typically used in conjunction
	/// with `update_activity` to record when an event occurred.
	///
	/// # Returns
	/// - `Timepoint`: A structure containing the current block number and extrinsic index.
	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}

// TODO:
// Check if more checks are required for `update`, `revoke` & `reinstate`.
// Right now ensure_authorization_origin() is being done similar to statements.
// Should there be a check if the account is either a account or the creator of the entry.
// Ex: Check for is_admin = pallet_registries::<T>::ensure_admin_authorization_origin()
//     And check for the creator of the entry.
// It should only be allowed when `is_allowed = is_admin || is_creator`.
