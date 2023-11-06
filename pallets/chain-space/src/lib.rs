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
//! - `approve`: Approves a space for use, setting its capacity and governance
//!   status.
//! - `archive`: Marks a space as archived, effectively freezing its state.
//! - `restore`: Unarchives a space, returning it to active status.
//! - `add_delegate`: Adds a delegate to a space, granting them specific
//!   permissions.
//! - `remove_delegate`: Removes a delegate from a space, revoking their
//!   permissions.
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
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_runtime::traits::UniqueSaturatedInto;

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
pub type SpaceDetailsOf<T> = SpaceDetails<SpaceCodeOf<T>, SpaceCreatorOf<T>, StatusOf>;

pub type SpaceAuthorizationOf<T> = SpaceAuthorization<SpaceIdOf, SpaceCreatorOf<T>, Permissions>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{curi::Ss58Identifier, StatusOf};
	use cord_utilities::traits::CallSources;
	use frame_support::{pallet_prelude::*, sp_runtime::traits::Hash};
	use frame_system::pallet_prelude::*;

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
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, SpaceIdOf, SpaceDetailsOf<T>, OptionQuery>;

	/// Space authorizations stored on-chain.
	/// It maps from an identifier to delegates.
	#[pallet::storage]
	#[pallet::getter(fn authorizations)]
	pub type Authorizations<T> =
		StorageMap<_, Blake2_128Concat, AuthorizationIdOf, SpaceAuthorizationOf<T>, OptionQuery>;

	/// Space delegates stored on chain.
	/// It maps from an identifier to a  bounded vec of delegates and
	/// permissions.
	#[pallet::storage]
	#[pallet::getter(fn delegates)]
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds a delegate to a specified space.
		///
		/// This function will add a new delegate to a space, given the space ID
		/// and the delegate's information. It performs several checks to ensure
		/// the operation is valid, such as verifying that the space exists,
		/// is not archived, is approved, and has not exceeded its capacity.
		/// Additionally, it checks if the creator is authorized to add a
		/// delegate to the space and whether the delegate has already been
		/// added.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be signed by
		///   the creator or an admin.
		/// - `space_id`: The identifier of the space to which the delegate is
		///   being added.
		/// - `delegate`: The identifier of the delegate being added to the
		///   space.
		/// - `authorization`: An identifier for the authorization being used to
		///   validate the addition.
		/// - `is_admin`: A boolean flag indicating if the delegate should be
		///   added with admin permissions.
		///
		/// # Returns
		/// - `DispatchResult`: This function returns `Ok(())` if the delegate
		///   is successfully added, or an error (`DispatchError`) if any of the
		///   checks fail.
		///
		/// # Errors
		/// - `UnauthorizedOperation`: If the origin is not authorized to add a
		///   delegate to the space.
		/// - `SpaceNotFound`: If the specified space ID does not correspond to
		///   an existing space.
		/// - `ArchivedSpace`: If the space is archived and no longer active.
		/// - `SpaceNotApproved`: If the space has not been approved for use.
		/// - `CapacityLimitExceeded`: If the space has reached its capacity for
		///   delegates.
		/// - `InvalidIdentifierLength`: If the constructed authorization ID is
		///   not of valid length.
		/// - `DelegateAlreadyAdded`: If the delegate is already added to the
		///   space.
		/// - `SpaceDelegatesLimitExceeded`: If the space cannot accept more
		///   delegates.
		///
		///   # Events
		///
		/// - `Authorization`: This event is emitted after a delegate has been
		///   successfully added to a space. It includes the unique identifiers
		///   for the space (`space_id`), the authorization
		///   (`authorization_id`), and the delegate (`delegate_id`).
		///
		/// Upon successful execution of the `add_delegate` function, the
		/// `Authorization` event is dispatched to signal the successful
		/// addition of a delegate to a space. The event carries
		/// essential identifiers that can be utilized by external systems to
		/// acknowledge the new delegation relationship within the space. This
		/// event is particularly useful for tracking changes in permissions and
		/// for audit purposes.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_delegate())]
		pub fn add_delegate(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			delegate: SpaceCreatorOf<T>,
			authorization: AuthorizationIdOf,
			is_admin: bool,
		) -> DispatchResult {
			let creator = T::EnsureOrigin::ensure_origin(origin)?.subject();

			// Determine the space_id from the authorization
			let admin_space_id = Self::is_a_space_admin(&authorization, creator.clone())
				.map_err(Error::<T>::from)?;
			ensure!(admin_space_id == space_id, Error::<T>::UnauthorizedOperation);

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);
			ensure!(
				space_details.capacity == 0 || space_details.usage < space_details.capacity,
				Error::<T>::CapacityLimitExceeded
			);

			// Construct the authorization_id from the provided parameters.
			// Id Digest = concat (H(<scale_encoded_space_identifier>,
			// <scale_encoded_creator_identifier>, <scale_encoded_delegate_identifier>))
			let id_digest = T::Hashing::hash(
				&[&space_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let delegate_authorization_id =
				Ss58Identifier::to_authorization_id(&id_digest.encode())
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

			// Set permissions.
			let permissions = if is_admin { Permissions::all() } else { Permissions::default() };

			Authorizations::<T>::insert(
				&delegate_authorization_id,
				SpaceAuthorizationOf::<T> {
					space_id: admin_space_id,
					delegate: delegate.clone(),
					permissions,
					delegator: creator,
				},
			);

			Self::increment_usage(&space_id).map_err(Error::<T>::from)?;

			Self::update_activity(&space_id, CallTypeOf::Authorization)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Authorization {
				space: space_id,
				authorization: delegate_authorization_id,
				delegate,
			});

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
		/// - `origin`: The origin of the transaction, which must be signed by
		///   the creator or an admin.
		/// - `space_id`: The identifier of the space from which the delegate is
		///   being removed.
		/// - `remove_authorization`: The authorization ID of the delegate to be
		///   removed.
		/// - `authorization`: An identifier for the authorization being used to
		///   validate the removal.
		///
		/// # Returns
		/// - `DispatchResult`: This function returns `Ok(())` if the delegate
		///   is successfully removed, or an error (`DispatchError`) if any of
		///   the checks fail.
		///
		/// # Errors
		/// - `AuthorizationNotFound`: If the provided `remove_authorization`
		///   does not exist.
		/// - `UnauthorizedOperation`: If the origin is not authorized to remove
		///   a delegate from the space.
		/// - `SpaceNotFound`: If the specified space ID does not correspond to
		///   an existing space.
		/// - `ArchivedSpace`: If the space is archived and no longer active.
		/// - `SpaceNotApproved`: If the space has not been approved for use.
		/// - `DelegateNotFound`: If the delegate specified by
		///   `remove_authorization` is not found in the space.
		///
		/// # Events
		///
		/// - `Deauthorization`: Emitted when a delegate is successfully removed
		///   from a space. The event includes the space ID and the
		///   authorization ID of the removed delegate.
		///
		/// The function emits a `Deauthorization` event upon successful
		/// completion, indicating that a delegate's authorization has been
		/// revoked and they can no longer add entries to the space. This event
		/// serves as a notification for external systems to update their
		/// records, reflecting the change in delegation status for the space.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove_delegate())]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			remove_authorization: AuthorizationIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			// Ensure the authorization exists and retrieve its details.
			let authorization_details = Authorizations::<T>::get(&remove_authorization)
				.ok_or(Error::<T>::AuthorizationNotFound)?;

			// Determine the space_id from the authorization
			let admin_space_id = Self::is_a_space_admin(&authorization, creator.clone())
				.map_err(Error::<T>::from)?;
			ensure!(admin_space_id == space_id, Error::<T>::UnauthorizedOperation);

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

			let mut delegates = Delegates::<T>::get(&space_id);
			if let Some(index) = delegates.iter().position(|d| d == &authorization_details.delegate)
			{
				delegates.remove(index);
				Delegates::<T>::insert(&space_id, delegates);

				Authorizations::<T>::remove(&remove_authorization);

				Self::decrement_usage(&space_id).map_err(Error::<T>::from)?;

				Self::update_activity(&space_id, CallTypeOf::Deauthorization)
					.map_err(Error::<T>::from)?;

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
		/// - `origin`: The origin of the transaction, which must be signed by
		///   the creator.
		/// - `space_code`: A unique code representing the space to be created.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the space is successfully
		///   created, or an error (`DispatchError`) if:
		///   - The generated space identifier is already in use.
		///   - The generated authorization ID is of invalid length.
		///   - The space delegates limit is exceeded.
		///
		/// # Errors
		/// - `InvalidIdentifierLength`: If the generated identifiers for the
		///   space or authorization are of invalid length.
		/// - `SpaceAlreadyAnchored`: If the space identifier is already in use.
		/// - `SpaceDelegatesLimitExceeded`: If the space exceeds the limit of
		///   allowed delegates.
		///
		/// # Events
		/// - `Create`: Emitted when a new space is successfully created. It
		///   includes the space identifier, the creator's identifier, and the
		///   authorization ID.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(origin: OriginFor<T>, space_code: SpaceCodeOf<T>) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			// Id Digest = concat (H(<scale_encoded_registry_input>,
			// <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_code.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier = Ss58Identifier::to_registry_id(&id_digest.encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Spaces<T>>::contains_key(&identifier), Error::<T>::SpaceAlreadyAnchored);

			// Construct the authorization_id from the provided parameters.
			// Id Digest = concat (H(<scale_encoded_space_identifier>,
			// <scale_encoded_creator_identifier> ))
			let auth_id_digest =
				T::Hashing::hash(&[&identifier.encode()[..], &creator.encode()[..]].concat()[..]);

			let authorization_id = Ss58Identifier::to_authorization_id(&auth_id_digest.encode())
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
					capacity: 0,
					usage: 0,
					approved: false,
					archive: false,
				},
			);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(Error::<T>::from)?;

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
		/// has been previously created, setting its capacity and marking it as
		/// approved. It ensures that the space exists, is not archived, and has
		/// not already been approved.
		///
		/// # Parameters
		/// - `origin`: The origin of the transaction, which must be a council
		///   or root origin.
		/// - `space_id`: The identifier of the space to be approved.
		/// - `capacity`: The capacity to be set for the space, which determines
		///   the number of delegates or entries it can hold.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the space is successfully
		///   approved, or an error (`DispatchError`) if:
		///   - The origin is not a council or root origin.
		///   - The space does not exist.
		///   - The space is archived.
		///   - The space is already approved.
		///
		/// # Errors
		/// - `BadOrigin`: If the call does not come from a council or root
		///   origin.
		/// - `SpaceNotFound`: If the specified space ID does not correspond to
		///   an existing space.
		/// - `ArchivedSpace`: If the space is archived and no longer active.
		/// - `SpaceAlreadyApproved`: If the space has already been approved.
		///
		/// # Events
		/// - `Approve`: Emitted when a space is successfully approved. It
		///   includes the space identifier.
		///
		/// # Security Considerations
		/// Due to the privileged nature of this function, callers must ensure
		/// that they have the appropriate authority. Misuse can lead to
		/// unauthorized approval of spaces, which may have security
		/// implications.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn approve(origin: OriginFor<T>, space_id: SpaceIdOf, capacity: u64) -> DispatchResult {
			T::ChainSpaceOrigin::ensure_origin(origin)?;

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(!space_details.approved, Error::<T>::SpaceAlreadyApproved);

			<Spaces<T>>::insert(
				&space_id,
				SpaceDetailsOf::<T> { capacity, approved: true, ..space_details },
			);

			Self::update_activity(&space_id, CallTypeOf::Approved).map_err(Error::<T>::from)?;

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
		/// - `origin`: The origin of the transaction, which must be signed by
		///   the creator or an admin with the appropriate authority.
		/// - `space_id`: The identifier of the space to be archived.
		/// - `authorization`: An identifier for the authorization being used to
		///   validate the archival.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the space is successfully
		///   archived, or an error (`DispatchError`) if:
		///   - The space does not exist.
		/// - `ArchivedSpace`: If the space is already archived.
		/// - `SpaceNotApproved`: If the space has not been approved for use.
		/// - `UnauthorizedOperation`: If the caller does not have the authority
		///   to archive the space.
		///
		/// # Errors
		/// - `SpaceNotFound`: If the specified space ID does not correspond to
		///   an existing space.
		/// - `ArchivedSpace`: If the space is already archived.
		/// - `SpaceNotApproved`: If the space has not been approved for use.
		/// - `UnauthorizedOperation`: If the caller is not authorized to
		///   archive the space.
		///
		/// # Events
		/// - `Archive`: Emitted when a space is successfully archived. It
		///   includes the space ID and the authority who performed the
		///   archival.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::archive())]
		pub fn archive(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archive, Error::<T>::ArchivedSpace);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

			let admin_space_id = Self::is_a_space_admin(&authorization, creator.clone())
				.map_err(Error::<T>::from)?;
			ensure!(admin_space_id == space_id, Error::<T>::UnauthorizedOperation);

			<Spaces<T>>::insert(&space_id, SpaceDetailsOf::<T> { archive: true, ..space_details });

			Self::update_activity(&space_id, CallTypeOf::Archive).map_err(Error::<T>::from)?;

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
		/// - `origin`: The origin of the transaction, which must be signed by
		///   the creator or an admin with the appropriate authority.
		/// - `space_id`: The identifier of the space to be restored.
		/// - `authorization`: An identifier for the authorization being used to
		///   validate the restoration.
		///
		/// # Returns
		/// - `DispatchResult`: Returns `Ok(())` if the space is successfully
		///   restored, or an error (`DispatchError`) if:
		///   - The space does not exist.
		///   - The space is not archived.
		///   - The space is not approved.
		///   - The caller does not have the authority to restore the space.
		///
		/// # Errors
		/// - `SpaceNotFound`: If the specified space ID does not correspond to
		///   an existing space.
		/// - `SpaceNotArchived`: If the space is not currently archived.
		/// - `SpaceNotApproved`: If the space has not been approved for use.
		/// - `UnauthorizedOperation`: If the caller is not authorized to
		///   restore the space.
		///
		/// # Events
		/// - `Restore`: Emitted when a space is successfully restored. It
		///   includes the space ID and the authority who performed the
		///   restoration.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::restore())]
		pub fn restore(
			origin: OriginFor<T>,
			space_id: SpaceIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let space_details = Spaces::<T>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(space_details.archive, Error::<T>::SpaceNotArchived);
			ensure!(space_details.approved, Error::<T>::SpaceNotApproved);

			let admin_space_id = Self::is_a_space_admin(&authorization, creator.clone())
				.map_err(Error::<T>::from)?;
			ensure!(admin_space_id == space_id, Error::<T>::UnauthorizedOperation);

			<Spaces<T>>::insert(&space_id, SpaceDetailsOf::<T> { archive: false, ..space_details });

			Self::update_activity(&space_id, CallTypeOf::Restore).map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Restore { space: space_id, authority: creator });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Checks if a given entity is a delegate for the specified space.
	///
	/// This function retrieves the list of delegates for a space and determines
	/// whether the specified delegate is among them. It is a read-only
	/// operation and does not modify the state.
	///
	/// # Parameters
	/// - `tx_id`: The identifier of the space to check for the delegation.
	/// - `delegate`: The entity to check for delegate status.
	///
	/// # Returns
	/// - `bool`: Returns `true` if the specified entity is a delegate of the
	///   given space, otherwise `false`.
	pub fn is_a_delegate(tx_id: &SpaceIdOf, delegate: SpaceCreatorOf<T>) -> bool {
		<Delegates<T>>::get(tx_id).iter().any(|d| d == &delegate)
	}

	/// Verifies if a given delegate has a specific authorization.
	///
	/// This function checks if the provided delegate is associated with the
	/// given authorization ID and has the 'ASSERT' permission.
	///
	/// # Parameters
	/// - `authorization_id`: A reference to the identifier of the authorization
	///   to check.
	/// - `delegate`: A reference to the delegate whose status is being
	///   verified.
	///
	/// # Returns
	/// - `Result<SpaceIdOf, Error<T>>`: Returns `Ok(space_id)` if the delegate
	///   has the 'ASSERT' permission for the space, otherwise returns an `Err`
	///   with the appropriate error.
	///
	/// # Errors
	/// - `AuthorizationNotFound`: If the authorization ID does not correspond
	///   to any existing authorization.
	/// - `UnauthorizedOperation`: If the delegate does not have the 'ASSERT'
	///   permission or is not the delegate associated with the authorization
	///   ID.
	pub fn is_a_space_delegate(
		authorization_id: &AuthorizationIdOf,
		delegate: &SpaceCreatorOf<T>,
	) -> Result<SpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == *delegate, Error::<T>::UnauthorizedOperation);
		ensure!(d.permissions.contains(Permissions::ASSERT), Error::<T>::UnauthorizedOperation);

		Ok(d.space_id)
	}

	/// Checks if a given delegate is an admin for the space associated with the
	/// authorization ID.
	///
	/// This function verifies whether the specified delegate is the admin of
	/// the space by checking the 'ADMIN' permission within the authorization
	/// tied to the provided authorization ID.
	///
	/// # Parameters
	/// - `authorization_id`: A reference to the identifier of the authorization
	///   to check.
	/// - `delegate`: The delegate whose admin status is being verified.
	///
	/// # Returns
	/// - `Result<SpaceIdOf, Error<T>>`: Returns `Ok(space_id)` if the delegate
	///   is an admin for the space, otherwise returns an `Err` with the
	///   appropriate error.
	///
	/// # Errors
	/// - `AuthorizationNotFound`: If the authorization ID does not correspond
	///   to any existing authorization.
	/// - `UnauthorizedOperation`: If the delegate does not have the 'ADMIN'
	///   permission or is not the delegate associated with the authorization
	///   ID.
	pub fn is_a_space_admin(
		authorization_id: &AuthorizationIdOf,
		delegate: SpaceCreatorOf<T>,
	) -> Result<SpaceIdOf, Error<T>> {
		let d =
			<Authorizations<T>>::get(authorization_id).ok_or(Error::<T>::AuthorizationNotFound)?;

		ensure!(d.delegate == delegate, Error::<T>::UnauthorizedOperation);
		ensure!(d.permissions.contains(Permissions::ADMIN), Error::<T>::UnauthorizedOperation);

		Ok(d.space_id)
	}
	/// Ensures that the space has not exceeded its capacity.
	///
	/// This function checks if the space identified by `space_id` has exceeded
	/// its capacity for delegates or entries.
	///
	/// # Parameters
	/// - `space_id`: A reference to the identifier of the space to check.
	///
	/// # Returns
	/// - `Result<(), Error<T>>`: Returns `Ok(())` if the space has not exceeded
	///   its capacity, otherwise returns an `Err` with the appropriate error.
	///
	/// # Errors
	/// - `SpaceNotFound`: If the space ID does not correspond to any existing
	///   space.
	/// - `CapacityLimitExceeded`: If the space has reached or exceeded its
	///   capacity limit.
	pub fn ensure_capacity_not_exceeded(space_id: &SpaceIdOf) -> Result<(), Error<T>> {
		Spaces::<T>::get(space_id)
			.ok_or(Error::<T>::SpaceNotFound)
			.and_then(|space_details| {
				if space_details.capacity == 0 || space_details.usage < space_details.capacity {
					Ok(())
				} else {
					Err(Error::<T>::CapacityLimitExceeded)
				}
			})
	}

	/// Ensures that the space has not exceeded its capacity, considering a
	/// batch of entries.
	///
	/// This function checks if the space identified by `space_id` can
	/// accommodate a specified number of new entries without exceeding its
	/// capacity.
	///
	/// # Parameters
	/// - `space_id`: A reference to the identifier of the space to check.
	/// - `entries`: The number of new entries to be added to the space.
	///
	/// # Returns
	/// - `Result<(), Error<T>>`: Returns `Ok(())` if the space can accommodate
	///   the new entries, otherwise returns an `Err` with the appropriate
	///   error.
	///
	/// # Errors
	/// - `SpaceNotFound`: If the space ID does not correspond to any existing
	///   space.
	/// - `CapacityLimitExceeded`: If adding the new entries would exceed the
	///   space's capacity limit.
	pub fn ensure_capacity_not_exceeded_batch(
		space_id: &SpaceIdOf,
		entries: u16,
	) -> Result<(), Error<T>> {
		Spaces::<T>::get(space_id)
			.ok_or(Error::<T>::SpaceNotFound)
			.and_then(|space_details| {
				if space_details.capacity == 0 ||
					space_details.usage + entries as u64 <= space_details.capacity
				{
					Ok(())
				} else {
					Err(Error::<T>::CapacityLimitExceeded)
				}
			})
	}

	/// Increments the usage count of a space by one unit.
	///
	/// This function is used to increase the usage counter of a space,
	/// typically when a new delegate or entry is added. It ensures that the
	/// usage count does not overflow.
	///
	/// # Parameters
	/// - `tx_id`: A reference to the identifier of the space whose usage is
	///   being incremented.
	///
	/// # Returns
	/// - `Result<(), Error<T>>`: Returns `Ok(())` if the usage is successfully
	///   incremented, or an `Err` if the space does not exist.
	///
	/// # Errors
	/// - `SpaceNotFound`: If the space ID does not correspond to any existing
	///   space.
	pub fn increment_usage(tx_id: &SpaceIdOf) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.usage = space_details.usage.saturating_add(1);
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound.into())
			}
		})
	}

	/// Decrements the usage count of a space by one unit.
	///
	/// This function is used to decrease the usage counter of a space,
	/// typically when a delegate or entry is removed. It ensures that the usage
	/// count does not underflow.
	///
	/// # Parameters
	/// - `tx_id`: A reference to the identifier of the space whose usage is
	///   being decremented.
	///
	/// # Returns
	/// - `Result<(), Error<T>>`: Returns `Ok(())` if the usage is successfully
	///   decremented, or an `Err` if the space does not exist.
	///
	/// # Errors
	/// - `SpaceNotFound`: If the space ID does not correspond to any existing
	///   space.
	pub fn decrement_usage(tx_id: &SpaceIdOf) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.usage = space_details.usage.saturating_sub(1);
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound.into())
			}
		})
	}

	/// Increments the usage count of a space by a specified unit.
	///
	/// This function increases the usage counter of a space by the amount
	/// specified in `increment`, which is useful for batch operations.
	/// It ensures that the usage count does not overflow.
	///
	/// # Parameters
	/// - `tx_id`: A reference to the identifier of the space whose usage is
	///   being incremented.
	/// - `increment`: The amount by which to increment the usage count.
	///
	/// # Returns
	/// - `Result<(), Error<T>>`: Returns `Ok(())` if the usage is successfully
	///   incremented by the specified amount, or an `Err` if the space does not
	///   exist.
	///
	/// # Errors
	/// - `SpaceNotFound`: If the space ID does not correspond to any existing
	///   space.
	pub fn increment_usage_batch(tx_id: &SpaceIdOf, increment: u16) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.usage = space_details.usage.saturating_add(increment.into());
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound.into())
			}
		})
	}

	/// Decrements the usage count of a space by a specified amount.
	///
	/// This function decreases the usage counter of a space by the amount
	/// specified in `decrement`, which is useful for batch removals. It ensures
	/// that the usage count does not underflow.
	///
	/// # Parameters
	/// - `tx_id`: A reference to the identifier of the space whose usage is
	///   being decremented.
	/// - `decrement`: The amount by which to decrement the usage count.
	///
	/// # Returns
	/// - `Result<(), Error<T>>`: Returns `Ok(())` if the usage is successfully
	///   decremented by the specified amount, or an `Err` if the space does not
	///   exist.
	///
	/// # Errors
	/// - `SpaceNotFound`: If the space ID does not correspond to any existing
	///   space.
	pub fn decrement_usage_batch(tx_id: &SpaceIdOf, decrement: u16) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.usage = space_details.usage.saturating_sub(decrement.into());
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound.into())
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
	///
	/// # Parameters
	/// - `tx_id`: A reference to the identifier of the space that has been
	///   updated.
	/// - `tx_action`: The type of activity that has occurred, encapsulated
	///   within `CallTypeOf`.
	///
	/// # Returns
	/// - `Result<(), Error<T>>`: Returns `Ok(())` if the activity is
	///   successfully recorded on the global timeline. In the context of the
	///   system's automatic invocation, this function will typically not return
	///   an error to the caller, but will handle any errors internally.
	///
	/// # Note
	/// This function is not intended to be called directly by external
	/// entities. It is triggered by the system in response to actions such as
	/// creating, modifying, or deleting a space, as well as adding or removing
	/// delegates.
	///
	/// The function ensures that the global timeline reflects all updates,
	/// providing a reliable audit trail for the space's lifecycle.
	pub fn update_activity(tx_id: &SpaceIdOf, tx_action: CallTypeOf) -> Result<(), Error<T>> {
		let tx_moment = Self::timepoint();

		let tx_entry = EventEntryOf { action: tx_action, location: tx_moment };
		let _ =
			identifier::Pallet::<T>::update_timeline(tx_id, IdentifierTypeOf::Statement, tx_entry);
		Ok(())
	}

	/// Retrieves the current timepoint.
	///
	/// This function returns a `Timepoint` structure containing the current
	/// block number and extrinsic index. It is typically used in conjunction
	/// with `update_activity` to record when an event occurred.
	///
	/// # Returns
	/// - `Timepoint`: A structure containing the current block number and
	///   extrinsic index.
	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
