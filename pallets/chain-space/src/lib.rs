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

//! # ChainSpace Pallet
//!
//! The ChainSpace pallet provides a framework for creating and managing
//! isolated spaces within the CORD blockchain that can be governed and
//! moderated with a fine-grained permission system. It allows for the creation,
//! approval, and archival of spaces, as well as the management of delegates
//! within these spaces.
//!
//! ## Overview
//!
//! The ChainSpace pallet allows for the creation of distinct spaces on the CORD
//! blockchain, each with its own set of rules and governance. These spaces can
//! be used to manage different ecosystems or communities within the larger
//! blockchain environment. Spaces are created with a unique identifier and can
//! be managed by appointed delegates.
//!
//! ## Interface
//!
//! The pallet provides dispatchable functions for space management:
//!
//! - `create`: Initializes a new space with a unique identifier.
//! - `approve`: Approves a space for use, setting its capacity and governance status.
//! - `archive`: Marks a space as archived, effectively freezing its state.
//! - `restore`: Unarchives a space, returning it to active status.
//! - `add_delegate`: Adds a delegate to a space, granting them specific permissions.
//! - `add_admin_delegate`: Adds an admin delegate to a space, granting them administrative
//!   permissions.
//! - `add_audit_delegate`: Adds an audit delegate to a space, granting them audit permissions.
//! - `remove_delegate`: Removes a delegate from a space, revoking their permissions.
//!
//! ## Permissions
//!
//! The pallet uses a permissions system to manage the actions that delegates
//! can perform within a space. Permissions are granular and can be assigned to
//! different roles, such as an admin or a regular delegate.
//!
//! ## Data Privacy
//!
//! The ChainSpace pallet is designed with data privacy as a core consideration.
//! It does not directly store any personal or sensitive information on-chain.
//! Instead, it manages references to off-chain data, ensuring that the
//! blockchain layer remains compliant with data privacy regulations. Users and
//! developers are responsible for ensuring that the off-chain data handling
//! processes adhere to the applicable laws and standards.
//!
//! ## Usage
//!
//! The ChainSpace pallet can be used by other pallets to create
//! compartmentalized and governed sections of the blockchain. This is
//! particularly useful for applications that require distinct governance models
//! or privacy settings within a shared ecosystem.
//!
//! ## Governance Integration
//!
//! The ChainSpace pallet is integrated with on-chain governance pallets to
//! allow space administrators and delegates to propose changes, vote on
//! initiatives, or manage the space in accordance with the collective decisions
//! of its members.
//!
//! ## Examples
//!
//! - Creating a new space for a community-driven project.
//! - Approving a space for official use after meeting certain criteria.
//! - Archiving a space that is no longer active or has violated terms of use.
//! - Adding delegates to a space to ensure ongoing compliance with governance standards.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod weights;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod tests;

use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
pub mod types;
pub use crate::{pallet::*, types::*, weights::WeightInfo};
use codec::Encode;
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_runtime::traits::{Hash, UniqueSaturatedInto};

/// Type of a CORD account.
pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
/// Type of a space creator.
pub type SpaceCreatorOf<T> = <T as Config>::SpaceCreatorId;
/// Space Identifier
pub type SpaceIdOf = Ss58Identifier;
/// Authorization Identifier
pub type AuthorizationIdOf = Ss58Identifier;
/// Chain space input code.
pub type SpaceCodeOf<T> = <T as frame_system::Config>::Hash;
/// Type of on-chain registry entry.
pub type SpaceDetailsOf<T> = SpaceDetails<SpaceCodeOf<T>, SpaceCreatorOf<T>, StatusOf, SpaceIdOf>;

