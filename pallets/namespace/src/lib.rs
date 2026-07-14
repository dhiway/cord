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

//! # NameSpace Pallet
//!
//! The NameSpace pallet provides a framework for creating and managing
//! isolated namespaces within the CORD blockchain that can be governed and
//! moderated with a fine-grained permission system. It allows for the creation,
//! approval, and archival of namespaces, as well as the management of delegates
//! within these namespaces.
//!
//! ## Overview
//!
//! The NameSpace pallet allows for the creation of distinct namespaces on the CORD
//! blockchain, each with its own set of rules and governance. These namespaces can
//! be used to manage different ecosystems or communities within the larger
//! blockchain environment. NameSpaces are created with a unique identifier and can
//! be managed by appointed delegates.
//!
//! ## Interface
//!
//! The pallet provides dispatchable functions for namespace management:
//!
//! - `create`: Initializes a new namespace with a unique identifier.
//! - `archive`: Marks a namespace as archived, effectively freezing its state.
//! - `restore`: Unarchives a namespace, returning it to active status.
//! - `add_delegate`: Adds a delegate to a namespace, granting them specific permissions.
//! - `add_admin_delegate`: Adds an admin delegate to a namespace, granting them administrative
//!   permissions.
//! - `add_audit_delegate`: Adds an audit delegate to a namespace, granting them audit permissions.
//! - `remove_delegate`: Removes a delegate from a namespace, revoking their permissions.
//!
//! ## Permissions
//!
//! The pallet uses a permissions system to manage the actions that delegates
//! can perform within a namespace. Permissions are granular and can be assigned to
//! different roles, such as an admin or a regular delegate.
//!
//! ## Data Privacy
//!
//! The NameSpace pallet is designed with data privacy as a core consideration.
//! It does not directly store any personal or sensitive information on-chain.
//! Instead, it manages references to off-chain data, ensuring that the
//! blockchain layer remains compliant with data privacy regulations. Users and
//! developers are responsible for ensuring that the off-chain data handling
//! processes adhere to the applicable laws and standards.
//!
//! ## Usage
//!
//! The NameSpace pallet can be used by other pallets to create
//! compartmentalized and governed sections of the blockchain. This is
//! particularly useful for applications that require distinct governance models
//! or privacy settings within a shared ecosystem.
//!
//! ## Governance Integration
//!
//! The NameSpace pallet is integrated with on-chain governance pallets to
//! allow namespace administrators and delegates to propose changes, vote on
//! initiatives, or manage the namespace in accordance with the collective decisions
//! of its members.
//!
//! ## Examples
//!
//! - Creating a new namespace for a community-driven project.
//! - Archiving a namespace that is no longer active or has violated terms of use.
//! - Restoring an archived namespace to reactivate it for further use after compliance checks.
//! - Adding delegates to a namespace to ensure ongoing compliance with governance standards.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#[cfg(test)]
pub mod mock;

#[cfg(test)]
mod tests;

use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
pub mod types;
pub use crate::{pallet::*, types::*};
use codec::Encode;
use frame_system::WeightInfo;
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_runtime::traits::{Hash, UniqueSaturatedInto};

/// Type of a NameSpace Hash
pub type NameSpaceHashOf<T> = <T as frame_system::Config>::Hash;

/// Type of Maximum allowed size of a NameSpace Blob
pub type MaxNameSpaceBlobSizeOf<T> = <T as crate::Config>::MaxNameSpaceBlobSize;

/// Type of a NameSpace Blob
pub type NameSpaceBlobOf<T> = BoundedVec<u8, MaxNameSpaceBlobSizeOf<T>>;

/// Type of a namespace creator.
pub type NameSpaceCreatorOf<T> = <T as frame_system::Config>::AccountId;

/// Namespace Identifier
pub type NameSpaceIdOf = Ss58Identifier;

/// Registry Identifier
pub type RegistryIdOf = Ss58Identifier;

/// Authorization Identifier
pub type AuthorizationIdOf = Ss58Identifier;

