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

pub use cord_primitives::{mark, CidOf, IdentifierOf, StatusOf, VersionOf};
use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};

pub mod schemas;
pub mod weights;

pub use crate::schemas::*;
use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Hash of the schema.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	// schema identifier prefix.
	pub const SCHEMA_IDENTIFIER_PREFIX: u16 = 33;
	/// Type for a cord signature.
	pub type SignatureOf<T> = <T as Config>::Signature;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_space::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// The maximum number of delegates for a schema.
		#[pallet::constant]
		type MaxSchemaDelegates: Get<u32>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer> + Parameter;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// schemas stored on chain.
	/// It maps from a schema identifier to its details.
	#[pallet::storage]
	#[pallet::getter(fn schemas)]
	pub type Schemas<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, SchemaDetails<T>, OptionQuery>;

	/// schema identifiers stored on chain.
	/// It maps from a schema identifier to hash.
	#[pallet::storage]
	#[pallet::getter(fn schema_hashes)]
	pub type SchemaHashes<T> =
		StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	/// schema delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::getter(fn delegations)]
	pub(super) type Delegations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<CordAccountOf<T>, T::MaxSchemaDelegates>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[schema identifier, version, controller\]
		Anchor(HashOf<T>, IdentifierOf, CordAccountOf<T>),
		/// A schema has been updated.
		/// \[schema identifier, version, controller\]
		Update(IdentifierOf, VersionOf, CordAccountOf<T>),
		/// A schema status has been changed.
		/// \[schema identifier, controller\]
		Revoke(IdentifierOf, CordAccountOf<T>),
		/// Schema delegates has been added.
		/// \[schema identifier,  controller\]
		AddDelegates(IdentifierOf, CordAccountOf<T>),
		/// Schema delegates has been removed.
		/// \[schema identifier,  controller\]
		RemoveDelegates(IdentifierOf, CordAccountOf<T>),
		/// A schema status has been changed.
		/// \[schema identifier, version, controller\]
		Permission(IdentifierOf, CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Schema identifier is not unique
		SchemaAlreadyAnchored,
		/// Schema not found
		SchemaNotFound,
		/// Schema is revoked
		SchemaRevoked,
		/// When the author is not the controller or delegate.
		UnauthorizedOperation,
		// Maximum Number of delegates reached.
		TooManyDelegates,
		// When the author is not the controller or delegate
		UnauthorizedDelegation,
		// Invalid Schema Identifier
		InvalidSchemaIdentifier,
		// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		// Invalid Schema Identifier Prefix
		InvalidIdentifierPrefix,
		// Schema is not part of the Space
		SchemaSpaceMismatch,
		// Invalid creator signature
		InvalidSignature,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add schema authorisations (delegation).
		///
		/// This transaction can only be performed by the schema controller or
		/// delegates.
		///
		/// * origin: the identity of the schema anchor.
		/// * creator: creator (controller) of the schema.
		/// * schema: unique identifier of the schema.
		/// * tx_hash: transaction hash to verify the signature.
		/// * delegates: authorised identities to add.
		/// * space: \[OPTIONAL\] schema space link identifier.
		/// * tx_signature: creator signature.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn authorise(
			origin: OriginFor<T>,
			creator: CordAccountOf<T>,
			schema: IdentifierOf,
			tx_hash: HashOf<T>,
			delegates: Vec<CordAccountOf<T>>,
			space: Option<IdentifierOf>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&tx_hash).encode()[..], &creator),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&schema, SCHEMA_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSchemaIdentifier)?;

			let schema_details = <Schemas<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaRevoked);

			if let Some(ref space) = space {
				ensure!(
					schema_details.space == Some(space.to_vec()),
					Error::<T>::SchemaSpaceMismatch
				);

				if schema_details.controller != creator {
					pallet_space::SpaceDetails::<T>::from_space_identities(&space, creator.clone())
						.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(schema_details.controller == creator, Error::<T>::UnauthorizedDelegation);
			}

			Delegations::<T>::try_mutate(schema.clone(), |ref mut delegation| {
				ensure!(
					delegation.len() + delegates.len() <= T::MaxSchemaDelegates::get() as usize,
					Error::<T>::TooManyDelegates
				);
				for delegate in delegates {
					delegation
						.try_push(delegate)
						.expect("delegates length is less than T::MaxSchemaDelegates; qed");
				}

				Self::deposit_event(Event::AddDelegates(schema, creator));
				Ok(())
			})
		}
		/// Remove schema authorisations (delegation).
		///
		/// This transaction can only be performed by the schema controller or
		/// delegates.
		///
		/// * origin: the identity of the schema anchor.
		/// * updater: updater (controller) of the schema.
		/// * schema: unique identifier of the schema.
		/// * tx_hash: transaction hash to verify the signature.
		/// * delegates: authorised identities to add.
		/// * space: \[OPTIONAL\] schema space link identifier.
		/// * tx_signature: updater signature.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn deauthorise(
			origin: OriginFor<T>,
			updater: CordAccountOf<T>,
			schema: IdentifierOf,
			tx_hash: HashOf<T>,
			delegates: Vec<CordAccountOf<T>>,
			space: Option<IdentifierOf>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&tx_hash).encode()[..], &updater),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&schema, SCHEMA_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSchemaIdentifier)?;

			let schema_details = <Schemas<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaRevoked);

			if let Some(ref space) = space {
				ensure!(
					schema_details.space == Some(space.to_vec()),
					Error::<T>::SchemaSpaceMismatch
				);

				if schema_details.controller != updater {
					pallet_space::SpaceDetails::<T>::from_space_identities(&space, updater.clone())
						.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(schema_details.controller == updater, Error::<T>::UnauthorizedOperation);
			}

			Delegations::<T>::try_mutate(schema.clone(), |ref mut delegation| {
				for delegate in delegates {
					delegation.retain(|x| x != &delegate);
				}

				Self::deposit_event(Event::RemoveDelegates(schema, updater));
				Ok(())
			})
		}

		/// Create a new schema and associates with its identifier.
		///
		/// * origin: the identity of the schema anchor.
		/// * creator: creator (controller) of the schema.
		/// * schema_hash: hash of the incoming schema stream.
		/// * space: \[OPTIONAL\] schema space link identifier.
		/// * tx_signature: creator signature.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn create(
			origin: OriginFor<T>,
			creator: CordAccountOf<T>,
			schema_hash: HashOf<T>,
			space: Option<IdentifierOf>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&schema_hash).encode()[..], &creator),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf =
				mark::generate(&(&schema_hash).encode()[..], SCHEMA_IDENTIFIER_PREFIX).into_bytes();
			ensure!(!<Schemas<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);

			if let Some(ref space) = space {
				pallet_space::SpaceDetails::<T>::from_space_identities(&space, creator.clone())
					.map_err(<pallet_space::Error<T>>::from)?;
			}

			<SchemaHashes<T>>::insert(&schema_hash, &identifier);

			<Schemas<T>>::insert(
				&identifier,
				SchemaDetails {
					schema_hash: schema_hash.clone(),
					controller: creator.clone(),
					space,
					revoked: false,
				},
			);
			Self::deposit_event(Event::Anchor(schema_hash, identifier, creator));

			Ok(())
		}

		/// Revoke a Schema
		///
		///This transaction can only be performed by the schema controller or
		/// delegates
		///
		/// * origin: the identity of the schema controller.
		/// * updater: updater (controller) of the schema.
		/// * identifier: unique identifier of the incoming stream.
		/// * tx_hash: transaction hash to verify the signature.
		/// * space: \[OPTIONAL\] schema space link identifier.
		/// * tx_signature: updater signature.
		#[pallet::weight(20_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn revoke(
			origin: OriginFor<T>,
			updater: CordAccountOf<T>,
			schema: IdentifierOf,
			tx_hash: HashOf<T>,
			space: Option<IdentifierOf>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&tx_hash).encode()[..], &updater),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&schema, SCHEMA_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSchemaIdentifier)?;

			let schema_details = <Schemas<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaRevoked);

			if let Some(ref space) = space {
				ensure!(
					schema_details.space == Some(space.to_vec()),
					Error::<T>::SchemaSpaceMismatch
				);

				if schema_details.controller != updater {
					pallet_space::SpaceDetails::<T>::from_space_identities(&space, updater.clone())
						.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(schema_details.controller == updater, Error::<T>::UnauthorizedOperation);
			}

			<Schemas<T>>::insert(&schema, SchemaDetails { revoked: true, ..schema_details });
			Self::deposit_event(Event::Revoke(schema, updater));

			Ok(())
		}
	}
}