pub type SpaceAuthorizationOf<T> = SpaceAuthorization<SpaceIdOf, SpaceCreatorOf<T>, Permissions>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::StatusOf;
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config + identifier::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, SpaceCreatorOf<Self>>;
		type SpaceCreatorId: Parameter + MaxEncodedLen;
		type ChainSpaceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		#[pallet::constant]
		type MaxSpaceDelegates: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Space information stored on chain.
	/// It maps from an identifier to its details.
	#[pallet::storage]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, SpaceIdOf, SpaceDetailsOf<T>, OptionQuery>;

	/// Space authorizations stored on-chain.
	/// It maps from an identifier to delegates.
	#[pallet::storage]
	pub type Authorizations<T> =
		StorageMap<_, Blake2_128Concat, AuthorizationIdOf, SpaceAuthorizationOf<T>, OptionQuery>;

	/// Space delegates stored on chain.
	/// It maps from an identifier to a  bounded vec of delegates and
	/// permissions.
	#[pallet::storage]
	pub(super) type Delegates<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		SpaceIdOf,
		BoundedVec<SpaceCreatorOf<T>, T::MaxSpaceDelegates>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new space authorization has been added.
		/// \[space identifier, authorization,  delegate\]
		Authorization {
			space: SpaceIdOf,
			authorization: AuthorizationIdOf,
			delegate: SpaceCreatorOf<T>,
		},
		/// A space authorization has been removed.
		/// \[space identifier, authorization, ]
		Deauthorization { space: SpaceIdOf, authorization: AuthorizationIdOf },
		/// A new chain space has been created.
		/// \[space identifier, creator, authorization\]
		Create { space: SpaceIdOf, creator: SpaceCreatorOf<T>, authorization: AuthorizationIdOf },
		/// A new chain space has been approved.
		/// \[space identifier \]
		Approve { space: SpaceIdOf },
		/// A space has been archived.
		/// \[space identifier,  authority\]
		Archive { space: SpaceIdOf, authority: SpaceCreatorOf<T> },
		/// A space has been restored.
		/// \[space identifier,  authority\]
		Restore { space: SpaceIdOf, authority: SpaceCreatorOf<T> },
		/// A space has been restored.
		/// \[space identifier, \]
		Revoke { space: SpaceIdOf },
		/// A space approval has been revoked.
		/// \[space identifier, \]
		ApprovalRevoke { space: SpaceIdOf },
		/// A space approval has been restored.
		/// \[space identifier, \]
		ApprovalRestore { space: SpaceIdOf },
		/// A chain space capacity has been updated.
		/// \[space identifier \]
		UpdateCapacity { space: SpaceIdOf },
		/// A chain space usage has been reset.
		/// \[space identifier \]
		ResetUsage { space: SpaceIdOf },
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// Space identifier is not unique
		SpaceAlreadyAnchored,
		/// Space identifier not found
		SpaceNotFound,
		/// Only when the author is not the controller or delegate.
		UnauthorizedOperation,
		/// Invalid Identifier
		InvalidIdentifier,
		/// Invalid Identifier Length
		InvalidIdentifierLength,
		/// Invalid Identifier Prefix
		InvalidIdentifierPrefix,
		/// Archived Space
		ArchivedSpace,
		/// Space not Archived
		SpaceNotArchived,
		/// Space delegation limit exceeded
		SpaceDelegatesLimitExceeded,
		/// Empty transaction.
		EmptyTransaction,
		/// Authority already added
		DelegateAlreadyAdded,
		/// Authorization Id not found
		AuthorizationNotFound,
		/// Delegate not found.
		DelegateNotFound,
		/// Space already approved
		SpaceAlreadyApproved,
		/// Space not approved.
		SpaceNotApproved,
		/// The capacity limit for the space has been exceeded.
		CapacityLimitExceeded,
		/// The new capacity value is lower than the current usage
		CapacityLessThanUsage,
		/// Type capacity overflow
		TypeCapacityOverflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds a delegate with the ability to assert new entries to a space.
		///
		/// The `ASSERT` permission allows the delegate to sign and add new
		/// entries within the space. This function is called to grant a
		/// delegate this specific permission. It checks that the caller has the
		/// necessary authorization (admin rights) to add a delegate to the
		/// space. If the caller is authorized, the delegate is added with the
		/// `ASSERT` permission using the `space_delegate_addition`
		/// internal function.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an admin of the space.
		/// - `space_id`: The identifier of the space to which the delegate is being added.
		/// - `delegate`: The identifier of the delegate being added to the space.
		/// - `authorization`: The authorization ID used to validate the addition.
		///
		/// # Returns
		/// Returns `Ok(())` if the delegate was successfully added with
		/// `ASSERT` permission, or an `Err` with an appropriate error if the
		/// operation fails.
		///
		/// # Errors
		/// - `UnauthorizedOperation`: If the caller is not an admin of the space.
		/// - Propagates errors from `space_delegate_addition` if it fails.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_delegate())]
		pub fn add_delegate(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			delegate: SpaceCreatorOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = T::EnsureOrigin::ensure_origin(origin)?.subject();
			let auth_space_id =
				Self::ensure_authorization_delegator_origin(&authorization, &creator)?;
			ensure!(auth_space_id == space_id, Error::<T>::UnauthorizedOperation);

			let permissions = Permissions::ASSERT;
			Self::space_delegate_addition(auth_space_id, delegate, creator, permissions)?;

			Ok(())
		}

		/// Adds an administrative delegate to a space.
		///
		/// The `ADMIN` permission grants the delegate extensive control over
		/// the space, including the ability to manage other delegates and
		/// change space configurations. This function is called to
		/// grant a delegate these administrative privileges. It verifies that
		/// the caller has the necessary authorization (admin rights) to add an
		/// admin delegate to the space. If the caller is authorized,
		/// the delegate is added with the `ADMIN` permission using the
		/// `space_delegate_addition` internal function.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an existing admin of the
		///   space.
		/// - `space_id`: The identifier of the space to which the admin delegate is being added.
		/// - `delegate`: The identifier of the delegate being granted admin permissions.
		/// - `authorization`: The authorization ID used to validate the addition.
		///
		/// # Returns
		/// Returns `Ok(())` if the admin delegate was successfully added, or an
		/// `Err` with an appropriate error if the operation fails.
		///
		/// # Errors
		/// - `UnauthorizedOperation`: If the caller is not an admin of the space.
		/// - Propagates errors from `space_delegate_addition` if it fails.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_admin_delegate())]
		pub fn add_admin_delegate(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			delegate: SpaceCreatorOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = T::EnsureOrigin::ensure_origin(origin)?.subject();
			let auth_space_id = Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_space_id == space_id, Error::<T>::UnauthorizedOperation);

			let permissions = Permissions::ADMIN;
			Self::space_delegate_addition(auth_space_id, delegate, creator, permissions)?;

			Ok(())
		}

		/// Adds an audit delegate to a space.
		///
		/// The `AUDIT` permission grants the delegate the ability to perform
		/// oversight and compliance checks within the space. This function is
		/// used to assign a delegate these audit privileges. It ensures that
		/// the caller has the necessary authorization (admin rights) to add an
		/// audit delegate to the space. If the caller is authorized, the
		/// delegate is added with the `AUDIT` permission using the
		/// `space_delegate_addition` internal function.
		///
		/// # Parameters
		/// - `origin`: The origin of the call, which must be signed by an existing admin of the
		///   space.
		/// - `space_id`: The identifier of the space to which the audit delegate is being added.
		/// - `delegate`: The identifier of the delegate being granted audit permissions.
		/// - `authorization`: The authorization ID used to validate the addition.
		///
		/// # Returns
		/// Returns `Ok(())` if the audit delegate was successfully added, or an
		/// `Err` with an appropriate error if the operation fails.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_delegator())]
		pub fn add_delegator(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			delegate: SpaceCreatorOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = T::EnsureOrigin::ensure_origin(origin)?.subject();
			let auth_space_id = Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_space_id == space_id, Error::<T>::UnauthorizedOperation);

			let permissions = Permissions::DELEGATE;
			Self::space_delegate_addition(auth_space_id, delegate, creator, permissions)?;

			Ok(())
		}

		/// Removes a delegate from a specified space.
		///
		/// This function will remove an existing delegate from a space, given
		/// the space ID and the delegate's authorization ID. It checks that the
		/// space exists, is not archived, is approved, and that the provided
		/// authorization corresponds to a delegate of the space. It also
		/// verifies that the caller has the authority to remove a delegate.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin.
		/// - `space_id`: The identifier of the space from which the delegate is being removed.
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
		///   space.
		/// - `SpaceNotFound`: If the specified space ID does not correspond to an existing space.
		/// - `ArchivedSpace`: If the space is archived and no longer active.
		/// - `SpaceNotApproved`: If the space has not been approved for use.
		/// - `DelegateNotFound`: If the delegate specified by `remove_authorization` is not found
		///   in the space.
		///
		/// # Events
		///
		/// - `Deauthorization`: Emitted when a delegate is successfully removed from a space. The
		///   event includes the space ID and the authorization ID of the removed delegate.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove_delegate())]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			remove_authorization: AuthorizationIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let auth_space_id =
				Self::ensure_authorization_admin_remove_origin(&authorization, &creator)?;

			ensure!(auth_space_id == space_id, Error::<T>::UnauthorizedOperation);

			// Ensure the authorization exists and retrieve its details.
			let authorization_details = Authorizations::<T>::get(&remove_authorization)
				.ok_or(Error::<T>::AuthorizationNotFound)?;

			let mut delegates = Delegates::<T>::get(&space_id);
			if let Some(index) = delegates.iter().position(|d| d == &authorization_details.delegate)
			{
				delegates.remove(index);
				Delegates::<T>::insert(&space_id, delegates);

				Authorizations::<T>::remove(&remove_authorization);

				Self::decrement_usage(&space_id).map_err(Error::<T>::from)?;

				Self::update_activity(
					&space_id,
					IdentifierTypeOf::Auth,
					CallTypeOf::Deauthorization,
				)?;

				Self::deposit_event(Event::Deauthorization {
					space: space_id,
					authorization: remove_authorization,
				});

				Ok(())
			} else {
				Err(Error::<T>::DelegateNotFound.into())
			}
		}

		/// Creates a new space with a unique identifier based on the provided
		/// space code and the creator's identity.
		///
		/// This function generates a unique identifier for the space by hashing
		/// the encoded space code and creator's identifier. It ensures that the
		/// generated space identifier is not already in use. An authorization
		/// ID is also created for the new space, which is used to manage
		/// delegations. The creator is automatically added as a delegate with
		/// all permissions.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator.
		/// - `space_code`: A unique code representing the space to be created.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the space is successfully created, or an error
		///   (`DispatchError`) if:
		///   - The generated space identifier is already in use.
		///   - The generated authorization ID is of invalid length.
		///   - The space delegates limit is exceeded.
		///
		/// # Errors
		/// - `InvalidIdentifierLength`: If the generated identifiers for the space or authorization
		///   are of invalid length.
		/// - `SpaceAlreadyAnchored`: If the space identifier is already in use.
		/// - `SpaceDelegatesLimitExceeded`: If the space exceeds the limit of allowed delegates.
		///
		/// # Events
		/// - `Create`: Emitted when a new space is successfully created. It includes the space
		///   identifier, the creator's identifier, and the authorization ID.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(origin: OriginFor<T>, space_code: SpaceCodeOf<T>) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			// Id Digest = concat (H(<scale_encoded_registry_input>,
			// <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_code.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier =
				Ss58Identifier::create_identifier(&id_digest.encode()[..], IdentifierType::Space)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Spaces<T>>::contains_key(&identifier), Error::<T>::SpaceAlreadyAnchored);

			// Construct the authorization_id from the provided parameters.
			// Id Digest = concat (H(<scale_encoded_space_identifier>,
			// <scale_encoded_creator_identifier> ))
			let auth_id_digest =
				T::Hashing::hash(&[&identifier.encode()[..], &creator.encode()[..]].concat()[..]);

			let authorization_id = Ss58Identifier::create_identifier(
				&auth_id_digest.encode(),
				IdentifierType::Authorization,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			let mut delegates: BoundedVec<SpaceCreatorOf<T>, T::MaxSpaceDelegates> =
				BoundedVec::default();
			delegates
				.try_push(creator.clone())
				.map_err(|_| Error::<T>::SpaceDelegatesLimitExceeded)?;

			Delegates::<T>::insert(&identifier, delegates);

			Authorizations::<T>::insert(
				&authorization_id,
				SpaceAuthorizationOf::<T> {
					space_id: identifier.clone(),
					delegate: creator.clone(),
					permissions: Permissions::all(),
					delegator: creator.clone(),
				},
			);

			<Spaces<T>>::insert(
				&identifier,
				SpaceDetailsOf::<T> {
					code: space_code,
					creator: creator.clone(),
					txn_capacity: 0,
					txn_reserve: 0,
					txn_count: 0,
					approved: false,
					archive: false,
					parent: identifier.clone(),
				},
			);

			Self::update_activity(&identifier, IdentifierTypeOf::ChainSpace, CallTypeOf::Genesis)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Create {
				space: identifier,
				creator,
				authorization: authorization_id,
			});

			Ok(())
		}

		/// Approves a space and sets its capacity.
		///
		/// This function can only be called by a council or root origin,
		/// reflecting its privileged nature. It is used to approve a space that
		/// has been previously created, setting its transaction capacity and
		/// marking it as approved. It ensures that the space exists, is not
		/// archived, and has not already been approved.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be a council or root origin.
		/// - `space_id`: The identifier of the space to be approved.
		/// - `txn_capacity`: The transaction capacity to be set for the space.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the space is successfully approved, or an error
		///   (`DispatchError`) if:
		///   - The origin is not a council or root origin.
		///   - The space does not exist.
		///   - The space is archived.
		///   - The space is already approved.
		///
		/// # Errors
		/// - `BadOrigin`: If the call does not come from a council or root origin.
		/// - `SpaceNotFound`: If the specified space ID does not correspond to an existing space.
		/// - `ArchivedSpace`: If the space is archived and no longer active.
		/// - `SpaceAlreadyApproved`: If the space has already been approved.
		///
		/// # Events
		/// - `Approve`: Emitted when a space is successfully approved. It includes the space
		///   identifier.
		///
		/// # Security Considerations
		/// Due to the privileged nature of this function, callers must ensure
		/// that they have the appropriate authority. Misuse can lead to
		/// unauthorized approval of spaces, which may have security
		/// implications.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::approve())]
		pub fn approve(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			txn_capacity: u64,
		) -> DispatchResult {
			T::ChainSpaceOrigin::ensure_origin(origin)?;

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(!space_details.approved, Error::<T>::SpaceAlreadyApproved);

			<Spaces<T>>::insert(
				&space_id,
				SpaceDetailsOf::<T> { txn_capacity, approved: true, ..space_details },
			);

			Self::update_activity(&space_id, IdentifierTypeOf::ChainSpace, CallTypeOf::Approved)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Approve { space: space_id });

			Ok(())
		}

		/// Archives a space, rendering it inactive.
		///
		/// This function marks a space as archived based on the provided space
		/// ID. It checks that the space exists, is not already archived, and is
		/// approved. Additionally, it verifies that the caller has the
		/// authority to archive the space, as indicated by the provided
		/// authorization ID.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin with the appropriate authority.
		/// - `space_id`: The identifier of the space to be archived.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   archival.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the space is successfully archived, or an error
		///   (`DispatchError`) if:
		///   - The space does not exist.
		/// - `ArchivedSpace`: If the space is already archived.
		/// - `SpaceNotApproved`: If the space has not been approved for use.
		/// - `UnauthorizedOperation`: If the caller does not have the authority to archive the
		///   space.
		///
		/// # Errors
		/// - `SpaceNotFound`: If the specified space ID does not correspond to an existing space.
		/// - `ArchivedSpace`: If the space is already archived.
		/// - `SpaceNotApproved`: If the space has not been approved for use.
		/// - `UnauthorizedOperation`: If the caller is not authorized to archive the space.
		///
		/// # Events
		/// - `Archive`: Emitted when a space is successfully archived. It includes the space ID and
		///   the authority who performed the archival.
		#[pallet::call_index(6)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::archive())]
		pub fn archive(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let auth_space_id = Self::ensure_authorization_admin_origin(&authorization, &creator)?;

			ensure!(auth_space_id == space_id, Error::<T>::UnauthorizedOperation);

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

			<Spaces<T>>::insert(&space_id, SpaceDetailsOf::<T> { archive: true, ..space_details });

			Self::update_activity(&space_id, IdentifierTypeOf::ChainSpace, CallTypeOf::Archive)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Archive { space: space_id, authority: creator });

			Ok(())
		}

		/// Restores an archived space, making it active again.
		///
		/// This function unarchives a space based on the provided space ID. It
		/// checks that the space exists, is currently archived, and is
		/// approved. It also verifies that the caller has the authority to
		/// restore the space, as indicated by the provided authorization ID.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator or an
		///   admin with the appropriate authority.
		/// - `space_id`: The identifier of the space to be restored.
		/// - `authorization`: An identifier for the authorization being used to validate the
		///   restoration.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the space is successfully restored, or an error
		///   (`DispatchError`) if:
		///   - The space does not exist.
		///   - The space is not archived.
		///   - The space is not approved.
		///   - The caller does not have the authority to restore the space.
		///
		/// # Errors
		/// - `SpaceNotFound`: If the specified space ID does not correspond to an existing space.
		/// - `SpaceNotArchived`: If the space is not currently archived.
		/// - `SpaceNotApproved`: If the space has not been approved for use.
		/// - `UnauthorizedOperation`: If the caller is not authorized to restore the space.
		///
		/// # Events
		/// - `Restore`: Emitted when a space is successfully restored. It includes the space ID and
		///   the authority who performed the restoration.
		#[pallet::call_index(7)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::restore())]
		pub fn restore(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let auth_space_id =
				Self::ensure_authorization_restore_origin(&authorization, &creator)?;

			ensure!(auth_space_id == space_id, Error::<T>::UnauthorizedOperation);

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(space_details.archive, Error::<T>::SpaceNotArchived);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

			<Spaces<T>>::insert(&space_id, SpaceDetailsOf::<T> { archive: false, ..space_details });

			Self::update_activity(&space_id, IdentifierTypeOf::ChainSpace, CallTypeOf::Restore)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Restore { space: space_id, authority: creator });

			Ok(())
		}

		/// Updates the transaction capacity of an existing space.
		///
		/// This extrinsic updates the capacity limit of a space, ensuring that
		/// the new limit is not less than the current usage to prevent
		/// over-allocation. It can only be called by an authorized origin and
		/// not on archived or unapproved spaces.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be from an authorized source.
		/// * `space_id` - The identifier of the space for which the capacity is being updated.
		/// * `new_txn_capacity` - The new capacity limit to be set for the space.
		///
		/// # Errors
		/// * `SpaceNotFound` - If the space with the given ID does not exist.
		/// * `ArchivedSpace` - If the space is archived and thus cannot be modified.
		/// * `SpaceNotApproved` - If the space has not been approved for use yet.
		/// * `CapacityLessThanUsage` - If the new capacity is less than the current usage of the
		///   space.
		///
		/// # Events
		/// * `UpdateCapacity` - Emits the space ID when the capacity is successfully updated.
		#[pallet::call_index(8)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::update_transaction_capacity())]
		pub fn update_transaction_capacity(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			new_txn_capacity: u64,
		) -> DispatchResult {
			T::ChainSpaceOrigin::ensure_origin(origin)?;
			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

			// Ensure the new capacity is greater than the current usage
			ensure!(
				new_txn_capacity >= (space_details.txn_count + space_details.txn_reserve),
				Error::<T>::CapacityLessThanUsage
			);

			if space_id.clone() != space_details.parent.clone() {
				let parent_details = Spaces::<T>::get(&space_details.parent.clone())
					.ok_or(Error::<T>::SpaceNotFound)?;

				// Ensure the new capacity is greater than the current usage
				ensure!(
					(parent_details.txn_capacity >=
						(parent_details.txn_count +
							parent_details.txn_reserve + new_txn_capacity -
							space_details.txn_capacity)),
					Error::<T>::CapacityLessThanUsage
				);

				<Spaces<T>>::insert(
					&space_details.parent.clone(),
					SpaceDetailsOf::<T> {
						txn_reserve: parent_details.txn_reserve - space_details.txn_capacity +
							new_txn_capacity,
						..parent_details.clone()
					},
				);
			}

			<Spaces<T>>::insert(
				&space_id,
				SpaceDetailsOf::<T> { txn_capacity: new_txn_capacity, ..space_details },
			);

			Self::update_activity(&space_id, IdentifierTypeOf::ChainSpace, CallTypeOf::Capacity)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::UpdateCapacity { space: space_id });

			Ok(())
		}

		/// Resets the usage counter of a specified space to zero.
		///
		/// This function can only be called by an authorized origin, defined by
		/// `ChainSpaceOrigin`, and is used to reset the usage metrics for a
		/// given space on the chain, identified by `space_id`. The reset action
		/// is only permissible if the space exists, is not archived, and is
		/// approved for operations.
		///
		/// # Parameters
		/// - `origin`: The transaction's origin, which must pass the `ChainSpaceOrigin` check.
		/// - `space_id`: The identifier of the space for which the usage counter will be reset.
		///
		/// # Errors
		/// - Returns `SpaceNotFound` if the specified `space_id` does not correspond to any
		///   existing space.
		/// - Returns `ArchivedSpace` if the space is archived and thus cannot be modified.
		/// - Returns `SpaceNotApproved` if the space is not approved for operations.
		///
		/// # Events
		/// - Emits `UpdateCapacity` upon successfully resetting the space's usage counter.
		#[pallet::call_index(9)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::reset_transaction_count())]
		pub fn reset_transaction_count(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
		) -> DispatchResult {
			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

			if space_id.clone() != space_details.parent.clone() {
				let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

				let parent_details = Spaces::<T>::get(&space_details.parent.clone())
					.ok_or(Error::<T>::SpaceNotFound)?;
				ensure!(
					parent_details.creator.clone() == creator,
					Error::<T>::UnauthorizedOperation
				);
			} else {
				T::ChainSpaceOrigin::ensure_origin(origin)?;
			}

			<Spaces<T>>::insert(&space_id, SpaceDetailsOf::<T> { txn_count: 0, ..space_details });

			Self::update_activity(&space_id, IdentifierTypeOf::ChainSpace, CallTypeOf::Usage)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::ResetUsage { space: space_id });

			Ok(())
		}

		/// Revokes approval for a specified space.
		///
		/// This function can be executed by an authorized origin, as determined
		/// by `ChainSpaceOrigin`. It is designed to change the status of a
		/// given space, referred to by `space_id`, to unapproved.
		/// The revocation is only allowed if the space is currently approved,
		/// and not archived.
		///
		/// # Parameters
		/// - `origin`: The transaction's origin, which must satisfy the `ChainSpaceOrigin` policy.
		/// - `space_id`: The identifier of the space whose approval status is being revoked.
		///
		/// # Errors
		/// - Returns `SpaceNotFound` if no space corresponds to the provided `space_id`.
		/// - Returns `ArchivedSpace` if the space is archived, in which case its status cannot be
		///   altered.
		/// - Returns `SpaceNotApproved` if the space is already unapproved.
		///
		/// # Events
		/// - Emits `Revoke` when the space's approved status is successfully revoked.
		#[pallet::call_index(10)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::approval_revoke())]
		pub fn approval_revoke(origin: OriginFor<T>, space_id: SpaceIdOf) -> DispatchResult {
			T::ChainSpaceOrigin::ensure_origin(origin)?;

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

			<Spaces<T>>::insert(
				&space_id,
				SpaceDetailsOf::<T> { approved: false, ..space_details },
			);

			Self::update_activity(
				&space_id,
				IdentifierTypeOf::ChainSpace,
				CallTypeOf::CouncilRevoke,
			)
			.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::ApprovalRevoke { space: space_id });

			Ok(())
		}

		#[pallet::call_index(11)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::approval_restore())]
		pub fn approval_restore(origin: OriginFor<T>, space_id: SpaceIdOf) -> DispatchResult {
			T::ChainSpaceOrigin::ensure_origin(origin)?;

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(!space_details.approved, Error::<T>::SpaceAlreadyApproved);

			<Spaces<T>>::insert(&space_id, SpaceDetailsOf::<T> { approved: true, ..space_details });

			Self::update_activity(
				&space_id,
				IdentifierTypeOf::ChainSpace,
				CallTypeOf::CouncilRestore,
			)
			.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::ApprovalRestore { space: space_id });

			Ok(())
		}

		/// Creates a new space with a unique identifier based on the provided
		/// space code and the creator's identity, along with parent space ID.
		///
		/// This function generates a unique identifier for the space by hashing
		/// the encoded space code and creator's identifier. It ensures that the
		/// generated space identifier is not already in use. An authorization
		/// ID is also created for the new space, which is used to manage
		/// delegations. The creator is automatically added as a delegate with
		/// all permissions.
		/// NOTE: this call is different from create() in just 1 main step. This
		/// space can be created from the already 'approved' space, as a
		/// 'space-approval' is a council activity, instead in this case, its
		/// owner/creator's task. Thus reducing the involvement of council once
		/// the top level approval is present.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by the creator.
		/// - `space_code`: A unique code representing the space to be created.
		/// - `count`: Number of approved transaction capacity in the sub-space.
		/// - `space_id`: Identifier of the parent space.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the space is successfully created, or an error
		///   (`DispatchError`) if:
		///   - The generated space identifier is already in use.
		///   - The generated authorization ID is of invalid length.
		///   - The space delegates limit is exceeded.
		///
		/// # Errors
		/// - `InvalidIdentifierLength`: If the generated identifiers for the space or authorization
		///   are of invalid length.
		/// - `SpaceAlreadyAnchored`: If the space identifier is already in use.
		/// - `SpaceDelegatesLimitExceeded`: If the space exceeds the limit of allowed delegates.
		///
		/// # Events
		/// - `Create`: Emitted when a new space is successfully created. It includes the space
		///   identifier, the creator's identifier, and the authorization ID.
		#[pallet::call_index(12)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::subspace_create())]
		pub fn subspace_create(
			origin: OriginFor<T>,
			space_code: SpaceCodeOf<T>,
			count: u64,
			space_id: SpaceIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);
			ensure!(space_details.creator == creator.clone(), Error::<T>::UnauthorizedOperation);

			// Ensure the new capacity is greater than the current usage
			ensure!(
				count <=
					(space_details.txn_capacity -
						(space_details.txn_count + space_details.txn_reserve)),
				Error::<T>::CapacityLimitExceeded
			);

			// Id Digest = concat (H(<scale_encoded_registry_input>,
			// <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_code.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier =
				Ss58Identifier::create_identifier(&id_digest.encode()[..], IdentifierType::Space)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Spaces<T>>::contains_key(&identifier), Error::<T>::SpaceAlreadyAnchored);

			// Construct the authorization_id from the provided parameters.
			// Id Digest = concat (H(<scale_encoded_space_identifier>,
			// <scale_encoded_creator_identifier> ))
			let auth_id_digest =
				T::Hashing::hash(&[&identifier.encode()[..], &creator.encode()[..]].concat()[..]);

			let authorization_id = Ss58Identifier::create_identifier(
				&auth_id_digest.encode(),
				IdentifierType::Authorization,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			let mut delegates: BoundedVec<SpaceCreatorOf<T>, T::MaxSpaceDelegates> =
				BoundedVec::default();
			delegates
				.try_push(creator.clone())
				.map_err(|_| Error::<T>::SpaceDelegatesLimitExceeded)?;

			Delegates::<T>::insert(&identifier, delegates);

			Authorizations::<T>::insert(
				&authorization_id,
				SpaceAuthorizationOf::<T> {
					space_id: identifier.clone(),
					delegate: creator.clone(),
					permissions: Permissions::all(),
					delegator: creator.clone(),
				},
			);

			/* Update the parent space with added count */
			<Spaces<T>>::insert(
				&space_id.clone(),
				SpaceDetailsOf::<T> {
					txn_count: space_details.txn_count + 1,
					txn_reserve: space_details.txn_reserve + count,
					..space_details
				},
			);
			<Spaces<T>>::insert(
				&identifier,
				SpaceDetailsOf::<T> {
					code: space_code,
					creator: creator.clone(),
					txn_capacity: count,
					txn_reserve: 0,
					txn_count: 0,
					approved: true,
					archive: false,
					parent: space_id,
				},
			);

			Self::update_activity(&identifier, IdentifierTypeOf::ChainSpace, CallTypeOf::Genesis)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Create {
				space: identifier,
				creator,
				authorization: authorization_id,
			});

			Ok(())
		}

		/// Updates the transaction capacity of an existing subspace.
		///
		/// This extrinsic updates the capacity limit of a space, ensuring that
		/// the new limit is not less than the current usage to prevent
		/// over-allocation. It can only be called by an authorized origin and
		/// not on archived or unapproved spaces.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which must be from an authorized source.
		/// * `space_id` - The identifier of the space for which the capacity is being updated.
		/// * `new_txn_capacity` - The new capacity limit to be set for the space.
		///
		/// # Errors
		/// * `SpaceNotFound` - If the space with the given ID does not exist.
		/// * `ArchivedSpace` - If the space is archived and thus cannot be modified.
		/// * `SpaceNotApproved` - If the space has not been approved for use yet.
		/// * `CapacityLessThanUsage` - If the new capacity is less than the current usage of the
		///   space.
		///
		/// # Events
		/// * `UpdateCapacity` - Emits the space ID when the capacity is successfully updated.
		#[pallet::call_index(13)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::update_transaction_capacity())]
		pub fn update_transaction_capacity_sub(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			new_txn_capacity: u64,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

			// Ensure the new capacity is greater than the current usage
			ensure!(
				new_txn_capacity >= (space_details.txn_count + space_details.txn_reserve),
				Error::<T>::CapacityLessThanUsage
			);

			// If its a top level Space, then this is unauthorized.
			ensure!(
				space_details.parent.clone() != space_id.clone(),
				Error::<T>::UnauthorizedOperation
			);

			let parent_details =
				Spaces::<T>::get(&space_details.parent.clone()).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(parent_details.creator.clone() == creator, Error::<T>::UnauthorizedOperation);

			// Ensure the new capacity is greater than the current usage
			ensure!(
				(parent_details.txn_capacity >=
					(parent_details.txn_count + parent_details.txn_reserve + new_txn_capacity -
						space_details.txn_capacity)),
				Error::<T>::CapacityLessThanUsage
			);

			<Spaces<T>>::insert(
				&space_details.parent.clone(),
				SpaceDetailsOf::<T> {
					txn_reserve: parent_details.txn_reserve - space_details.txn_capacity +
						new_txn_capacity,
					..parent_details.clone()
				},
			);

			<Spaces<T>>::insert(
				&space_id,
				SpaceDetailsOf::<T> { txn_capacity: new_txn_capacity, ..space_details },
			);

			Self::update_activity(&space_id, IdentifierTypeOf::ChainSpace, CallTypeOf::Capacity)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::UpdateCapacity { space: space_id });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Adds a delegate to a space with specified permissions.
	///
	/// This function will add a new delegate to a space, given the space's ID,
	/// the delegate's information, and the required permissions. It constructs
	/// an authorization ID based on the space ID, delegate, and creator,
	/// ensuring that the delegate is not already added. It also checks that the
	/// space is not archived, is approved, and has not exceeded its capacity.
	fn space_delegate_addition(
		space_id: SpaceIdOf,
		delegate: SpaceCreatorOf<T>,
		creator: SpaceCreatorOf<T>,
		permissions: Permissions,
	) -> Result<(), Error<T>> {
		// Id Digest = concat (H(<scale_encoded_space_identifier>,
		// <scale_encoded_creator_identifier>, <scale_encoded_delegate_identifier>))
		let id_digest = T::Hashing::hash(
			&[&space_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
		);

		let delegate_authorization_id =
			Ss58Identifier::create_identifier(&id_digest.encode(), IdentifierType::Authorization)
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

		ensure!(
			!Authorizations::<T>::contains_key(&delegate_authorization_id),
			Error::<T>::DelegateAlreadyAdded
		);

		let mut delegates = Delegates::<T>::get(&space_id);
		delegates
			.try_push(delegate.clone())
			.map_err(|_| Error::<T>::SpaceDelegatesLimitExceeded)?;
		Delegates::<T>::insert(&space_id, delegates);

		Authorizations::<T>::insert(
			&delegate_authorization_id,
			SpaceAuthorizationOf::<T> {
				space_id: space_id.clone(),
				delegate: delegate.clone(),
				permissions,
				delegator: creator,
			},
		);

		Self::update_activity(&space_id, IdentifierTypeOf::Auth, CallTypeOf::Authorization)
			.map_err(Error::<T>::from)?;

		Self::deposit_event(Event::Authorization {
			space: space_id,
			authorization: delegate_authorization_id,
			delegate,
		});

		Ok(())
	}

	/// Checks if a given entity is a delegate for the specified space.
	///
	/// This function retrieves the list of delegates for a space and determines
	/// whether the specified delegate is among them. It is a read-only
	/// operation and does not modify the state.
	pub fn is_a_delegate(tx_id: &SpaceIdOf, delegate: SpaceCreatorOf<T>) -> bool {
		<Delegates<T>>::get(tx_id).iter().any(|d| d == &delegate)
	}

	/// Verifies if a given delegate has a specific authorization.
	///
	/// This function checks if the provided delegate is associated with the
	/// given authorization ID and has the 'ASSERT' permission.
	pub fn ensure_authorization_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &SpaceCreatorOf<T>,
	) -> Result<SpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::increment_usage(&d.space_id)?;

		Self::validate_space_for_transaction(&d.space_id)?;

		ensure!(d.permissions.contains(Permissions::ASSERT), Error::<T>::UnauthorizedOperation);

		Ok(d.space_id)
	}

	pub fn ensure_authorization_restore_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &SpaceCreatorOf<T>,
	) -> Result<SpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::increment_usage(&d.space_id)?;

		Self::validate_space_for_restore_transaction(&d.space_id)?;

		ensure!(d.permissions.contains(Permissions::ASSERT), Error::<T>::UnauthorizedOperation);

		Ok(d.space_id)
	}

	/// Checks if a given delegate is an admin for the space associated with the
	/// authorization ID.
	///
	/// This function verifies whether the specified delegate is the admin of
	/// the space by checking the 'ADMIN' permission within the authorization
	/// tied to the provided authorization ID.
	pub fn ensure_authorization_admin_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &SpaceCreatorOf<T>,
	) -> Result<SpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::increment_usage(&d.space_id)?;

		Self::validate_space_for_transaction(&d.space_id)?;

		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);

		Ok(d.space_id)
	}

	/// Ensures that the given delegate is authorized to perform an audit
	/// operation on a space.
	///
	/// This function checks whether the provided `authorization_id` corresponds
	/// to an existing authorization and whether the delegate associated with
	/// that authorization is allowed to perform audit operations. It also
	/// increments usage and validates the space for transactions.
	pub fn ensure_authorization_delegator_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &SpaceCreatorOf<T>,
	) -> Result<SpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::increment_usage(&d.space_id)?;

		Self::validate_space_for_transaction(&d.space_id)?;

		ensure!(
			d.permissions.contains(Permissions::DELEGATE | Permissions::ADMIN),
			Error::<T>::UnauthorizedOperation
		);

		Ok(d.space_id)
	}

	/// Checks if a given delegate is an admin for the space associated with the
	/// authorization ID.
	///
	/// This function verifies whether the specified delegate is the admin of
	/// the space by checking the 'ADMIN' permission within the authorization
	/// tied to the provided authorization ID.
	pub fn ensure_authorization_admin_remove_origin(
		authorization_id: &AuthorizationIdOf,
		delegate: &SpaceCreatorOf<T>,
	) -> Result<SpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);

		Self::validate_space_for_transaction(&d.space_id)?;

		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);

		Ok(d.space_id)
	}

	/// Validates that a space is eligible for a new transaction.
	///
	/// This function ensures that a space is not archived, is approved, and has
	/// not exceeded its capacity limit before allowing a new transaction to be
	/// recorded. It is a critical check that enforces the integrity and
	/// constraints of space usage on the chain.
	pub fn validate_space_for_transaction(space_id: &SpaceIdOf) -> Result<(), Error<T>> {
		let space_details = Spaces::<T>::get(space_id).ok_or(Error::<T>::SpaceNotFound)?;

		// Ensure the space is not archived.
		ensure!(!space_details.archive, Error::<T>::ArchivedSpace);

		// Ensure the space is approved for transactions.
		ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

		// Ensure the space has not exceeded its capacity limit.
		if space_details.txn_capacity == 0 || space_details.txn_count < space_details.txn_capacity {
			Ok(())
		} else {
			Err(Error::<T>::CapacityLimitExceeded)
		}
	}

	/// Validates a space for restore transactions.
	///
	/// This function checks that the specified space is approved and has not
	/// exceeded its capacity limit. It is designed to be called before
	/// performing any administrative actions on a space to ensure
	/// that the space is in a proper state for such transactions.
	pub fn validate_space_for_restore_transaction(space_id: &SpaceIdOf) -> Result<(), Error<T>> {
		let space_details = Spaces::<T>::get(space_id).ok_or(Error::<T>::SpaceNotFound)?;

		// Ensure the space is archived.
		ensure!(space_details.archive, Error::<T>::SpaceNotArchived);

		// Ensure the space is approved for transactions.
		ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

		// Ensure the space has not exceeded its capacity limit.
		if space_details.txn_capacity == 0 || space_details.txn_count < space_details.txn_capacity {
			Ok(())
		} else {
			Err(Error::<T>::CapacityLimitExceeded)
		}
	}

	/// Validates that a space can accommodate a batch of new entries without
	/// exceeding its capacity.
	///
	/// This function ensures that a space is not archived, is approved, and has
	/// enough remaining capacity to accommodate a specified number of new
	/// entries. It is a critical check that enforces the integrity and
	/// constraints of space usage on the chain, especially when dealing
	/// with batch operations.
	pub fn validate_space_for_transaction_entries(
		space_id: &SpaceIdOf,
		entries: u16,
	) -> Result<(), Error<T>> {
		let space_details = Spaces::<T>::get(space_id).ok_or(Error::<T>::SpaceNotFound)?;

		// Ensure the space is not archived.
		ensure!(!space_details.archive, Error::<T>::ArchivedSpace);

		// Ensure the space is approved for adding new entries.
		ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

		// Calculate the new usage to check against the capacity.
		let new_usage = space_details
			.txn_count
			.checked_add(entries as u64)
			.ok_or(Error::<T>::TypeCapacityOverflow)?;

		// Ensure the space has enough capacity to accommodate the new entries.
		if space_details.txn_capacity == 0 || new_usage <= space_details.txn_capacity {
			Ok(())
		} else {
			Err(Error::<T>::CapacityLimitExceeded)
		}
	}

	//
	/// Increments the usage count of a space by one unit.
	///
	/// This function is used to increase the usage counter of a space,
	/// typically when a new delegate or entry is added. It ensures that the
	/// usage count does not overflow.
	pub fn increment_usage(tx_id: &SpaceIdOf) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.txn_count = space_details.txn_count.saturating_add(1);
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound)
			}
		})
	}

	/// Decrements the usage count of a space by one unit.
	///
	/// This function is used to decrease the usage counter of a space,
	/// typically when a delegate or entry is removed. It ensures that the usage
	/// count does not underflow.
	pub fn decrement_usage(tx_id: &SpaceIdOf) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.txn_count = space_details.txn_count.saturating_sub(1);
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound)
			}
		})
	}

	/// Increments the usage count of a space by a specified unit.
	///
	/// This function increases the usage counter of a space by the amount
	/// specified in `increment`, which is useful for batch operations.
	/// It ensures that the usage count does not overflow.
	pub fn increment_usage_entries(tx_id: &SpaceIdOf, increment: u16) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.txn_count = space_details.txn_count.saturating_add(increment.into());
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound)
			}
		})
	}

	/// Decrements the usage count of a space by a specified amount.
	///
	/// This function decreases the usage counter of a space by the amount
	/// specified in `decrement`, which is useful for batch removals. It ensures
	/// that the usage count does not underflow.
	pub fn decrement_usage_entries(tx_id: &SpaceIdOf, decrement: u16) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.txn_count = space_details.txn_count.saturating_sub(decrement.into());
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound)
			}
		})
	}

	/// Updates the global timeline with a new activity event for a space.
	///
	/// This function is an internal mechanism that logs each significant change
	/// to a space on the global timeline. It is automatically called by the
	/// system whenever an update to a space occurs, capturing the type of
	/// activity and the precise time at which it happened. This automated
	/// tracking is crucial for maintaining a consistent and auditable record of
	/// all space-related activities.
	pub fn update_activity(
		tx_id: &SpaceIdOf,
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
