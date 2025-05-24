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

extern crate alloc;
use alloc::str;
use frame_support::{ensure, storage::types::StorageMap};
pub mod types;
pub use crate::{pallet::*, types::*};
use alloc::vec::Vec;
use codec::Encode;
use cord_primitives::identifier::Ss58Identifier;
use frame_support::dispatch::DispatchResult;
use frame_system::{pallet_prelude::BlockNumberFor, WeightInfo};
use pallet_doken::{Doken, EventBlock, EventTypeOf};
use pallet_profile::ProfileIdOf;
use sp_runtime::traits::{Hash, One, Saturating};

#[cfg(test)]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod tests;

/// Identifier
pub type CollectionIdentifierOf = Ss58Identifier;
pub type RegistryIdentifierOf = Ss58Identifier;
pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_doken::Config + pallet_profile::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Stores collection details (creator, status) for each collection.
	#[pallet::storage]
	pub type Collections<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CollectionIdentifierOf,
		CollectionDetails<ProfileIdOf, Status>,
		OptionQuery,
	>;

	/// Stores collection-level delegates (account → permissions). Using a double map
	/// avoids a bounded collection.
	#[pallet::storage]
	pub type Delegates<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CollectionIdentifierOf,
		Blake2_128Concat,
		ProfileIdOf,
		Permissions,
		OptionQuery,
	>;

	/// Maps a collection to its registries. The value is unit type because the
	/// registry information (and status) is managed by another pallet.
	#[pallet::storage]
	#[pallet::getter(fn collection_registries)]
	pub type CollectionRegistries<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CollectionIdentifierOf,
		Blake2_128Concat,
		RegistryIdentifierOf,
		(),
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CollectionCreated {
			collection: CollectionIdentifierOf,
			creator: CordAccountOf<T>,
			creator_profile_id: ProfileIdOf,
		},
		CollectionArchived {
			collection: CollectionIdentifierOf,
			authority: CordAccountOf<T>,
			authority_profile_id: ProfileIdOf,
		},
		CollectionRestored {
			collection: CollectionIdentifierOf,
			authority: CordAccountOf<T>,
			authority_profile_id: ProfileIdOf,
		},
		DelegateAdded {
			collection: CollectionIdentifierOf,
			delegate: CordAccountOf<T>,
			delegate_profile_id: ProfileIdOf,
		},
		DelegateRemoved {
			collection: CollectionIdentifierOf,
			delegate: CordAccountOf<T>,
			delegate_profile_id: ProfileIdOf,
		},
		RegistryAdded {
			collection: CollectionIdentifierOf,
			registry: RegistryIdentifierOf,
		},
		RegistryRemoved {
			collection: CollectionIdentifierOf,
			registry: RegistryIdentifierOf,
		},
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// The caller does not have the required permissions.
		UnauthorizedOperation,
		/// The provided identifier length is invalid.
		InvalidIdentifierLength,
		/// A collection with the given identifier already exists.
		CollectionAlreadyExists,
		/// The specified collection was not found.
		CollectionNotFound,
		/// The collection is already archived.
		ArchivedCollection,
		/// The collection is not archived (and thus cannot be restored).
		CollectionNotArchived,
		/// The delegate is already added to the collection.
		DelegateAlreadyExists,
		/// The specified delegate was not found.
		DelegateNotFound,
		/// A registry with the given identifier already exists in the collection.
		RegistryAlreadyExists,
		/// The specified registry was not found in the collection.
		RegistryNotFound,
		/// The provided entry type input is invalid.
		InvalidEntryTypeInput,
		/// The provided event type is invalid.
		InvalidEventType,
		/// The activity update operation failed.
		EventUpdateFailed,
		// State Update Failed
		StateUpdateFailed,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a delegate with given permissions to a collection.
		#[pallet::call_index(0)]
		#[pallet::weight({10_000})]
		pub fn add_delegate(
			origin: OriginFor<T>,
			collection_id: CollectionIdentifierOf,
			delegate: CordAccountOf<T>,
			roles: Vec<PermissionVariant>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let who_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let delegate_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&delegate)
				.map_err(<pallet_profile::Error<T>>::from)?;

			ensure!(
				Self::has_permission(
					&collection_id,
					&who_profile_id,
					Permissions::ADMIN | Permissions::DELEGATE
				),
				Error::<T>::UnauthorizedOperation
			);
			ensure!(
				!Delegates::<T>::contains_key(&collection_id, &delegate_profile_id),
				Error::<T>::DelegateAlreadyExists
			);

			let permissions = Permissions::from_variants(&roles);
			let digest = T::Hashing::hash(&delegate.encode());

			Self::record_activity(&collection_id, digest, b"DelegateAdded")?;
			Delegates::<T>::insert(&collection_id, &delegate_profile_id, permissions);
			Self::deposit_event(Event::DelegateAdded {
				collection: collection_id,
				delegate,
				delegate_profile_id,
			});

			Ok(())
		}

		/// Adds an administrative delegate to a namespace.
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			collection_id: CollectionIdentifierOf,
			delegate: CordAccountOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let who_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let delegate_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&delegate)
				.map_err(<pallet_profile::Error<T>>::from)?;

			ensure!(
				Self::has_permission(&collection_id, &who_profile_id, Permissions::ADMIN),
				Error::<T>::UnauthorizedOperation
			);
			ensure!(
				Delegates::<T>::contains_key(&collection_id, &delegate_profile_id),
				Error::<T>::DelegateNotFound
			);
			let digest = T::Hashing::hash(&delegate.encode());

			Self::record_activity(&collection_id, digest, b"DelegateRemoved")?;
			Delegates::<T>::remove(&collection_id, &delegate_profile_id);
			Self::deposit_event(Event::DelegateRemoved {
				collection: collection_id,
				delegate,
				delegate_profile_id,
			});

			Ok(())
		}

		/// Create a new collection.
		#[pallet::call_index(2)]
		#[pallet::weight({10_000})]
		pub fn create(origin: OriginFor<T>) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&creator)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let pallet_name = <Self as PalletInfoAccess>::name();
			let previous_block_hash = <frame_system::Pallet<T>>::block_hash(
				<frame_system::Pallet<T>>::block_number().saturating_sub(One::one()),
			);

			let mut input = previous_block_hash.encode();
			input.extend_from_slice(&creator.encode());
			let digest = T::Hashing::hash(&input);

			let identifier =
				<pallet_doken::Pallet<T> as Doken<T>>::build(&(digest).encode()[..], pallet_name)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!Collections::<T>::contains_key(&identifier),
				Error::<T>::CollectionAlreadyExists
			);

			let details = CollectionDetails { creator: profile_id.clone(), status: Status::Active };

			Self::record_activity(&identifier, digest, b"CollectionCreated")?;

			Collections::<T>::insert(&identifier, details);
			Delegates::<T>::insert(&identifier, &profile_id, Permissions::all());

			Self::deposit_event(Event::CollectionCreated {
				collection: identifier,
				creator,
				creator_profile_id: profile_id,
			});

			Ok(())
		}

		/// Archive a collection.
		#[pallet::call_index(3)]
		#[pallet::weight({10_000})]
		pub fn archive(
			origin: OriginFor<T>,
			collection_id: CollectionIdentifierOf,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			Collections::<T>::try_mutate(&collection_id, |maybe_collection| -> DispatchResult {
				let collection = maybe_collection.as_mut().ok_or(Error::<T>::CollectionNotFound)?;

				ensure!(
					Self::has_permission(&collection_id, &profile_id, Permissions::ADMIN),
					Error::<T>::UnauthorizedOperation
				);
				ensure!(collection.status == Status::Active, Error::<T>::ArchivedCollection);
				collection.status = Status::Archived;

				Ok(())
			})?;

			let digest = T::Hashing::hash(&collection_id.encode());
			Self::record_activity(&collection_id, digest, b"CollectionArchived")?;

			Self::deposit_event(Event::CollectionArchived {
				collection: collection_id,
				authority: who,
				authority_profile_id: profile_id,
			});

			Ok(())
		}

		/// Restore a collection.
		#[pallet::call_index(4)]
		#[pallet::weight({10_000})]
		pub fn restore(
			origin: OriginFor<T>,
			collection_id: CollectionIdentifierOf,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			Collections::<T>::try_mutate(&collection_id, |maybe_collection| -> DispatchResult {
				let collection = maybe_collection.as_mut().ok_or(Error::<T>::CollectionNotFound)?;
				ensure!(
					Self::has_permission(&collection_id, &profile_id, Permissions::ADMIN),
					Error::<T>::UnauthorizedOperation
				);
				ensure!(collection.status == Status::Archived, Error::<T>::CollectionNotArchived);
				collection.status = Status::Active;

				Ok(())
			})?;

			let digest = T::Hashing::hash(&collection_id.encode());
			Self::record_activity(&collection_id, digest, b"CollectionRestored")?;

			Self::deposit_event(Event::CollectionRestored {
				collection: collection_id,
				authority: who,
				authority_profile_id: profile_id,
			});

			Ok(())
		}

		/// Add a registry to a collection.
		#[pallet::call_index(5)]
		#[pallet::weight({10_000})]
		pub fn add_registry(
			origin: OriginFor<T>,
			collection_id: CollectionIdentifierOf,
			registry_id: RegistryIdentifierOf,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let collection =
				Collections::<T>::get(&collection_id).ok_or(Error::<T>::CollectionNotFound)?;
			ensure!(collection.status == Status::Active, Error::<T>::ArchivedCollection);

			ensure!(
				Self::has_permission(
					&collection_id,
					&profile_id,
					Permissions::ADMIN | Permissions::ENTRY
				),
				Error::<T>::UnauthorizedOperation
			);

			ensure!(
				!CollectionRegistries::<T>::contains_key(&collection_id, &registry_id),
				Error::<T>::RegistryAlreadyExists
			);

			let digest = T::Hashing::hash(&registry_id.encode());
			Self::record_activity(&collection_id, digest, b"RegistryAdded")?;

			CollectionRegistries::<T>::insert(&collection_id, &registry_id, ());
			Self::deposit_event(Event::RegistryAdded {
				collection: collection_id,
				registry: registry_id,
			});

			Ok(())
		}

		/// Remove a registry from a collection.
		#[pallet::call_index(6)]
		#[pallet::weight({10_000})]
		pub fn remove_registry(
			origin: OriginFor<T>,
			collection_id: CollectionIdentifierOf,
			registry_id: RegistryIdentifierOf,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let collection =
				Collections::<T>::get(&collection_id).ok_or(Error::<T>::CollectionNotFound)?;
			ensure!(collection.status == Status::Active, Error::<T>::ArchivedCollection);

			ensure!(
				Self::has_permission(&collection_id, &profile_id, Permissions::ADMIN),
				Error::<T>::UnauthorizedOperation
			);

			ensure!(
				CollectionRegistries::<T>::contains_key(&collection_id, &registry_id),
				Error::<T>::RegistryNotFound
			);

			let digest = T::Hashing::hash(&registry_id.encode());
			Self::record_activity(&collection_id, digest, b"RegistryRemoved")?;

			CollectionRegistries::<T>::remove(&collection_id, &registry_id);
			Self::deposit_event(Event::RegistryRemoved {
				collection: collection_id,
				registry: registry_id,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Checks if the delegate for `who` on the given collection has any of the required
	/// permissions.
	pub fn has_permission(
		collection_id: &CollectionIdentifierOf,
		who: &ProfileIdOf,
		required: Permissions,
	) -> bool {
		Delegates::<T>::get(collection_id, who)
			.unwrap_or(Permissions::empty())
			.intersects(required)
	}

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
}
