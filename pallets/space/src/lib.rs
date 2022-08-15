// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
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

pub use cord_primitives::{ss58identifier, IdentifierOf, StatusOf, SPACE_INDEX};
use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_std::{prelude::Clone, str, vec::Vec};
pub mod space;
pub mod weights;

pub use crate::space::*;
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
	/// Type for a cord signature.
	pub type SignatureOf<T> = <T as Config>::Signature;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		#[pallet::constant]
		type MaxSpaceDelegates: Get<u32>;
		#[pallet::constant]
		type MaxSpaceSchemas: Get<u32>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer>
			+ Parameter
			+ MaxEncodedLen
			+ TypeInfo;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// space information stored on chain.
	/// It maps from an identifier to its details.
	#[pallet::storage]
	#[pallet::storage_prefix = "Identifiers"]
	pub type Spaces<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, SpaceDetails<T>, OptionQuery>;

	/// space stream identifiers stored on chain.
	/// It maps from hash to an identifier.
	#[pallet::storage]
	#[pallet::storage_prefix = "Hashes"]
	pub type SpaceHashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	/// space delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::storage_prefix = "Delegates"]
	pub(super) type SpaceDelegates<T: Config> = StorageMap<
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
		AddDelegates { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// Space delegates has been removed.
		/// \[space identifier,  controller\]
		RemoveDelegates { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A new space has been created.
		/// \[space hash, space identifier, controller\]
		Create { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A space controller has changed.
		/// \[space identifier, new controller\]
		Transfer { identifier: IdentifierOf, transfer: CordAccountOf<T>, author: CordAccountOf<T> },
		/// A space has been archived.
		/// \[space identifier\]
		Archive { identifier: IdentifierOf, author: CordAccountOf<T> },
		/// A space has been restored.
		/// \[space identifier\]
		Restore { identifier: IdentifierOf, author: CordAccountOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Space identifier is not unique
		SpaceAlreadyAnchored,
		/// Space identifier not found
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
		// Archived space
		ArchivedSpace,
		// Space not Archived
		SpaceNotArchived,
		// Invalid transaction hash
		InvalidTransactionHash,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller
		/// or delegates.
		///
		/// * origin: the identity of the space controller.
		/// * tx_space: space transaction details.
		/// * delegates: authorised identities to add.
		/// * tx_signature: creator signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::authorise())]
		pub fn delegate(
			origin: OriginFor<T>,
			tx_space: SpaceParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&tx_space.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&tx_space.space.digest).encode()[..], &tx_space.space.controller),
				Error::<T>::InvalidSignature
			);

			SpaceDetails::from_space_identities(
				&tx_space.identifier,
				tx_space.space.controller.clone(),
			)
			.map_err(Error::<T>::from)?;

			SpaceDelegates::<T>::try_mutate(tx_space.identifier.clone(), |ref mut delegation| {
				ensure!(
					delegation.len() + delegates.len() <= T::MaxSpaceDelegates::get() as usize,
					Error::<T>::TooManyDelegates
				);
				for delegate in delegates {
					delegation
						.try_push(delegate)
						.expect("delegates length is less than T::MaxCollectionDelegates; qed");
				}

				<SpaceHashes<T>>::insert(&tx_space.space.digest, &tx_space.identifier);

				Self::deposit_event(Event::AddDelegates {
					identifier: tx_space.identifier,
					digest: tx_space.space.digest,
					author: tx_space.space.controller,
				});

				Ok(())
			})
		}
		/// Remove space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller
		/// or delegates.
		///
		/// * origin: the identity of the space controller.
		/// * auth: space transaction details.
		/// * delegates: authorised identities to add.
		/// * tx_signature: creator signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::deauthorise())]
		pub fn undelegate(
			origin: OriginFor<T>,
			tx_space: SpaceParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&tx_space.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&tx_space.space.digest).encode()[..], &tx_space.space.controller),
				Error::<T>::InvalidSignature
			);

			SpaceDetails::from_space_identities(
				&tx_space.identifier,
				tx_space.space.controller.clone(),
			)
			.map_err(Error::<T>::from)?;

			SpaceDelegates::<T>::try_mutate(tx_space.identifier.clone(), |ref mut delegation| {
				for delegate in delegates {
					delegation.retain(|x| x != &delegate);
				}

				<SpaceHashes<T>>::insert(&tx_space.space.digest, &tx_space.identifier);

				Self::deposit_event(Event::RemoveDelegates {
					identifier: tx_space.identifier,
					digest: tx_space.space.digest,
					author: tx_space.space.controller,
				});

				Ok(())
			})
		}

		/// Create a new space and associates with its identifier.
		///
		/// * origin: the identity of the space controller.
		/// * space: incoming space stream.
		/// * tx_signature: creator signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			tx_space: SpaceType<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&tx_space.digest).encode()[..], &tx_space.controller),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(&tx_space.digest).encode()[..], SPACE_INDEX)
					.into_bytes(),
			)
			.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Spaces<T>>::contains_key(&identifier), Error::<T>::SpaceAlreadyAnchored);

			<SpaceHashes<T>>::insert(&tx_space.digest, &identifier);

			<Spaces<T>>::insert(
				&identifier,
				SpaceDetails {
					space: tx_space.clone(),
					schema: None,
					archived: false,
					metadata: false,
				},
			);
			Self::deposit_event(Event::Create {
				identifier,
				digest: tx_space.digest,
				author: tx_space.controller,
			});

			Ok(())
		}
		/// Archive a space
		///
		///This transaction can only be performed by the space controller
		/// or delegates
		///
		/// * origin: the identity of the space controller.
		/// * arch: space params to archive.
		/// * tx_signature: updater signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::archive())]
		pub fn archive(
			origin: OriginFor<T>,
			tx_space: SpaceParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&tx_space.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&tx_space.space.digest).encode()[..], &tx_space.space.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_space.identifier, SPACE_INDEX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let registry_details =
				<Spaces<T>>::get(&tx_space.identifier).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!registry_details.archived, Error::<T>::ArchivedSpace);

			SpaceDetails::from_space_delegates(
				&tx_space.identifier,
				tx_space.space.controller.clone(),
				registry_details.space.controller.clone(),
			)
			.map_err(<Error<T>>::from)?;

			<SpaceHashes<T>>::insert(&tx_space.space.digest, &tx_space.identifier);

			<Spaces<T>>::insert(
				&tx_space.identifier,
				SpaceDetails { archived: true, ..registry_details },
			);
			Self::deposit_event(Event::Archive {
				identifier: tx_space.identifier,
				author: tx_space.space.controller,
			});

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
		#[pallet::weight(<T as pallet::Config>::WeightInfo::restore())]
		pub fn restore(
			origin: OriginFor<T>,
			tx_space: SpaceParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&tx_space.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&tx_space.space.digest).encode()[..], &tx_space.space.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_space.identifier, SPACE_INDEX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let registry_details =
				<Spaces<T>>::get(&tx_space.identifier).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!registry_details.archived, Error::<T>::ArchivedSpace);

			SpaceDetails::from_space_delegates(
				&tx_space.identifier,
				tx_space.space.controller.clone(),
				registry_details.space.controller.clone(),
			)
			.map_err(<Error<T>>::from)?;

			<SpaceHashes<T>>::insert(&tx_space.space.digest, &tx_space.identifier);

			<Spaces<T>>::insert(
				&tx_space.identifier,
				SpaceDetails { archived: false, ..registry_details },
			);
			Self::deposit_event(Event::Archive {
				identifier: tx_space.identifier,
				author: tx_space.space.controller,
			});

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
		#[pallet::weight(<T as pallet::Config>::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			tx_space: SpaceParams<T>,
			transfer_to: CordAccountOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&tx_space.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&tx_space.space.digest).encode()[..], &tx_space.space.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_space.identifier, SPACE_INDEX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let registry_details =
				<Spaces<T>>::get(&tx_space.identifier).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!registry_details.archived, Error::<T>::ArchivedSpace);

			SpaceDetails::from_space_delegates(
				&tx_space.identifier,
				tx_space.space.controller.clone(),
				registry_details.space.controller.clone(),
			)
			.map_err(<Error<T>>::from)?;

			<Spaces<T>>::insert(
				&tx_space.identifier,
				SpaceDetails {
					archived: false,
					space: {
						SpaceType { controller: transfer_to.clone(), ..registry_details.space }
					},
					..registry_details
				},
			);
			Self::deposit_event(Event::Transfer {
				identifier: tx_space.identifier,
				transfer: transfer_to,
				author: tx_space.space.controller,
			});

			Ok(())
		}
	}
}
