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
pub use cord_primitives::{CidOf, IdentifierOf, StatusOf, VersionOf};
use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
use semver::Version;
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};

pub mod schemas;
pub mod weights;

pub use crate::schemas::*;
use crate::weights::WeightInfo;
pub use pallet::*;
pub use sp_cid::{Cid, Version as CidType};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Hash of the schema.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub type CordAccountOf<T> = <T as Config>::CordAccountId;
	// schema identifier prefix.
	pub const SCHEMA_IDENTIFIER_PREFIX: u16 = 33;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type CordAccountId: Parameter;

		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// The maximum number of delegates for a schema.
		#[pallet::constant]
		type MaxDelegates: Get<u32>;
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
	pub type Schemas<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, SchemaDetails<T>>;

	/// schema identifiers stored on chain.
	/// It maps from a schema identifier to hash.
	#[pallet::storage]
	#[pallet::getter(fn schemaid)]
	pub type SchemaId<T> = StorageMap<_, Blake2_128Concat, IdentifierOf, HashOf<T>>;

	/// schema delegations stored on chain.
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
		/// A new schema has been created.
		/// \[schema identifier, version, controller\]
		Anchor(IdentifierOf, VersionOf, CordAccountOf<T>),
		/// A schema has been updated.
		/// \[schema identifier, version, controller\]
		Update(IdentifierOf, VersionOf, CordAccountOf<T>),
		/// A schema status has been changed.
		/// \[schema identifier, controller\]
		Status(IdentifierOf, CordAccountOf<T>),
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
		/// Invalid CID encoding.
		InvalidCidEncoding,
		/// Invalid CID version
		InvalidCidVersion,
		/// no status change required
		StatusChangeNotRequired,
		/// Only when the author is not the controller.
		UnauthorizedOperation,
		// Maximum Number of delegates reached.
		TooManyDelegates,
		// Not a permissioned schema
		SchemaNotPermissioned,
		// Schema permission matching with the change request
		NoPermissionChangeRequired,
		// Invalid Schema Semver
		InvalidSchemaVersion,
		// Base schema link not found
		SchemaGenesisNotFound,
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
		/// Add schema authorisations (delegation).
		///
		/// This transaction can only be performed by the schema controller on
		/// permissioned schemas.
		///
		/// * origin: the identity of the schema controller.
		/// * schema: unique identifier of the schema.
		/// * creator: controller of the schema.
		/// * delegates: authorised identities to add.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn authorise(
			origin: OriginFor<T>,
			schema: IdentifierOf,
			creator: CordAccountOf<T>,
			delegates: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let schema_hash = <SchemaId<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			let schema_details =
				<Schemas<T>>::get(&schema_hash).ok_or(Error::<T>::SchemaNotFound)?;

			ensure!(schema_details.permissioned, Error::<T>::SchemaNotPermissioned);
			ensure!(schema_details.creator == creator, Error::<T>::UnauthorizedDelegation);

			Delegations::<T>::try_mutate(schema.clone(), |ref mut delegation| {
				ensure!(
					delegation.len() + delegates.len() <= T::MaxDelegates::get() as usize,
					Error::<T>::TooManyDelegates
				);
				for delegate in delegates {
					delegation
						.try_push(delegate)
						.expect("delegates length is less than T::MaxDelegates; qed");
				}
				Self::deposit_event(Event::AddDelegates(schema, creator));
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
		/// * creator: controller of the schema.
		/// * delegates: identities (delegates) to be removed.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn deauthorise(
			origin: OriginFor<T>,
			schema: IdentifierOf,
			creator: CordAccountOf<T>,
			delegates: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema_hash = <SchemaId<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			let schema_details =
				<Schemas<T>>::get(&schema_hash).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.permissioned, Error::<T>::SchemaNotPermissioned);
			ensure!(schema_details.creator == creator, Error::<T>::UnauthorizedDelegation);

			Delegations::<T>::try_mutate(schema.clone(), |ref mut delegation| {
				for delegate in delegates {
					delegation.retain(|x| x != &delegate);
				}
				Self::deposit_event(Event::RemoveDelegates(schema, creator));
				Ok(())
			})
		}

		/// Create a new schema and associates with its identifier.
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming schema stream.
		/// * creator: controller of the schema.
		/// * version: version of the  schema stream.
		/// * schema_hash: hash of the incoming schema stream.
		/// * cid: \[OPTIONAL\] storage Id of the incoming stream.
		/// * permissioned: schema type - permissioned or not.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn create(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			creator: CordAccountOf<T>,
			version: VersionOf,
			schema_hash: HashOf<T>,
			cid: Option<CidOf>,
			permissioned: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<Schemas<T>>::contains_key(&schema_hash), Error::<T>::SchemaAlreadyAnchored);
			SchemaDetails::<T>::is_valid_identifier(&identifier, SCHEMA_IDENTIFIER_PREFIX)?;
			Version::parse(
				str::from_utf8(&version).map_err(|_err| Error::<T>::InvalidSchemaVersion)?,
			)
			.map_err(|_err| Error::<T>::InvalidSchemaVersion)?;
			if let Some(ref cid) = cid {
				SchemaDetails::<T>::is_valid_cid(cid)?;
			}
			ensure!(!<SchemaId<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);
			<SchemaId<T>>::insert(&identifier, &schema_hash);

			<Schemas<T>>::insert(
				&schema_hash,
				SchemaDetails {
					version: version.clone(),
					schema_id: identifier.clone(),
					creator: creator.clone(),
					parent: None,
					cid,
					permissioned,
					revoked: false,
				},
			);

			Self::deposit_event(Event::Anchor(identifier, version, creator));

			Ok(())
		}
		/// Update version of an existing schema.
		///
		///This transaction can only be performed by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming schema stream.
		/// * updater: controller of the schema.
		/// * version: version of the  schema stream.
		/// * schema_hash: hash of the new schema stream.
		/// * cid: \[OPTIONAL\] storage Id of the incoming stream.
		#[pallet::weight(50_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn version(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			updater: CordAccountOf<T>,
			version: VersionOf,
			schema_hash: HashOf<T>,
			cid: Option<CidOf>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let prev_schema_hash =
				<SchemaId<T>>::get(&identifier).ok_or(Error::<T>::SchemaNotFound)?;

			if let Some(ref cid) = cid {
				SchemaDetails::<T>::is_valid_cid(cid)?;
			}

			let new_version = Version::parse(
				str::from_utf8(&version).map_err(|_err| Error::<T>::InvalidSchemaVersion)?,
			)
			.map_err(|_err| Error::<T>::InvalidSchemaVersion)?;

			let schema_details =
				<Schemas<T>>::get(&prev_schema_hash).ok_or(Error::<T>::SchemaGenesisNotFound)?;

			ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);
			ensure!(schema_details.creator == updater, Error::<T>::UnauthorizedOperation);

			let old_version = Version::parse(str::from_utf8(&schema_details.version).unwrap())
				.map_err(|_err| Error::<T>::InvalidSchemaVersion)?;
			ensure!(new_version > old_version, Error::<T>::InvalidSchemaVersion);

			<SchemaId<T>>::insert(&identifier, &schema_hash);

			<Schemas<T>>::insert(
				&schema_hash,
				SchemaDetails {
					version: version.clone(),
					schema_id: identifier.clone(),
					creator: updater.clone(),
					parent: Some(prev_schema_hash),
					cid,
					..schema_details
				},
			);

			Self::deposit_event(Event::Update(identifier, version, updater));
			Ok(())
		}

		/// Update the status of the schema - revoked or not
		///
		///This transaction can only be performed by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * status: status to be updated
		#[pallet::weight(20_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn status(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			updater: CordAccountOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let schema_hash = <SchemaId<T>>::get(&identifier).ok_or(Error::<T>::SchemaNotFound)?;

			let schema_details =
				<Schemas<T>>::get(&schema_hash).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked != status, Error::<T>::StatusChangeNotRequired);
			ensure!(schema_details.creator == updater, Error::<T>::UnauthorizedOperation);

			<Schemas<T>>::insert(&schema_hash, SchemaDetails { revoked: status, ..schema_details });
			Self::deposit_event(Event::Status(identifier, updater));

			Ok(())
		}
		/// Update the schema type - permissioned or not
		///
		/// This update can only be performed by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * status: status to be updated
		#[pallet::weight(20_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn permission(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			updater: CordAccountOf<T>,
			permissioned: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema_hash = <SchemaId<T>>::get(&identifier).ok_or(Error::<T>::SchemaNotFound)?;

			let schema_details =
				<Schemas<T>>::get(&schema_hash).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaRevoked);
			ensure!(
				schema_details.permissioned != permissioned,
				Error::<T>::NoPermissionChangeRequired
			);
			ensure!(schema_details.creator == updater, Error::<T>::UnauthorizedOperation);

			<Schemas<T>>::insert(&schema_hash, SchemaDetails { permissioned, ..schema_details });
			Self::deposit_event(Event::Permission(identifier, updater));

			Ok(())
		}
	}
}
