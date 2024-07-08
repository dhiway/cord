// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2024 Dhiway Networks Pvt. Ltd.
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

pub mod types;

pub use crate::{pallet::*, types::*};
use frame_support::{ensure, traits::Get};
use frame_system::WeightInfo;
use identifier::types::Timepoint;
use pallet_chain_space::AuthorizationIdOf;
use sp_runtime::traits::UniqueSaturatedInto;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};
	use sp_runtime::{traits::Hash, BoundedVec};
	use sp_std::{prelude::Clone, str};

	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

	// Type of a document identifier.
	pub type StoreEntryIdOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;

	// Type of store entry creator
	pub type StoreEntryCreatorOf<T> = pallet_chain_space::SpaceCreatorOf<T>;

	// Type of store entry containing store entry details.
	pub type StoreEntryOf<T> =
		StoreEntry<StoreEntryCreatorOf<T>, EntryHashOf<T>, StoreEntryStateOf, BlockNumberFor<T>>;

	// Type of Raw Data with max length of 1MB.
	pub type RawDataOf<T> = BoundedVec<u8, <T as Config>::MaxRawDataValueLength>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_chain_space::Config + identifier::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;

		type OriginSuccess: CallSources<AccountIdOf<Self>, StoreEntryCreatorOf<Self>>;

		#[pallet::constant]
		type MaxEncodedValueLength: Get<u32>;

		#[pallet::constant]
		type MaxRawDataValueLength: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::storage]
	pub type Store<T> =
		StorageMap<_, Blake2_128Concat, StoreEntryIdOf<T>, StoreEntryOf<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new store entry has been added.
		/// \[store entry identifier, creator, digest\]
		StoreEntryAdded {
			identifier: StoreEntryIdOf<T>,
			creator: StoreEntryCreatorOf<T>,
			digest: EntryHashOf<T>,
		},

		/// Existing identifider has been updated with newer store entry.
		/// \[store entry identifier, creator, digest\]
		StoreEntryUpdated {
			identifier: StoreEntryIdOf<T>,
			creator: StoreEntryCreatorOf<T>,
			digest: EntryHashOf<T>,
		},

		/// Existing identifier has been removed from the store.
		/// \[store entry identifier\]
		StoreEntryRedacted { identifier: StoreEntryIdOf<T>, creator: StoreEntryCreatorOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unauthorized operation
		UnauthorizedOperation,
		/// Store Entry Id not found in the storage
		StoreEntryIdNotFound,
		/// Store Entry Id already exits
		StoreEntryIdAlreadyExists,
		/// Raw Data and digest provided should be same,
		RawDataAndDigestDoNotMatch,
		/// Entry digest during updation should not already exist,
		EntryDigestAlreadyAnchored,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Creates a new store entry into the chain.
		///
		/// This function allows a user to submit a new store entry document.
		/// The store entry hash is recorded along with various metadata, including the
		/// author of the store entry, state of the entry and block number.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `identifier` - The identifier is the unique identifier of the store entry.
		/// * `digest` - A hash representing some unique aspects of the document, used for
		///   identification and integrity purposes.
		/// * `authorization` - An identifier for authorization, used to validate the origin's
		///   permission to make this rating.
		///
		/// # Errors
		/// Returns `Error::<T>::RawDataAndDigestDoNotMatch` if the given digest and actual entry
		/// digest do not match.
		/// Returns `Error::<T>::StoreEntryIdAlreadyExists` if the store entry has been already
		/// made with the given identifier.
		///
		/// # Events
		/// Emits `StoreEntryAdded` when a store entry has been successfully created.
		///
		/// # Example
		/// ```
		/// add(origin, identifier, digest, raw_data, authorization)?;
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn add(
			origin: OriginFor<T>,
			identifier: StoreEntryIdOf<T>,
			digest: EntryHashOf<T>,
			raw_data: RawDataOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let _space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator.clone(),
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			/* Check if the input hash and Raw Data matches */
			let raw_data_digest =
				<T as frame_system::Config>::Hashing::hash(&raw_data.encode()[..]);

			let _raw_data_digest_without_encode =
				<T as frame_system::Config>::Hashing::hash(&raw_data);

			/* Ensure given hash and actual hash of the entry matches */
			ensure!(digest == raw_data_digest, Error::<T>::RawDataAndDigestDoNotMatch);

			/* Ensure given identifier do not already exist */
			ensure!(!<Store<T>>::contains_key(&identifier), Error::<T>::StoreEntryIdAlreadyExists);

			let block_number = frame_system::Pallet::<T>::block_number();

			<Store<T>>::insert(
				&identifier,
				StoreEntryOf::<T> {
					entry_creator: creator.clone(),
					digest,
					entry_state: StoreEntryStateOf::ACTIVE,
					created_at: block_number,
				},
			);

			Self::deposit_event(Event::StoreEntryAdded { identifier, creator, digest });

			Ok(())
		}

		/// Updates the existing store entry in the chain.
		/// The operation is supposed to made by the same creator of the store entry.
		///
		/// This function allows a user to submit a new store update for the existing
		/// entry document.
		/// The store entry is updated with the newer entry such as digest, block number
		/// whereas the creator and identifier will remain the same.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `identifier` - The identifier is the unique identifier of the store entry to be
		///   updated.
		/// * `digest` - A hash representing some unique aspects of the document, used for
		///   identification and integrity purposes. The
		/// * `authorization` - An identifier for authorization, used to validate the origin's
		///   permission to make this rating.
		///
		/// # Errors
		/// Returns `Error::<T>::StoreEntryIdNotFound` if the given identifier do not exist.
		/// Returns `Error::<T>::UnauthorizedOperation` if the store entry updation is made by
		/// unauthorized author.
		/// Returns `Error::<T>::EntryDigestAlreadyAnchored` if the store entry digest already
		/// exists and no need of updation.
		///
		/// # Events
		/// Emits `StoreEntryUpdated` when a store entry has been successfully updated.
		///
		/// # Example
		/// ```
		/// update(origin, identifier, digest, authorization)?;
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn update(
			origin: OriginFor<T>,
			identifier: StoreEntryIdOf<T>,
			digest: EntryHashOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let _space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator.clone(),
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let store_entry =
				<Store<T>>::get(&identifier).ok_or(Error::<T>::StoreEntryIdNotFound)?;

			// TODO:
			/* Ensure not to update the store_entry when the status is INACTIVE */

			/* Ensure the data is updated by the original creator */
			ensure!(store_entry.entry_creator != creator, Error::<T>::UnauthorizedOperation);

			/* Ensure the entry update hash is not the same as existing hash */
			ensure!(store_entry.digest != digest, Error::<T>::EntryDigestAlreadyAnchored);

			let block_number = frame_system::Pallet::<T>::block_number();

			<Store<T>>::insert(
				&identifier,
				StoreEntryOf::<T> { digest, created_at: block_number, ..store_entry },
			);

			Self::deposit_event(Event::StoreEntryUpdated { identifier, creator, digest });

			Ok(())
		}

		/// Redacts the existing store entry in the chain.
		/// The operation is supposed to made by the same creator of the store entry.
		///
		/// This function allows a user to delete a existing store entry from the chain.
		/// The store entry is deleted from the storage of the chain.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `identifier` - The identifier is the unique identifier of the store entry to be
		///   updated.
		///
		/// # Errors
		/// Returns `Error::<T>::StoreEntryIdNotFound` if the given identifier do not exist.
		/// Returns `Error::<T>::UnauthorizedOperation` if the store entry deletion is made by
		/// unauthorized author.
		///
		/// # Events
		/// Emits `StoreEntryRedacted` when a store entry has been successfully deleted.
		///
		/// # Example
		/// ```
		/// redact(origin, identifier)?;
		/// ```
		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn redact(origin: OriginFor<T>, identifier: StoreEntryIdOf<T>) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let store_entry =
				<Store<T>>::get(&identifier).ok_or(Error::<T>::StoreEntryIdNotFound)?;

			/* Ensure the data is redacted only by original creator */
			ensure!(creator == store_entry.entry_creator, Error::<T>::UnauthorizedOperation);

			/* Delete the store entry */
			<Store<T>>::remove(&identifier);

			Self::deposit_event(Event::StoreEntryRedacted { identifier, creator: creator.clone() });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
