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
//! * `create_registry` - Create a registry, with blob and digest.
//! * `create_registry_entry` - Create a registry entry for the created registry.
//! * `registry_entry_state_change` - Change the status of the registry entry.
//! * `add_delegate` - Add a account as a delegate with specific permission.
//! * `remove_delegate` - Add a existing account from authorized delegates list.
//! * `update_delegate_permission` - Update the permission of an existing delegate.

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

use sp_runtime::traits::Hash;

pub use frame_system::WeightInfo;
pub use types::{
	DelegateInfo, Delegates, Entry, Permissions, Registry, RegistryEntry, RegistrySupportedStateOf,
};

use codec::Encode;

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
	pub type MaxDelegatesOf<T> = <T as crate::Config>::MaxRegistryDelegates;

	pub type MaxRegistryBlobSizeOf<T> = <T as crate::Config>::MaxRegistryBlobSize;

	pub type OwnerOf<T> = <T as frame_system::Config>::AccountId;
	pub type DelegateOf<T> = <T as frame_system::Config>::AccountId;

	pub type DelegateEntryOf<T> =
		BoundedVec<DelegateInfo<DelegateOf<T>, Permissions>, MaxDelegatesOf<T>>;

	pub type RegistryBlobOf<T> = BoundedVec<u8, MaxRegistryBlobSizeOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The maximum number of bytes in size a Registry Blob can hold.
		#[pallet::constant]
		type MaxRegistryBlobSize: Get<u32>;

		/// The maximum number of Registry Entries supported for a Registry.
		#[pallet::constant]
		type MaxRegistryDelegates: Get<u32>;

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
		Registry<RegistryBlobOf<T>, RegistryHashOf<T>, OwnerOf<T>>,
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
			RegistryBlobOf<T>,
			RegistryEntryHashOf<T>,
			RegistryIdOf,
			RegistrySupportedStateOf,
		>,
		OptionQuery,
	>;

	/// Single Storage Map to hold the details of Delegate
	/// information for a Registry.
	#[pallet::storage]
	pub type DelegatesList<T: Config> =
		StorageMap<_, Blake2_128Concat, RegistryIdOf, Delegates<DelegateEntryOf<T>>, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid Identifer Length
		InvalidIdentifierLength,
		/// Identifier Invalid or Not of DeDir Type
		InvalidDeDirIdentifier,
		/// Account has no valid authorization
		UnauthorizedOperation,
		/// Registry Identifier Already Exists
		RegistryIdAlreadyExists,
		/// Registry Identifier Does Not Exists
		RegistryIdDoesNotExist,
		/// Registry Entry Identifier Already Exists
		RegistryEntryIdAlreadyExists,
		/// Registry Entry Identifier Does Not Exists
		RegistryEntryIdDoesNotExist,
		/// State Not Found In Declared Registry
		StateNotSupported,
		/// Max Delegates Storage Upper Bound Breached
		MaxDelegatesStorageOverflow,
		/// Delegates List not found for Registry Id
		DelegatesListNotFound,
		/// Registry Owner cannot be removed from Delegates List
		CannotRemoveRegistryOwnerAsDelegate,
		/// Delegate not found in DelegatesList
		DelegateNotFound,
		/// Invalid Permission
		InvalidPermission,
		/// Admin is unauthorized from removing another Admin
		AdminCannotRemoveAnotherAdmin,
		/// Delegate is unauthorized from Delegate Operations.
		DelegateCannotRemoveAccounts,
		/// Delegate already exists with same permissions.
		DelegateWithSamePermissionExists,
		/// Registry Owner Permissions cannot be updated.
		CannotUpdateOwnerPermission,
		/// Delegator cannot be removed.
		DelegatorCannotBeRemoved,
		/// Delegator cannot be added.
		DelegatorCannotBeAdded,
		/// Delegator cannot be updated.
		DelegatorCannotBeUpdated,
		/// Delegate alreay exists.
		DelegateAlreadyAdded,
		/// Blob and Digest Does not match.
		BlobDoesNotMatchDigest,
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
		/// A new Delegate has been added to the Registry.
		/// \[delegator, registry_id, delegate, permission]
		RegistryDelegateAdded {
			delegator: T::AccountId,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
			permission: Permissions,
		},
		/// A existing Delegate has been removed from the Registry.
		/// \[delegator, registry_id, delegate]
		RegistryDelegateRemoved {
			delegator: T::AccountId,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		},
		/// A existing Registry Delegate permissions have been updated.
		/// \[delegator, registry_id, delegate, new_permission]
		RegistryDelegatePermissionUpdated {
			delegator: T::AccountId,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
			new_permission: Permissions,
		},
	}

	#[pallet::call]
	/// DeDir pallet declaration.
	impl<T: Config> Pallet<T> {
		/// A new Registry has been created.
		///
		/// This function allows a user to submit a new create registry request.
		/// The Registry is created along with various metadata, including the
		/// blob, digest.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `registry_id` - The `registry_id` which is to be of SS58Identifier and must be of
		///   Ident type `DeDir`.
		/// * `digest` - The digest to be bound to the Registry.
		/// * `blob` - (Optional) Bounded Vector of Blob which is derived from same file as digest.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdAlreadyExists` if the registry identifier
		/// exists already.
		///
		/// # Events
		/// Emits `CreatedRegistry` when a new Registry is created successfully.
		///
		/// # Example
		/// ```
		/// create_registry(origin, registry_id, digest, blob)?;
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create_registry(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			digest: RegistryHashOf<T>,
			blob: Option<RegistryBlobOf<T>>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			/* Identifier Management will happen at SDK.
			 * It is to be constructed as below.
			 */
			// let id_digest = <T as frame_system::Config>::Hashing::hash(
			// 	&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
			// );
			// let registry_id =
			// 	Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::DeDir)
			// 		.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			/* Ensure that registry_id is of valid ss58 format,
			 * and also the type matches to be of `DeDir`
			 */
			ensure!(Self::is_valid_ss58_format(&registry_id), Error::<T>::InvalidDeDirIdentifier);

			/* Ensure that the registry_id does not already exist */
			ensure!(
				!Registries::<T>::contains_key(&registry_id),
				Error::<T>::RegistryIdAlreadyExists
			);

			let mut registry =
				Registry { blob: BoundedVec::default(), digest, owner: creator.clone() };

			if let Some(blob) = blob {
				/* Check if blob and digest matches */
				ensure!(
					Self::does_blob_matches_digest(&blob, &digest),
					Error::<T>::BlobDoesNotMatchDigest
				);

				registry.blob = blob;
			}

			Registries::<T>::insert(&registry_id, registry);

			let mut delegates = Delegates { entries: BoundedVec::default() };

			/* Set `delegator` to `None` for owner */
			if delegates
				.entries
				.try_push(DelegateInfo {
					delegate: creator.clone(),
					permission: Permissions::OWNER,
					delegator: None,
				})
				.is_err()
			{
				return Err(Error::<T>::MaxDelegatesStorageOverflow.into());
			};

			DelegatesList::<T>::insert(&registry_id, delegates);

			Self::deposit_event(Event::CreatedRegistry { creator, registry_id });

			Ok(())
		}

		/// Registry Entry has been created for a Registry.
		///
		/// This function allows a user to submit a new create registry entry request
		/// for a existing Registry.
		/// The Registry Entry is created along with various metadata, including the
		/// blob, digest, and the state of the Registry Entry.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `registry_id` - Registry Identifier associated with a existing Registry.
		/// * `registry_entry_id` - The `registry_entry_id` which is to be of SS58Identifier and
		///   must be of Ident type `DeDir`.
		/// * `digest` - The digest to be bound to the Registry Entry.
		/// * `blob` - (Optional) Bounded Vector of Blob which is derived from same file as digest.
		/// * `state` - (Optional) Valid registry state for the registry entry to be associated
		///   with.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdDoesNotExists` if the registry identifier
		/// does not exist.
		/// Returns `Error::<T>::RegistryEntryIdAlreadyExists` if the registry entry
		/// identifier exists already.
		/// Attributes Key addition does not have a associated key during Registry Creation.
		///
		/// # Events
		/// Emits `CreatedRegistryEntry` when a new Registry Entry is created successfully.
		///
		/// # Example
		/// ```
		/// create_registry_entry(origin, registry_id,
		///     registry_entry_id, digest, blob, state)?;
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn create_registry_entry(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			registry_entry_id: RegistryEntryIdOf,
			digest: RegistryEntryHashOf<T>,
			blob: Option<RegistryBlobOf<T>>,
			state: Option<RegistrySupportedStateOf>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			/* Ensure RegistryId exists */
			let _registry =
				Registries::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure that registry_entry is created from authorized account */
			let delegates =
				DelegatesList::<T>::get(&registry_id).ok_or(Error::<T>::DelegatesListNotFound)?;

			/* Ensure there exists a valid_delegate */
			ensure!(
				Self::is_valid_delegate(
					&delegates.entries,
					&creator,
					&[Permissions::OWNER, Permissions::ADMIN, Permissions::DELEGATE]
				),
				Error::<T>::UnauthorizedOperation
			);

			/* Identifier Management will happen at SDK.
			 * It is to be constructed as below.
			 */
			// let id_digest = <T as frame_system::Config>::Hashing::hash(
			// 	&[&registry_id.encode()[..], &digest.encode()[..]].concat()[..],
			// );
			// let registry_entry_id =
			// 	Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::DeDir)
			// 		.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

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
				blob: BoundedVec::default(),
				registry_id: registry_id.clone(),
				digest,
				current_state: current_state.clone(),
			};

			if let Some(blob) = blob {
				/* Check if blob and digest matches */
				ensure!(
					Self::does_blob_matches_digest(&blob, &digest),
					Error::<T>::BlobDoesNotMatchDigest
				);

				registry_entry.blob = blob;
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

			/* Ensure that registry state updation happens from authorized account */
			let delegates =
				DelegatesList::<T>::get(&registry_id).ok_or(Error::<T>::DelegatesListNotFound)?;

			ensure!(
				Self::is_valid_delegate(
					&delegates.entries,
					&who,
					&[Permissions::OWNER, Permissions::ADMIN, Permissions::DELEGATE]
				),
				Error::<T>::UnauthorizedOperation
			);

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

		/// Add a Delegate to a Registry.
		///
		/// This function allows a user to add a delegate with a specified permission to an
		/// existing registry. The function ensures that the addition complies with the rules
		/// governing delegate permissions and authorization.
		///
		/// # Rules for Adding a Delegate:
		///
		/// - **Registry Existence:** The registry identified by `registry_id` must exist. If the
		///   registry does not exist, the function will return an error.
		///
		/// - **Unique Delegator and Delegate:** The delegate being added cannot be the same as the
		///   delegator initiating the operation. This ensures that a user cannot delegate
		///   permissions to themselves.
		///
		/// - **Owner Permission Constraint:** The `OWNER` permission cannot be assigned to any
		///   delegate. This prevents multiple delegates from having `OWNER` permissions within the
		///   same registry.
		///
		/// - **Authorization Requirements:** Only delegates with `OWNER` or `ADMIN` permissions can
		///   add new delegates. Delegates with `DELEGATE` permissions are not authorized to add
		///   other delegates.
		///
		/// - **Existing Delegate Check:** The function checks whether the delegate already exists
		///   in the registry. If the delegate is already listed, the operation will return an
		///   error. This also ensures that an existing `OWNER` cannot be added with a different
		///   permission.
		///
		/// - **Storage Overflow Handling:** If the addition of the new delegate exceeds the maximum
		///   allowed storage, an error will be returned.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `registry_id` - The registry identifier associated with an existing registry.
		/// * `delegate` - The account to be added as a delegate.
		/// * `permission` - The permission to be assigned to the delegate. Valid permissions are
		///   `ADMIN` and `DELEGATE`.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdDoesNotExist` if the registry identifier does not exist.
		/// Returns `Error::<T>::DelegatorCannotBeAdded` if the delegate and delegator are the same.
		/// Returns `Error::<T>::InvalidPermission` if an attempt is made to assign the `OWNER`
		/// permission. Returns `Error::<T>::DelegatesListNotFound` if the list of delegates for
		/// the registry cannot be found. Returns `Error::<T>::UnauthorizedOperation` if the
		/// origin does not have sufficient permissions to add a delegate.
		/// Returns `Error::<T>::DelegateAlreadyAdded` if the delegate already exists in the
		/// registry. Returns `Error::<T>::MaxDelegatesStorageOverflow` if the addition of the new
		/// delegate exceeds the maximum allowed storage.
		///
		/// # Events
		/// Emits `RegistryDelegateAdded` when a new delegate is successfully added to the registry.
		///
		/// # Example
		/// ```
		/// add_delegate(origin, registry_id, delegate, permission)?;
		/// ```
		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn add_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
			permission: Permissions,
		) -> DispatchResult {
			let delegator = ensure_signed(origin)?;

			/* Ensure RegistryId exists */
			let _registry =
				Registries::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure delegator and delegate are not same */
			ensure!(delegate != delegator, Error::<T>::DelegatorCannotBeAdded);

			/* Ensure OWNER permission is not assigned */
			ensure!(!matches!(permission, Permissions::OWNER), Error::<T>::InvalidPermission);

			/* Ensure that registry_entry is created from authorized account */
			let mut delegates =
				DelegatesList::<T>::get(&registry_id).ok_or(Error::<T>::DelegatesListNotFound)?;

			/* Ensure there exists a valid_delegate with
			 * OWNER or ADMIN permissions to add a delegate.
			 */
			ensure!(
				Self::is_valid_delegate(
					&delegates.entries,
					&delegator,
					&[Permissions::OWNER, Permissions::ADMIN]
				),
				Error::<T>::UnauthorizedOperation
			);

			/* Check if the delegate already exists */
			/* As a side-effect it also prevents a existing OWNER getting added with different
			 * permission */
			let existing_entry = delegates.entries.iter().find(|d| d.delegate == delegate);
			if existing_entry.is_some() {
				return Err(Error::<T>::DelegateAlreadyAdded.into());
			}

			if delegates
				.entries
				.try_push(DelegateInfo {
					delegate: delegate.clone(),
					permission: permission.clone(),
					delegator: Some(delegator.clone()),
				})
				.is_err()
			{
				return Err(Error::<T>::MaxDelegatesStorageOverflow.into());
			}

			DelegatesList::<T>::insert(&registry_id, delegates);

			Self::deposit_event(Event::RegistryDelegateAdded {
				delegator: delegator.clone(),
				registry_id,
				delegate,
				permission,
			});

			Ok(())
		}

		/// Remove a Delegate from the Registry.
		///
		/// This function allows an authorized user to remove a delegate from a registry.
		/// The operation is subject to specific rules based on the permission level of the
		/// user initiating the request and the delegate being removed.
		///
		/// # Rules for Removing a Delegate:
		///
		/// **Ownership Constraints:**
		/// - **OWNER Cannot Be Removed:** The OWNER of a registry cannot be removed as a delegate.
		///   This ensures the integrity and stability of the registry by maintaining a consistent
		///   owner.
		///
		/// **Authorization Requirements:**
		/// - **Authorized Users:** Only users with OWNER or ADMIN permissions are allowed to
		///   perform the removal operation. DELEGATE-level users do not have the authority to
		///   remove any accounts.
		/// - **Admin Removal Constraints:** An ADMIN cannot remove another ADMIN. This restriction
		///   ensures that ADMIN users cannot interfere with each other's permissions.
		///
		/// **Delegator-Delegate Relationship:**
		/// - **Self-Removal Restriction:** The delegator (user initiating the removal) cannot be
		///   the same as the delegate being removed. This prevents users from accidentally or
		///   maliciously removing themselves.
		///
		/// **Permission Enforcement:**
		/// - **OWNER Authority:** The OWNER has the authority to remove any other permissioned
		///   accounts (i.e., ADMINs or DELEGATEs).
		/// - **ADMIN Restrictions:** ADMINs are restricted from removing other ADMINs, ensuring a
		///   balanced distribution of power within the registry.
		/// - **DELEGATE Limitations:** DELEGATE-level users cannot remove any accounts, regardless
		///   of their permissions.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user.
		/// * `registry_id` - The identifier of the registry from which the delegate should be
		///   removed.
		/// * `delegate` - The account identifier of the delegate to be removed.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdDoesNotExist` if the registry identifier does not exist.
		/// Returns `Error::<T>::DelegatorCannotBeRemoved` if the delegator attempts to remove
		/// themselves. Returns `Error::<T>::DelegatesListNotFound` if the delegate list for the
		/// given registry does not exist.
		/// Returns `Error::<T>::CannotRemoveRegistryOwnerAsDelegate` if an attempt is made to
		/// remove the OWNER. Returns `Error::<T>::AdminCannotRemoveAnotherAdmin` if an ADMIN
		/// attempts to remove another ADMIN. Returns `Error::<T>::DelegateCannotRemoveAccounts`
		/// if a DELEGATE attempts to remove any account. Returns `Error::<T>::DelegateNotFound`
		/// if the delegate to be removed is not found in the registry.
		///
		/// # Events
		/// Emits `RegistryDelegateRemoved` when a delegate is successfully removed from the
		/// registry.
		///
		/// # Example
		/// ```
		/// remove_delegate(origin, registry_id, delegate)?;
		/// ```
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		) -> DispatchResult {
			let delegator = ensure_signed(origin)?;

			/* Ensure RegistryId exists */
			let _registry =
				Registries::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			ensure!(delegator != delegate, Error::<T>::DelegatorCannotBeRemoved);

			/* Ensure that registry_entry is created from authorized account */
			let mut delegates =
				DelegatesList::<T>::get(&registry_id).ok_or(Error::<T>::DelegatesListNotFound)?;

			let mut delegator_permission = None;
			let mut delegate_index = None;

			for (i, entry) in delegates.entries.iter().enumerate() {
				if entry.delegate == delegator {
					delegator_permission = Some(entry.permission.clone());
				}
				if entry.delegate == delegate {
					delegate_index = Some(i);
				}
			}

			/* Ensure the delegator has the required permissions of being either OWNER/ADNIN */
			ensure!(
				delegator_permission
					.clone()
					.map_or(false, |perm| matches!(perm, Permissions::OWNER | Permissions::ADMIN)),
				Error::<T>::UnauthorizedOperation
			);

			/* Ensure that the delegate to be removed is found */
			if let Some(index) = delegate_index {
				/* Ensure OWNER cannot be removed */
				if delegates.entries[index].delegator.is_none() {
					return Err(Error::<T>::CannotRemoveRegistryOwnerAsDelegate.into());
				}

				/* Ensure that permissions are correctly enforced
				 * OWNER can remove any other permissioned accounts
				 * ADMIN cannot remove another ADMIN
				 * DELEGATE cannot remove any accounts
				 */
				match delegator_permission.unwrap() {
					Permissions::OWNER => {},
					Permissions::ADMIN => {
						ensure!(
							delegates.entries[index].permission != Permissions::ADMIN,
							Error::<T>::AdminCannotRemoveAnotherAdmin
						);
					},
					Permissions::DELEGATE => {
						return Err(Error::<T>::DelegateCannotRemoveAccounts.into());
					},
				}

				delegates.entries.remove(index);
				DelegatesList::<T>::insert(&registry_id, delegates);

				Self::deposit_event(Event::RegistryDelegateRemoved {
					delegator: delegator.clone(),
					registry_id,
					delegate,
				});
			} else {
				return Err(Error::<T>::DelegateNotFound.into());
			}

			Ok(())
		}

		/// Update the permissions of an existing Delegate.
		///
		/// This function allows a user to submit a change of permission request for an
		/// existing Delegate. The permission of the Registry is updated with the `new_permission`
		/// which is part of the existing supported Permissions List.
		///
		/// # Rules for Updating Permissions of a Delegate:
		///
		/// **Existence of Delegate:**
		/// - The delegate whose permission is to be updated must already exist in the registry.
		///
		/// **Permission Constraints:**
		/// - **No Ownership Updates:** Permissions of type OWNER cannot be assigned to any
		///   delegate. This ensures that only one OWNER exists for a given registry.
		/// - **Valid Permission Levels:** The new permission must be either ADMIN or DELEGATE.
		///   Assigning OWNER is not permitted.
		///
		/// **Authorization Requirements:**
		/// - **Authorized Users:** The update operation can only be performed by users with OWNER
		///   or ADMIN permissions. DELEGATE-level users are not authorized to perform this
		///   operation.
		/// - **Admin Downgrades:** If the new permission is DELEGATE, only an OWNER can downgrade
		///   an ADMIN to DELEGATE. An ADMIN cannot perform this downgrade.
		///
		/// **Delegator Restrictions:**
		/// - **Same User Update:** The delegator cannot be the same as the delegate whose
		///   permission is being updated.
		/// - **Permission Upgrade:** ADMIN users are allowed to upgrade a delegate to ADMIN or
		///   DELEGATE, but cannot downgrade an ADMIN to DELEGATE unless performed by an OWNER.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user.
		/// * `registry_id` - The Registry Identifier associated with an existing Registry.
		/// * `delegate` - The account for which the permission update should take place.
		/// * `new_permission` - The `new_permission` to which the `delegate` should be updated.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdDoesNotExist` if the registry identifier does not exist.
		/// Returns `Error::<T>::DelegatorCannotBeUpdated` if the delegate and delegator are the
		/// same. Returns `Error::<T>::InvalidPermission` if the `new_permission` is not a valid
		/// permission. Returns `Error::<T>::DelegatesListNotFound` if the DelegateList does not
		/// exist for the given RegistryId.
		/// Returns `Error::<T>::DelegateNotFound` if the given `delegate` does not exist.
		/// Returns `Error::<T>::UnauthorizedOperation` if the given operation is not valid.
		/// Returns `Error::<T>::CannotUpdateOwnerPermission` if the owner's permission is attempted
		/// to be changed.
		/// Returns `Error::<T>::DelegateWithSamePermissionExists` if the given `delegate` already
		/// exists with the same `new_permission`.
		///
		/// # Events
		/// Emits `RegistryDelegatePermissionUpdated` when a Registry Delegate Permission is
		/// updated.
		///
		/// # Example
		/// ```
		/// update_delegate_permission(origin, registry_id, delegate, new_permission)?;
		/// ```
		#[pallet::call_index(5)]
		#[pallet::weight({0})]
		pub fn update_delegate_permission(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
			new_permission: Permissions,
		) -> DispatchResult {
			let delegator = ensure_signed(origin)?;

			/* Ensure RegistryId exists */
			let _registry =
				Registries::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			ensure!(delegator != delegate, Error::<T>::DelegatorCannotBeUpdated);

			/* Ensure that new_permission is not OWNER */
			ensure!(
				matches!(new_permission, Permissions::ADMIN | Permissions::DELEGATE),
				Error::<T>::InvalidPermission
			);

			/* Ensure that registry_entry is created from authorized account */
			let mut delegates =
				DelegatesList::<T>::get(&registry_id).ok_or(Error::<T>::DelegatesListNotFound)?;

			/* Ensure that the delegate already exists */
			let delegate_index = delegates
				.entries
				.iter()
				.position(|d| d.delegate == delegate)
				.ok_or(Error::<T>::DelegateNotFound)?;

			/* Ensure the delegator is either an OWNER or an ADMIN */
			let delegator_permission = delegates
				.entries
				.iter()
				.find(|d| d.delegate == delegator)
				.map(|d| d.permission.clone())
				.ok_or(Error::<T>::UnauthorizedOperation)?;

			ensure!(
				matches!(delegator_permission, Permissions::OWNER | Permissions::ADMIN),
				Error::<T>::UnauthorizedOperation
			);

			let delegate_entry = &delegates.entries[delegate_index];

			/* Ensure that the delegate is not the OWNER */
			ensure!(
				delegate_entry.permission != Permissions::OWNER,
				Error::<T>::CannotUpdateOwnerPermission
			);

			/* Ensure Delegate with same permission does not already exist */
			ensure!(
				delegate_entry.permission != new_permission,
				Error::<T>::DelegateWithSamePermissionExists
			);

			/* Ensure only an OWNER can downgrade an ADMIN to DELEGATE */
			if matches!(new_permission, Permissions::DELEGATE) {
				if matches!(delegate_entry.permission, Permissions::ADMIN) &&
					delegator_permission != Permissions::OWNER
				{
					return Err(Error::<T>::UnauthorizedOperation.into());
				}
			}

			delegates.entries[delegate_index].permission = new_permission.clone();

			DelegatesList::<T>::insert(&registry_id, delegates);

			Self::deposit_event(Event::RegistryDelegatePermissionUpdated {
				delegator: delegator.clone(),
				registry_id,
				delegate,
				new_permission,
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

	/// Method to check if there exists a valid delegate
	pub fn is_valid_delegate(
		delegates: &DelegateEntryOf<T>,
		creator: &OwnerOf<T>,
		permissions: &[Permissions],
	) -> bool {
		for entry in delegates.iter() {
			if entry.delegate == *creator && permissions.contains(&entry.permission) {
				return true;
			}
		}
		false
	}

	/// Method to check if blob matches the given digest
	pub fn does_blob_matches_digest(blob: &RegistryBlobOf<T>, digest: &RegistryHashOf<T>) -> bool {
		let blob_digest = <T as frame_system::Config>::Hashing::hash(&blob.encode()[..]);

		log::debug!("digest: {:?}, blob_digest: {:?}", *digest, blob_digest);

		blob_digest == *digest
	}
}
