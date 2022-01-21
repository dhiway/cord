// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
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
pub use cord_primitives::{IdentifierOf, StatusOf};
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

	/// Milti-format Identifier of a schema.
	pub type IdOf<T> = BoundedVec<u8, <T as Config>::MaxLength>;
	/// Hash of the schema.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub type CordAccountOf<T> = <T as Config>::CordAccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type CordAccountId: Parameter + Default;

		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// The maximum number of delegates for a schema.
		#[pallet::constant]
		type MaxDelegates: Get<u32>;
		/// The maximum length of an Identifier.
		#[pallet::constant]
		type MaxLength: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// schemas stored on chain.
	/// It maps from a schema identifier to its details.
	#[pallet::storage]
	#[pallet::getter(fn schemas)]
	pub type Schemas<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, SchemaDetails<T>>;

	/// schema commits stored on chain.
	/// It maps from a schema Id to a vector of commit details.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<SchemaCommit<T>>>;

	/// schema delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::getter(fn delegations)]
	pub(super) type Delegations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdOf<T>,
		BoundedVec<CordAccountOf<T>, T::MaxDelegates>,
		ValueQuery,
	>;
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[schema identifier, version, controller\]
		TxAdd(IdOf<T>, IdOf<T>, CordAccountOf<T>),
		/// A schema has been updated.
		/// \[schema identifier, version, controller\]
		TxUpdate(IdOf<T>, IdOf<T>, CordAccountOf<T>),
		/// A schema status has been changed.
		/// \[schema identifier, version, controller\]
		TxStatus(IdOf<T>, IdOf<T>, CordAccountOf<T>),
		/// Schema delegates has been added.
		/// \[schema identifier, version, controller\]
		TxAddDelegates(IdOf<T>, IdOf<T>, CordAccountOf<T>),
		/// Schema delegates has been removed.
		/// \[schema identifier, version, controller\]
		TxRemoveDelegates(IdOf<T>, IdOf<T>, CordAccountOf<T>),
		/// A schema status has been changed.
		/// \[schema identifier, version, controller\]
		TxPermission(IdOf<T>, IdOf<T>, CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid request
		InvalidRequest,
		/// Hash and Identifier are the same
		SameIdentifierAndHash,
		/// Schema idenfier is not unique
		SchemaAlreadyAnchored,
		//Schema Hash is not unique
		HashAlreadyAnchored,
		/// Schema idenfier not found
		SchemaNotFound,
		/// Schema delegate not found
		SchemaDelegateNotFound,
		/// Schema revoked
		SchemaRevoked,
		/// Invalid CID encoding.
		InvalidCidEncoding,
		/// CID already anchored
		CidAlreadyAnchored,
		/// Invalid CID version
		InvalidCidVersion,
		/// no status change required
		StatusChangeNotRequired,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		// Maximum Number of delegates reached.
		TooManyDelegates,
		// Not a permissioned schema
		SchemaNotPermissioned,
		// Schema permission matching with the change request
		NoPermissionChangeRequired,
		// Invalid Schema Semver
		InvalidSchemaVersion,
		// No Delegates found for this schema
		DelegatesNotFound,
		// Base schema link not found
		SchemaBaseNotFound,
		DelegateSchemaNotFound,
		DelegateSchemaRevoked,
		UnauthorizedDelegation,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add delegates to a schema.
		///
		/// This transaction can only be performed by the schema controller.
		///
		/// * origin: the identity of the schema controller.
		/// * schema: unique identifier of the schema.
		/// * creator: controller of the schema.
		/// * delegates: schema delegates to add.
		#[pallet::weight(126_475_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn add_delegates(
			origin: OriginFor<T>,
			schema: IdOf<T>,
			creator: CordAccountOf<T>,
			delegates: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema_details = <Schemas<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.permissioned, Error::<T>::SchemaNotPermissioned);
			ensure!(schema_details.creator == creator, Error::<T>::UnauthorizedDelegation);
			let block_number = <frame_system::Pallet<T>>::block_number();

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
				SchemaCommit::<T>::store_tx(
					&schema,
					SchemaCommit {
						id: schema.clone(),
						version: schema_details.version.clone(),
						block: block_number,
						commit: SchemaCommitOf::Delegates,
					},
				)?;
				Self::deposit_event(Event::TxAddDelegates(schema, schema_details.version, creator));
				Ok(())
			})
		}
		/// Remove schema delegates.
		///
		/// This transaction can only be performed by the schema controller.
		///
		/// * origin: the identity of the schema controller.
		/// * schema: unique identifier of the schema.
		/// * creator: controller of the schema.
		/// * delegates: schema delegates to be removed.
		#[pallet::weight(126_475_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn remove_delegates(
			origin: OriginFor<T>,
			schema: IdOf<T>,
			creator: CordAccountOf<T>,
			delegates: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema_details = <Schemas<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.permissioned, Error::<T>::SchemaNotPermissioned);
			ensure!(schema_details.creator == creator, Error::<T>::UnauthorizedDelegation);
			let block_number = <frame_system::Pallet<T>>::block_number();

			Delegations::<T>::try_mutate(schema.clone(), |ref mut delegation| {
				for delegate in delegates {
					delegation.retain(|x| x != &delegate);
				}
				SchemaCommit::<T>::store_tx(
					&schema,
					SchemaCommit {
						id: schema.clone(),
						version: schema_details.version.clone(),
						block: block_number,
						commit: SchemaCommitOf::RevokeDelegates,
					},
				)?;
				Self::deposit_event(Event::TxRemoveDelegates(
					schema,
					schema_details.version,
					creator,
				));
				Ok(())
			})
		}

		/// Create a new schema and associates with its identifier.
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming schema stream.
		/// * creator: controller of the schema.
		/// * version: version of the  schema stream.
		/// * hash: hash of the incoming schema stream.
		/// * permissioned: schema type - permissioned or not.
		#[pallet::weight(570_952_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn create(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			creator: CordAccountOf<T>,
			version: IdOf<T>,
			hash: HashOf<T>,
			permissioned: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<Schemas<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);
			Version::parse(str::from_utf8(&version).unwrap())
				.map_err(|_err| Error::<T>::InvalidSchemaVersion)?;

			SchemaDetails::<T>::is_valid(&identifier)?;

			let block_number = <frame_system::Pallet<T>>::block_number();

			SchemaCommit::<T>::store_tx(
				&identifier,
				SchemaCommit {
					id: identifier.clone(),
					version: version.clone(),
					block: block_number.clone(),
					commit: SchemaCommitOf::Genesis,
				},
			)?;
			<Schemas<T>>::insert(
				&identifier,
				SchemaDetails {
					version: version.clone(),
					schema_hash: hash,
					creator: creator.clone(),
					base_schema: None,
					permissioned,
					revoked: false,
				},
			);

			Self::deposit_event(Event::TxAdd(identifier, version, creator));

			Ok(())
		}
		/// Update an existing schema.
		///
		///This transaction can only be performed by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming schema stream.
		/// * updater: controller of the schema.
		/// * version: version of the  schema stream.
		/// * hash: hash of the incoming schema stream.
		/// * delegated: status of the parent
		/// * cid: CID of the incoming schema stream.
		#[pallet::weight(191_780_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn update(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			updater: CordAccountOf<T>,
			version: IdOf<T>,
			hash: HashOf<T>,
			base: IdOf<T>,
			permissioned: StatusOf,
			delegate_from: Option<IdOf<T>>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			SchemaDetails::<T>::is_valid(&identifier)?;
			ensure!(!<Schemas<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);
			let new_version = Version::parse(str::from_utf8(&version).unwrap())
				.map_err(|_err| Error::<T>::InvalidSchemaVersion)?;

			let schema_details = <Schemas<T>>::get(&base).ok_or(Error::<T>::SchemaBaseNotFound)?;
			let old_version = Version::parse(str::from_utf8(&schema_details.version).unwrap())
				.map_err(|_err| Error::<T>::InvalidSchemaVersion)?;
			ensure!(new_version > old_version, Error::<T>::InvalidSchemaVersion);
			ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);
			ensure!(schema_details.creator == updater, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			if permissioned {
				if let Some(ref delegate_from) = delegate_from {
					let delegate_from_schema =
						<Schemas<T>>::get(&base).ok_or(Error::<T>::DelegateSchemaNotFound)?;
					ensure!(!delegate_from_schema.revoked, Error::<T>::DelegateSchemaRevoked);
					ensure!(
						delegate_from_schema.creator == updater,
						Error::<T>::UnauthorizedDelegation
					);

					let delegates = Delegations::<T>::get(&delegate_from).into_inner();

					Delegations::<T>::try_mutate(delegate_from.clone(), |ref mut delegation| {
						for delegate in delegates {
							delegation
								.try_push(delegate)
								.expect("delegates length is less than T::MaxDelegates; qed");
						}
						SchemaCommit::<T>::store_tx(
							&base,
							SchemaCommit {
								id: base.clone(),
								version: schema_details.version,
								block: block_number,
								commit: SchemaCommitOf::VersionUpdate,
							},
						)?;
						<Schemas<T>>::insert(
							&identifier,
							SchemaDetails {
								version: version.clone(),
								schema_hash: hash,
								creator: updater.clone(),
								base_schema: Some(base),
								permissioned,
								revoked: false,
							},
						);
						Self::deposit_event(Event::TxUpdate(identifier, version, updater));

						Ok(())
					})
				} else {
					Ok(())
				}
			} else {
				SchemaCommit::<T>::store_tx(
					&base,
					SchemaCommit {
						id: base.clone(),
						version: schema_details.version,
						block: block_number,
						commit: SchemaCommitOf::VersionUpdate,
					},
				)?;
				<Schemas<T>>::insert(
					&identifier,
					SchemaDetails {
						version: version.clone(),
						schema_hash: hash,
						creator: updater.clone(),
						base_schema: Some(base),
						permissioned,
						revoked: false,
					},
				);

				Self::deposit_event(Event::TxUpdate(identifier, version, updater));
				Ok(())
			}
		}
		/// Update the status of the schema - revoked or not
		///
		///This transaction can only be performed by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * status: status to be updated
		#[pallet::weight(124_410_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn set_status(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			updater: CordAccountOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let schema_details =
				<Schemas<T>>::get(&identifier).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked != status, Error::<T>::StatusChangeNotRequired);
			ensure!(schema_details.creator == updater, Error::<T>::UnauthorizedOperation);
			let block_number = <frame_system::Pallet<T>>::block_number();

			SchemaCommit::<T>::store_tx(
				&identifier,
				SchemaCommit {
					id: identifier.clone(),
					version: schema_details.version.clone(),
					block: block_number,
					commit: SchemaCommitOf::StatusChange,
				},
			)?;

			<Schemas<T>>::insert(
				&identifier,
				SchemaDetails {
					version: schema_details.version.clone(),
					revoked: status,
					..schema_details
				},
			);
			Self::deposit_event(Event::TxStatus(identifier, schema_details.version, updater));

			Ok(())
		}
		/// Update the schema type - permissioned or not
		///
		/// This update can only be performed by by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * status: status to be updated
		#[pallet::weight(124_410_000 + T::DbWeight::get().reads_writes(1, 2))]
		pub fn set_permission(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			updater: CordAccountOf<T>,
			permissioned: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let schema_details =
				<Schemas<T>>::get(&identifier).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaRevoked);
			ensure!(
				schema_details.permissioned != permissioned,
				Error::<T>::NoPermissionChangeRequired
			);
			ensure!(schema_details.creator == updater, Error::<T>::UnauthorizedOperation);
			let block_number = <frame_system::Pallet<T>>::block_number();

			SchemaCommit::<T>::store_tx(
				&identifier,
				SchemaCommit {
					id: identifier.clone(),
					version: schema_details.version.clone(),
					block: block_number,
					commit: SchemaCommitOf::Permission,
				},
			)?;

			<Schemas<T>>::insert(
				&identifier,
				SchemaDetails {
					version: schema_details.version.clone(),
					permissioned,
					..schema_details
				},
			);
			Self::deposit_event(Event::TxPermission(identifier, schema_details.version, updater));

			Ok(())
		}
	}
}
