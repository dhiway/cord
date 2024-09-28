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

//! # Registries Pallet
//!
//! The Registries pallet provides a framework for creating and managing
//! isolated registries within the CORD blockchain that can be governed and
//! moderated with a fine-grained permission system. It allows for the creation,
//! changing the status of the registry, as well as the management of delegates
//! within these registry.
//!
//! ## Overview
//!
//! The Registry pallet allows for the creation of distinct registry on the CORD
//! blockchain, each with its own set of rules and governance. These registry can
//! be used to manage different ecosystems or communities within the larger
//! blockchain environment. Registry are created with a unique identifier and can
//! be managed by appointed delegates.
//!
//! ## Interface
//!
//! The pallet provides dispatchable functions for registry management:
//!
//! - `create`: Initializes a new registry with a unique identifier.
//! - `update`: Updates the existing registry with newer data.
//! - `revoke`: Marks a registry as revoked, effectively changing it to revoked status.
//! - `reinstate`: Changes the status of the registry, returning it to active status.
//! - `archive`: Marks a registry as archived, effectively changing it to archived status.
//! - `restore`: Changes the status of the registry, returning it to non-archival status.
//! - `add_delegate`: Adds a delegate to a registry, granting them specific permissions.
//! - `add_admin_delegate`: Adds an admin delegate to a registry, granting them administrative
//!   permissions.
//! - `add_audit_delegate`: Adds an audit delegate to a registry, granting them audit permissions.
//! - `remove_delegate`: Removes a delegate from a registry, revoking their permissions.
//!
//!
//! ## Permissions
//!
//! The pallet uses a permissions system to manage the actions that delegates
//! can perform within a registry. Permissions are granular and can be assigned to
//! different roles, such as an admin or a regular delegate.
//!
//! ## Data Privacy
//!
//! The Registries pallet is designed with data privacy as a core consideration.
//! It does not directly store any personal or sensitive information on-chain.
//! Instead, it manages references to off-chain data, ensuring that the
//! blockchain layer remains compliant with data privacy regulations. Users and
//! developers are responsible for ensuring that the off-chain data handling
//! processes adhere to the applicable laws and standards.
//!
//! ## Usage
//!
//! The Registries pallet can be used by other pallets ex. Entries pallet to create
//! compartmentalized and governed sections of the blockchain. This is
//! particularly useful for applications that require distinct governance models
//! or privacy settings within a shared ecosystem.
//!
//! ## Governance Integration
//!
//! The Registries pallet is integrated with on-chain governance pallets to
//! allow registry administrators and delegates to propose changes, vote on
//! initiatives, or manage the registry in accordance with the collective decisions
//! of its members.
//!
//! ## Examples
//!
//! - Creating a new registry for a community-driven project.
//! - Archiving, Restoring a registry that is to be stashed for a while.
//! - Revoking, Re-instating a registry that is no longer active or has violated terms of use.
//! - Adding delegates to a registry to ensure ongoing compliance with governance standards.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(test)]
mod tests;

use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
pub mod types;
pub use crate::{pallet::*, types::*};
use codec::Encode;
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_runtime::traits::{Hash, UniqueSaturatedInto};

/// Authorization Identifier
pub type AuthorizationIdOf = Ss58Identifier;
/// Type of the Registry Id
pub type RegistryIdOf = Ss58Identifier;
/// Tyoe of the Registry Digest
pub type RegistryHashOf<T> = <T as frame_system::Config>::Hash;
/// Type of the Registry Creator
pub type RegistryCreatorOf<T> = <T as frame_system::Config>::AccountId;
/// Type of the Registry Template Id
pub type TemplateIdOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedInputLength>;
/// Type of the Schema Id
pub type SchemaIdOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedInputLength>;
/// Type of Maximum allowed size of the Registry Blob
pub type MaxRegistryBlobSizeOf<T> = <T as crate::Config>::MaxRegistryBlobSize;
/// Type of Registry Blob
pub type RegistryBlobOf<T> = BoundedVec<u8, MaxRegistryBlobSizeOf<T>>;
/// Type of the Registry Authorization Details
pub type RegistryAuthorizationOf<T> =
	RegistryAuthorization<RegistryIdOf, RegistryCreatorOf<T>, Permissions>;
