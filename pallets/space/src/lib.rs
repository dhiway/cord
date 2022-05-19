// This file is part of Cord – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// Cord is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cord is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cord. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![warn(unused_crate_dependencies)]

pub use cord_primitives::{mark, IdentifierOf, StatusOf};
use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
use sp_runtime::traits::{IdentifyAccount, Verify};
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
	pub const SPACE_IDENTIFIER_PREFIX: u16 = 13;
	/// Type for a cord signature.
	pub type SignatureOf<T> = <T as Config>::Signature;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// The maximum number of delegates for a space.
		#[pallet::constant]
		type MaxSpaceDelegates: Get<u32>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer> + Parameter;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// spacess stored on chain.
	/// It maps from a space identifier to its details.
	#[pallet::storage]
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, SpaceDetails<T>, OptionQuery>;

	/// space delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::getter(fn delegations)]
	pub(super) type Delegations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<CordAccountOf<T>, T::MaxSpaceDelegates>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Space delegates has been added.
		/// \[space identifier,  controller\]
		AddDelegates(IdentifierOf, CordAccountOf<T>),
		/// Space delegates has been removed.
		/// \[space identifier,  controller\]
		RemoveDelegates(IdentifierOf, CordAccountOf<T>),
		/// A new space has been created.
		/// \[space hash, space identifier, controller\]
		Create(HashOf<T>, IdentifierOf, CordAccountOf<T>),
		/// A space controller has changed.
		/// \[space identifier, new controller\]
		Transfer(IdentifierOf, CordAccountOf<T>),
		/// A spaces has been archived.
		/// \[space identifier\]
		Archive(IdentifierOf, CordAccountOf<T>),
		/// A spaces has been restored.
		/// \[space identifier\]
		Restore(IdentifierOf, CordAccountOf<T>),
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
		InvalidSpaceIdentifier,
		// Invalid Identifier Length
		InvalidIdentifierLength,
		// Invalid Identifier Prefix
		InvalidIdentifierPrefix,
		// Invalid creator signature
		InvalidSignature,
		// Archived Space
		ArchivedSpace,
		// Space Archived
		SpaceAlreadyArchived,
		// Space not Archived
		SpaceNotArchived,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller or
		/// delegates.
		///
		/// * origin: the identity of the space controller.
		/// * creator: creator (controller) of the space.
		/// * space: unique identifier of the space.
		/// * tx_hash: transaction hash to verify the signature.
		/// * delegates: authorised identities to add.
		/// * tx_signature: creator signature.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn authorise(
			origin: OriginFor<T>,
			creator: CordAccountOf<T>,
			space: IdentifierOf,
			tx_hash: HashOf<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				tx_signature.verify(&(&tx_hash).encode()[..], &creator),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&space, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			SpaceDetails::from_space_identities(&space, creator.clone())
				.map_err(Error::<T>::from)?;

			Delegations::<T>::try_mutate(space.clone(), |ref mut delegation| {
				ensure!(
					delegation.len() + delegates.len() <= T::MaxSpaceDelegates::get() as usize,
					Error::<T>::TooManyDelegates
				);
				for delegate in delegates {
					delegation
						.try_push(delegate)
						.expect("delegates length is less than T::MaxDelegates; qed");
				}

				Self::deposit_event(Event::AddDelegates(space, creator));
				Ok(())
			})
		}
		/// Remove space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller or
		/// delegates.
		///
		/// * origin: the identity of the space controller.
		/// * updater: updater (controller) of the space.
		/// * space: unique identifier of the space.
		/// * tx_hash: transaction hash to verify the signature.
		/// * delegates: identities (delegates) to be removed.
		/// * tx_signature: updater signature.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn deauthorise(
			origin: OriginFor<T>,
			updater: CordAccountOf<T>,
			space: IdentifierOf,
			tx_hash: HashOf<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&tx_hash).encode()[..], &updater),
				Error::<T>::InvalidSignature
			);
			mark::from_known_format(&space, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			SpaceDetails::from_space_identities(&space, updater.clone())
				.map_err(<Error<T>>::from)?;

			Delegations::<T>::try_mutate(space.clone(), |ref mut delegation| {
				for delegate in delegates {
					delegation.retain(|x| x != &delegate);
				}

				Self::deposit_event(Event::RemoveDelegates(space, updater));
				Ok(())
			})
		}

		/// Create a new space and associates with its identifier.
		///
		/// * origin: the identity of the space controller.
		/// * creator: creator (controller) of the space.
		/// * space_hash: hash of the incoming space stream.
		/// * tx_signature: creator signature.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn create(
			origin: OriginFor<T>,
			creator: CordAccountOf<T>,
			space_hash: HashOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&space_hash).encode()[..], &creator),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf =
				mark::generate(&(&space_hash).encode()[..], SPACE_IDENTIFIER_PREFIX).into_bytes();

			ensure!(!<Spaces<T>>::contains_key(&identifier), Error::<T>::SpaceAlreadyAnchored);

			<Spaces<T>>::insert(
				&identifier,
				SpaceDetails { space_hash, controller: creator.clone(), archived: false },
			);
			Self::deposit_event(Event::Create(space_hash, identifier, creator));

			Ok(())
		}
		/// Archive a Space
		///
		///This transaction can only be performed by the space controller or
		/// delegates
		///
		/// * origin: the identity of the space controller.
		/// * updater: updater (controller) of the space.
		/// * identifier: unique identifier of the space.
		/// * tx_hash: transaction hash to verify the signature.
		/// * tx_signature: updater signature.
		#[pallet::weight(20_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn archive(
			origin: OriginFor<T>,
			updater: CordAccountOf<T>,
			space: IdentifierOf,
			tx_hash: HashOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&tx_hash).encode()[..], &updater),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&space, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let space_details = <Spaces<T>>::get(&space).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(space_details.archived, Error::<T>::SpaceAlreadyArchived);

			if space_details.controller != updater {
				let delegates = <Delegations<T>>::get(&space);
				ensure!(
					(delegates.iter().find(|&delegate| *delegate == updater) == Some(&updater)),
					Error::<T>::UnauthorizedOperation
				);
			} else {
				ensure!(space_details.controller == updater, Error::<T>::UnauthorizedOperation);
			}

			<Spaces<T>>::insert(&space, SpaceDetails { archived: true, ..space_details });
			Self::deposit_event(Event::Archive(space, updater));

			Ok(())
		}
		/// Restore an archived space
		///
		/// This transaction can only be performed by the space controller or
		/// delegates
		///
		/// * origin: the identity of the space controller.
		/// * updater: updater (controller) of the space.
		/// * identifier: unique identifier of the space.
		/// * tx_hash: transaction hash to verify the signature.
		/// * tx_signature: updater signature.
		#[pallet::weight(20_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn restore(
			origin: OriginFor<T>,
			updater: CordAccountOf<T>,
			space: IdentifierOf,
			tx_hash: HashOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&tx_hash).encode()[..], &updater),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&space, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let space_details = <Spaces<T>>::get(&space).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archived, Error::<T>::SpaceNotArchived);

			if space_details.controller != updater {
				let delegates = <Delegations<T>>::get(&space);
				ensure!(
					(delegates.iter().find(|&delegate| *delegate == updater) == Some(&updater)),
					Error::<T>::UnauthorizedOperation
				);
			} else {
				ensure!(space_details.controller == updater, Error::<T>::UnauthorizedOperation);
			}

			<Spaces<T>>::insert(&space, SpaceDetails { archived: false, ..space_details });
			Self::deposit_event(Event::Restore(space, updater));

			Ok(())
		}
		/// Transfer an active space to a new controller.
		///
		///This transaction can only be performed by the space controller
		///
		/// * origin: the identity of the space controller.
		/// * updater: updater (controller) of the space.
		/// * identifier: unique identifier of the incoming space stream.
		/// * transfer_to: new controller of the space.
		/// * tx_hash: transaction hash to verify the signature.
		/// * tx_signature: creator signature.
		#[pallet::weight(50_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn transfer(
			origin: OriginFor<T>,
			space: IdentifierOf,
			updater: CordAccountOf<T>,
			transfer_to: CordAccountOf<T>,
			tx_hash: HashOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&tx_hash).encode()[..], &updater),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&space, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let space_details = <Spaces<T>>::get(&space).ok_or(Error::<T>::SpaceNotFound)?;
			if space_details.controller != updater {
				SpaceDetails::<T>::from_space_identities(&space, updater.clone())
					.map_err(<Error<T>>::from)?;
			}

			<Spaces<T>>::insert(
				&space,
				SpaceDetails { controller: transfer_to.clone(), ..space_details },
			);

			Self::deposit_event(Event::Transfer(space, transfer_to));
			Ok(())
		}
	}
}
