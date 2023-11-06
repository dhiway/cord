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
/// Registry Identifier
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
		// Invalid Identifier Length
		InvalidIdentifierLength,
		// Invalid Identifier Prefix
		InvalidIdentifierPrefix,
		// Archived Registry
		ArchivedSpace,
		// Registry not Archived
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
		/// Adds a delegate to a registry.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `registry_id`: The registry to which the delegate is being added.
		/// * `delegate`: The delegate to add to the registry.
		///
		/// Returns:
		///
		/// DispatchResult
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

		/// Removes a delegate from a registry.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `registry_id`: The registry_id of the registry you want to remove
		///   the delegate from.
		/// * `authorization_id`: The transaction authorization id .
		///
		/// Returns:
		///
		/// DispatchResult
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

		/// Create a new registry.
		///
		/// Arguments:
		///
		/// * `origin`: OriginFor<T>
		/// * `tx_registry`: The new registry detail
		/// * `tx_schema`: Optional schema identifier. Schema Identifier is used
		///   to restrict the registry
		/// * content to a specific schema type.
		///
		/// Returns:
		///
		/// DispatchResult
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

		/// Archives a registry
		///
		/// Arguments:
		///
		/// * `origin`: OriginFor<T>
		/// * `registry_id`: The id of the registry to archive.
		///
		/// Returns:
		///
		/// DispatchResult
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

		/// Restores an archived registry
		///
		/// Arguments:
		///
		/// * `origin`: OriginFor<T>
		/// * `registry_id`: The id of the registry to be restored.
		///
		/// Returns:
		///
		/// DispatchResult
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
	pub fn is_a_delegate(tx_id: &SpaceIdOf, delegate: SpaceCreatorOf<T>) -> bool {
		<Delegates<T>>::get(tx_id).iter().any(|d| d == &delegate)
	}

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

	/// Ensures that the space has not exceeded its capacity for batch.
	pub fn ensure_capacity_not_exceeded_batch(
		space_id: &SpaceIdOf,
		entries: u64,
	) -> Result<(), Error<T>> {
		Spaces::<T>::get(space_id)
			.ok_or(Error::<T>::SpaceNotFound)
			.and_then(|space_details| {
				if space_details.capacity == 0 ||
					space_details.usage + entries <= space_details.capacity
				{
					Ok(())
				} else {
					Err(Error::<T>::CapacityLimitExceeded)
				}
			})
	}

	/// Increments the usage of the space by one unit.
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

	/// Decrements the usage of the space by one unit.
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

	/// Increments the usage of the space by one unit.
	pub fn increment_usage_batch(tx_id: &SpaceIdOf, increment: u64) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.usage = space_details.usage.saturating_add(increment);
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound.into())
			}
		})
	}

	/// Decrements the usage of the space by one unit.
	pub fn decrement_usage_batch(tx_id: &SpaceIdOf, decrement: u64) -> Result<(), Error<T>> {
		Spaces::<T>::try_mutate(tx_id, |space_opt| {
			if let Some(space_details) = space_opt {
				space_details.usage = space_details.usage.saturating_sub(decrement);
				Ok(())
			} else {
				Err(Error::<T>::SpaceNotFound.into())
			}
		})
	}

	pub fn update_activity(tx_id: &SpaceIdOf, tx_action: CallTypeOf) -> Result<(), Error<T>> {
		let tx_moment = Self::timepoint();

		let tx_entry = EventEntryOf { action: tx_action, location: tx_moment };
		let _ =
			identifier::Pallet::<T>::update_timeline(tx_id, IdentifierTypeOf::Statement, tx_entry);
		Ok(())
	}

	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
