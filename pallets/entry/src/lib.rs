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

//! # Entry Pallet - Part of `DeDir (Decentralized Directory)`.
//!
//! This pallet support CORD URI which supports multi-chain deployments.
//!
//! ## Overview
//!
//! The Entry pallet which is part of the `DeDir (Decentralized Directory)` aims to implement a
//! decentralized version of a Registry Entry (record). Enabling creation, updation of entries in a
//! decentralized manner. Thereby enabling trust and transperency of Registries utilizing CORD
//! blockchain. Registry & Delegation management is handled by the Registries Pallet.
//! 
//! This supports new CORD URI for multichain deployments.
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

#[cfg(test)]
pub mod mock;

#[cfg(test)]
mod tests;

use frame_support::{
	ensure,
	pallet_prelude::DispatchResult,
	traits::{Get, StorageVersion},
	BoundedVec,
};
use sp_runtime::traits::Hash;

use cord_uri::{
	EntryTypeOf, EventStamp, 
	Identifier, Ss58Identifier
};

pub use pallet::*;
use sp_std::{prelude::*, str};

pub use frame_system::WeightInfo;
pub use types::RegistryEntryDetails;

pub use cord_primitives::StatusOf;
use pallet_profile::ProfileIdOf;
use pallet_registry::{RegistryIdentifierOf, Permissions};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

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
		RegistryEntryDetails<RegistryEntryHashOf<T>, StatusOf, ProfileIdOf, RegistryIdOf>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + cord_uri::Config + pallet_profile::Config + pallet_registry::Config
	{
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
		/// New Registry Entry owner cannot be same as existing owner.
		NewOwnerCannotBeSameAsExistingOwner,
		/// Validation or Access of the Registry has failed.
		RegistryAccessValidationFailed,
		/// Activity input type is invalid.
		InvalidEntryTypeInput,
		/// Activity update has failed.
		ActivityUpdateFailed
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
			creator_profile_id: ProfileIdOf,
		},

		/// A existing registry entry has been updated.
		/// \[updater, registry_entry_identifier\]
		RegistryEntryUpdated { 
			updater: T::AccountId, 
			registry_entry_id: RegistryEntryIdOf,
			updater_profile_id: ProfileIdOf,
		},

		/// A existing registry entry has been revoked.
		/// \[updater, registry_entry_identifier\]
		RegistryEntryRevoked { 
			updater: T::AccountId, 
			registry_entry_id: RegistryEntryIdOf,
			updater_profile_id: ProfileIdOf,
		},

		/// A existing registry entry has been reinstated.
		/// \[updater, registry_enrtry_identifier\]
		RegistryEntryReinstated { 
			updater: T::AccountId, 
			registry_entry_id: RegistryEntryIdOf,
			updater_profile_id: ProfileIdOf,
		},

		/// A existing registry entry ownership has been updated.
		/// \[updater, new_owner, registry_entry_identifier\]
		RegistryEntryOwnershipUpdated {
			updater: T::AccountId,
			new_owner: T::AccountId,
			registry_entry_id: RegistryEntryIdOf,
			updater_profile_id: ProfileIdOf,
			new_owner_profile_id: ProfileIdOf,
		},
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
			registry_id: RegistryIdentifierOf,
			tx_hash: RegistryEntryHashOf<T>,
			_blob: Option<RegistryEntryBlobOf<T>>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(
				&creator
			)
			.map_err(<pallet_profile::Error<T>>::from)?;
			
			pallet_registry::Pallet::<T>::validate_registry_for_tx(
				&profile_id, &registry_id
			)
			.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;

			let mut data = Vec::with_capacity(256);
			data.extend_from_slice(tx_hash.as_ref());
			data.extend_from_slice(&registry_id.encode());
			data.extend_from_slice(&profile_id.encode());

			let digest = T::Hashing::hash(&data);
			let pallet_name = <crate::pallet::Pallet<T> as frame_support::traits::PalletInfoAccess>::name();

			let registry_entry_id =
				<cord_uri::Pallet<T> as Identifier>::build(&(digest).encode()[..], pallet_name)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			/* Ensure that the registry_entry_id does not already exist */
			ensure!(
				!RegistryEntries::<T>::contains_key(&registry_entry_id),
				Error::<T>::RegistryEntryIdentifierAlreadyExists
			);

			let registry_entry = RegistryEntryDetails {
				tx_hash,
				revoked: false,
				creator: profile_id.clone(),
				registry_id: registry_id.clone(),
			};

			RegistryEntries::<T>::insert(&registry_entry_id, registry_entry);

            Self::record_activity(&registry_entry_id, b"RegistryEntryCreated")?;

			Self::deposit_event(Event::RegistryEntryCreated {
				creator,
				registry_id,
				registry_entry_id,
				creator_profile_id: profile_id
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
			registry_id: RegistryIdentifierOf,
			registry_entry_id: RegistryEntryIdOf,
			tx_hash: RegistryEntryHashOf<T>,
			_blob: Option<RegistryEntryBlobOf<T>>,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(
				&updater
			)
			.map_err(<pallet_profile::Error<T>>::from)?;

			pallet_registry::Pallet::<T>::validate_registry_for_tx(
				&profile_id, &registry_id
			)
			.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;

			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(registry_id == entry.registry_id, Error::<T>::UnauthorizedOperation);

			/* Should be allowed only by the admin of the Registry or Creator of the document */
			let is_admin =
				pallet_registry::Pallet::<T>::is_admin(&profile_id, &registry_id);

			let is_creator = entry.creator == profile_id;

			ensure!(is_admin || is_creator, Error::<T>::UnauthorizedOperation);

			entry.tx_hash = tx_hash;

			RegistryEntries::<T>::insert(&registry_entry_id, entry);

            Self::record_activity(&registry_entry_id, b"RegistryEntryUpdated")?;

			Self::deposit_event(Event::RegistryEntryUpdated { 
				updater,
				registry_entry_id,
				updater_profile_id: profile_id,
			});

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
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(
				&updater
			)
			.map_err(<pallet_profile::Error<T>>::from)?;

			pallet_registry::Pallet::<T>::validate_registry_for_tx(
				&profile_id, &registry_id
			)
			.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;

			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(entry.registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			/* Should be allowed only by the admin of the Registry or Creator of the document */
			let is_admin =
				pallet_registry::Pallet::<T>::is_admin(&profile_id, &registry_id);

			let is_creator = entry.creator == profile_id;

			ensure!(is_admin || is_creator, Error::<T>::UnauthorizedOperation);

			entry.revoked = true;

			RegistryEntries::<T>::insert(&registry_entry_id, entry);

            Self::record_activity(&registry_entry_id, b"RegistryEntryRevoked")?;

			Self::deposit_event(Event::RegistryEntryRevoked { 
				updater,
				registry_entry_id,
				updater_profile_id: profile_id
			});

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
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(
				&updater
			)
			.map_err(<pallet_profile::Error<T>>::from)?;

			pallet_registry::Pallet::<T>::validate_registry_for_tx(
				&profile_id, &registry_id
			)
			.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;
			
			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(entry.registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			ensure!(entry.revoked, Error::<T>::RegistryEntryNotRevoked);

			/* Should be allowed only by the admin of the Registry or Creator of the document */
			let is_admin =
				pallet_registry::Pallet::<T>::is_admin(&profile_id, &registry_id);

			let is_creator = entry.creator == profile_id;

			ensure!(is_admin || is_creator, Error::<T>::UnauthorizedOperation);

			entry.revoked = false;

			RegistryEntries::<T>::insert(&registry_entry_id, entry);

            Self::record_activity(&registry_entry_id, b"RegistryEntryReinstated")?;

			Self::deposit_event(Event::RegistryEntryReinstated { 
				updater, 
				registry_entry_id,
				updater_profile_id: profile_id
			});

			Ok(())
		}

		/// Updates the ownership of an existing Registry Entry.
		///
		/// This function allows an authorized user (creator or admin) to update the ownership
		/// of an existing Registry Entry. Ownership can be transferred to a new owner within
		/// the same Registry.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be a signed account (updater).
		/// * `registry_entry_id` - The unique identifier of the Registry Entry to update ownership.
		/// * `authorization` - The authorization identifier that links the updater to the Registry.
		/// * `new_owner` - The account identifier of the new owner of the Registry Entry.
		/// * `new_owner_authorization` - The authorization identifier that links the new owner to
		///   the Registry.
		///
		/// # Conditions
		/// - Only the current creator (owner) of the Registry Entry or an admin of the Registry can
		///   perform this operation.
		/// - The new owner must be authorized within the same Registry.
		/// - The new owner cannot be the same as the current owner to avoid unnecessary storage
		///   writes.
		///
		/// # Errors
		/// This function returns an error in the following cases:
		/// * `RegistryEntryIdentifierDoesNotExist` - If the specified `registry_entry_id` does not
		///   exist.
		/// * `UnauthorizedOperation` - If the caller does not have permission to update the
		///   ownership or if the new owner is not authorized under the same Registry.
		/// * `NewOwnerCannotBeSameAsExistingOwner` - If the new owner is the same as the current
		///   owner.
		///
		/// # Events
		/// Emits the `Event::RegistryEntryOwnershipUpdated` event upon successful ownership update.
		/// This event includes the `updater`, the `new_owner`, and the `registry_entry_id`.
		///
		/// # Example
		/// ```rust
		/// update_ownership(
		///     origin,
		///     registry_entry_id,
		///     authorization,
		///     new_owner,
		///     new_owner_authorization,
		/// )?;
		/// ```
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn update_ownership(
			origin: OriginFor<T>,
			registry_id: RegistryIdentifierOf,
			registry_entry_id: RegistryEntryIdOf,
			new_owner: CreatorOf<T>,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;

			let existing_owner_profile_id = pallet_profile::Pallet::<T>::get_profile_id(
				&updater
			)
			.map_err(<pallet_profile::Error<T>>::from)?;

			pallet_registry::Pallet::<T>::validate_registry_for_tx(
				&existing_owner_profile_id, &registry_id
			)
			.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;

			let new_owner_profile_id = pallet_profile::Pallet::<T>::get_profile_id(
				&new_owner
			)
			.map_err(<pallet_profile::Error<T>>::from)?;

			let entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(registry_id == entry.registry_id, Error::<T>::UnauthorizedOperation);

			let is_admin =
				pallet_registry::Pallet::<T>::is_admin(
					&existing_owner_profile_id, &registry_id
			);

			let is_creator = entry.creator == new_owner_profile_id.clone();

			ensure!(is_admin || is_creator, Error::<T>::UnauthorizedOperation);

			/* Avoid having a unneccessary storage write */
			ensure!(updater != new_owner, Error::<T>::NewOwnerCannotBeSameAsExistingOwner);

			/* New Owner of the Entry(record) should be a part of the same registry */
			ensure!(
				pallet_registry::Pallet::<T>::has_permission(
					&registry_id, &new_owner_profile_id, Permissions::ENTRY),
				Error::<T>::UnauthorizedOperation
        	);
			
			RegistryEntries::<T>::mutate(&registry_entry_id, |entry| {
				if let Some(existing_entry) = entry {
					existing_entry.creator = new_owner_profile_id.clone();
				}
			});

            Self::record_activity(&registry_entry_id, b"RegistryEntryOwnerShipUpdated")?;

			Self::deposit_event(Event::RegistryEntryOwnershipUpdated {
				updater,
				new_owner: new_owner.clone(),
				registry_entry_id,
				updater_profile_id: existing_owner_profile_id,
				new_owner_profile_id: new_owner_profile_id,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Records an activity using a provided event message.
	pub fn record_activity(identifier: &Ss58Identifier, msg: &[u8]) -> DispatchResult {
		let entry: EntryTypeOf =
			msg.to_vec().try_into().map_err(|_| Error::<T>::InvalidEntryTypeInput)?;
		let stamp = EventStamp::current::<T>();
		<cord_uri::Pallet<T> as Identifier>::record_activity(identifier, entry, stamp)
			.map_err(|_| Error::<T>::ActivityUpdateFailed)?;
		Ok(())
	}
}
