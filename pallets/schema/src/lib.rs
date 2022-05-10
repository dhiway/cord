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
		/// Schema idenfier is not unique
		SchemaAlreadyAnchored,
		/// Schema idenfier not found
		SchemaNotFound,
		/// Schema revoked
		SchemaRevoked,
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
		// Schema not part of Space
		SchemaSpaceMismatch,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add schema authorisations (delegation).
		///
		/// This transaction can only be performed by the schema controller on
		/// permissioned schemas.
		///
		/// * origin: the identity of the schema controller.
		/// * schema: unique identifier of the schema.
		/// * delegates: authorised identities to add.
		/// * space_id: \[OPTIONAL\] schema space link identifier.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn authorise(
			origin: OriginFor<T>,
			schema: IdentifierOf,
			delegates: Vec<CordAccountOf<T>>,
			space_id: Option<IdentifierOf>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema_details = <Schemas<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaRevoked);

			if let Some(ref space_id) = space_id {
				ensure!(
					schema_details.space_id == Some(space_id.to_vec()),
					Error::<T>::SchemaSpaceMismatch
				);

				if schema_details.controller != controller {
					pallet_space::SpaceDetails::<T>::from_known_identities(
						&space_id,
						controller.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(
					schema_details.controller == controller,
					Error::<T>::UnauthorizedDelegation
				);
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

				Self::deposit_event(Event::AddDelegates(schema, controller));
				Ok(())
			})
		}
		/// Remove schema authorisations (delegation).
		///
		/// This transaction can only be performed by the schema controller
		/// permissioned schemas.
		///
		/// * origin: the identity of the schema controller.
		/// * schema: unique identifier of the schema.
		/// * delegates: identities (delegates) to be removed.
		/// * space_id: \[OPTIONAL\] schema space link identifier.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn deauthorise(
			origin: OriginFor<T>,
			schema: IdentifierOf,
			space_id: Option<IdentifierOf>,
			delegates: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema_details = <Schemas<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaRevoked);

			if let Some(ref space_id) = space_id {
				ensure!(
					schema_details.space_id == Some(space_id.to_vec()),
					Error::<T>::SchemaSpaceMismatch
				);

				if schema_details.controller != controller {
					pallet_space::SpaceDetails::<T>::from_known_identities(
						&space_id,
						controller.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(schema_details.controller == controller, Error::<T>::UnauthorizedOperation);
			}

			Delegations::<T>::try_mutate(schema.clone(), |ref mut delegation| {
				for delegate in delegates {
					delegation.retain(|x| x != &delegate);
				}

				Self::deposit_event(Event::RemoveDelegates(schema, controller));
				Ok(())
			})
		}

		/// Create a new schema and associates with its identifier.
		///
		/// * origin: the identity of the schema controller.
		/// * version: version of the  schema stream.
		/// * schema_hash: hash of the incoming schema stream.
		/// * space_id: \[OPTIONAL\] schema space link identifier.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn create(
			origin: OriginFor<T>,
			schema_hash: HashOf<T>,
			space_id: Option<IdentifierOf>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let identifier: IdentifierOf =
				mark::generate(&(&schema_hash).encode()[..], SCHEMA_IDENTIFIER_PREFIX).into_bytes();
			ensure!(!<Schemas<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);

			if let Some(ref space_id) = space_id {
				ensure!(
					!<pallet_space::Spaces<T>>::contains_key(&space_id),
					<pallet_space::Error<T>>::SpaceNotFound
				);
			}
			<SchemaHashes<T>>::insert(&schema_hash, &identifier);

			<Schemas<T>>::insert(
				&identifier,
				SchemaDetails {
					schema_hash: schema_hash.clone(),
					controller: controller.clone(),
					space_id,
					revoked: false,
				},
			);
			Self::deposit_event(Event::Anchor(schema_hash, identifier, controller));

			Ok(())
		}

		/// Revoke a Schema
		///
		///This transaction can only be performed by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * space_id: \[OPTIONAL\] schema space link identifier.
		#[pallet::weight(20_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn revoke(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			space_id: Option<IdentifierOf>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			mark::from_known_format(&identifier, SCHEMA_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidIdentifier)?;

			let schema_details =
				<Schemas<T>>::get(&identifier).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaRevoked);

			if let Some(ref space_id) = space_id {
				ensure!(
					schema_details.space_id == Some(space_id.to_vec()),
					Error::<T>::SchemaSpaceMismatch
				);

				if schema_details.controller != controller {
					pallet_space::SpaceDetails::<T>::from_known_identities(
						&space_id,
						controller.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(schema_details.controller == controller, Error::<T>::UnauthorizedOperation);
			}

			<Schemas<T>>::insert(&identifier, SchemaDetails { revoked: true, ..schema_details });
			Self::deposit_event(Event::Revoke(identifier, controller));

			Ok(())
		}
	}
}