/// Type of Registry Details
pub type RegistryDetailsOf<T> = RegistryDetails<RegistryCreatorOf<T>, StatusOf, RegistryHashOf<T>>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{IsPermissioned, StatusOf};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use frame_system::WeightInfo;
	pub use identifier::{
		CordIdentifierType, IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier,
	};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config + identifier::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type MaxRegistryDelegates: Get<u32>;

		#[pallet::constant]
		type MaxRegistryBlobSize: Get<u32>;

		#[pallet::constant]
		type MaxEncodedInputLength: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Registry information stored on chain.
	/// It maps from an identifier to its details.
	#[pallet::storage]
	pub type RegistryInfo<T> =
		StorageMap<_, Blake2_128Concat, RegistryIdOf, RegistryDetailsOf<T>, OptionQuery>;

	/// Registry authorizations stored on-chain.
	/// It maps from an identifier to delegates.
	#[pallet::storage]
	pub type Authorizations<T> =
		StorageMap<_, Blake2_128Concat, AuthorizationIdOf, RegistryAuthorizationOf<T>, OptionQuery>;

	/// Registry delegates stored on chain.
	/// It maps from an identifier to a  bounded vec of delegates and
	/// permissions.
	#[pallet::storage]
	pub(super) type Delegates<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		RegistryIdOf,
		BoundedVec<RegistryCreatorOf<T>, T::MaxRegistryDelegates>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new registry authorization has been added.
		/// \[registry identifier, authorization,  delegate\]
		Authorization {
			registry_id: RegistryIdOf,
			authorization: AuthorizationIdOf,
			delegate: RegistryCreatorOf<T>,
		},
		/// A registry authorization has been removed.
		/// \[registry identifier, authorization, ]
		Deauthorization { registry_id: RegistryIdOf, authorization: AuthorizationIdOf },
		/// A new registry has been created.
		/// \[registry identifier, creator, authorization\]
		Create {
			registry_id: RegistryIdOf,
			creator: RegistryCreatorOf<T>,
			authorization: AuthorizationIdOf,
		},
		/// A registry has been revoked.
		/// \[registry identifier, authority\]
		Revoke { registry_id: RegistryIdOf, authority: RegistryCreatorOf<T> },
		/// A registry has been reinstated.
		/// \[registry identifier,  authority\]
		Reinstate { registry_id: RegistryIdOf, authority: RegistryCreatorOf<T> },
		/// A existing registry has been updated.
		/// \[registry identifier, updater, authorization\]
		Update {
			registry_id: RegistryIdOf,
			updater: RegistryCreatorOf<T>,
			authorization: AuthorizationIdOf,
		},
		/// A registry has been archived.
		/// \[registry identifier,  authority\]
		Archive { registry_id: RegistryIdOf, authority: RegistryCreatorOf<T> },
		/// A registry has been restored.
		/// \[registry identifier, authority\]
		Restore { registry_id: RegistryIdOf, authority: RegistryCreatorOf<T> },
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// Registry identifier is not unique
		RegistryAlreadyAnchored,
		/// Registry identifier not found
		RegistryNotFound,
		/// Only when the author is not the controller or delegate.
		UnauthorizedOperation,
		/// Invalid Identifier
		InvalidIdentifier,
		/// Invalid Identifier Length
		InvalidIdentifierLength,
		/// Registry delegation limit exceeded
		RegistryDelegatesLimitExceeded,
		/// Authority already added
		DelegateAlreadyAdded,
		/// Authorization Id not found
		AuthorizationNotFound,
		/// Delegate not found.
		DelegateNotFound,
		/// Registry not revoked.
		RegistryNotRevoked,
		/// Registry already revoked
		RegistryAlreadyRevoked,
		/// Registry revoked.
		RegistryRevoked,
		/// Registry not archived.
		RegistryNotArchived,
		/// Registry already arhived.
		RegistryAlreadyArchived,
		/// Registry not archived.
		RegistryArchived,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds a delegate with permission to assert new entries to a registry.
		///
		/// The `ASSERT` permission enables a delegate to add and sign new entries
		/// within the specified registry. This function is used to grant this
		/// permission to a delegate, provided that the caller has sufficient
		/// authorization, typically as an admin of the registry.
		///
		/// The function checks that the caller is authorized (as an admin) to add
		/// a delegate with `ASSERT` permissions to the registry. If the caller's
		/// authorization is verified, the delegate is added using the internal
		/// `registry_delegate_addition` function.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an admin of the registry.
		/// - `registry_id`: The unique identifier of the registry to which the delegate is being
		///   added.
		/// - `delegate`: The account identifier of the delegate being granted the `ASSERT`
		///   permission.
		/// - `authorization`: The authorization ID used to validate the caller's permission to add
		///   a delegate.
		///
		/// # Returns
		/// Returns `Ok(())` if the delegate is successfully added with `ASSERT`
		/// permissions, or an `Err` if the operation fails due to authorization issues
		/// or internal errors during delegate addition.
		///
		/// # Errors
		/// - `UnauthorizedOperation`: If the caller does not have the necessary admin permissions
		///   for the registry.
		/// - Propagates errors from `registry_delegate_addition` if the addition fails.
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn add_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: RegistryCreatorOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let auth_registry_id =
				Self::ensure_authorization_delegator_origin(&authorization, &creator)?;
			ensure!(auth_registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			let permissions = Permissions::ASSERT;
			Self::registry_delegate_addition(auth_registry_id, delegate, creator, permissions)?;

			Ok(())
		}

		/// Adds an administrative delegate to a registry.
		///
		/// This function grants the `ADMIN` permission to a specified delegate,
		/// allowing the delegate to manage other delegates and modify registry
		/// configurations. Only existing registry administrators can invoke this
		/// function to add another admin delegate.
		///
		/// The function ensures that the caller has sufficient administrative
		/// privileges in the registry and that the `registry_id` matches the
		/// authorization. If the checks pass, the delegate is added with `ADMIN`
		/// permissions using the internal `registry_delegate_addition` function.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an existing administrator of
		///   the registry.
		/// - `registry_id`: The unique identifier of the registry to which the admin delegate is
		///   being added.
		/// - `delegate`: The account identifier of the delegate being granted admin permissions.
		/// - `authorization`: The authorization ID used to validate the caller's permission to add
		///   an admin delegate to the specified registry.
		///
		/// # Returns
		/// Returns `Ok(())` if the admin delegate is successfully added, or an `Err`
		/// if the operation fails, such as when the caller lacks the necessary
		/// permissions or if there's an internal error during delegate addition.
		///
		/// # Errors
		/// - `UnauthorizedOperation`: If the caller does not have admin permissions in the
		///   registry.
		/// - Propagates errors from `registry_delegate_addition` if delegate addition fails.
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn add_admin_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: RegistryCreatorOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let auth_registry_id =
				Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			let permissions = Permissions::ADMIN;
			Self::registry_delegate_addition(auth_registry_id, delegate, creator, permissions)?;

			Ok(())
		}

		/// Adds an audit delegate to a registry.
		///
		/// The `AUDIT` permission allows the delegate to perform oversight and
		/// compliance checks within the registry. This function is used to grant
		/// these audit privileges to a delegate. It checks that the caller has the
		/// necessary administrative rights to add an audit delegate to the registry.
		///
		/// If the caller is authorized, the delegate is added with the `AUDIT`
		/// permission using the internal `registry_delegate_addition` function.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an existing administrator of
		///   the registry.
		/// - `registry_id`: The unique identifier of the registry to which the audit delegate is
		///   being added.
		/// - `delegate`: The account identifier of the delegate being granted audit permissions.
		/// - `authorization`: The authorization ID used to validate the caller's permission to add
		///   the audit delegate.
		///
		/// # Returns
		/// Returns `Ok(())` if the audit delegate is successfully added, or an `Err`
		/// if the operation fails due to authorization issues or internal errors
		/// during delegate addition.
		///
		/// # Errors
		/// - `UnauthorizedOperation`: If the caller does not have the necessary admin permissions
		///   for the registry.
		/// - Propagates errors from `registry_delegate_addition` if delegate addition fails.
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn add_delegator(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: RegistryCreatorOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let auth_registry_id =
				Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			let permissions = Permissions::DELEGATE;
			Self::registry_delegate_addition(auth_registry_id, delegate, creator, permissions)?;

			Ok(())
		}

		/// Removes a delegate from a specified registry.
		///
		/// This function removes an existing delegate from a registry, identified
		/// by the `registry_id` and the delegate's `remove_authorization` ID.
		/// It ensures that the registry exists, is not archived or revoked, and that
		/// the provided authorization corresponds to a delegate in the registry.
		/// Additionally, it verifies that the caller has the authority (admin rights)
		/// to remove the delegate.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an admin of the registry.
		/// - `registry_id`: The unique identifier of the registry from which the delegate is being
		///   removed.
		/// - `remove_authorization`: The authorization ID of the delegate to be removed.
		/// - `authorization`: The authorization ID validating the caller’s permission to perform
		///   the removal.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the delegate was successfully removed, or an
		///   error (`DispatchError`) if any of the checks fail.
		///
		/// # Errors
		/// - `AuthorizationNotFound`: If the provided `remove_authorization` does not exist.
		/// - `UnauthorizedOperation`: If the origin is not authorized to remove a delegate from the
		///   registry.
		/// - `RegistryNotFound`: If the specified `registry_id` does not correspond to an existing
		///   registry.
		/// - `RegistryArchived`: If the registry is archived and no longer active.
		/// - `RegistryRevoked`: If the registry has been revoked.
		/// - `DelegateNotFound`: If the delegate specified by `remove_authorization` is not found
		///   in the registry.
		///
		/// # Events
		/// - `Deauthorization`: Emitted when a delegate is successfully removed from the registry.
		///   The event includes the registry ID and the authorization ID of the removed delegate.
		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			remove_authorization: AuthorizationIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let auth_registry_id =
				Self::ensure_authorization_admin_remove_origin(&authorization, &creator)?;

			// Ensure remover does not de-delagate themselves &
			// remover has valid authoirzation for this particular registry-id.
			ensure!(authorization != remove_authorization, Error::<T>::UnauthorizedOperation);
			ensure!(auth_registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			// Ensure the authorization exists and retrieve its details.
			let authorization_details = Authorizations::<T>::get(&remove_authorization)
				.ok_or(Error::<T>::AuthorizationNotFound)?;

			let mut delegates = Delegates::<T>::get(&registry_id);
			if let Some(index) = delegates.iter().position(|d| d == &authorization_details.delegate)
			{
				delegates.remove(index);
				Delegates::<T>::insert(&registry_id, delegates);

				Authorizations::<T>::remove(&remove_authorization);

				Self::update_activity(
					&registry_id,
					IdentifierTypeOf::RegistryAuthorization,
					CallTypeOf::Deauthorization,
				)?;

				Self::deposit_event(Event::Deauthorization {
					registry_id,
					authorization: remove_authorization,
				});

				Ok(())
			} else {
				Err(Error::<T>::DelegateNotFound.into())
			}
		}

		/// Creates a new registry with a unique identifier based on the provided
		/// registry digest and the creator's identity.
		///
		/// This function generates a unique identifier for the registry by hashing
		/// the encoded digest of the registry and the creator's identifier. It ensures that the
		/// generated registry identifier is not already in use. An authorization
		/// ID is also created for the new registry, which is used to manage
		/// delegations. The creator is automatically added as a delegate with
		/// full permissions.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, signed by the creator.
		/// - `registry_id`: A unique code created to identify the registry.
		/// - `digest`: The digest representing the registry data to be created.
		/// - `blob`: Optional metadata or data associated with the registry.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the registry is successfully created, or an
		///   error (`DispatchError`) if:
		///   - The generated registry identifier is already in use.
		///   - The generated authorization ID has an invalid length.
		///   - The registry exceeds the allowed delegate limit.
		///
		/// # Errors
		/// - `InvalidIdentifierLength`: If the generated identifiers for the registry or
		///   authorization have invalid lengths.
		/// - `RegistryAlreadyAnchored`: If the registry identifier already exists.
		/// - `RegistryDelegatesLimitExceeded`: If the registry exceeds the maximum number of
		///   allowed delegates.
		///
		/// # Events
		/// - `Create`: Emitted when a new registry is successfully created. It includes the
		///   registry identifier, the creator's identifier, and the authorization ID.
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			_registry_id: RegistryIdOf,
			digest: RegistryHashOf<T>,
			_schema_id: Option<SchemaIdOf<T>>,
			_blob: Option<RegistryBlobOf<T>>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			// TODO: Create the identifier at SDK level & validate at chain level.
			// Id Digest = concat (H(<scale_encoded_registry_input_digest>,
			// <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &creator.encode()[..]].concat()[..],
			);

			// /* Ensure that registry_id is of valid ss58 format,
			//  * and also the type matches to be of `Registries`.
			//  */
			// ensure!(
			// 	Self::is_valid_ss58_format(&registry_id),
			// 	Error::<T>::InvalidRegistryIdentifier
			// );

			let identifier = Ss58Identifier::create_identifier(
				&id_digest.encode()[..],
				IdentifierType::Registries,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<RegistryInfo<T>>::contains_key(&identifier),
				Error::<T>::RegistryAlreadyAnchored
			);

			// Construct the authorization_id from the provided parameters.
			// Id Digest = concat (H(<scale_encoded_registry_identifier>,
			// <scale_encoded_creator_identifier> ))
			let auth_id_digest = T::Hashing::hash(
				&[&identifier.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()
					[..],
			);

			let authorization_id = Ss58Identifier::create_identifier(
				&auth_id_digest.encode(),
				IdentifierType::RegistryAuthorization,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			let mut delegates: BoundedVec<RegistryCreatorOf<T>, T::MaxRegistryDelegates> =
				BoundedVec::default();
			delegates
				.try_push(creator.clone())
				.map_err(|_| Error::<T>::RegistryDelegatesLimitExceeded)?;

			Delegates::<T>::insert(&identifier, delegates);

			Authorizations::<T>::insert(
				&authorization_id,
				RegistryAuthorizationOf::<T> {
					registry_id: identifier.clone(),
					delegate: creator.clone(),
					permissions: Permissions::all(),
					delegator: creator.clone(),
				},
			);

			<RegistryInfo<T>>::insert(
				&identifier,
				RegistryDetailsOf::<T> {
					creator: creator.clone(),
					revoked: false,
					archived: false,
					digest,
				},
			);

			Self::update_activity(&identifier, IdentifierTypeOf::Registries, CallTypeOf::Genesis)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Create {
				registry_id: identifier,
				creator,
				authorization: authorization_id,
			});

			Ok(())
		}

		/// Revokes a registry, marking it as no longer active.
		///
		/// This function marks a registry as revoked based on the provided registry
		/// ID. It checks that the registry exists, is not already revoked, and
		/// ensures that the caller has the authority to revoke the registry, as
		/// indicated by the provided authorization ID.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin with the appropriate authority.
		/// - `registry_id`: The identifier of the registry to be revoked.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   revocation.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the registry is successfully revoked, or an
		///   error (`DispatchError`) if:
		///   - The registry does not exist.
		///   - The registry is already revoked.
		///   - The caller does not have the authority to revoke the registry.
		///
		/// # Errors
		/// - `RegistryNotFound`: If the specified registry ID does not correspond to an existing
		///   registry.
		/// - `RegistryAlreadyRevoked`: If the registry has already been revoked.
		/// - `UnauthorizedOperation`: If the caller is not authorized to revoke the registry.
		///
		/// # Events
		/// - `Revoke`: Emitted when a registry is successfully revoked. It includes the registry ID
		///   and the authority who performed the revocation.
		#[pallet::call_index(6)]
		#[pallet::weight({0})]
		pub fn revoke(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let auth_registry_id =
				Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			let registry_details =
				RegistryInfo::<T>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;

			ensure!(!registry_details.revoked, Error::<T>::RegistryAlreadyRevoked);

			<RegistryInfo<T>>::insert(
				&registry_id,
				RegistryDetailsOf::<T> { revoked: true, ..registry_details },
			);

			Self::update_activity(&registry_id, IdentifierTypeOf::Registries, CallTypeOf::Revoke)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Revoke { registry_id, authority: creator });

			Ok(())
		}

		/// Reinstates a revoked registry, making it active again.
		///
		/// This function changes the status of a previously revoked registry to active
		/// based on the provided registry ID. It checks that the registry exists, is
		/// currently revoked, and ensures that the caller has the authority to reinstate
		/// the registry as indicated by the provided authorization ID.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin with the appropriate authority.
		/// - `registry_id`: The identifier of the registry to be reinstated.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   reinstatement.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the registry is successfully reinstated, or an
		///   error (`DispatchError`) if:
		///   - The registry does not exist.
		///   - The registry is not revoked.
		///   - The caller does not have the authority to reinstate the registry.
		///
		/// # Errors
		/// - `RegistryNotFound`: If the specified registry ID does not correspond to an existing
		///   registry.
		/// - `RegistryNotRevoked`: If the registry is not currently revoked.
		/// - `UnauthorizedOperation`: If the caller is not authorized to reinstate the registry.
		///
		/// # Events
		/// - `Reinstate`: Emitted when a registry is successfully reinstated. It includes the
		///   registry ID and the authority who performed the reinstatement.
		#[pallet::call_index(7)]
		#[pallet::weight({0})]
		pub fn reinstate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let auth_registry_id =
				Self::ensure_authorization_reinstate_origin(&authorization, &creator)?;

			ensure!(auth_registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			let registry_details =
				RegistryInfo::<T>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;

			ensure!(registry_details.revoked, Error::<T>::RegistryNotRevoked);

			<RegistryInfo<T>>::insert(
				&registry_id,
				RegistryDetailsOf::<T> { revoked: false, ..registry_details },
			);

			Self::update_activity(
				&registry_id,
				IdentifierTypeOf::Registries,
				CallTypeOf::Reinstate,
			)
			.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Reinstate { registry_id, authority: creator });

			Ok(())
		}

		/// Updates the digest and optional blob of a registry.
		///
		/// This function allows the creator or an admin with the appropriate authority
		/// to update the digest and optionally the blob of an existing registry. It checks
		/// that the registry exists, ensures that the caller has the necessary authorization,
		/// and updates the registry with the new digest and blob (if provided).
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin with the appropriate authority.
		/// - `registry_id`: The identifier of the registry to be updated.
		/// - `digest`: The new digest (hash) to be assigned to the registry.
		/// - `blob`: An optional new blob (data) to be assigned to the registry. If `None`, the
		///   existing blob remains unchanged.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   update.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the registry is successfully updated, or an
		///   error (`DispatchError`) if:
		///   - The registry does not exist.
		///   - The caller does not have the authority to update the registry.
		///
		/// # Errors
		/// - `RegistryNotFound`: If the specified registry ID does not correspond to an existing
		///   registry.
		/// - `UnauthorizedOperation`: If the caller is not authorized to update the registry.
		///
		/// # Events
		/// - `Update`: Emitted when a registry is successfully updated. It includes the registry
		///   ID, the updater, and the authorization used.
		#[pallet::call_index(8)]
		#[pallet::weight({0})]
		pub fn update(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			digest: RegistryHashOf<T>,
			_blob: Option<RegistryBlobOf<T>>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let mut registry =
				RegistryInfo::<T>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;

			let auth_registry_id =
				Self::ensure_authorization_admin_origin(&authorization, &creator)?;
			ensure!(auth_registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			registry.digest = digest;

			<RegistryInfo<T>>::insert(&registry_id, registry);

			Self::update_activity(&registry_id, IdentifierTypeOf::Registries, CallTypeOf::Update)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Update {
				registry_id: registry_id.clone(),
				updater: creator,
				authorization,
			});

			Ok(())
		}

		/// Archives a registry, marking it as inactive.
		///
		/// This function allows the creator or an admin with the appropriate authority
		/// to archive an existing registry. It checks that the registry exists, is not already
		/// archived, and ensures that the caller has the necessary authorization to perform the
		/// archival.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin with the appropriate authority.
		/// - `registry_id`: The identifier of the registry to be archived.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   archival.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the registry is successfully archived, or an
		///   error (`DispatchError`) if:
		///   - The registry does not exist.
		///   - The registry is already archived.
		///   - The caller does not have the authority to archive the registry.
		///
		/// # Errors
		/// - `RegistryNotFound`: If the specified registry ID does not correspond to an existing
		///   registry.
		/// - `RegistryAlreadyArchived`: If the registry is already archived.
		/// - `UnauthorizedOperation`: If the caller is not authorized to archive the registry.
		///
		/// # Events
		/// - `Archive`: Emitted when a registry is successfully archived. It includes the registry
		///   ID and the authority who performed the archival.
		#[pallet::call_index(9)]
		#[pallet::weight({0})]
		pub fn archive(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let auth_registry_id =
				Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			let registry_details =
				RegistryInfo::<T>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;

			ensure!(!registry_details.archived, Error::<T>::RegistryAlreadyArchived);

			<RegistryInfo<T>>::insert(
				&registry_id,
				RegistryDetailsOf::<T> { archived: true, ..registry_details },
			);

			Self::update_activity(&registry_id, IdentifierTypeOf::Registries, CallTypeOf::Archive)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Archive { registry_id, authority: creator });

			Ok(())
		}

		/// Restores an archived registry, making it active again.
		///
		/// This function allows the creator or an admin with the appropriate authority
		/// to restore an archived registry. It checks that the registry exists, is currently
		/// archived, and ensures that the caller has the necessary authorization to perform the
		/// restoration.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin with the appropriate authority.
		/// - `registry_id`: The identifier of the registry to be restored.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   restoration.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the registry is successfully restored, or an
		///   error (`DispatchError`) if:
		///   - The registry does not exist.
		///   - The registry is not archived.
		///   - The caller does not have the authority to restore the registry.
		///
		/// # Errors
		/// - `RegistryNotFound`: If the specified registry ID does not correspond to an existing
		///   registry.
		/// - `RegistryNotArchived`: If the registry is not currently archived.
		/// - `UnauthorizedOperation`: If the caller is not authorized to restore the registry.
		///
		/// # Events
		/// - `Restore`: Emitted when a registry is successfully restored. It includes the registry
		///   ID and the authority who performed the restoration.
		#[pallet::call_index(10)]
		#[pallet::weight({0})]
		pub fn restore(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let auth_registry_id =
				Self::ensure_authorization_restore_origin(&authorization, &creator)?;

			ensure!(auth_registry_id == registry_id, Error::<T>::UnauthorizedOperation);

			let registry_details =
				RegistryInfo::<T>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;

			ensure!(registry_details.archived, Error::<T>::RegistryNotArchived);

			<RegistryInfo<T>>::insert(
				&registry_id,
				RegistryDetailsOf::<T> { archived: false, ..registry_details },
			);

			Self::update_activity(&registry_id, IdentifierTypeOf::Registries, CallTypeOf::Restore)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Restore { registry_id, authority: creator });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Adds a delegate to a registry with specified permissions.
	///
	/// This function will add a new delegate to a registry, given the registry's ID,
	/// the delegate's information, and the required permissions. It constructs
	/// an authorization ID based on the registry ID, delegate, and creator,
	/// ensuring that the delegate is not already added. It also checks that the
	/// registry is not archived and is not revoked.
	fn registry_delegate_addition(
		registry_id: RegistryIdOf,
		delegate: RegistryCreatorOf<T>,
		creator: RegistryCreatorOf<T>,
		permissions: Permissions,
	) -> Result<(), Error<T>> {
		// Id Digest = concat (H(<scale_encoded_registry_identifier>,
		// <scale_encoded_creator_identifier>, <scale_encoded_delegate_identifier>))
		let id_digest = T::Hashing::hash(
			&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
		);

		let delegate_authorization_id = Ss58Identifier::create_identifier(
			&id_digest.encode(),
			IdentifierType::RegistryAuthorization,
		)
		.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

		ensure!(
			!Authorizations::<T>::contains_key(&delegate_authorization_id),
			Error::<T>::DelegateAlreadyAdded
		);

		let mut delegates = Delegates::<T>::get(&registry_id);
		delegates
			.try_push(delegate.clone())
			.map_err(|_| Error::<T>::RegistryDelegatesLimitExceeded)?;
		Delegates::<T>::insert(&registry_id, delegates);

		Authorizations::<T>::insert(
			&delegate_authorization_id,
			RegistryAuthorizationOf::<T> {
				registry_id: registry_id.clone(),
				delegate: delegate.clone(),
				permissions,
				delegator: creator,
			},
		);

		Self::update_activity(
			&registry_id,
			IdentifierTypeOf::RegistryAuthorization,
			CallTypeOf::Authorization,
		)
		.map_err(Error::<T>::from)?;

		Self::deposit_event(Event::Authorization {
			registry_id,
			authorization: delegate_authorization_id,
			delegate,
		});

		Ok(())
	}

	/// Checks if a given entity is a delegate for the specified registry.
	///
	/// This function retrieves the list of delegates for a registry and determines
	/// whether the specified delegate is among them. It is a read-only
	/// operation and does not modify the state.
	pub fn is_a_delegate(tx_id: &RegistryIdOf, delegate: RegistryCreatorOf<T>) -> bool {
		<Delegates<T>>::get(tx_id).iter().any(|d| d == &delegate)
	}

	/// Verifies if a given delegate has a specific authorization.
	///
	/// This function checks if the provided delegate is associated with the
	/// given authorization ID and has the 'ASSERT' permission.
	pub fn ensure_authorization_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &RegistryCreatorOf<T>,
	) -> Result<RegistryIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_registry_for_transaction(&d.registry_id)?;

		ensure!(d.permissions.contains(Permissions::ASSERT), Error::<T>::UnauthorizedOperation);

		Ok(d.registry_id)
	}

	/// Verifies if a given delegate has a specific authorization.
	///
	/// This function checks if the provided delegate is associated with the
	/// given authorization ID and has the 'ADMIN' permission.
	/// This asserts for delegates authorization has the permission to reinstate.
	pub fn ensure_authorization_reinstate_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &RegistryCreatorOf<T>,
	) -> Result<RegistryIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_registry_for_reinstate_transaction(&d.registry_id)?;

		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);
		// ensure!(d.permissions.contains(Permissions::ASSERT), Error::<T>::UnauthorizedOperation);

		Ok(d.registry_id)
	}

	/// Verifies if a given delegate has a specific authorization.
	///
	/// This function checks if the provided delegate is associated with the
	/// given authorization ID and has the 'ADMIN' permission.
	/// This asserts for delegates authorization has the permission to restore.
	pub fn ensure_authorization_restore_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &RegistryCreatorOf<T>,
	) -> Result<RegistryIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_registry_for_restore_transaction(&d.registry_id)?;

		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);
		// ensure!(d.permissions.contains(Permissions::ASSERT), Error::<T>::UnauthorizedOperation);

		Ok(d.registry_id)
	}

	/// Checks if a given delegate is an admin for the registry associated with the
	/// authorization ID.
	///
	/// This function verifies whether the specified delegate is the admin of
	/// the registry by checking the 'ADMIN' permission within the authorization
	/// tied to the provided authorization ID.
	pub fn ensure_authorization_admin_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &RegistryCreatorOf<T>,
	) -> Result<RegistryIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_registry_for_transaction(&d.registry_id)?;

		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);

		Ok(d.registry_id)
	}

	/// Ensures that the given delegate is authorized to perform an audit
	/// operation on a registry.
	///
	/// This function checks whether the provided `authorization_id` corresponds
	/// to an existing authorization and whether the delegate associated with
	/// that authorization is allowed to perform audit operations. It also
	/// increments usage and validates the registry for transactions.
	pub fn ensure_authorization_delegator_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &RegistryCreatorOf<T>,
	) -> Result<RegistryIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_registry_for_transaction(&d.registry_id)?;

		ensure!(
			d.permissions.contains(Permissions::DELEGATE | Permissions::ADMIN),
			Error::<T>::UnauthorizedOperation
		);

		Ok(d.registry_id)
	}

	/// Checks if a given delegate is an admin for the registry associated with the
	/// authorization ID.
	///
	/// This function verifies whether the specified delegate is the admin of
	/// the registry by checking the 'ADMIN' permission within the authorization
	/// tied to the provided authorization ID.
	pub fn ensure_authorization_admin_remove_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &RegistryCreatorOf<T>,
	) -> Result<RegistryIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_registry_for_transaction(&d.registry_id)?;

		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);

		Ok(d.registry_id)
	}

	/// Validates that a registry is eligible for a new transaction.
	///
	/// This function ensures that a registry is not archived, is not revoked.
	/// It is a critical check that enforces the integrity and
	/// constraints of registry usage on the chain.
	pub fn validate_registry_for_transaction(registry_id: &RegistryIdOf) -> Result<(), Error<T>> {
		let registry_details =
			RegistryInfo::<T>::get(registry_id).ok_or(Error::<T>::RegistryNotFound)?;

		// Ensure the Registry is not `archived`.
		if registry_details.archived {
			return Err(Error::<T>::RegistryArchived);
		}

		// Ensure the Registry is not `revoked`.
		if registry_details.revoked {
			return Err(Error::<T>::RegistryRevoked);
		}

		Ok(())
	}

	/// Validates a registry for restore transactions.
	///
	/// This function checks that the specified registry exists.
	/// It is designed to be called before,
	/// performing any administrative actions on a registry to ensure
	/// that the registry is in a proper state for such transactions.
	pub fn validate_registry_for_restore_transaction(
		registry_id: &RegistryIdOf,
	) -> Result<(), Error<T>> {
		let registry_details =
			RegistryInfo::<T>::get(registry_id).ok_or(Error::<T>::RegistryNotFound)?;

		// Ensure the Registry is `archived`.
		if !registry_details.archived {
			return Err(Error::<T>::RegistryNotArchived);
		}

		Ok(())
	}

	/// Validates a registry for reinstate transactions.
	///
	/// This function checks that the specified registry exists.
	/// It is designed to be called before performing any administrative
	/// actions on a registry to ensure either the registry is in a proper state for such
	/// transactions.
	pub fn validate_registry_for_reinstate_transaction(
		registry_id: &RegistryIdOf,
	) -> Result<(), Error<T>> {
		let registry_details =
			RegistryInfo::<T>::get(registry_id).ok_or(Error::<T>::RegistryNotFound)?;

		// Ensure the Registry is `revoked`.
		if !registry_details.revoked {
			return Err(Error::<T>::RegistryNotRevoked);
		}

		Ok(())
	}

	/// Updates the global timeline with a new activity event for a registry.
	///
	/// This function is an internal mechanism that logs each significant change
	/// to a registry on the global timeline. It is automatically called by the
	/// system whenever an update to a registry occurs, capturing the type of
	/// activity and the precise time at which it happened. This automated
	/// tracking is crucial for maintaining a consistent and auditable record of
	/// all registry-related activities.
	pub fn update_activity(
		tx_id: &RegistryIdOf,
		tx_type: IdentifierTypeOf,
		tx_action: CallTypeOf,
	) -> Result<(), Error<T>> {
		let tx_moment = Self::timepoint();

		let tx_entry = EventEntryOf { action: tx_action, location: tx_moment };
		let _ = IdentifierTimeline::update_timeline::<T>(tx_id, tx_type, tx_entry);
		Ok(())
	}

	/// Retrieves the current timepoint.
	///
	/// This function returns a `Timepoint` structure containing the current
	/// block number and extrinsic index. It is typically used in conjunction
	/// with `update_activity` to record when an event occurred.
	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}

	/// Method to check if the input identifier calculated from sdk
	/// is actually a valid SS58 Identifier Format and of valid type `Registries`.
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
}

// TODO:
// Check permission required for `restore` & `reinstate`. Currently ASSERT is being checked.
// In chainspace implementation it requires ASSERT permission for `restore`.
