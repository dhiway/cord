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

pub use cord_primitives::{ss58identifier, IdentifierOf, StatusOf, REGISTRY_INDEX};
use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_std::{prelude::Clone, str, vec::Vec};
pub mod registry;
pub mod weights;

pub use crate::registry::*;
use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Hash of the registry.
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
		type MaxRegistryDelegates: Get<u32>;
		#[pallet::constant]
		type MaxRegistrySchemas: Get<u32>;
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

	/// registry information stored on chain.
	/// It maps from an identifier to its details.
	#[pallet::storage]
	#[pallet::storage_prefix = "Identifiers"]
	pub type Registries<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, RegistryDetails<T>, OptionQuery>;

	/// registry stream identifiers stored on chain.
	/// It maps from hash to an identifier.
	#[pallet::storage]
	#[pallet::storage_prefix = "Hashes"]
	pub type RegistryHashes<T> =
		StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	/// registry delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::storage_prefix = "Delegates"]
	pub(super) type RegistryDelegations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<CordAccountOf<T>, T::MaxRegistryDelegates>,
		ValueQuery,
	>;

	// /// registry delegations stored on chain.
	// /// It maps from an identifier to a vector of delegates.
	// #[pallet::storage]
	// #[pallet::storage_prefix = "Schemas"]
	// pub(super) type RegistrySchemas<T: Config> = StorageMap<
	// 	_,
	// 	Blake2_128Concat,
	// 	IdentifierOf,
	// 	BoundedVec<IdentifierOf, T::MaxRegistrySchemas>,
	// 	ValueQuery,
	// >;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Registry delegates has been added.
		/// \[registry identifier,  controller\]
		AddDelegates { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// Registry delegates has been removed.
		/// \[registry identifier,  controller\]
		RemoveDelegates { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A new registry has been created.
		/// \[registry hash, registry identifier, controller\]
		Create { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A registry controller has changed.
		/// \[registry identifier, new controller\]
		Transfer { identifier: IdentifierOf, transfer: CordAccountOf<T>, author: CordAccountOf<T> },
		/// A registry has been archived.
		/// \[registry identifier\]
		Archive { identifier: IdentifierOf, author: CordAccountOf<T> },
		/// A registry has been restored.
		/// \[registry identifier\]
		Restore { identifier: IdentifierOf, author: CordAccountOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Registry identifier is not unique
		RegistryAlreadyAnchored,
		/// Registry identifier not found
		RegistryNotFound,
		/// Only when the author is not the controller.
		UnauthorizedOperation,
		// Maximum Number of delegates reached.
		TooManyDelegates,
		// Only when the author is not the controller
		UnauthorizedDelegation,
		// Invalid Identifier
		InvalidRegistryIdentifier,
		// Invalid Identifier Length
		InvalidIdentifierLength,
		// Invalid Identifier Prefix
		InvalidIdentifierPrefix,
		// Invalid creator signature
		InvalidSignature,
		// Archived registry
		ArchivedRegistry,
		// Registry not Archived
		RegistryNotArchived,
		// Invalid transaction hash
		InvalidTransactionHash,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add registry authorisations (delegation).
		///
		/// This transaction can only be performed by the registry controller
		/// or delegates.
		///
		/// * origin: the identity of the registry controller.
		/// * auth: registry transaction details.
		/// * delegates: authorised identities to add.
		/// * tx_signature: creator signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::authorise())]
		pub fn authorise(
			origin: OriginFor<T>,
			auth: RegistryParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<RegistryHashes<T>>::contains_key(&auth.register.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&auth.register.digest).encode()[..], &auth.register.controller),
				Error::<T>::InvalidSignature
			);

			RegistryDetails::from_registry_identities(
				&auth.identifier,
				auth.register.controller.clone(),
			)
			.map_err(Error::<T>::from)?;

			RegistryDelegations::<T>::try_mutate(auth.identifier.clone(), |ref mut delegation| {
				ensure!(
					delegation.len() + delegates.len() <= T::MaxRegistryDelegates::get() as usize,
					Error::<T>::TooManyDelegates
				);
				for delegate in delegates {
					delegation
						.try_push(delegate)
						.expect("delegates length is less than T::MaxCollectionDelegates; qed");
				}

				<RegistryHashes<T>>::insert(&auth.register.digest, &auth.identifier);

				Self::deposit_event(Event::AddDelegates {
					identifier: auth.identifier,
					digest: auth.register.digest,
					author: auth.register.controller,
				});

				Ok(())
			})
		}
		/// Remove registry authorisations (delegation).
		///
		/// This transaction can only be performed by the registry controller
		/// or delegates.
		///
		/// * origin: the identity of the registry controller.
		/// * auth: registry transaction details.
		/// * delegates: authorised identities to add.
		/// * tx_signature: creator signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::deauthorise())]
		pub fn deauthorise(
			origin: OriginFor<T>,
			deauth: RegistryParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<RegistryHashes<T>>::contains_key(&deauth.register.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&deauth.register.digest).encode()[..], &deauth.register.controller),
				Error::<T>::InvalidSignature
			);

			RegistryDetails::from_registry_identities(
				&deauth.identifier,
				deauth.register.controller.clone(),
			)
			.map_err(Error::<T>::from)?;

			RegistryDelegations::<T>::try_mutate(deauth.identifier.clone(), |ref mut delegation| {
				for delegate in delegates {
					delegation.retain(|x| x != &delegate);
				}

				<RegistryHashes<T>>::insert(&deauth.register.digest, &deauth.identifier);

				Self::deposit_event(Event::RemoveDelegates {
					identifier: deauth.identifier,
					digest: deauth.register.digest,
					author: deauth.register.controller,
				});

				Ok(())
			})
		}

		/// Create a new registry and associates with its identifier.
		///
		/// * origin: the identity of the registry controller.
		/// * registry: incoming registry stream.
		/// * tx_signature: creator signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			tx_registry: RegistryType<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&tx_registry.digest).encode()[..], &tx_registry.controller),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(&tx_registry.digest).encode()[..], REGISTRY_INDEX)
					.into_bytes(),
			)
			.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Registries<T>>::contains_key(&identifier),
				Error::<T>::RegistryAlreadyAnchored
			);

			<RegistryHashes<T>>::insert(&tx_registry.digest, &identifier);

			<Registries<T>>::insert(
				&identifier,
				RegistryDetails {
					register: tx_registry.clone(),
					schema: None,
					archived: false,
					metadata: false,
				},
			);
			Self::deposit_event(Event::Create {
				identifier,
				digest: tx_registry.digest,
				author: tx_registry.controller,
			});

			Ok(())
		}
		/// Archive a registry
		///
		///This transaction can only be performed by the registry controller
		/// or delegates
		///
		/// * origin: the identity of the registry controller.
		/// * arch: registry params to archive.
		/// * tx_signature: updater signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::archive())]
		pub fn archive(
			origin: OriginFor<T>,
			tx_registry: RegistryParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<RegistryHashes<T>>::contains_key(&tx_registry.register.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(
					&(&tx_registry.register.digest).encode()[..],
					&tx_registry.register.controller
				),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_registry.identifier, REGISTRY_INDEX)
				.map_err(|_| Error::<T>::InvalidRegistryIdentifier)?;

			let registry_details = <Registries<T>>::get(&tx_registry.identifier)
				.ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_details.archived, Error::<T>::ArchivedRegistry);

			RegistryDetails::from_registry_delegates(
				&tx_registry.identifier,
				tx_registry.register.controller.clone(),
				registry_details.register.controller.clone(),
			)
			.map_err(<Error<T>>::from)?;

			<RegistryHashes<T>>::insert(&tx_registry.register.digest, &tx_registry.identifier);

			<Registries<T>>::insert(
				&tx_registry.identifier,
				RegistryDetails { archived: true, ..registry_details },
			);
			Self::deposit_event(Event::Archive {
				identifier: tx_registry.identifier,
				author: tx_registry.register.controller,
			});

			Ok(())
		}
		/// Restore an archived registry
		///
		/// This transaction can only be performed by the registry controller or
		/// delegates
		///
		/// * origin: the identity of the registry controller.
		/// * updater: updater (controller) of the registry.
		/// * identifier: unique identifier of the registry.
		/// * tx_hash: transaction hash to verify the signature.
		/// * tx_signature: updater signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::restore())]
		pub fn restore(
			origin: OriginFor<T>,
			tx_registry: RegistryParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<RegistryHashes<T>>::contains_key(&tx_registry.register.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(
					&(&tx_registry.register.digest).encode()[..],
					&tx_registry.register.controller
				),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_registry.identifier, REGISTRY_INDEX)
				.map_err(|_| Error::<T>::InvalidRegistryIdentifier)?;

			let registry_details = <Registries<T>>::get(&tx_registry.identifier)
				.ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_details.archived, Error::<T>::ArchivedRegistry);

			RegistryDetails::from_registry_delegates(
				&tx_registry.identifier,
				tx_registry.register.controller.clone(),
				registry_details.register.controller.clone(),
			)
			.map_err(<Error<T>>::from)?;

			<RegistryHashes<T>>::insert(&tx_registry.register.digest, &tx_registry.identifier);

			<Registries<T>>::insert(
				&tx_registry.identifier,
				RegistryDetails { archived: false, ..registry_details },
			);
			Self::deposit_event(Event::Archive {
				identifier: tx_registry.identifier,
				author: tx_registry.register.controller,
			});

			Ok(())
		}
		/// Transfer an active registry to a new controller.
		///
		///This transaction can only be performed by the registry controller
		///
		/// * origin: the identity of the registry controller.
		/// * updater: updater (controller) of the registry.
		/// * identifier: unique identifier of the incoming registry stream.
		/// * transfer_to: new controller of the registry.
		/// * tx_hash: transaction hash to verify the signature.
		/// * tx_signature: creator signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			tx_registry: RegistryParams<T>,
			transfer_to: CordAccountOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<RegistryHashes<T>>::contains_key(&tx_registry.register.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(
					&(&tx_registry.register.digest).encode()[..],
					&tx_registry.register.controller
				),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_registry.identifier, REGISTRY_INDEX)
				.map_err(|_| Error::<T>::InvalidRegistryIdentifier)?;

			let registry_details = <Registries<T>>::get(&tx_registry.identifier)
				.ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_details.archived, Error::<T>::ArchivedRegistry);

			RegistryDetails::from_registry_delegates(
				&tx_registry.identifier,
				tx_registry.register.controller.clone(),
				registry_details.register.controller.clone(),
			)
			.map_err(<Error<T>>::from)?;

			<Registries<T>>::insert(
				&tx_registry.identifier,
				RegistryDetails {
					archived: false,
					register: {
						RegistryType {
							controller: transfer_to.clone(),
							..registry_details.register
						}
					},
					..registry_details
				},
			);
			Self::deposit_event(Event::Transfer {
				identifier: tx_registry.identifier,
				transfer: transfer_to,
				author: tx_registry.register.controller,
			});

			Ok(())
		}
	}
}
