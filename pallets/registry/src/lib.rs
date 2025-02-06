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
use alloc::vec::Vec;
pub mod delegation;
pub mod registry;

use alloc::str;
use frame_support::{ensure, storage::types::StorageMap};
pub mod types;
pub use crate::{pallet::*, types::*};
use cord_uri::{EntryTypeOf, EventStamp, Identifier, RegistryIdentifierCheck, Ss58Identifier};
use frame_support::dispatch::DispatchResult;
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::WeightInfo;

#[cfg(test)]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod tests;

/// Identifier
pub type CollectionIdentifierOf = Ss58Identifier;
pub type RegistryIdentifierOf = Ss58Identifier;
pub type HashOf<T> = <T as frame_system::Config>::Hash;
pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config + cord_uri::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Stores registry details
	#[pallet::storage]
	pub type Registries<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		RegistryIdentifierOf,
		RegistryDetails<CordAccountOf<T>, HashOf<T>, Status>,
		OptionQuery,
	>;

	/// Stores collection-level delegates (account → permissions).
	#[pallet::storage]
	pub type Delegates<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		CordAccountOf<T>,
		Permissions,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DelegateAdded { identifier: Ss58Identifier, delegate: CordAccountOf<T> },
		DelegateRemoved { identifier: Ss58Identifier, delegate: CordAccountOf<T> },
		RegistryCreated { registry: RegistryIdentifierOf, creator: CordAccountOf<T> },
		RegistryUpdated { registry: RegistryIdentifierOf, authority: CordAccountOf<T> },
		RegistryArchived { registry: RegistryIdentifierOf, authority: CordAccountOf<T> },
		RegistryRestored { registry: RegistryIdentifierOf, authority: CordAccountOf<T> },
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// The caller does not have the required permissions.
		UnauthorizedOperation,
		/// The provided identifier length is invalid.
		InvalidIdentifierLength,
		/// The delegate is already added to the collection.
		DelegateAlreadyExists,
		/// The specified delegate was not found.
		DelegateNotFound,
		/// A registry with the given identifier already exists in the collection.
		RegistryAlreadyExists,
		/// The specified registry was not found in the collection.
		RegistryNotFound,
		/// The registry is already archived.
		ArchivedRegistry,
		/// The registry is not archived (and thus cannot be restored).
		RegistryNotArchived,
		/// The provided entry type input is invalid.
		InvalidEntryTypeInput,
		/// The activity update operation failed.
		ActivityUpdateFailed,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a delegate with given permissions.
		#[pallet::call_index(0)]
		#[pallet::weight({10_000})]
		pub fn add_delegate(
			origin: OriginFor<T>,
			identifier: Ss58Identifier,
			delegate: CordAccountOf<T>,
			roles: Vec<PermissionVariant>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			delegation::add_delegate::<T>(&identifier, &who, &delegate, roles)?;
			Self::deposit_event(Event::DelegateAdded { identifier, delegate });
			Ok(())
		}

		/// Removes a delegate
		#[pallet::call_index(1)]
		#[pallet::weight({10_000})]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			identifier: Ss58Identifier,
			delegate: CordAccountOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			delegation::remove_delegate::<T>(&identifier, &who, &delegate)?;
			Self::deposit_event(Event::DelegateRemoved { identifier, delegate });
			Ok(())
		}

		/// Create a new registry.
		#[pallet::call_index(2)]
		#[pallet::weight({10_000})]
		pub fn create(
			origin: OriginFor<T>,
			tx_hash: HashOf<T>,
			doc_id: Option<Vec<u8>>,
			doc_author_id: Option<CordAccountOf<T>>,
			doc_node_id: Option<Vec<u8>>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let registry_id = registry::create_registry::<T>(
				tx_hash,
				doc_id,
				doc_author_id,
				doc_node_id,
				creator.clone(),
			)?;
			Self::deposit_event(Event::RegistryCreated { registry: registry_id, creator });
			Ok(())
		}

		/// Archive registry
		#[pallet::call_index(3)]
		#[pallet::weight({10_000})]
		pub fn archive(origin: OriginFor<T>, registry_id: RegistryIdentifierOf) -> DispatchResult {
			let who = ensure_signed(origin)?;
			registry::archive_registry::<T>(&registry_id, who.clone())?;
			Self::deposit_event(Event::RegistryArchived { registry: registry_id, authority: who });
			Ok(())
		}

		/// Restore registry
		#[pallet::call_index(4)]
		#[pallet::weight({10_000})]
		pub fn restore(origin: OriginFor<T>, registry_id: RegistryIdentifierOf) -> DispatchResult {
			let who = ensure_signed(origin)?;
			registry::restore_registry::<T>(&registry_id, who.clone())?;
			Self::deposit_event(Event::RegistryRestored { registry: registry_id, authority: who });
			Ok(())
		}

		/// Update registry entry author
		#[pallet::call_index(5)]
		#[pallet::weight({10_000})]
		pub fn update_author(
			origin: OriginFor<T>,
			registry_id: RegistryIdentifierOf,
			new_doc_author_id: CordAccountOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			registry::update_registry_author::<T>(
				&registry_id,
				new_doc_author_id.clone(),
				who.clone(),
			)?;
			Self::deposit_event(Event::RegistryUpdated {
				registry: registry_id,
				authority: new_doc_author_id,
			});
			Ok(())
		}

		/// Update registry creator
		#[pallet::call_index(6)]
		#[pallet::weight({10_000})]
		pub fn update_creator(
			origin: OriginFor<T>,
			registry_id: RegistryIdentifierOf,
			new_creator: CordAccountOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			registry::update_registry_creator::<T>(&registry_id, new_creator.clone(), who.clone())?;
			Self::deposit_event(Event::RegistryUpdated {
				registry: registry_id,
				authority: new_creator,
			});
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Checks that the registry identified by `registry_id` exists and is active.
	pub fn inherent_ensure_active_registry(registry_id: &Ss58Identifier) -> DispatchResult {
		let registry = Registries::<T>::get(registry_id).ok_or(Error::<T>::RegistryNotFound)?;
		ensure!(registry.status == Status::Active, Error::<T>::ArchivedRegistry);
		Ok(())
	}

	/// Checks if the delegate for `who` on the given collection has any of the required permissions.
	pub fn has_permission(
		identifier: &Ss58Identifier,
		who: &CordAccountOf<T>,
		required: Permissions,
	) -> bool {
		Delegates::<T>::get(identifier, who).unwrap_or_default().intersects(required)
	}

	/// Helper function to encode an optional field into a byte buffer.
	/// Pushes a flag byte (1 if the field is present, 0 if not) followed by the field's bytes if present.
	pub fn push_option(buf: &mut Vec<u8>, field: Option<&[u8]>) {
		if let Some(bytes) = field {
			buf.push(1u8);
			buf.extend_from_slice(bytes);
		} else {
			buf.push(0u8);
		}
	}
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

impl<T: Config> RegistryIdentifierCheck for Pallet<T> {
	fn ensure_active_registry(registry_id: &Ss58Identifier) -> DispatchResult {
		Self::inherent_ensure_active_registry(registry_id)
	}
}
