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

pub use cord_primitives::{ss58identifier, IdentifierOf, StatusOf};
use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_std::{prelude::Clone, str, vec::Vec};
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
	pub const SPACE_IDENTIFIER_PREFIX: u16 = 31;
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

	/// spacess stored on chain.
	/// It maps from a space identifier to its details.
	#[pallet::storage]
	#[pallet::storage_prefix = "Identifiers"]
	pub type Spaces<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, SpaceDetails<T>, OptionQuery>;

	/// schema identifiers stored on chain.
	/// It maps from a schema identifier to hash.
	#[pallet::storage]
	#[pallet::storage_prefix = "Hashes"]
	pub type SpaceHashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	/// space delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::storage_prefix = "Delegates"]
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
		AddDelegates { identifier: IdentifierOf, hash: HashOf<T>, author: CordAccountOf<T> },
		/// Space delegates has been removed.
		/// \[space identifier,  controller\]
		RemoveDelegates { identifier: IdentifierOf, hash: HashOf<T>, author: CordAccountOf<T> },
		/// A new space has been created.
		/// \[space hash, space identifier, controller\]
		Create { identifier: IdentifierOf, hash: HashOf<T>, author: CordAccountOf<T> },
		/// A space controller has changed.
		/// \[space identifier, new controller\]
		Transfer { identifier: IdentifierOf, transfer: CordAccountOf<T>, author: CordAccountOf<T> },
		/// A spaces has been archived.
		/// \[space identifier\]
		Archive { identifier: IdentifierOf, author: CordAccountOf<T> },
		/// A spaces has been restored.
		/// \[space identifier\]
		Restore { identifier: IdentifierOf, author: CordAccountOf<T> },
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
		// Invalid transaction hash
		InvalidTransactionHash,
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
			auth: SpaceParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&auth.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&auth.space.digest).encode()[..], &auth.space.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&auth.identifier, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			SpaceDetails::from_space_identities(&auth.identifier, auth.space.controller.clone())
				.map_err(Error::<T>::from)?;

			Delegations::<T>::try_mutate(auth.identifier.clone(), |ref mut delegation| {
				ensure!(
					delegation.len() + delegates.len() <= T::MaxSpaceDelegates::get() as usize,
					Error::<T>::TooManyDelegates
				);
				for delegate in delegates {
					delegation
						.try_push(delegate)
						.expect("delegates length is less than T::MaxDelegates; qed");
				}

				<SpaceHashes<T>>::insert(&auth.space.digest, &auth.identifier);

				Self::deposit_event(Event::AddDelegates {
					identifier: auth.identifier,
					hash: auth.space.digest,
					author: auth.space.controller,
				});

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
			deauth: SpaceParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&deauth.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&deauth.space.digest).encode()[..], &deauth.space.controller),
				Error::<T>::InvalidSignature
			);
			ss58identifier::from_known_format(&deauth.identifier, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			SpaceDetails::from_space_identities(
				&deauth.identifier,
				deauth.space.controller.clone(),
			)
			.map_err(<Error<T>>::from)?;

			Delegations::<T>::try_mutate(deauth.identifier.clone(), |ref mut delegation| {
				for delegate in delegates {
					delegation.retain(|x| x != &delegate);
				}

				<SpaceHashes<T>>::insert(&deauth.space.digest, &deauth.identifier);

				Self::deposit_event(Event::RemoveDelegates {
					identifier: deauth.identifier,
					hash: deauth.space.digest,
					author: deauth.space.controller,
				});

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
			space: SpaceType<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&space.digest).encode()[..], &space.controller),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(&space.digest).encode()[..], SPACE_IDENTIFIER_PREFIX)
					.into_bytes(),
			)
			.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Spaces<T>>::contains_key(&identifier), Error::<T>::SpaceAlreadyAnchored);

			<SpaceHashes<T>>::insert(&space.digest, &identifier);

			<Spaces<T>>::insert(
				&identifier,
				SpaceDetails { space: space.clone(), archived: false },
			);
			Self::deposit_event(Event::Create {
				identifier,
				hash: space.digest,
				author: space.controller,
			});

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
			arch: SpaceParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&arch.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&arch.space.digest).encode()[..], &arch.space.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&arch.identifier, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let space_details =
				<Spaces<T>>::get(&arch.identifier).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.archived, Error::<T>::SpaceAlreadyArchived);

			if space_details.space.controller != arch.space.controller {
				let delegates = <Delegations<T>>::get(&arch.identifier);
				ensure!(
					(delegates.iter().find(|&delegate| *delegate == arch.space.controller)
						== Some(&arch.space.controller)),
					Error::<T>::UnauthorizedOperation
				);
			} else {
				ensure!(
					space_details.space.controller == arch.space.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			<Spaces<T>>::insert(&arch.identifier, SpaceDetails { archived: true, ..space_details });
			Self::deposit_event(Event::Archive {
				identifier: arch.identifier,
				author: arch.space.controller,
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
		#[pallet::weight(20_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn restore(
			origin: OriginFor<T>,
			resto: SpaceParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&resto.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&resto.space.digest).encode()[..], &resto.space.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&resto.identifier, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let space_details =
				<Spaces<T>>::get(&resto.identifier).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(space_details.archived, Error::<T>::SpaceNotArchived);

			if space_details.space.controller != resto.space.controller {
				let delegates = <Delegations<T>>::get(&resto.identifier);
				ensure!(
					(delegates.iter().find(|&delegate| *delegate == resto.space.controller)
						== Some(&resto.space.controller)),
					Error::<T>::UnauthorizedOperation
				);
			} else {
				ensure!(
					space_details.space.controller == resto.space.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			<SpaceHashes<T>>::insert(&resto.space.digest, &resto.identifier);

			<Spaces<T>>::insert(
				&resto.identifier,
				SpaceDetails { archived: false, ..space_details },
			);
			Self::deposit_event(Event::Archive {
				identifier: resto.identifier,
				author: resto.space.controller,
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
		#[pallet::weight(50_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn transfer(
			origin: OriginFor<T>,
			trans: SpaceParams<T>,
			transfer_to: CordAccountOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SpaceHashes<T>>::contains_key(&trans.space.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&trans.space.digest).encode()[..], &trans.space.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&trans.identifier, SPACE_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let space_details =
				<Spaces<T>>::get(&trans.identifier).ok_or(Error::<T>::SpaceNotFound)?;

			ensure!(!space_details.archived, Error::<T>::ArchivedSpace);

			if space_details.space.controller != trans.space.controller {
				SpaceDetails::<T>::from_space_identities(
					&trans.identifier,
					trans.space.controller.clone(),
				)
				.map_err(<Error<T>>::from)?;
			}

			<Spaces<T>>::insert(
				&trans.identifier,
				SpaceDetails {
					archived: false,
					space: { SpaceType { controller: transfer_to.clone(), ..space_details.space } },
				},
			);
			Self::deposit_event(Event::Transfer {
				identifier: trans.identifier,
				transfer: transfer_to,
				author: trans.space.controller,
			});

			Ok(())
		}
	}
}
