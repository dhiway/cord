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

//! # Registries Pallet - Part of `DeDir (Decentralized Directory)`.
//!
//!
//! ## Overview
//!
//! The Registries pallet which is part of the `DeDir (Decentralized Directory)` aims to implement a
//! decentralized version of a Registry. Enabling creation, updation of registries, delegation
//! management in a decentralized manner. Thereby enabling trust and transperency of Registries
//! utilizing CORD blockchain.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `create` - Create a new Registry.
//! * `update` - Update the existing Registry.
//! * `update_state` - Change the state of the Registry.
//! * `add_owner_delegate` - Add a account as part of delegates with `OWNER` permission.
//! * `add_admin_delegate` - Add a account as part of delegates with `ADMIN` permission.
//! * `add_delegate` - Add a account as part of delegates with `DELEGATE` permission.
//! * `remove_admin_delegate` - Remove `ADMIN` permission for a account.
//! * `remove_delegate` - Remove `DELEGATE` permission for a account.

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

pub use frame_system::WeightInfo;
pub use types::{DelegateInfo, Permissions, RegistryDetails, RegistrySupportedStateOf};

//use sp_runtime::traits::Hash;
//use codec::Encode;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use identifier::{
		CordIdentifierType, IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier,
	};

	pub type RegistryHashOf<T> = <T as frame_system::Config>::Hash;

	pub type RegistryIdOf = Ss58Identifier;
	pub type TemplateIdOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedInputLength>;
	pub type MaxDelegatesOf<T> = <T as crate::Config>::MaxRegistryDelegates;

	pub type MaxRegistryBlobSizeOf<T> = <T as crate::Config>::MaxRegistryBlobSize;

	pub type DelegateOf<T> = <T as frame_system::Config>::AccountId;

	pub type DelegateEntryOf<T> =
		BoundedVec<DelegateInfo<DelegateOf<T>, Permissions>, MaxDelegatesOf<T>>;

	pub type RegistryBlobOf<T> = BoundedVec<u8, MaxRegistryBlobSizeOf<T>>;

	pub type RegistryDetailsOf<T> =
		RegistryDetails<RegistryHashOf<T>, RegistrySupportedStateOf, DelegateEntryOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The maximum number of bytes in size a Registry Blob can hold.
		#[pallet::constant]
		type MaxRegistryBlobSize: Get<u32>;

		/// The maximum number of `Delegates` can be part of a Registry.
		#[pallet::constant]
		type MaxRegistryDelegates: Get<u32>;

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

	/// Storage for Registries
	/// Maps Registry Identifier to its RegistryDetails
	#[pallet::storage]
	pub type Registry<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		RegistryIdOf,
		RegistryDetails<RegistryHashOf<T>, RegistrySupportedStateOf, DelegateEntryOf<T>>,
		OptionQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid Identifer Length
		InvalidIdentifierLength,
		/// Identifier Invalid or Not of Registry Type
		InvalidRegistryIdentifier,
		/// Account has no valid authorization
		UnauthorizedOperation,
		/// Registry Identifier Already Exists
		RegistryIdAlreadyExists,
		/// Registry Identifier Does Not Exists
		RegistryIdDoesNotExist,
		/// State Not Found In Declared Registry
		StateNotSupported,
		/// Max Delegates Storage Upper Bound Breached
		MaxDelegatesStorageOverflow,
		/// Delegate not found in DelegatesList
		DelegateNotFound,
		/// Blob and Digest Does not match.
		BlobDoesNotMatchDigest,
		/// Registry Already exists in same state.
		RegistryAlreadyInSameState,
		/// Delegate already has OWNER Permission.
		DelegateAlreadyHasOwnerPermission,
		/// Delegate already has ADMIN Permission.
		DelegateAlreadyHasAdminPermission,
		/// Delegate already has DELEGATE Permission.
		DelegateAlreadyHasPermission,
		/// Delegate to be removed does not have ADMIN Permission.
		DelegateDoesNotHaveAdminPermission,
		/// Delegate to be removed does not have DELEGATE Permission.
		DelegatePermissionDoesNotExist,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new registry has been created.
		///
		/// \[creator, registry identifier\]
		RegistryCreated { creator: T::AccountId, registry_id: RegistryIdOf },

		/// A registry has been updated.
		///
		/// \[updater, registry identifier\]
		RegistryUpdated { updater: T::AccountId, registry_id: RegistryIdOf },

		/// The state of a registry has been changed.
		///
		/// \[who, registry identifier, new state\]
		RegistryStateChanged {
			who: T::AccountId,
			registry_id: RegistryIdOf,
			new_state: RegistrySupportedStateOf,
		},

		/// An OWNER delegate has been added to a registry.
		///
		/// \[delegator, registry identifier, delegate\]
		RegistryOwnerDelegateAdded {
			delegator: T::AccountId,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		},

		/// An ADMIN delegate has been added to a registry.
		///
		/// \[delegator, registry identifier, delegate\]
		RegistryAdminDelegateAdded {
			delegator: T::AccountId,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		},

		/// A DELEGATE has been added to a registry.
		///
		/// \[delegator, registry identifier, delegate\]
		RegistryDelegateAdded {
			delegator: T::AccountId,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		},

		/// An ADMIN delegate has been removed from a registry.
		///
		/// \[remover, registry identifier, delegate\]
		RegistryAdminDelegateRemoved {
			remover: T::AccountId,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		},

		/// A DELEGATE has been removed from a registry.
		///
		/// \[remover, registry identifier, delegate\]
		RegistryDelegateRemoved {
			remover: T::AccountId,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		},
	}

	#[pallet::call]
	/// Registries pallet declaration.
	impl<T: Config> Pallet<T> {
		/// Creates a new Registry with the specified parameters.
		///
		/// This function allows a user to submit a request to create a new Registry.
		/// The Registry is initialized with various metadata including the `digest`,
		/// `blob`, and `state`. The creator of the Registry is automatically granted
		/// OWNER permissions.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user.
		/// * `registry_id` - The SS58-encoded identifier for the Registry. This must be a valid
		///   SS58 identifier of type `Registry`.
		/// * `digest` - The hash value to be associated with the Registry.
		/// * `_template_id` - (Optional) An identifier for the template to be used for the
		///   Registry.
		/// * `blob` - (Optional) A Bounded Vector of data associated with the Registry, which could
		///   be derived from the same file as the `digest`.
		/// * `state` - (Optional) The state of the Registry. If not provided, defaults to `ACTIVE`.
		///
		/// # Errors
		/// Returns `Error::<T>::InvalidRegistryIdentifier` if the provided `registry_id` is not
		/// in a valid SS58 format or does not match the expected type.
		/// Returns `Error::<T>::RegistryIdAlreadyExists` if a Registry with the same `registry_id`
		/// already exists in the storage.
		/// Returns `Error::<T>::StateNotSupported` if the provided `state` is invalid.
		///
		/// # Events
		/// Emits `Event::RegistryCreated` when a new Registry is created successfully. This event
		/// includes the `creator` of the Registry and the `registry_id`.
		///
		/// # Example
		/// ```
		/// create(origin, registry_id, None, Some(digest), Some(blob), Some(state))?;
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			digest: RegistryHashOf<T>,
			_template_id: Option<TemplateIdOf<T>>,
			blob: Option<RegistryBlobOf<T>>,
			state: Option<RegistrySupportedStateOf>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			/* Identifier Management will happen at SDK.
			 * It is to be constructed as below.
			 */
			// let mut id_digest = <T as frame_system::Config>::Hashing::hash(
			// 		&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
			// 	);
			// if blob.is_some() {
			// 	id_digest = <T as frame_system::Config>::Hashing::hash(
			// 		&[&creator.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
			// 	);
			// }
			// let registry_id =
			// 	Ss58Identifier::create_identifier(&(id_digest).encode()[..],
			// IdentifierType::Registries) 		.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			/* Ensure that registry_id is of valid ss58 format,
			 * and also the type matches to be of `Registry`
			 */
			ensure!(
				Self::is_valid_ss58_format(&registry_id),
				Error::<T>::InvalidRegistryIdentifier
			);

			/* Ensure that the registry_id does not already exist */
			ensure!(
				!Registry::<T>::contains_key(&registry_id),
				Error::<T>::RegistryIdAlreadyExists
			);

			if let Some(_local_blob) = blob {
				/* TODO:
				 * Handle blob. Should it be matched against digest.
				 */
			}

			/* Set default state to `ACTIVE`. If state is passed, ensure it is valid. */
			let registry_state = if let Some(local_state) = state {
				ensure!(local_state.is_valid_state(), Error::<T>::StateNotSupported);
				local_state
			} else {
				RegistrySupportedStateOf::ACTIVE
			};

			let mut registry: RegistryDetails<
				RegistryHashOf<T>,
				RegistrySupportedStateOf,
				DelegateEntryOf<T>,
			> = RegistryDetails {
				digest,
				state: registry_state.clone(),
				delegates: BoundedVec::default(),
			};

			/* Registry creator will be the first OWNER */
			let delegate_info =
				DelegateInfo { delegate: creator.clone(), permissions: Permissions::OWNER };

			if registry.delegates.try_push(delegate_info).is_err() {
				return Err(Error::<T>::MaxDelegatesStorageOverflow.into());
			}

			Registry::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::RegistryCreated { creator, registry_id });

			Ok(())
		}

		/// Updates the state of an existing Registry.
		///
		/// This function allows a user to update the state of a Registry identified by
		/// `registry_id`. The new state is validated to ensure it is supported, and the operation
		/// is only permitted for authorized accounts with OWNER or ADMIN permissions. If the new
		/// state is the same as the current state, the function will return an error.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user.
		/// * `registry_id` - The SS58-encoded identifier of the Registry whose state is to be
		///   updated.
		/// * `new_state` - The new state to set for the Registry. This must be a valid supported
		///   state.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdDoesNotExist` if the `registry_id` does not exist in
		/// storage. Returns `Error::<T>::StateNotSupported` if the `new_state` is not a valid
		/// supported state. Returns `Error::<T>::RegistryAlreadyInSameState` if the Registry is
		/// already in the provided `new_state`.
		/// Returns `Error::<T>::UnauthorizedOperation` if the caller does not have the necessary
		/// OWNER or ADMIN permissions to perform the state update.
		///
		/// # Events
		/// Emits `Event::RegistryStateChanged` when the Registry state is successfully updated.
		/// This event includes the `who` (the updater), the `registry_id`, and the `new_state`.
		///
		/// # Example
		/// ```
		/// update_state(origin, registry_id, new_state)?;
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn update_state(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			new_state: RegistrySupportedStateOf,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut registry =
				Registry::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure given `new_state` is a valid supported state */
			ensure!(new_state.is_valid_state(), Error::<T>::StateNotSupported);

			if registry.state == new_state {
				return Err(Error::<T>::RegistryAlreadyInSameState.into());
			}

			/* Ensure that the registry state update happens from an authorized OWNER/ADMIN
			 * account */
			let is_authorized = Self::ensure_owner_or_admin_authorization(&registry, &who);
			ensure!(is_authorized, Error::<T>::UnauthorizedOperation);

			registry.state = new_state.clone();
			Registry::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::RegistryStateChanged { who, registry_id, new_state });

			Ok(())
		}

		/// Updates the details of an existing Registry.
		///
		/// This function allows a user to update various attributes of a Registry identified by
		/// `registry_id`. It supports updating the `digest`, `blob`, and `state` of the Registry.
		/// The update operation is only allowed for accounts with OWNER or ADMIN permissions. If
		/// the new state is provided, it must be a valid supported state.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user.
		/// * `registry_id` - The SS58-encoded identifier of the Registry to be updated.
		/// * `digest` - The new digest to set for the Registry. If provided, it replaces the
		///   current digest.
		/// * `blob` - (Optional) The new blob data to be associated with the Registry.
		/// * `state` - (Optional) The new state to set for the Registry. If provided, it must be a
		///   valid supported state.
		///
		/// # Errors
		/// Returns `Error::<T>::RegistryIdDoesNotExist` if the `registry_id` does not exist in
		/// storage. Returns `Error::<T>::UnauthorizedOperation` if the caller does not have the
		/// necessary OWNER or ADMIN permissions to perform the update.
		/// Returns `Error::<T>::StateNotSupported` if the `new_state` is not a valid supported
		/// state.
		///
		/// # Events
		/// Emits `Event::RegistryUpdated` when the Registry is successfully updated. This event
		/// includes the `updater` (the caller) and the `registry_id`.
		///
		/// # Example
		/// ```
		/// update(origin, registry_id, new_digest, Some(new_blob), Some(new_state))?;
		/// ```
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		// TODO:
		// Should `template_id` be allowed to be upadted for a Registry.
		pub fn update(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			digest: RegistryHashOf<T>,
			blob: Option<RegistryBlobOf<T>>,
			state: Option<RegistrySupportedStateOf>,
		) -> DispatchResult {
			let updater = ensure_signed(origin)?;

			let mut registry =
				Registry::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure account is authorized with OWNER/ ADMIN permissions */
			let is_authorized = Self::ensure_owner_or_admin_authorization(&registry, &updater);
			ensure!(is_authorized, Error::<T>::UnauthorizedOperation);

			registry.digest = digest;

			if let Some(_new_blob) = blob {
				/* TODO:
				 * Handle blob updates. Should it be matched against digest.
				 */
			}

			if let Some(new_state) = state {
				ensure!(new_state.is_valid_state(), Error::<T>::StateNotSupported);
				registry.state = new_state;
			}

			Registry::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::RegistryUpdated { updater, registry_id });

			Ok(())
		}

		/// Adds a new delegate with OWNER permission to the specified registry.
		///
		/// The origin of this call must be a delegate with OWNER permission for the registry. The
		/// function will update the permissions of an existing delegate or add a new delegate
		/// with OWNER permissions depending of delegate's existence in the list.
		///
		/// # Parameters
		/// - `origin`: The account making the call. Must be signed and authorized as an OWNER for
		///   the registry.
		/// - `registry_id`: The identifier of the registry to which the delegate is being added.
		/// - `delegate`: The account ID of the delegate to be added or updated.
		///
		/// # Errors
		/// - `RegistryIdDoesNotExist`: The specified `registry_id` does not exist in the storage.
		/// - `UnauthorizedOperation`: The caller does not have OWNER permission for the registry.
		/// - `DelegateAlreadyHasOwnerPermission`: The specified `delegate` already has OWNER
		///   permission in the registry.
		/// - `MaxDelegatesStorageOverflow`: The storage for delegates has reached its maximum
		///   capacity.
		///
		/// # Success
		/// - If the delegate already exists but does not have OWNER permission, their permissions
		///   are updated to include OWNER.
		/// - If the delegate does not exist, they are added to the Registry `delegates` list with
		///   OWNER permission.
		///
		/// # Event
		/// - `RegistryOwnerDelegateAdded`: Emitted when a delegate is successfully added or updated
		///   with OWNER permission.
		///
		/// # Example
		/// ```rust
		/// let result = add_owner_delegate(origin, registry_id, delegate);
		/// ```

		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn add_owner_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		) -> DispatchResult {
			let delegator = ensure_signed(origin)?;

			let mut registry =
				Registry::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure account is authorized with OWNER permission */
			let is_delegator_owner = Self::ensure_owner_authorization(&registry, &delegator);
			ensure!(is_delegator_owner, Error::<T>::UnauthorizedOperation);

			/* Search for the delegate in the list. If found with OWNER permission, return an
			 * error. Otherwise, update the permissions to include OWNER.
			 * If the delegate is not found, append them to the list with OWNER permission as a
			 * new delegate.
			 */
			if let Some(delegate_info) =
				registry.delegates.iter_mut().find(|info| info.delegate == delegate)
			{
				if delegate_info.permissions.contains(Permissions::OWNER) {
					return Err(Error::<T>::DelegateAlreadyHasOwnerPermission.into());
				}

				delegate_info.permissions.insert(Permissions::OWNER);
			} else {
				let new_delegate =
					DelegateInfo { delegate: delegate.clone(), permissions: Permissions::OWNER };

				if registry.delegates.try_push(new_delegate).is_err() {
					return Err(Error::<T>::MaxDelegatesStorageOverflow.into());
				}
			}

			Registry::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::RegistryOwnerDelegateAdded {
				delegator,
				registry_id,
				delegate,
			});

			Ok(())
		}

		/// Adds a new delegate with ADMIN permission to the specified registry.
		///
		/// The origin of this call must be a signed account with OWNER permission for the registry.
		/// The function will update the permissions of an existing delegate or add a new delegate
		/// with ADMIN permissions, depending on the existence of the delegate in the list.
		///
		/// # Parameters
		/// - `origin`: The account making the call. Must be signed and authorized as an OWNER for
		///   the registry.
		/// - `registry_id`: The identifier of the registry to which the delegate is being added.
		/// - `delegate`: The account ID of the delegate to be added or updated.
		///
		/// # Errors
		/// - `RegistryIdDoesNotExist`: The specified `registry_id` does not exist in the storage.
		/// - `UnauthorizedOperation`: The caller does not have OWNER permission for the registry.
		/// - `DelegateAlreadyHasAdminPermission`: The specified `delegate` already has ADMIN
		///   permission in the registry.
		/// - `MaxDelegatesStorageOverflow`: The storage for delegates has reached its maximum
		///   capacity.
		///
		/// # Success
		/// - If the delegate already exists but does not have ADMIN permission, their permissions
		///   are updated to include ADMIN.
		/// - If the delegate does not exist, they are added to the registry with ADMIN permission.
		///
		/// # Event
		/// - `RegistryAdminDelegateAdded`: Emitted when a delegate is successfully added or updated
		///   with ADMIN permission.
		///
		/// # Example
		/// ```rust
		/// let result = add_admin_delegate(origin, registry_id, delegate);
		/// ```
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn add_admin_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		) -> DispatchResult {
			let delegator = ensure_signed(origin)?;

			let mut registry =
				Registry::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure account is authorized with OWNER permission */
			let is_delegator_owner = Self::ensure_owner_authorization(&registry, &delegator);
			ensure!(is_delegator_owner, Error::<T>::UnauthorizedOperation);

			/* Search for the delegate in the list. If found with ADMIN permission, return an
			 * error. Otherwise, update the permissions to include ADMIN.
			 * If the delegate is not found, append them to the list with ADMIN permission as a
			 * new delegate.
			 */
			if let Some(delegate_info) =
				registry.delegates.iter_mut().find(|d| d.delegate == delegate)
			{
				if delegate_info.permissions.contains(Permissions::ADMIN) {
					return Err(Error::<T>::DelegateAlreadyHasAdminPermission.into());
				}
				delegate_info.permissions.insert(Permissions::ADMIN);
			} else {
				let new_delegate =
					DelegateInfo { delegate: delegate.clone(), permissions: Permissions::ADMIN };

				if registry.delegates.try_push(new_delegate).is_err() {
					return Err(Error::<T>::MaxDelegatesStorageOverflow.into());
				}
			}

			Registry::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::RegistryAdminDelegateAdded {
				delegator,
				registry_id,
				delegate,
			});

			Ok(())
		}

		/// Adds a new delegate with DELEGATE permission to the specified registry.
		///
		/// The origin of this call must be a signed account with either OWNER or ADMIN permission
		/// for the registry. The function will update the permissions of an existing delegate or
		/// add a new delegate with DELEGATE permissions, depending on the existence of the
		/// delegate in the list.
		///
		/// # Parameters
		/// - `origin`: The account making the call. Must be signed and authorized as OWNER or ADMIN
		///   for the registry.
		/// - `registry_id`: The identifier of the registry to which the delegate is being added.
		/// - `delegate`: The account ID of the delegate to be added or updated.
		///
		/// # Errors
		/// - `RegistryIdDoesNotExist`: The specified `registry_id` does not exist in the storage.
		/// - `UnauthorizedOperation`: The caller does not have OWNER or ADMIN permission for the
		///   registry.
		/// - `DelegateAlreadyHasPermission`: The specified `delegate` already has DELEGATE
		///   permission in the registry.
		/// - `MaxDelegatesStorageOverflow`: The storage for delegates has reached its maximum
		///   capacity.
		///
		/// # Success
		/// - If the delegate already exists but does not have DELEGATE permission, their
		///   permissions are updated to include DELEGATE.
		/// - If the delegate does not exist, they are added to the registry with DELEGATE
		///   permission.
		///
		/// # Event
		/// - `RegistryDelegateAdded`: Emitted when a delegate is successfully added or updated with
		///   DELEGATE permission.
		///
		/// # Example
		/// ```rust
		/// let result = add_delegate(origin, registry_id, delegate);
		/// ```
		#[pallet::call_index(5)]
		#[pallet::weight({0})]
		pub fn add_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		) -> DispatchResult {
			let delegator = ensure_signed(origin)?;

			let mut registry =
				Registry::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure account is authorized with OWNER/ ADMIN permission */
			let is_authorized = Self::ensure_owner_or_admin_authorization(&registry, &delegator);
			ensure!(is_authorized, Error::<T>::UnauthorizedOperation);

			/* Search for the delegate in the list. If found with DELEGATE permission, return an
			 * error. Otherwise, update the permissions to include DELEGATE.
			 * If the delegate is not found, append them to the list with DELEGATE permission as
			 * a new delegate.
			 */
			if let Some(delegate_info) =
				registry.delegates.iter_mut().find(|d| d.delegate == delegate)
			{
				if delegate_info.permissions.contains(Permissions::DELEGATE) {
					return Err(Error::<T>::DelegateAlreadyHasPermission.into());
				}
				delegate_info.permissions.insert(Permissions::DELEGATE);
			} else {
				let new_delegate =
					DelegateInfo { delegate: delegate.clone(), permissions: Permissions::DELEGATE };

				if registry.delegates.try_push(new_delegate).is_err() {
					return Err(Error::<T>::MaxDelegatesStorageOverflow.into());
				}
			}

			Registry::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::RegistryDelegateAdded { delegator, registry_id, delegate });

			Ok(())
		}

		/// Removes the ADMIN permission from a delegate in the specified registry.
		///
		/// The origin of this call must be a signed account with OWNER permission for the registry.
		/// The function will update the permissions of the specified delegate by removing their
		/// ADMIN permission. If the delegate has no other permissions, they will be removed from
		/// the registry's delegate list.
		///
		/// # Parameters
		/// - `origin`: The account making the call. Must be signed and authorized as OWNER for the
		///   registry.
		/// - `registry_id`: The identifier of the registry from which the delegate's ADMIN
		///   permission is being removed.
		/// - `delegate`: The account ID of the delegate whose ADMIN permission is to be removed.
		///
		/// # Errors
		/// - `RegistryIdDoesNotExist`: The specified `registry_id` does not exist in the storage.
		/// - `UnauthorizedOperation`: The caller does not have OWNER permission for the registry.
		/// - `DelegateDoesNotHaveAdminPermission`: The specified `delegate` does not have ADMIN
		///   permission, so it cannot be removed.
		/// - `DelegateNotFound`: The specified `delegate` does not exist in the list of delegates.
		///
		/// # Success
		/// - Removes the ADMIN permission from the specified delegate, leaving other permissions
		///   intact.
		/// - If the delegate has no remaining permissions, they are removed from the delegate list.
		///
		/// # Event
		/// - `RegistryAdminDelegateRemoved`: Emitted when a delegate's ADMIN permission is
		///   successfully removed, or if the delegate is removed from the list.
		///
		/// # Example
		/// ```rust
		/// let result = remove_admin_delegate(origin, registry_id, delegate);
		/// ```
		#[pallet::call_index(6)]
		#[pallet::weight({0})]
		pub fn remove_admin_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		) -> DispatchResult {
			let remover = ensure_signed(origin)?;

			let mut registry =
				Registry::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure account is authorized with OWNER permission */
			let is_owner = Self::ensure_owner_authorization(&registry, &remover);
			ensure!(is_owner, Error::<T>::UnauthorizedOperation);

			/*
			 * Search the delegate list and modify the permissions of the matching delegate.
			 * Ensure the delegate possesses ADMIN permission, otherwise return an error.
			 * Remove only the ADMIN permission, leaving other permissions intact.
			 * If the delegate has no remaining permissions, remove them from the list.
			 */
			let mut delegate_found = false;
			for delegate_info in registry.delegates.iter_mut() {
				if delegate_info.delegate == delegate {
					delegate_found = true;

					ensure!(
						delegate_info.permissions.contains(Permissions::ADMIN),
						Error::<T>::DelegateDoesNotHaveAdminPermission
					);

					delegate_info.permissions.remove(Permissions::ADMIN);

					if delegate_info.permissions.is_empty() {
						registry.delegates.retain(|d| d.delegate != delegate);
					}
					break;
				}
			}

			/* Ensure the delegate is found in the Delegates list */
			ensure!(delegate_found, Error::<T>::DelegateNotFound);

			Registry::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::RegistryAdminDelegateRemoved {
				remover,
				registry_id,
				delegate,
			});

			Ok(())
		}

		/// Removes the DELEGATE permission from a delegate in the specified registry.
		///
		/// The origin of this call must be a signed account with OWNER or ADMIN permission for the
		/// registry. The function updates the permissions of the specified delegate by removing
		/// their DELEGATE permission. If the delegate has no other permissions, they are removed
		/// from the registry's delegate list.
		///
		/// # Parameters
		/// - `origin`: The account making the call. Must be signed and authorized as OWNER or ADMIN
		///   for the registry.
		/// - `registry_id`: The identifier of the registry from which the delegate's DELEGATE
		///   permission is being removed.
		/// - `delegate`: The account ID of the delegate whose DELEGATE permission is to be removed.
		///
		/// # Errors
		/// - `RegistryIdDoesNotExist`: The specified `registry_id` does not exist in the storage.
		/// - `UnauthorizedOperation`: The caller does not have OWNER or ADMIN permission for the
		///   registry.
		/// - `DelegatePermissionDoesNotExist`: The specified `delegate` does not have DELEGATE
		///   permission, so it cannot be removed.
		/// - `DelegateNotFound`: The specified `delegate` does not exist in the list of delegates.
		///
		/// # Success
		/// - Removes the DELEGATE permission from the specified delegate, leaving other permissions
		///   intact.
		/// - If the delegate has no remaining permissions, they are removed from the delegate list.
		///
		/// # Event
		/// - `RegistryDelegateRemoved`: Emitted when a delegate's DELEGATE permission is
		///   successfully removed, or if the delegate is removed from the list.
		///
		/// # Example
		/// ```rust
		/// let result = remove_delegate(origin, registry_id, delegate);
		/// ```
		#[pallet::call_index(7)]
		#[pallet::weight({0})]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: DelegateOf<T>,
		) -> DispatchResult {
			let remover = ensure_signed(origin)?;

			let mut registry =
				Registry::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

			/* Ensure account is authorized with OWNER/ ADMIN permission */
			let is_authorized = Self::ensure_owner_or_admin_authorization(&registry, &remover);
			ensure!(is_authorized, Error::<T>::UnauthorizedOperation);

			/*
			 * Search the delegate list and modify the permissions of the matching delegate.
			 * Ensure the delegate possesses DELEGATE permission, otherwise return an error.
			 * Remove only the DELEGATE permission, leaving other permissions intact.
			 * If the delegate has no remaining permissions, remove them from the list.
			 */
			let mut delegate_found = false;
			for delegate_info in registry.delegates.iter_mut() {
				if delegate_info.delegate == delegate {
					delegate_found = true;

					ensure!(
						delegate_info.permissions.contains(Permissions::DELEGATE),
						Error::<T>::DelegatePermissionDoesNotExist
					);

					delegate_info.permissions.remove(Permissions::DELEGATE);

					if delegate_info.permissions.is_empty() {
						registry.delegates.retain(|d| d.delegate != delegate);
					}
					break;
				}
			}

			/* Ensure the delegate is found on Delegates list */
			ensure!(delegate_found, Error::<T>::DelegateNotFound);

			Registry::<T>::insert(&registry_id, registry);

			Self::deposit_event(Event::RegistryDelegateRemoved { remover, registry_id, delegate });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Method to check if the `registry_id` is a valid one.
	/// Returns the registry on success.
	pub fn ensure_valid_registry_id(
		registry_id: &RegistryIdOf,
	) -> Result<RegistryDetailsOf<T>, Error<T>> {
		let registry =
			Registry::<T>::get(&registry_id).ok_or(Error::<T>::RegistryIdDoesNotExist)?;

		Ok(registry)
	}

	/// Method to check if the `delegate` has either one of OWNER/ADMIN/DELEGATE permissions &
	/// is part of the `delegates` list.
	pub fn ensure_delegate_authorization(
		registry: &RegistryDetailsOf<T>,
		delegate: &DelegateOf<T>,
	) -> bool {
		let is_authorized = registry.delegates.iter().any(|delegate_info| {
			delegate_info.delegate == *delegate &&
				delegate_info
					.permissions
					.intersects(Permissions::OWNER | Permissions::ADMIN | Permissions::DELEGATE)
		});

		if is_authorized {
			return true
		}
		return false
	}

	/// Method to check if the `delegate` is authorized with either one of OWNER/ADMIN permission.
	pub fn ensure_owner_or_admin_authorization(
		registry: &RegistryDetailsOf<T>,
		delegate: &DelegateOf<T>,
	) -> bool {
		let is_authorized = registry.delegates.iter().any(|delegate_info| {
			delegate_info.delegate == *delegate &&
				delegate_info.permissions.intersects(Permissions::OWNER | Permissions::ADMIN)
		});

		if is_authorized {
			return true
		}
		return false
	}

	/// Method to check if the `delegate` is authorized with OWNER permission.
	pub fn ensure_owner_authorization(
		registry: &RegistryDetailsOf<T>,
		delegate: &DelegateOf<T>,
	) -> bool {
		let is_authorized = registry.delegates.iter().any(|delegate_info| {
			delegate_info.delegate == *delegate &&
				delegate_info.permissions.contains(Permissions::OWNER)
		});

		if is_authorized {
			return true
		}
		return false
	}

	/// Method to check if the input identifier calculated from sdk
	/// is actually a valid SS58 Identifier Format and of valid type `DeDir`.
	pub fn is_valid_ss58_format(identifier: &Ss58Identifier) -> bool {
		match identifier.get_type() {
			Ok(id_type) =>
				if id_type == IdentifierType::Registries {
					log::debug!("The SS58 identifier is of type Registries.");
					true
				} else {
					log::debug!("The SS58 identifier is not of type Registries.");
					false
				},
			Err(e) => {
				log::debug!("Invalid SS58 identifier. Error: {:?}", e);
				false
			},
		}
	}

	// /// Method to check if blob matches the given digest
	// pub fn does_blob_matches_digest(blob: &RegistryBlobOf<T>, digest: &RegistryHashOf<T>) -> bool
	// { 	let blob_digest = <T as frame_system::Config>::Hashing::hash(&blob.encode()[..]);

	// 	log::info!("digest: {:?}, blob_digest: {:?}, blob: {:?}", *digest, blob_digest, blob);

	// 	blob_digest == *digest
	// }
}
