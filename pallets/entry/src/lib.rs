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
//! * `update_ownership` - Updates the ownership of the Registry Entry.
#![cfg_attr(not(feature = "std"), no_std)]

mod types;

#[cfg(test)]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub use cord_primitives::Ss58Identifier;
use frame_support::{
	ensure,
	pallet_prelude::DispatchResult,
	traits::{Get, StorageVersion},
	BoundedVec,
};
use pallet_doken::{Doken, EventBlock, EventTypeOf};
use sp_runtime::traits::Hash;

pub use pallet::*;

extern crate alloc;
use alloc::{str, vec::Vec};

pub use frame_system::WeightInfo;
pub use types::RegistryEntryDetails;

pub use cord_primitives::StatusOf;
use pallet_profile::ProfileIdOf;
use pallet_registry::{Permissions, RegistryIdentifierOf};
// use alloc::string::String;

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
		frame_system::Config
		+ pallet_doken::Config
		+ pallet_profile::Config
		+ pallet_registry::Config
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

	/// Storage to map for Entry hashes to corresponding Registry Identifiers.
	/// It being a storage double-map will have the Registry Entry Hash and the Registry ID
	/// as the key, whereas the value resulted is the Registry Entry Identifier.
	#[pallet::storage]
	pub type HashToIdentifier<T> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		RegistryEntryHashOf<T>,
		Blake2_128Concat,
		RegistryIdentifierOf,
		RegistryEntryIdOf,
		OptionQuery,
	>;

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
		/// The event activity update has failed.
		EventUpdateFailed,
		/// The provided event type is invalid.
		InvalidEventType,
		// State Update Failed
		StateUpdateFailed,
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
	impl<T: Config> Pallet<T> {
		/// Creates a new registry entry within a specified registry.
		///
		/// Constructs a unique entry identifier from the transaction hash, registry ID, and
		/// creator's profile ID, ensuring it doesn’t already exist. The creator must have a valid
		/// profile and permission to create entries in the registry. The entry is stored with its
		/// hash, creator, and registry ID, marked as active.
		///
		/// # Arguments
		/// * `origin` - The signed account creating the entry.
		/// * `registry_id` - The SS58 identifier of the registry.
		/// * `tx_hash` - The hash of the entry’s content.
		/// * `_blob` - Optional data associated with the entry.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the creator lacks permission.
		/// * `RegistryAccessValidationFailed` - If registry access validation fails.
		/// * `RegistryEntryIdentifierAlreadyExists` - If the entry ID already exists.
		/// * `InvalidIdentifierLength` - If the generated ID is invalid.
		/// * `pallet_profile::Error` - If the creator’s profile is invalid.
		///
		/// # Events
		/// * `RegistryEntryCreated` - Emitted with `creator`, `registry_id`, `registry_entry_id`,
		///   `creator_profile_id`.
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

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&creator)
				.map_err(<pallet_profile::Error<T>>::from)?;

			pallet_registry::Pallet::<T>::validate_registry_for_tx(&profile_id, &registry_id)
				.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;

			let mut data = Vec::with_capacity(256);
			data.extend_from_slice(tx_hash.as_ref());
			data.extend_from_slice(&registry_id.encode());
			data.extend_from_slice(&profile_id.encode());

			let digest = T::Hashing::hash(&data);
			let pallet_name =
				<crate::pallet::Pallet<T> as frame_support::traits::PalletInfoAccess>::name();

			let registry_entry_id =
				<pallet_doken::Pallet<T> as Doken<T>>::build(&(digest).encode()[..], pallet_name)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			/* Ensure that the registry_entry_id does not already exist */
			ensure!(
				!RegistryEntries::<T>::contains_key(&registry_entry_id),
				Error::<T>::RegistryEntryIdentifierAlreadyExists
			);

			let registry_entry = RegistryEntryDetails {
				tx_hash: tx_hash.clone(),
				revoked: false,
				creator: profile_id.clone(),
				registry_id: registry_id.clone(),
			};

			RegistryEntries::<T>::insert(&registry_entry_id, registry_entry);

			HashToIdentifier::<T>::insert(&tx_hash, &registry_id, &registry_entry_id);

			Self::record_activity(&registry_entry_id, tx_hash, b"RegistryEntryCreated")?;

			Self::deposit_event(Event::RegistryEntryCreated {
				creator,
				registry_id,
				registry_entry_id,
				creator_profile_id: profile_id,
			});

			Ok(())
		}

		/// Updates an existing registry entry’s transaction hash.
		///
		/// Updates the transaction hash of an entry if the caller is the registry admin or the
		/// entry’s creator. The entry must exist and belong to the specified registry. The new
		/// hash is stored, and an activity is recorded.
		///
		/// # Arguments
		/// * `origin` - The signed account updating the entry.
		/// * `registry_id` - The SS58 identifier of the registry.
		/// * `registry_entry_id` - The SS58 identifier of the entry.
		/// * `tx_hash` - The new hash of the entry’s content.
		/// * `_blob` - Optional updated data.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks permission or registry ID mismatches.
		/// * `RegistryAccessValidationFailed` - If registry access validation fails.
		/// * `RegistryEntryIdentifierDoesNotExist` - If the entry ID doesn’t exist.
		/// * `pallet_profile::Error` - If the updater’s profile is invalid.
		///
		/// # Events
		/// * `RegistryEntryUpdated` - Emitted with `updater`, `registry_entry_id`,
		///   `updater_profile_id`.
		///
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

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&updater)
				.map_err(<pallet_profile::Error<T>>::from)?;

			pallet_registry::Pallet::<T>::validate_registry_for_tx(&profile_id, &registry_id)
				.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;

			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(registry_id == entry.registry_id, Error::<T>::UnauthorizedOperation);

			/* Should be allowed only by the admin of the Registry or Creator of the document */
			let is_admin = pallet_registry::Pallet::<T>::is_admin(&profile_id, &registry_id);

			let is_creator = entry.creator == profile_id;

			ensure!(is_admin || is_creator, Error::<T>::UnauthorizedOperation);

			entry.tx_hash = tx_hash.clone();

			RegistryEntries::<T>::insert(&registry_entry_id, entry);

			HashToIdentifier::<T>::insert(&tx_hash, &registry_id, &registry_entry_id);

			Self::record_activity(&registry_entry_id, tx_hash, b"RegistryEntryUpdated")?;

			Self::deposit_event(Event::RegistryEntryUpdated {
				updater,
				registry_entry_id,
				updater_profile_id: profile_id,
			});

			Ok(())
		}

		/// Revokes an existing registry entry.
		///
		/// Marks an entry as revoked if the caller is the registry admin or the entry’s creator.
		/// The entry must exist and belong to the specified registry. The revoked status is updated
		/// in storage.
		///
		/// # Arguments
		/// * `origin` - The signed account revoking the entry.
		/// * `registry_id` - The SS58 identifier of the registry.
		/// * `registry_entry_id` - The SS58 identifier of the entry.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks permission or registry ID mismatches.
		/// * `RegistryAccessValidationFailed` - If registry access validation fails.
		/// * `RegistryEntryIdentifierDoesNotExist` - If the entry ID doesn’t exist.
		/// * `pallet_profile::Error` - If the updater’s profile is invalid.
		///
		/// # Events
		/// * `RegistryEntryRevoked` - Emitted with `updater`, `registry_entry_id`,
		///   `updater_profile_id`.
		///
		/// ```
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn revoke(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&updater)
				.map_err(<pallet_profile::Error<T>>::from)?;

			pallet_registry::Pallet::<T>::validate_registry_for_tx(&profile_id, &registry_id)
				.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;

			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(entry.registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			/* Should be allowed only by the admin of the Registry or Creator of the document */
			let is_admin = pallet_registry::Pallet::<T>::is_admin(&profile_id, &registry_id);

			let is_creator = entry.creator == profile_id;

			ensure!(is_admin || is_creator, Error::<T>::UnauthorizedOperation);

			entry.revoked = true;
			let digest = entry.tx_hash.clone();
			RegistryEntries::<T>::insert(&registry_entry_id, entry);

			Self::record_activity(&registry_entry_id, digest, b"RegistryEntryRevoked")?;

			Self::deposit_event(Event::RegistryEntryRevoked {
				updater,
				registry_entry_id,
				updater_profile_id: profile_id,
			});

			Ok(())
		}

		/// Reinstates an existing revoked registry entry.
		///
		/// Restores an entry to active status if the caller is the registry admin or the entry’s
		/// creator. The entry must exist, belong to the specified registry, and be revoked. The
		/// status is updated in storage.
		///
		/// # Arguments
		/// * `origin` - The signed account reinstating the entry.
		/// * `registry_id` - The SS58 identifier of the registry.
		/// * `registry_entry_id` - The SS58 identifier of the entry.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks permission or registry ID mismatches.
		/// * `RegistryAccessValidationFailed` - If registry access validation fails.
		/// * `RegistryEntryIdentifierDoesNotExist` - If the entry ID doesn’t exist.
		/// * `RegistryEntryNotRevoked` - If the entry is not revoked.
		/// * `pallet_profile::Error` - If the updater’s profile is invalid.
		///
		/// # Events
		/// * `RegistryEntryReinstated` - Emitted with `updater`, `registry_entry_id`,
		///   `updater_profile_id`.
		///
		/// ```
		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn reinstate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&updater)
				.map_err(<pallet_profile::Error<T>>::from)?;

			pallet_registry::Pallet::<T>::validate_registry_for_tx(&profile_id, &registry_id)
				.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;

			let mut entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(entry.registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			ensure!(entry.revoked, Error::<T>::RegistryEntryNotRevoked);

			/* Should be allowed only by the admin of the Registry or Creator of the document */
			let is_admin = pallet_registry::Pallet::<T>::is_admin(&profile_id, &registry_id);

			let is_creator = entry.creator == profile_id;

			ensure!(is_admin || is_creator, Error::<T>::UnauthorizedOperation);

			entry.revoked = false;
			let digest = entry.tx_hash.clone();

			RegistryEntries::<T>::insert(&registry_entry_id, entry);

			Self::record_activity(&registry_entry_id, digest, b"RegistryEntryReinstated")?;

			Self::deposit_event(Event::RegistryEntryReinstated {
				updater,
				registry_entry_id,
				updater_profile_id: profile_id,
			});

			Ok(())
		}

		/// Updates the ownership of an existing registry entry.
		///
		/// Transfers ownership to a new account if the caller is the registry admin or the entry’s
		/// creator. The entry must exist and belong to the specified registry. The new owner must
		/// have a valid profile and permission in the registry. The creator field is updated in
		/// storage.
		///
		/// # Arguments
		/// * `origin` - The signed account updating ownership.
		/// * `registry_id` - The SS58 identifier of the registry.
		/// * `registry_entry_id` - The SS58 identifier of the entry.
		/// * `new_owner` - The account ID of the new owner.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller or new owner lacks permission, or registry ID
		///   mismatches.
		/// * `RegistryAccessValidationFailed` - If registry access validation fails.
		/// * `RegistryEntryIdentifierDoesNotExist` - If the entry ID doesn’t exist.
		/// * `NewOwnerCannotBeSameAsExistingOwner` - If the new owner matches the current owner.
		/// * `pallet_profile::Error` - If the updater’s or new owner’s profile is invalid.
		///
		/// # Events
		/// * `RegistryEntryOwnershipUpdated` - Emitted with `updater`, `new_owner`,
		///   `registry_entry_id`, `updater_profile_id`, `new_owner_profile_id`.
		///
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

			let existing_owner_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&updater)
				.map_err(<pallet_profile::Error<T>>::from)?;

			pallet_registry::Pallet::<T>::validate_registry_for_tx(
				&existing_owner_profile_id,
				&registry_id,
			)
			.map_err(|_| Error::<T>::RegistryAccessValidationFailed)?;

			let new_owner_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&new_owner)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let entry = RegistryEntries::<T>::get(&registry_entry_id)
				.ok_or(Error::<T>::RegistryEntryIdentifierDoesNotExist)?;

			ensure!(registry_id == entry.registry_id, Error::<T>::UnauthorizedOperation);

			let is_admin =
				pallet_registry::Pallet::<T>::is_admin(&existing_owner_profile_id, &registry_id);

			let is_creator = entry.creator == existing_owner_profile_id.clone();

			ensure!(is_admin || is_creator, Error::<T>::UnauthorizedOperation);

			/* Avoid having a unneccessary storage write */
			ensure!(updater != new_owner, Error::<T>::NewOwnerCannotBeSameAsExistingOwner);

			/* New Owner of the Entry(record) should be a part of the same registry */
			ensure!(
				pallet_registry::Pallet::<T>::has_permission(
					&registry_id,
					&new_owner_profile_id,
					Permissions::ENTRY
				),
				Error::<T>::UnauthorizedOperation
			);

			RegistryEntries::<T>::mutate(&registry_entry_id, |entry| {
				if let Some(existing_entry) = entry {
					existing_entry.creator = new_owner_profile_id.clone();
				}
			});

			Self::record_activity(
				&registry_entry_id,
				entry.tx_hash,
				b"RegistryEntryOwnerShipUpdated",
			)?;

			Self::deposit_event(Event::RegistryEntryOwnershipUpdated {
				updater,
				new_owner: new_owner.clone(),
				registry_entry_id,
				updater_profile_id: existing_owner_profile_id,
				new_owner_profile_id,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Records an activity using a provided event message.
	pub fn record_activity(
		identifier: &Ss58Identifier,
		digest: T::Hash,
		msg: &[u8],
	) -> DispatchResult {
		let action: EventTypeOf =
			msg.to_vec().try_into().map_err(|_| Error::<T>::InvalidEventType)?;
		let stamp = EventBlock::current::<T>();
		<pallet_doken::Pallet<T> as Doken<T>>::state_event(identifier, digest, action, stamp)
			.map_err(|_| Error::<T>::StateUpdateFailed)?;
		Ok(())
	}

	// Verify the existence of digest in Registry Entry.
	pub fn verify_digest(
		digest: T::Hash,
		registry_id: Option<RegistryIdOf>,
	) -> Result<Option<RegistryEntryIdOf>, Error<T>> {
		let registry_entry_id = if let Some(reg_id) = registry_id {
			HashToIdentifier::<T>::get(&digest, reg_id)
		} else {
			HashToIdentifier::<T>::iter_prefix(&digest)
				.next()
				.map(|(_reg_id, entry_id)| entry_id)
		};

		Ok(registry_entry_id)
	}
}
