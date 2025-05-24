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
use cord_primitives::identifier::Ss58Identifier;
use frame_support::{dispatch::DispatchResult, traits::ConstU32, BoundedVec};
use frame_system::{pallet_prelude::BlockNumberFor, WeightInfo};
use pallet_doken::{Doken, EventBlock, EventTypeOf};
use pallet_profile::ProfileIdOf;

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

pub type DocIdOf = BoundedVec<u8, ConstU32<64>>;
pub type DocNodeIdOf = BoundedVec<u8, ConstU32<64>>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Type of the Maximum size of Registry Blob
	pub type MaxRegistryBlobSizeOf<T> = <T as crate::Config>::MaxRegistryBlobSize;

	/// Type of Registry Blob
	pub type RegistryBlobOf<T> = BoundedVec<u8, MaxRegistryBlobSizeOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_doken::Config + pallet_profile::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The maximum number of bytes in size a Registry Blob can hold.
		#[pallet::constant]
		type MaxRegistryBlobSize: Get<u32>;

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
		RegistryDetails<HashOf<T>, Status, DocIdOf, DocNodeIdOf>,
		OptionQuery,
	>;

	/// Stores registry-level delegates (account → permissions).
	#[pallet::storage]
	pub type Delegates<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Ss58Identifier,
		Blake2_128Concat,
		ProfileIdOf,
		Permissions,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DelegateAdded {
			identifier: Ss58Identifier,
			delegate: CordAccountOf<T>,
			delegate_profile_id: ProfileIdOf,
		},
		DelegateRemoved {
			identifier: Ss58Identifier,
			delegate: CordAccountOf<T>,
			delegate_profile_id: ProfileIdOf,
		},
		RegistryCreated {
			registry: RegistryIdentifierOf,
			creator: CordAccountOf<T>,
			profile_id: ProfileIdOf,
		},
		RegistryStoreCreated {
			registry: RegistryIdentifierOf,
			creator: CordAccountOf<T>,
			profile_id: ProfileIdOf,
		},
		RegistryUpdated {
			registry: RegistryIdentifierOf,
			authority: CordAccountOf<T>,
			authority_profile_id: ProfileIdOf,
		},
		RegistryArchived {
			registry: RegistryIdentifierOf,
			authority: CordAccountOf<T>,
			authority_profile_id: ProfileIdOf,
		},
		RegistryRestored {
			registry: RegistryIdentifierOf,
			authority: CordAccountOf<T>,
			authority_profile_id: ProfileIdOf,
		},
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
		/// The provided event type is invalid.
		InvalidEventType,
		/// The activity update operation failed.
		EventUpdateFailed,
		// State Update Failed
		StateUpdateFailed,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds a delegate with specified permissions to a registry.
		///
		/// Assigns permissions (e.g., ENTRY, ADMIN) to a delegate for the given registry. The
		/// caller must have ADMIN permissions, and both the caller and delegate must have valid
		/// profiles. The delegate is added via the `delegation::add_delegate` function, and an
		/// activity is recorded.
		///
		/// # Arguments
		/// * `origin` - The signed account adding the delegate.
		/// * `identifier` - The SS58 identifier of the registry.
		/// * `delegate` - The account ID of the delegate to add.
		/// * `roles` - A vector of permission variants (e.g., ENTRY, ADMIN).
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks ADMIN permissions.
		/// * `DelegateAlreadyExists` - If the delegate is already added.
		/// * `pallet_profile::Error` - If the caller’s or delegate’s profile is invalid.
		///
		/// # Events
		/// * `DelegateAdded` - Emitted with `identifier`, `delegate`, `delegate_profile_id`.
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({10_000})]
		pub fn add_delegate(
			origin: OriginFor<T>,
			identifier: Ss58Identifier,
			delegate: CordAccountOf<T>,
			roles: Vec<PermissionVariant>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let who_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let delegate_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&delegate)
				.map_err(<pallet_profile::Error<T>>::from)?;

			delegation::add_delegate::<T>(
				&identifier,
				&who_profile_id,
				&delegate_profile_id,
				roles,
			)?;
			Self::deposit_event(Event::DelegateAdded { identifier, delegate, delegate_profile_id });

			Ok(())
		}

		/// Removes a delegate from a registry.
		///
		/// Revokes all permissions for the specified delegate in the registry. The caller must have
		/// ADMIN permissions, and both the caller and delegate must have valid profiles. The
		/// delegate is removed via the `delegation::remove_delegate` function, and an activity is
		/// recorded.
		///
		/// # Arguments
		/// * `origin` - The signed account removing the delegate.
		/// * `identifier` - The SS58 identifier of the registry.
		/// * `delegate` - The account ID of the delegate to remove.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks ADMIN permissions.
		/// * `DelegateNotFound` - If the delegate is not found in the registry.
		/// * `pallet_profile::Error` - If the caller’s or delegate’s profile is invalid.
		///
		/// # Events
		/// * `DelegateRemoved` - Emitted with `identifier`, `delegate`, `delegate_profile_id`.
		///
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({10_000})]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			identifier: Ss58Identifier,
			delegate: CordAccountOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let delegate_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&delegate)
				.map_err(<pallet_profile::Error<T>>::from)?;

			delegation::remove_delegate::<T>(&identifier, &profile_id, &delegate_profile_id)?;
			Self::deposit_event(Event::DelegateRemoved {
				identifier,
				delegate,
				delegate_profile_id,
			});

			Ok(())
		}

		/// Creates a new registry.
		///
		/// Initializes a new registry with the provided transaction hash and a optional blob.
		/// The creator must have a valid profile. The registry is created via the
		/// `registry::create_registry` function, assigned a unique SS58 identifier, and marked as
		/// active.
		///
		/// # Arguments
		/// * `origin` - The signed account creating the registry.
		/// * `tx_hash` - The hash of the registry’s content.
		/// * `_blob` - Optional data associated with the registry.
		///
		/// # Errors
		/// * `InvalidIdentifierLength` - If the generated registry ID is invalid.
		/// * `RegistryAlreadyExists` - If a registry with the same ID exists.
		/// * `pallet_profile::Error` - If the creator’s or author’s profile is invalid.
		///
		/// # Events
		/// * `RegistryCreated` - Emitted with `registry`, `creator`, `profile_id`.
		///
		/// ```
		#[pallet::call_index(2)]
		#[pallet::weight({10_000})]
		pub fn create(
			origin: OriginFor<T>,
			tx_hash: HashOf<T>,
			_blob: Option<RegistryBlobOf<T>>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&creator)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let registry_id = registry::create_registry::<T>(tx_hash, profile_id.clone())?;

			Self::deposit_event(Event::RegistryCreated {
				registry: registry_id,
				creator,
				profile_id,
			});

			Ok(())
		}

		/// Creates a new registry store with cyra based credentials.
		///
		/// Initializes a new registry store with the provided transaction hash and optional
		/// metadata (document ID, author, node ID). The creator must have a valid profile. The
		/// registry is created via the `registry::create_registry_store` function, assigned a
		/// unique SS58 identifier, and marked as active.
		///
		/// The key difference between create and create-store is that create-store is to map the
		/// registry data present in Cyra into Cord.
		///
		/// # Arguments
		/// * `origin` - The signed account creating the registry.
		/// * `tx_hash` - The hash of the registry’s content.
		/// * `_blob` - Data associated with the registry.
		/// * `doc_id` - Document identifier.
		/// * `doc_author_id` - Account ID of the document author.
		/// * `doc_node_id` - Document node identifier.
		///
		/// # Errors
		/// * `InvalidIdentifierLength` - If the generated registry ID is invalid.
		/// * `RegistryAlreadyExists` - If a registry with the same ID exists.
		/// * `pallet_profile::Error` - If the creator’s or author’s profile is invalid.
		///
		/// # Events
		/// * `RegistryStoreCreated` - Emitted with `registry`, `creator`, `profile_id`.
		///
		/// ```
		#[pallet::call_index(3)]
		#[pallet::weight({10_000})]
		pub fn create_store(
			origin: OriginFor<T>,
			tx_hash: HashOf<T>,
			doc_id: Vec<u8>,
			doc_author_id: CordAccountOf<T>,
			doc_node_id: Vec<u8>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&creator)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let doc_author_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&doc_author_id)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let registry_id = registry::create_registry_store::<T>(
				tx_hash,
				doc_id,
				doc_author_profile_id,
				doc_node_id,
				profile_id.clone(),
			)?;

			Self::deposit_event(Event::RegistryStoreCreated {
				registry: registry_id,
				creator,
				profile_id,
			});

			Ok(())
		}

		/// Archives a registry.
		///
		/// Marks a registry as archived, preventing further operations. The caller must have ADMIN
		/// permissions and a valid profile. The operation is performed via the
		/// `registry::archive_registry` function, and an activity is recorded.
		///
		/// # Arguments
		/// * `origin` - The signed account archiving the registry.
		/// * `registry_id` - The SS58 identifier of the registry.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks ADMIN permissions.
		/// * `RegistryNotFound` - If the registry does not exist.
		/// * `ArchivedRegistry` - If the registry is already archived.
		/// * `pallet_profile::Error` - If the caller’s profile is invalid.
		///
		/// # Events
		/// * `RegistryArchived` - Emitted with `registry`, `authority`, `authority_profile_id`.
		///
		/// ```
		#[pallet::call_index(4)]
		#[pallet::weight({10_000})]
		pub fn archive(origin: OriginFor<T>, registry_id: RegistryIdentifierOf) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			registry::archive_registry::<T>(&registry_id, &profile_id)?;
			Self::deposit_event(Event::RegistryArchived {
				registry: registry_id,
				authority: who,
				authority_profile_id: profile_id,
			});
			Ok(())
		}

		/// Restores an archived registry.
		///
		/// Restores a previously archived registry to active status, allowing operations. The
		/// caller must have ADMIN permissions and a valid profile. The operation is performed via
		/// the `registry::restore_registry` function, and an activity is recorded.
		///
		/// # Arguments
		/// * `origin` - The signed account restoring the registry.
		/// * `registry_id` - The SS58 identifier of the registry.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks ADMIN permissions.
		/// * `RegistryNotFound` - If the registry does not exist.
		/// * `RegistryNotArchived` - If the registry is not archived.
		/// * `pallet_profile::Error` - If the caller’s profile is invalid.
		///
		/// # Events
		/// * `RegistryRestored` - Emitted with `registry`, `authority`, `authority_profile_id`.
		///
		/// ```
		#[pallet::call_index(5)]
		#[pallet::weight({10_000})]
		pub fn restore(origin: OriginFor<T>, registry_id: RegistryIdentifierOf) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			registry::restore_registry::<T>(&registry_id, &profile_id.clone())?;
			Self::deposit_event(Event::RegistryRestored {
				registry: registry_id,
				authority: who,
				authority_profile_id: profile_id,
			});
			Ok(())
		}

		/// Updates the document author of a registry.
		///
		/// Changes the document author of the registry to a new account. The caller must have ADMIN
		/// permissions, and both the caller and new author must have valid profiles. The operation
		/// is performed via the `registry::update_registry_author` function, and an activity is
		/// recorded.
		///
		/// # Arguments
		/// * `origin` - The signed account updating the author.
		/// * `registry_id` - The SS58 identifier of the registry.
		/// * `new_doc_author_id` - The account ID of the new document author.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks ADMIN permissions.
		/// * `RegistryNotFound` - If the registry does not exist.
		/// * `pallet_profile::Error` - If the caller’s or new author’s profile is invalid.
		///
		/// # Events
		/// * `RegistryUpdated` - Emitted with `registry`, `authority` (new author),
		///   `authority_profile_id`.
		///
		/// ```
		#[pallet::call_index(6)]
		#[pallet::weight({10_000})]
		pub fn update_author(
			origin: OriginFor<T>,
			registry_id: RegistryIdentifierOf,
			new_doc_author_id: CordAccountOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let who_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let new_doc_author_profile_id =
				pallet_profile::Pallet::<T>::get_profile_id(&new_doc_author_id)
					.map_err(<pallet_profile::Error<T>>::from)?;

			registry::update_registry_author::<T>(
				&registry_id,
				new_doc_author_profile_id.clone(),
				who_profile_id.clone(),
			)?;

			Self::deposit_event(Event::RegistryUpdated {
				registry: registry_id,
				authority: new_doc_author_id,
				authority_profile_id: new_doc_author_profile_id,
			});

			Ok(())
		}

		/// Updates the creator of a registry.
		///
		/// Transfers ownership of the registry to a new account. The caller must have ADMIN
		/// permissions, and both the caller and new creator must have valid profiles. The operation
		/// is performed via the `registry::update_registry_creator` function, and an activity is
		/// recorded.
		///
		/// # Arguments
		/// * `origin` - The signed account updating the creator.
		/// * `registry_id` - The SS58 identifier of the registry.
		/// * `new_creator` - The account ID of the new creator.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks ADMIN permissions.
		/// * `RegistryNotFound` - If the registry does not exist.
		/// * `pallet_profile::Error` - If the caller’s or new creator’s profile is invalid.
		///
		/// # Events
		/// * `RegistryUpdated` - Emitted with `registry`, `authority` (new creator),
		///   `authority_profile_id`.
		///
		/// ```
		#[pallet::call_index(7)]
		#[pallet::weight({10_000})]
		pub fn update_creator(
			origin: OriginFor<T>,
			registry_id: RegistryIdentifierOf,
			new_creator: CordAccountOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let who_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			let new_creator_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&new_creator)
				.map_err(<pallet_profile::Error<T>>::from)?;

			registry::update_registry_creator::<T>(
				&registry_id,
				new_creator_profile_id.clone(),
				&who_profile_id,
			)?;

			Self::deposit_event(Event::RegistryUpdated {
				registry: registry_id,
				authority: new_creator,
				authority_profile_id: new_creator_profile_id,
			});
			Ok(())
		}

		/// Updates the transaction hash of a registry.
		///
		/// Updates the registry’s transaction hash and optionally its blob. The caller must have
		/// ADMIN permissions and a valid profile. The operation is performed via the
		/// `registry::update_registry_hash` function, and an activity is recorded.
		///
		/// # Arguments
		/// * `origin` - The signed account updating the hash.
		/// * `registry_id` - The SS58 identifier of the registry.
		/// * `tx_hash` - The new hash of the registry’s content.
		/// * `_blob` - Optional updated data.
		///
		/// # Errors
		/// * `UnauthorizedOperation` - If the caller lacks ADMIN permissions.
		/// * `RegistryNotFound` - If the registry does not exist.
		/// * `pallet_profile::Error` - If the caller’s profile is invalid.
		///
		/// # Events
		/// * `RegistryUpdated` - Emitted with `registry`, `authority`, `authority_profile_id`.
		///
		/// ```
		#[pallet::call_index(8)]
		#[pallet::weight({10_000})]
		pub fn update_registry_hash(
			origin: OriginFor<T>,
			registry_id: RegistryIdentifierOf,
			tx_hash: HashOf<T>,
			_blob: Option<RegistryBlobOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let profile_id = pallet_profile::Pallet::<T>::get_profile_id(&who)
				.map_err(<pallet_profile::Error<T>>::from)?;

			registry::update_registry_hash::<T>(&registry_id, &profile_id, &tx_hash)?;

			Self::deposit_event(Event::RegistryUpdated {
				registry: registry_id,
				authority: who,
				authority_profile_id: profile_id,
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

	/// Checks if the delegate for `who` on the given collection has any of the required
	/// permissions.
	pub fn has_permission(
		identifier: &Ss58Identifier,
		who: &ProfileIdOf,
		required: Permissions,
	) -> bool {
		Delegates::<T>::get(identifier, who)
			.unwrap_or(Permissions::empty())
			.intersects(required)
	}

	/// Helper function to encode an optional field into a byte buffer.
	/// Pushes a flag byte (1 if the field is present, 0 if not) followed by the field's bytes if
	/// present.
	pub fn push_option(buf: &mut Vec<u8>, field: Option<&[u8]>) {
		if let Some(bytes) = field {
			buf.push(1u8);
			buf.extend_from_slice(bytes);
		} else {
			buf.push(0u8);
		}
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

	/// Verifies if the given account for a registry has sufficient
	/// permissions and the registry is valid for Entry operations.
	/// Returns Ok(()) only if both conditions pass; fails if either fails.
	pub fn validate_registry_for_tx(
		profile_id: &ProfileIdOf,
		registry_id: &RegistryIdentifierOf,
	) -> DispatchResult {
		let has_permission =
			Self::has_permission(registry_id, profile_id, Permissions::ENTRY | Permissions::ADMIN);

		let registry_active = Self::inherent_ensure_active_registry(registry_id).is_ok();

		ensure!(has_permission && registry_active, Error::<T>::UnauthorizedOperation);

		Ok(())
	}

	/// Verifies if the given account Profile for a registry is admin or not.
	pub fn is_admin(profile_id: &ProfileIdOf, registry_id: &RegistryIdentifierOf) -> bool {
		if Self::has_permission(registry_id, profile_id, Permissions::ADMIN) {
			return true;
		}

		false
	}
}

/// A trait that can be used to ensure that a registry identifier exists and is active..
pub trait RegistryIdentifierCheck {
	/// Checks that the registry identified by `registry_id` exists and is active.
	fn ensure_active_registry(registry_id: &Ss58Identifier) -> DispatchResult;
}

impl<T: Config> RegistryIdentifierCheck for Pallet<T> {
	fn ensure_active_registry(registry_id: &Ss58Identifier) -> DispatchResult {
		Self::inherent_ensure_active_registry(registry_id)
	}
}
