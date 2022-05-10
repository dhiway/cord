// CORD Chain Node – https://cord.network
// Copyright (C) 2019-2022 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
pub use cord_primitives::{mark, CidOf, IdentifierOf, StatusOf, VersionOf};
use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};

pub mod spaces;
pub mod weights;

pub use crate::spaces::*;
use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Hash of the space.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	// space identifier prefix.
	pub const SPACE_IDENTIFIER_PREFIX: u16 = 1;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// The maximum number of delegates for a space.
		#[pallet::constant]
		type MaxDelegates: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// schemas stored on chain.
	/// It maps from a space identifier to its details.
	#[pallet::storage]
	#[pallet::getter(fn schemas)]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, IdentifierOf, CordAccountOf<T>>;

	/// space delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::getter(fn delegations)]
	pub(super) type Delegations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<CordAccountOf<T>, T::MaxDelegates>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new space has been created.
		/// \[space hash, space identifier, controller\]
		Create(HashOf<T>, IdentifierOf, CordAccountOf<T>),
		/// A space controller has changed.
		/// \[space identifier, new controller\]
		Transfer(IdentifierOf, CordAccountOf<T>),
		/// Space delegates has been added.
		/// \[space identifier,  controller\]
		AddDelegates(IdentifierOf, CordAccountOf<T>),
		/// Space delegates has been removed.
		/// \[space identifier,  controller\]
		RemoveDelegates(IdentifierOf, CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Space idenfier is not unique
		SpaceAlreadyAnchored,
		/// Space idenfier not found
		SpaceNotFound,
		/// Only when the author is not the controller.
		UnauthorizedOperation,
		// Maximum Number of delegates reached.
		TooManyDelegates,
		// Only when the author is not the controller
		UnauthorizedDelegation,
		// Invalid Identifier
		InvalidIdentifier,
		// Invalid Identifier Length
		InvalidIdentifierLength,
		// Invalid Identifier Prefix
		InvalidIdentifierPrefix,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller or
		/// delegated identities.
		///
		/// * origin: the identity of the space controller.
		/// * space: unique identifier of the space.
		/// * delegates: authorised identities to add.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn authorise(
			origin: OriginFor<T>,
			space: IdentifierOf,
			delegates: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			SpaceDetails::from_known_identities(&space, controller.clone())
				.map_err(Error::<T>::from)?;

			Delegations::<T>::try_mutate(space.clone(), |ref mut delegation| {
				ensure!(
					delegation.len() + delegates.len() <= T::MaxDelegates::get() as usize,
					Error::<T>::TooManyDelegates
				);
				for delegate in delegates {
					delegation
						.try_push(delegate)
						.expect("delegates length is less than T::MaxDelegates; qed");
				}

				Self::deposit_event(Event::AddDelegates(space, controller));
				Ok(())
			})
		}
		/// Remove space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller or
		/// delegated identities.
		///
		/// * origin: the identity of the space controller.
		/// * space: unique identifier of the space.
		/// * delegates: identities (delegates) to be removed.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn deauthorise(
			origin: OriginFor<T>,
			space: IdentifierOf,
			delegates: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			SpaceDetails::from_known_identities(&space, controller.clone())
				.map_err(<Error<T>>::from)?;

			Delegations::<T>::try_mutate(space.clone(), |ref mut delegation| {
				for delegate in delegates {
					delegation.retain(|x| x != &delegate);
				}

				Self::deposit_event(Event::RemoveDelegates(space, controller));
				Ok(())
			})
		}

		/// Create a new space and associates with its identifier.
		///
		/// * origin: the identity of the space controller.
		/// * space_hash: hash of the incoming space stream.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn create(origin: OriginFor<T>, space_hash: HashOf<T>) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let identifier: IdentifierOf =
				mark::generate(&(&space_hash).encode()[..], SPACE_IDENTIFIER_PREFIX).into_bytes();

			ensure!(!<Spaces<T>>::contains_key(&identifier), Error::<T>::SpaceAlreadyAnchored);

			<Spaces<T>>::insert(&identifier, controller.clone());
			Self::deposit_event(Event::Create(space_hash, identifier, controller));

			Ok(())
		}
		/// Transfer the space to a new controller.
		///
		///This transaction can only be performed by the space controller
		///
		/// * origin: the identity of the space controller.
		/// * identifier: unique identifier of the incoming space stream.
		/// * transfer_to: new controller of the space.
		#[pallet::weight(50_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn transfer(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			transfer_to: CordAccountOf<T>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			mark::from_known_format(&identifier, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidIdentifier)?;

			let space_controller =
				<Spaces<T>>::get(&identifier).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(space_controller == controller, Error::<T>::UnauthorizedOperation);

			<Spaces<T>>::insert(&identifier, transfer_to.clone());

			Self::deposit_event(Event::Transfer(identifier, transfer_to));
			Ok(())
		}
	}
}