/// Namespace input code
pub type NameSpaceCodeOf<T> = <T as frame_system::Config>::Hash;

/// Registry Identifier mapped to a Namespace.
pub type MaxRegistriesOf<T> = <T as crate::Config>::MaxNameSpaceDelegates;

/// Type of on-chain Namespace details
pub type NameSpaceDetailsOf<T> = NameSpaceDetails<
	NameSpaceHashOf<T>,
	NameSpaceCreatorOf<T>,
	StatusOf,
	BoundedVec<RegistryIdOf, MaxRegistriesOf<T>>,
>;

/// Type of Namespace Authorization details
pub type NameSpaceAuthorizationOf<T> =
	NameSpaceAuthorization<NameSpaceIdOf, NameSpaceCreatorOf<T>, Permissions>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{IsPermissioned, StatusOf};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config + identifier::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		// TODO: Remove below two constants.
		type ChainSpaceOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type NetworkPermission: IsPermissioned;

		#[pallet::constant]
		type MaxNameSpaceDelegates: Get<u32>;

		#[pallet::constant]
		type MaxNameSpaceBlobSize: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Namespace information stored on chain.
	/// It maps from an identifier to its details.
	#[pallet::storage]
	pub type NameSpaces<T> =
		StorageMap<_, Blake2_128Concat, NameSpaceIdOf, NameSpaceDetailsOf<T>, OptionQuery>;

	/// Namespace authorizations stored on-chain.
	/// It maps from an identifier to delegates.
	#[pallet::storage]
	pub type Authorizations<T> = StorageMap<
		_,
		Blake2_128Concat,
		AuthorizationIdOf,
		NameSpaceAuthorizationOf<T>,
		OptionQuery,
	>;

	/// Namespace delegates stored on chain.
	/// It maps from an identifier to a  bounded vec of delegates and
	/// permissions.
	#[pallet::storage]
	pub(super) type Delegates<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		NameSpaceIdOf,
		BoundedVec<NameSpaceCreatorOf<T>, T::MaxNameSpaceDelegates>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new namespace authorization has been added.
		/// \[namespace identifier, authorization,  delegate\]
		Authorization {
			namespace: NameSpaceIdOf,
			authorization: AuthorizationIdOf,
			delegate: NameSpaceCreatorOf<T>,
		},
		/// A namespace authorization has been removed.
		/// \[namespace identifier, authorization, ]
		Deauthorization { namespace: NameSpaceIdOf, authorization: AuthorizationIdOf },
		/// A new namespace has been created.
		/// \[namespace identifier, creator, authorization\]
		Create {
			namespace: NameSpaceIdOf,
			creator: NameSpaceCreatorOf<T>,
			authorization: AuthorizationIdOf,
		},
		/// A namespace has been archived.
		/// \[namespace identifier,  authority\]
		Archive { namespace: NameSpaceIdOf, authority: NameSpaceCreatorOf<T> },
		/// A namespace has been restored.
		/// \[namespace identifier,  authority\]
		Restore { namespace: NameSpaceIdOf, authority: NameSpaceCreatorOf<T> },
		/// A namespace has been restored.
		/// \[namespace identifier, \]
		Revoke { namespace: NameSpaceIdOf },
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// NameSpace identifier is not unique
		NameSpaceAlreadyAnchored,
		/// NameSpace identifier not found
		NameSpaceNotFound,
		/// Only when the author is not the controller or delegate.
		UnauthorizedOperation,
		/// Invalid Identifier
		InvalidIdentifier,
		/// Invalid Identifier Length
		InvalidIdentifierLength,
		/// Invalid Identifier Prefix
		InvalidIdentifierPrefix,
		/// Archived NameSpace
		ArchivedNameSpace,
		/// NameSpace not Archived
		NameSpaceNotArchived,
		/// NameSpace delegation limit exceeded
		NameSpaceDelegatesLimitExceeded,
		/// Empty transaction.
		EmptyTransaction,
		/// Authority already added
		DelegateAlreadyAdded,
		/// Authorization Id not found
		AuthorizationNotFound,
		/// Delegate not found.
		DelegateNotFound,
		/// Namespace Registry list limit exceeded.
		NameSpaceRegistryListLimitExceeded,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds a delegate with the ability to assert new entries to a namespace.
		///
		/// The `ASSERT` permission allows the delegate to sign and add new
		/// entries within the namespace. This function is called to grant a
		/// delegate this specific permission. It checks that the caller has the
		/// necessary authorization (admin rights) to add a delegate to the
		/// namespace. If the caller is authorized, the delegate is added with the
		/// `ASSERT` permission using the `space_delegate_addition`
		/// internal function.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an admin of the namespace.
		/// - `namespace_id`: The identifier of the namespace to which the delegate is being added.
		/// - `delegate`: The identifier of the delegate being added to the namespace.
		/// - `authorization`: The authorization ID used to validate the addition.
		///
		/// # Returns
		/// Returns `Ok(())` if the delegate was successfully added with
		/// `ASSERT` permission, or an `Err` with an appropriate error if the
		/// operation fails.
		///
		/// # Errors
		/// - `UnauthorizedOperation`: If the caller is not an admin of the namespace.
		/// - Propagates errors from `space_delegate_addition` if it fails.
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn add_delegate(
			origin: OriginFor<T>,
			namespace_id: NameSpaceIdOf,
			delegate: NameSpaceCreatorOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let auth_space_id =
				Self::ensure_authorization_delegator_origin(&authorization, &creator)?;
			ensure!(auth_space_id == namespace_id, Error::<T>::UnauthorizedOperation);

			let permissions = Permissions::ASSERT;
			Self::space_delegate_addition(auth_space_id, delegate, creator, permissions)?;

			Ok(())
		}

		/// Adds an administrative delegate to a namespace.
		///
		/// The `ADMIN` permission grants the delegate extensive control over
		/// the namespace, including the ability to manage other delegates and
		/// change namespace configurations. This function is called to
		/// grant a delegate these administrative privileges. It verifies that
		/// the caller has the necessary authorization (admin rights) to add an
		/// admin delegate to the namespace. If the caller is authorized,
		/// the delegate is added with the `ADMIN` permission using the
		/// `space_delegate_addition` internal function.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an existing admin of the
		///   namespace.
		/// - `namespace_id`: The identifier of the namespace to which the admin delegate is being
		///   added.
		/// - `delegate`: The identifier of the delegate being granted admin permissions.
		/// - `authorization`: The authorization ID used to validate the addition.
		///
		/// # Returns
		/// Returns `Ok(())` if the admin delegate was successfully added, or an
		/// `Err` with an appropriate error if the operation fails.
		///
		/// # Errors
		/// - `UnauthorizedOperation`: If the caller is not an admin of the namespace.
		/// - Propagates errors from `space_delegate_addition` if it fails.
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn add_admin_delegate(
			origin: OriginFor<T>,
			namespace_id: NameSpaceIdOf,
			delegate: NameSpaceCreatorOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let auth_space_id = Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_space_id == namespace_id, Error::<T>::UnauthorizedOperation);

			let permissions = Permissions::ADMIN;
			Self::space_delegate_addition(auth_space_id, delegate, creator, permissions)?;

			Ok(())
		}

		/// Adds an audit delegate to a namespace.
		///
		/// The `AUDIT` permission grants the delegate the ability to perform
		/// oversight and compliance checks within the namespace. This function is
		/// used to assign a delegate these audit privileges. It ensures that
		/// the caller has the necessary authorization (admin rights) to add an
		/// audit delegate to the namespace. If the caller is authorized, the
		/// delegate is added with the `AUDIT` permission using the
		/// `space_delegate_addition` internal function.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an existing admin of the
		///   namespace.
		/// - `namespace_id`: The identifier of the namespace to which the audit delegate is being
		///   added.
		/// - `delegate`: The identifier of the delegate being granted audit permissions.
		/// - `authorization`: The authorization ID used to validate the addition.
		///
		/// # Returns
		/// Returns `Ok(())` if the audit delegate was successfully added, or an
		/// `Err` with an appropriate error if the operation fails.
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn add_delegator(
			origin: OriginFor<T>,
			namespace_id: NameSpaceIdOf,
			delegate: NameSpaceCreatorOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let auth_space_id = Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_space_id == namespace_id, Error::<T>::UnauthorizedOperation);

			let permissions = Permissions::DELEGATE;
			Self::space_delegate_addition(auth_space_id, delegate, creator, permissions)?;

			Ok(())
		}

		/// Removes a delegate from a specified namespace.
		///
		/// This function will remove an existing delegate from a namespace, given
		/// the namespace ID and the delegate's authorization ID. It checks that the
		/// namespace exists, is not archived and that the provided
		/// authorization corresponds to a delegate of the namespace. It also
		/// verifies that the caller has the authority to remove a delegate.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin.
		/// - `namespace_id`: The identifier of the namespace from which the delegate is being
		///   removed.
		/// - `remove_authorization`: The authorization ID of the delegate to be removed.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   removal.
		///
		/// # Returns
		/// - `DispatchResult`: This function returns `Ok(())` if the delegate is successfully
		///   removed, or an error (`DispatchError`) if any of the checks fail.
		///
		/// # Errors
		/// - `AuthorizationNotFound`: If the provided `remove_authorization` does not exist.
		/// - `UnauthorizedOperation`: If the origin is not authorized to remove a delegate from the
		///   namespace.
		/// - `NameSpaceNotFound`: If the specified namespace ID does not correspond to an existing
		///   namespace.
		/// - `ArchivedNameSpace`: If the namespace is archived and no longer active.
		/// - `DelegateNotFound`: If the delegate specified by `remove_authorization` is not found
		///   in the namespace.
		///
		/// # Events
		///
		/// - `Deauthorization`: Emitted when a delegate is successfully removed from a namespace.
		///   The event includes the namespace ID and the authorization ID of the removed delegate.
		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			namespace_id: NameSpaceIdOf,
			remove_authorization: AuthorizationIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let auth_space_id =
				Self::ensure_authorization_admin_remove_origin(&authorization, &creator)?;

			ensure!(auth_space_id == namespace_id, Error::<T>::UnauthorizedOperation);

			// Ensure the authorization exists and retrieve its details.
			let authorization_details = Authorizations::<T>::get(&remove_authorization)
				.ok_or(Error::<T>::AuthorizationNotFound)?;

			let mut delegates = Delegates::<T>::get(&namespace_id);
			if let Some(index) = delegates.iter().position(|d| d == &authorization_details.delegate)
			{
				delegates.remove(index);
				Delegates::<T>::insert(&namespace_id, delegates);

				Authorizations::<T>::remove(&remove_authorization);

				Self::update_activity(
					&namespace_id,
					IdentifierTypeOf::NameSpaceAuthorization,
					CallTypeOf::Deauthorization,
				)?;

				Self::deposit_event(Event::Deauthorization {
					namespace: namespace_id,
					authorization: remove_authorization,
				});

				Ok(())
			} else {
				Err(Error::<T>::DelegateNotFound.into())
			}
		}

		/// Creates a new namespace with a unique identifier based on the provided
		/// namespace code and the creator's identity.
		///
		/// This function generates a unique identifier for the namespace by hashing
		/// the encoded namespace code and creator's identifier. It ensures that the
		/// generated namespace identifier is not already in use. An authorization
		/// ID is also created for the new namespace, which is used to manage
		/// delegations. The creator is automatically added as a delegate with
		/// all permissions.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator.
		/// - `space_code`: A unique code representing the namespace to be created.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the namespace is successfully created, or an
		///   error (`DispatchError`) if:
		///   - The generated namespace identifier is already in use.
		///   - The generated authorization ID is of invalid length.
		///   - The namespace delegates limit is exceeded.
		///
		/// # Errors
		/// - `InvalidIdentifierLength`: If the generated identifiers for the namespace or
		///   authorization are of invalid length.
		/// - `NameSpaceAlreadyAnchored`: If the namespace identifier is already in use.
		/// - `NameSpaceDelegatesLimitExceeded`: If the namespace exceeds the limit of allowed
		///   delegates.
		///
		/// # Events
		/// - `Create`: Emitted when a new namespace is successfully created. It includes the
		///   namespace identifier, the creator's identifier, and the authorization ID.
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			digest: NameSpaceHashOf<T>,
			_blob: Option<NameSpaceBlobOf<T>>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			// Id Digest = concat (H(<scale_encoded_registry_input>,
			// <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier = Ss58Identifier::create_identifier(
				&id_digest.encode()[..],
				IdentifierType::NameSpace,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<NameSpaces<T>>::contains_key(&identifier),
				Error::<T>::NameSpaceAlreadyAnchored
			);

			// Construct the authorization_id from the provided parameters.
			// Id Digest = concat (H(<scale_encoded_space_identifier>,
			// <scale_encoded_creator_identifier> ))
			let auth_id_digest = T::Hashing::hash(
				&[&identifier.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()
					[..],
			);

			let authorization_id = Ss58Identifier::create_identifier(
				&auth_id_digest.encode(),
				IdentifierType::NameSpaceAuthorization,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			let mut delegates: BoundedVec<NameSpaceCreatorOf<T>, T::MaxNameSpaceDelegates> =
				BoundedVec::default();
			delegates
				.try_push(creator.clone())
				.map_err(|_| Error::<T>::NameSpaceDelegatesLimitExceeded)?;

			Delegates::<T>::insert(&identifier, delegates);

			Authorizations::<T>::insert(
				&authorization_id,
				NameSpaceAuthorizationOf::<T> {
					namespace_id: identifier.clone(),
					delegate: creator.clone(),
					permissions: Permissions::all(),
					delegator: creator.clone(),
				},
			);

			<NameSpaces<T>>::insert(
				&identifier,
				NameSpaceDetailsOf::<T> {
					digest,
					creator: creator.clone(),
					archive: false,
					registry_ids: Some(BoundedVec::default()),
				},
			);

			Self::update_activity(&identifier, IdentifierTypeOf::NameSpace, CallTypeOf::Genesis)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Create {
				namespace: identifier,
				creator,
				authorization: authorization_id,
			});

			Ok(())
		}

		/// Archives a namespace, rendering it inactive.
		///
		/// This function marks a namespace as archived based on the provided namespace
		/// ID. It checks that the namespace exists, is not already archived.
		/// Additionally, it verifies that the caller has the
		/// authority to archive the namespace, as indicated by the provided
		/// authorization ID.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin with the appropriate authority.
		/// - `namespace_id`: The identifier of the namespace to be archived.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   archival.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the namespace is successfully archived, or an
		///   error (`DispatchError`) if:
		///   - The namespace does not exist.
		/// - `ArchivedNameSpace`: If the namespace is already archived.
		/// - `UnauthorizedOperation`: If the caller does not have the authority to archive the
		///   namespace.
		///
		/// # Errors
		/// - `NameSpaceNotFound`: If the specified namespace ID does not correspond to an existing
		///   namespace.
		/// - `ArchivedNameSpace`: If the namespace is already archived.
		/// - `UnauthorizedOperation`: If the caller is not authorized to archive the namespace.
		///
		/// # Events
		/// - `Archive`: Emitted when a namespace is successfully archived. It includes the
		///   namespace ID and the authority who performed the archival.
		#[pallet::call_index(6)]
		#[pallet::weight({0})]
		pub fn archive(
			origin: OriginFor<T>,
			namespace_id: NameSpaceIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let auth_space_id = Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_space_id == namespace_id, Error::<T>::UnauthorizedOperation);

			let namespace_details =
				NameSpaces::<T>::get(&namespace_id).ok_or(Error::<T>::NameSpaceNotFound)?;
			ensure!(!namespace_details.archive, Error::<T>::ArchivedNameSpace);

			<NameSpaces<T>>::insert(
				&namespace_id,
				NameSpaceDetailsOf::<T> { archive: true, ..namespace_details },
			);

			Self::update_activity(&namespace_id, IdentifierTypeOf::NameSpace, CallTypeOf::Archive)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Archive { namespace: namespace_id, authority: creator });

			Ok(())
		}

		/// Restores an archived namespace, making it active again.
		///
		/// This function unarchives a namespace based on the provided namespace ID. It
		/// checks that the namespace exists, is currently archived.
		/// It also verifies that the caller has the authority to
		/// restore the namespace, as indicated by the provided authorization ID.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin with the appropriate authority.
		/// - `namespace_id`: The identifier of the namespace to be restored.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   restoration.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the namespace is successfully restored, or an
		///   error (`DispatchError`) if:
		///   - The namespace does not exist.
		///   - The namespace is not archived.
		///   - The caller does not have the authority to restore the namespace.
		///
		/// # Errors
		/// - `NameSpaceNotFound`: If the specified namespace ID does not correspond to an existing
		///   namespace.
		/// - `NameSpaceNotArchived`: If the namespace is not currently archived.
		/// - `UnauthorizedOperation`: If the caller is not authorized to restore the namespace.
		///
		/// # Events
		/// - `Restore`: Emitted when a namespace is successfully restored. It includes the
		///   namespace ID and the authority who performed the restoration.
		#[pallet::call_index(7)]
		#[pallet::weight({0})]
		pub fn restore(
			origin: OriginFor<T>,
			namespace_id: NameSpaceIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let auth_space_id =
				Self::ensure_authorization_restore_origin(&authorization, &creator)?;

			ensure!(auth_space_id == namespace_id, Error::<T>::UnauthorizedOperation);

			let namespace_details =
				NameSpaces::<T>::get(&namespace_id).ok_or(Error::<T>::NameSpaceNotFound)?;
			ensure!(namespace_details.archive, Error::<T>::NameSpaceNotArchived);

			<NameSpaces<T>>::insert(
				&namespace_id,
				NameSpaceDetailsOf::<T> { archive: false, ..namespace_details },
			);

			Self::update_activity(&namespace_id, IdentifierTypeOf::NameSpace, CallTypeOf::Restore)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Restore { namespace: namespace_id, authority: creator });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Adds a delegate to a namespace with specified permissions.
	///
	/// This function will add a new delegate to a namespace, given the namespace's ID,
	/// the delegate's information, and the required permissions. It constructs
	/// an authorization ID based on the namespace ID, delegate, and creator,
	/// ensuring that the delegate is not already added. It also checks that the
	/// namespace is not archived and has not exceeded its capacity.
	fn space_delegate_addition(
		namespace_id: NameSpaceIdOf,
		delegate: NameSpaceCreatorOf<T>,
		creator: NameSpaceCreatorOf<T>,
		permissions: Permissions,
	) -> Result<(), Error<T>> {
		// Id Digest = concat (H(<scale_encoded_space_identifier>,
		// <scale_encoded_creator_identifier>, <scale_encoded_delegate_identifier>))
		let id_digest = T::Hashing::hash(
			&[&namespace_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
		);

		let delegate_authorization_id = Ss58Identifier::create_identifier(
			&id_digest.encode(),
			IdentifierType::NameSpaceAuthorization,
		)
		.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

		ensure!(
			!Authorizations::<T>::contains_key(&delegate_authorization_id),
			Error::<T>::DelegateAlreadyAdded
		);

		let mut delegates = Delegates::<T>::get(&namespace_id);
		delegates
			.try_push(delegate.clone())
			.map_err(|_| Error::<T>::NameSpaceDelegatesLimitExceeded)?;
		Delegates::<T>::insert(&namespace_id, delegates);

		Authorizations::<T>::insert(
			&delegate_authorization_id,
			NameSpaceAuthorizationOf::<T> {
				namespace_id: namespace_id.clone(),
				delegate: delegate.clone(),
				permissions,
				delegator: creator,
			},
		);

		Self::update_activity(
			&namespace_id,
			IdentifierTypeOf::NameSpaceAuthorization,
			CallTypeOf::Authorization,
		)
		.map_err(Error::<T>::from)?;

		Self::deposit_event(Event::Authorization {
			namespace: namespace_id,
			authorization: delegate_authorization_id,
			delegate,
		});

		Ok(())
	}

	/// Checks if a given entity is a delegate for the specified namespace.
	///
	/// This function retrieves the list of delegates for a namespace and determines
	/// whether the specified delegate is among them. It is a read-only
	/// operation and does not modify the state.
	pub fn is_a_delegate(tx_id: &NameSpaceIdOf, delegate: NameSpaceCreatorOf<T>) -> bool {
		<Delegates<T>>::get(tx_id).iter().any(|d| d == &delegate)
	}

	/// Verifies if a given delegate has a specific authorization.
	///
	/// This function checks if the provided delegate is associated with the
	/// given authorization ID and has the 'ASSERT' permission.
	pub fn ensure_authorization_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &NameSpaceCreatorOf<T>,
	) -> Result<NameSpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		// TODO: Update all similar function names.
		Self::validate_space_for_transaction(&d.namespace_id)?;

		ensure!(d.permissions.contains(Permissions::ASSERT), Error::<T>::UnauthorizedOperation);

		Ok(d.namespace_id)
	}

	pub fn ensure_authorization_restore_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &NameSpaceCreatorOf<T>,
	) -> Result<NameSpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_space_for_restore_transaction(&d.namespace_id)?;

		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);

		Ok(d.namespace_id)
	}

	/// Checks if a given delegate is an admin for the namespace associated with the
	/// authorization ID.
	///
	/// This function verifies whether the specified delegate is the admin of
	/// the namespace by checking the 'ADMIN' permission within the authorization
	/// tied to the provided authorization ID.
	pub fn ensure_authorization_admin_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &NameSpaceCreatorOf<T>,
	) -> Result<NameSpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_space_for_transaction(&d.namespace_id)?;

		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);

		Ok(d.namespace_id)
	}

	/// Ensures that the given delegate is authorized to perform an audit
	/// operation on a namespace.
	///
	/// This function checks whether the provided `authorization_id` corresponds
	/// to an existing authorization and whether the delegate associated with
	/// that authorization is allowed to perform audit operations. It also
	/// increments usage and validates the namespace for transactions.
	pub fn ensure_authorization_delegator_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &NameSpaceCreatorOf<T>,
	) -> Result<NameSpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_space_for_transaction(&d.namespace_id)?;

		ensure!(
			d.permissions.contains(Permissions::DELEGATE | Permissions::ADMIN),
			Error::<T>::UnauthorizedOperation
		);

		Ok(d.namespace_id)
	}

	/// Checks if a given delegate is an admin for the namespace associated with the
	/// authorization ID.
	///
	/// This function verifies whether the specified delegate is the admin of
	/// the namespace by checking the 'ADMIN' permission within the authorization
	/// tied to the provided authorization ID.
	pub fn ensure_authorization_admin_remove_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &NameSpaceCreatorOf<T>,
	) -> Result<NameSpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_space_for_transaction(&d.namespace_id)?;

		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);

		Ok(d.namespace_id)
	}

	/// Validates that a namespace is eligible for a new transaction.
	///
	/// This function ensures that a namespace is not archived and has
	/// not exceeded its capacity limit before allowing a new transaction to be
	/// recorded. It is a critical check that enforces the integrity and
	/// constraints of namespace usage on the chain.
	pub fn validate_space_for_transaction(namespace_id: &NameSpaceIdOf) -> Result<(), Error<T>> {
		let namespace_details =
			NameSpaces::<T>::get(namespace_id).ok_or(Error::<T>::NameSpaceNotFound)?;

		// Ensure the namespace is not archived.
		ensure!(!namespace_details.archive, Error::<T>::ArchivedNameSpace);

		Ok(())
	}

	/// Validates a namespace for restore transactions.
	///
	/// This function checks that the specified namespace is archived alread.
	/// It is designed to be called before performing any administrative actions
	/// on a namespace to ensure that the namespace is in a proper state for such transactions.
	pub fn validate_space_for_restore_transaction(
		namespace_id: &NameSpaceIdOf,
	) -> Result<(), Error<T>> {
		let namespace_details =
			NameSpaces::<T>::get(namespace_id).ok_or(Error::<T>::NameSpaceNotFound)?;

		// Ensure the namespace is archived.
		ensure!(namespace_details.archive, Error::<T>::NameSpaceNotArchived);

		Ok(())
	}

	/// Validates that a namespace can accommodate a batch of new entries without
	/// exceeding its capacity.
	///
	/// This function ensures that a namespace is not archived and has
	/// enough remaining capacity to accommodate a specified number of new
	/// entries. It is a critical check that enforces the integrity and
	/// constraints of namespace usage on the chain, especially when dealing
	/// with batch operations.
	pub fn validate_space_for_transaction_entries(
		namespace_id: &NameSpaceIdOf,
		_entries: u16,
	) -> Result<(), Error<T>> {
		let namespace_details =
			NameSpaces::<T>::get(namespace_id).ok_or(Error::<T>::NameSpaceNotFound)?;

		// Ensure the namespace is not archived.
		ensure!(!namespace_details.archive, Error::<T>::ArchivedNameSpace);

		Ok(())
	}

	/// Adds a registry ID to the list of registry IDs associated with a namespace.
	///
	/// This function updates the namespace's `registry_ids` list by appending the specified
	/// `registry_id` if it is not already present. If the list is uninitialized (i.e., `None`),
	/// it initializes the list and adds the `registry_id`. The function ensures that the
	/// `BoundedVec` does not exceed its capacity, returning an error if the limit is reached.
	///
	/// # Parameters
	/// - `namespace_id`: A reference to the ID of the namespace where the registry ID should be
	///   added.
	/// - `registry_id`: A reference to the registry ID to be added to the namespace's list of
	///   registry IDs.
	///
	/// # Returns
	/// - `Ok(())`: If the `registry_id` was successfully added or was already present in the list.
	/// - `Err(Error<T>::NameSpaceNotFound)`: If the specified namespace does not exist.
	/// - `Err(Error<T>::NameSpaceRegistryListLimitExceeded)`: If the `registry_ids` list exceeds
	///   its maximum capacity while attempting to add the `registry_id`.
	///
	/// # Errors
	/// - Returns `NameSpaceNotFound` if the `namespace_id` does not exist in the storage.
	/// - Returns `NameSpaceRegistryListLimitExceeded` if the `registry_ids` list cannot accommodate
	///   any more entries.
	pub fn add_registry_id_to_namespace_details(
		namespace_id: &NameSpaceIdOf,
		registry_id: &RegistryIdOf,
	) -> Result<(), Error<T>> {
		NameSpaces::<T>::try_mutate(namespace_id, |space_opt| {
			if let Some(space_details) = space_opt {
				if let Some(ref mut registry_ids) = space_details.registry_ids {
					if !registry_ids.contains(registry_id) {
						registry_ids
							.try_push(registry_id.clone())
							.map_err(|_| Error::<T>::NameSpaceRegistryListLimitExceeded)?;
					}
				} else {
					// Below is required to avoid runtime panic when intialized with None.
					let mut new_registry_ids = BoundedVec::default();
					new_registry_ids
						.try_push(registry_id.clone())
						.map_err(|_| Error::<T>::NameSpaceRegistryListLimitExceeded)?;
					space_details.registry_ids = Some(new_registry_ids);
				}

				Ok(())
			} else {
				Err(Error::<T>::NameSpaceNotFound)
			}
		})
	}

	/// Updates the global timeline with a new activity event for a namespace.
	///
	/// This function is an internal mechanism that logs each significant change
	/// to a namespace on the global timeline. It is automatically called by the
	/// system whenever an update to a namespace occurs, capturing the type of
	/// activity and the precise time at which it happened. This automated
	/// tracking is crucial for maintaining a consistent and auditable record of
	/// all namespace-related activities.
	pub fn update_activity(
		tx_id: &NameSpaceIdOf,
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
}
