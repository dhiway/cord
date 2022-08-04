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

pub use cord_primitives::{
	ss58identifier, IdentifierOf, MetaDataOf, StatusOf, REGISTRY_IDENTIFIER_PREFIX,
};
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

	/// Hash of the space.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a cord signature.
	pub type SignatureOf<T> = <T as Config>::Signature;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_space::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		// #[pallet::constant]
		// type MetadataLimit: Get<u32>;
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
	/// Registry entries stored on chain.
	/// It maps from entry Id to its details.
	#[pallet::storage]
	#[pallet::storage_prefix = "Entries"]
	pub type Entries<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, EntryDetails<T>, OptionQuery>;

	/// registry entry hashes stored on chain.
	/// It maps from an entry hash to Id (resolve from hash).
	#[pallet::storage]
	#[pallet::storage_prefix = "Hashes"]
	pub type EntryHashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	/// registry entry metadata stored on chain.
	/// It maps from an identifier to metadata.
	#[pallet::storage]
	#[pallet::storage_prefix = "Metadata"]
	pub(super) type Metadata<T: Config> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, MetadataEntry<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new registry entry has been created.
		/// \[identifier, digest, controller\]
		Create { identifier: IdentifierOf, digest: HashOf<T>, controller: CordAccountOf<T> },
		/// A registery entry has been created.
		/// \[identifier, digest, controller\]
		Update { identifier: IdentifierOf, digest: HashOf<T>, controller: CordAccountOf<T> },
		/// A registry entry has been revoked.
		/// \[identifier, controller\]
		Revoke { identifier: IdentifierOf, controller: CordAccountOf<T> },
		/// A registry entry has been removed.
		/// \[identifier, controller\]
		Remove { identifier: IdentifierOf, controller: CordAccountOf<T> },
		/// A metedata entry has been added.
		/// \[identifier, controller\]
		MetadataSet { identifier: IdentifierOf, controller: CordAccountOf<T> },
		/// A metadata entry has been cleared.
		/// \[identifier, controller\]
		MetadataCleared { identifier: IdentifierOf, controller: CordAccountOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Entry idenfier is not unique
		EntryAlreadyAnchored,
		/// Entryidenfier not found
		EntryNotFound,
		/// Only when the author is not the controller.
		UnauthorizedOperation,
		// Invalid Identifier
		InvalidRegistryIdentifier,
		// Invalid Identifier Length
		InvalidIdentifierLength,
		// Invalid Identifier Prefix
		InvalidIdentifierPrefix,
		// Invalid creator signature
		InvalidSignature,
		// Entry Revoked
		EntryAlreadyRevoked,
		// Entry not Revoked
		EntryNotRevoked,
		// Invalid transaction hash
		InvalidTransactionHash,
		// Entry Revoked
		EntryRevoked,
		// Entry Space mismatch
		EntrySpaceMismatch,
		// Metadata limit exceeded
		MetadataLimitExceeded,
		// Metadata already set for the entry
		MetadataAlreadySet,
		// Metadata not found for the entry
		MetadataNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set metadata for a registry entry.
		///
		/// This transaction can only be performed by the entry controller or
		/// space delegates.
		///
		/// * origin: the identity of the space controller.
		/// * entry: anchored entry details.
		/// * tx_signature: creator signature.
		#[pallet::weight(10_255_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn set_metadata(
			origin: OriginFor<T>,
			entry: EntryParams<T>,
			meta: Vec<u8>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<EntryHashes<T>>::contains_key(&entry.stream.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&entry.stream.digest).encode()[..], &entry.stream.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&entry.identifier, REGISTRY_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidRegistryIdentifier)?;

			ensure!(
				!<Metadata<T>>::contains_key(&entry.identifier),
				Error::<T>::MetadataAlreadySet
			);

			let entry_details =
				<Entries<T>>::get(&entry.identifier).ok_or(Error::<T>::EntryNotFound)?;
			ensure!(!entry_details.revoked, Error::<T>::EntryRevoked);

			ensure!(
				entry_details.stream.space == entry.stream.space,
				Error::<T>::EntrySpaceMismatch
			);

			if entry_details.stream.controller != entry.stream.controller {
				pallet_space::SpaceDetails::<T>::from_space_identities(
					&entry.stream.space,
					entry.stream.controller.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			} else {
				ensure!(
					entry_details.stream.controller == entry.stream.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			let bounded_metadata: MetaDataOf =
				meta.try_into().map_err(|_| Error::<T>::MetadataLimitExceeded)?;

			Metadata::<T>::insert(
				entry.identifier.clone(),
				MetadataEntry {
					metadata: bounded_metadata,
					digest: entry.stream.digest,
					controller: entry.stream.controller.clone(),
				},
			);

			<Entries<T>>::insert(
				&entry.identifier,
				EntryDetails { metadata: true, ..entry_details },
			);

			Self::deposit_event(Event::MetadataSet {
				identifier: entry.identifier,
				controller: entry.stream.controller,
			});

			Ok(())
		}
		/// Clear metadata for a registry entry.
		///
		/// This transaction can only be performed by the entry controller or
		/// space delegates.
		///
		/// * origin: the identity of the space controller.
		/// * entry: anchored entry details.
		/// * tx_signature: creator signature.
		#[pallet::weight(55_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn clear_metadata(
			origin: OriginFor<T>,
			entry: EntryParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<EntryHashes<T>>::contains_key(&entry.stream.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&entry.stream.digest).encode()[..], &entry.stream.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&entry.identifier, REGISTRY_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidRegistryIdentifier)?;

			ensure!(<Metadata<T>>::contains_key(&entry.identifier), Error::<T>::MetadataNotFound);

			let entry_details =
				<Entries<T>>::get(&entry.identifier).ok_or(Error::<T>::EntryNotFound)?;
			ensure!(!entry_details.revoked, Error::<T>::EntryRevoked);

			ensure!(
				entry_details.stream.space == entry.stream.space,
				Error::<T>::EntrySpaceMismatch
			);

			if entry_details.stream.controller != entry.stream.controller {
				pallet_space::SpaceDetails::<T>::from_space_identities(
					&entry.stream.space,
					entry.stream.controller.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			} else {
				ensure!(
					entry_details.stream.controller == entry.stream.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			Metadata::<T>::remove(entry.identifier.clone());
			<Entries<T>>::insert(
				&entry.identifier,
				EntryDetails { metadata: false, ..entry_details },
			);

			Self::deposit_event(Event::MetadataCleared {
				identifier: entry.identifier,
				controller: entry.stream.controller,
			});

			Ok(())
		}

		/// Create a new registry entry and associates with its identifier.
		///
		/// * origin: the identity of the space controller.
		/// * entry: details of the registry entry.
		/// * tx_signature: creator signature.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn create(
			origin: OriginFor<T>,
			entry: EntryType<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&entry.digest).encode()[..], &entry.controller),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(&entry.digest).encode()[..], REGISTRY_IDENTIFIER_PREFIX)
					.into_bytes(),
			)
			.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Entries<T>>::contains_key(&identifier), Error::<T>::EntryAlreadyAnchored);

			<EntryHashes<T>>::insert(&entry.digest, &identifier);

			<Entries<T>>::insert(
				&identifier,
				EntryDetails { stream: entry.clone(), revoked: false, metadata: false },
			);
			Self::deposit_event(Event::Create {
				identifier,
				digest: entry.digest,
				controller: entry.controller,
			});

			Ok(())
		}

		/// Update the entry information.
		///
		/// * origin: the identity of the Tx Author.
		/// * entry: anchored entry details.
		/// * tx_signature: signature of the controller.
		#[pallet::weight(50_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn update(
			origin: OriginFor<T>,
			entry: EntryParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<EntryHashes<T>>::contains_key(&entry.stream.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&entry.stream.digest).encode()[..], &entry.stream.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&entry.identifier, REGISTRY_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidRegistryIdentifier)?;

			let entry_details =
				<Entries<T>>::get(&entry.identifier).ok_or(Error::<T>::EntryNotFound)?;
			ensure!(!entry_details.revoked, Error::<T>::EntryRevoked);

			ensure!(
				entry_details.stream.space == entry.stream.space,
				Error::<T>::EntrySpaceMismatch
			);

			if entry_details.stream.controller != entry.stream.controller {
				pallet_space::SpaceDetails::<T>::from_space_identities(
					&entry.stream.space,
					entry.stream.controller.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			} else {
				ensure!(
					entry_details.stream.controller == entry.stream.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			<EntryHashes<T>>::insert(&entry.stream.digest, &entry.identifier);

			<Entries<T>>::insert(
				&entry.identifier,
				EntryDetails { stream: entry.stream.clone(), ..entry_details },
			);
			Self::deposit_event(Event::Update {
				identifier: entry.identifier,
				digest: entry.stream.digest,
				controller: entry.stream.controller,
			});

			Ok(())
		}
		/// Revoke a registry entry
		///
		/// This transaction can only be performed by the entry controller or
		/// space delegates
		///
		/// * origin: the identity of the space controller.
		/// * entry: anchored entry details.
		/// * tx_signature: updater signature.
		#[pallet::weight(20_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn revoke(
			origin: OriginFor<T>,
			entry: EntryParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<EntryHashes<T>>::contains_key(&entry.stream.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&entry.stream.digest).encode()[..], &entry.stream.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&entry.identifier, REGISTRY_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidRegistryIdentifier)?;

			let entry_details =
				<Entries<T>>::get(&entry.identifier).ok_or(Error::<T>::EntryNotFound)?;
			ensure!(!entry_details.revoked, Error::<T>::EntryAlreadyRevoked);

			if entry_details.stream.controller != entry.stream.controller {
				pallet_space::SpaceDetails::<T>::from_space_identities(
					&entry.stream.space,
					entry.stream.controller.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			} else {
				ensure!(
					entry_details.stream.controller == entry.stream.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			<Entries<T>>::insert(
				&entry.identifier,
				EntryDetails { revoked: true, ..entry_details },
			);
			Self::deposit_event(Event::Revoke {
				identifier: entry.identifier,
				controller: entry.stream.controller,
			});

			Ok(())
		}
		/// Remove a registry entry and associated metadata
		///
		/// This transaction can only be performed by the space controller or
		/// space delegates
		///
		/// * origin: the identity of the space controller.
		/// * entru: anchored entry details.
		/// * tx_signature: updater signature.
		#[pallet::weight(20_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn remove(
			origin: OriginFor<T>,
			entry: EntryParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<EntryHashes<T>>::contains_key(&entry.stream.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&entry.stream.digest).encode()[..], &entry.stream.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&entry.identifier, REGISTRY_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidRegistryIdentifier)?;

			let entry_details =
				<Entries<T>>::get(&entry.identifier).ok_or(Error::<T>::EntryNotFound)?;
			ensure!(entry_details.revoked, Error::<T>::EntryNotRevoked);

			if entry_details.stream.controller != entry.stream.controller {
				pallet_space::SpaceDetails::<T>::from_space_identities(
					&entry.stream.space,
					entry.stream.controller.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			} else {
				ensure!(
					entry_details.stream.controller == entry.stream.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			<Entries<T>>::remove(&entry.identifier);
			if entry_details.metadata {
				<Metadata<T>>::remove(&entry.identifier);
			}

			Self::deposit_event(Event::Remove {
				identifier: entry.identifier,
				controller: entry.stream.controller,
			});

			Ok(())
		}
	}
}
