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
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};
pub mod schemas;
pub mod weights;

pub use crate::schemas::*;
use crate::weights::WeightInfo;
pub use pallet::*;
pub use sp_cid::{Cid, Version};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Identifier of a schema.
	pub type IdOf<T> = <T as frame_system::Config>::Hash;
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
		#[pallet::constant]
		type MaxDelegates: Get<u32>;
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

	/// transactions stored on chain.
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
		/// \[schema identifier, schema hash, controller\]
		TxAdd(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// A schema has been updated.
		/// \[schema identifier, schema hash, controller\]
		TxUpdate(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// A schema status has been changed.
		/// \[schema identifier, controller\]
		TxStatus(IdOf<T>, CordAccountOf<T>),
		/// A schema delegate has been added.
		/// \[schema identifier, controller\]
		TxAddDelegate(IdOf<T>),
		/// A schema delegate has been removed.
		/// \[schema identifier, controller\]
		TxRemoveDelegate(IdOf<T>, CordAccountOf<T>),
		/// A schema status has been changed.
		/// \[schema identifier, controller\]
		TxPermission(IdOf<T>, CordAccountOf<T>),
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new schema delegate.
		///
		/// This transaction can only be performed by the schema controller.
		///
		/// * origin: the identity of the schema controller.
		/// * schema: unique identifier of the schema.
		/// * delegates: schema delegates to add.
		#[pallet::weight(0)]
		pub fn add_delegates(
			origin: OriginFor<T>,
			schema: IdOf<T>,
			creator: CordAccountOf<T>,
			delegates: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema_details = <Schemas<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.permissioned, Error::<T>::SchemaNotPermissioned);
			ensure!(schema_details.creator == creator, Error::<T>::UnauthorizedOperation);
			let block_number = <frame_system::Pallet<T>>::block_number();

			Delegations::<T>::try_mutate(&schema, |ref mut delegation| {
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
						hash: schema_details.hash,
						cid: schema_details.cid,
						block: block_number,
						commit: SchemaCommitOf::Delegate,
					},
				)?;
				Self::deposit_event(Event::TxAddDelegate(schema));
				Ok(())
			})
		}
		/// Remove a schema delegate.
		///
		/// This transaction can only be performed by the schema controller.
		///
		/// * origin: the identity of the schema controller.
		/// * schema: unique identifier of the schema.
		/// * delegate: schema delegate to be removed.
		#[pallet::weight(0)]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			schema: IdOf<T>,
			creator: CordAccountOf<T>,
			delegate: CordAccountOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema_details = <Schemas<T>>::get(&schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.permissioned, Error::<T>::SchemaNotPermissioned);
			ensure!(schema_details.creator == creator, Error::<T>::UnauthorizedOperation);
			let block_number = <frame_system::Pallet<T>>::block_number();

			Delegations::<T>::try_mutate(&schema, |ref mut delegation| {
				delegation.retain(|x| x != &delegate);
				SchemaCommit::<T>::store_tx(
					&schema,
					SchemaCommit {
						hash: schema_details.hash,
						cid: schema_details.cid,
						block: block_number,
						commit: SchemaCommitOf::RevokeDelegation,
					},
				)?;
				Self::deposit_event(Event::TxRemoveDelegate(schema, delegate));
				Ok(())
			})
		}

		/// Create a new schema and associates with its identifier.
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming schema stream.
		/// * hash: hash of the incoming schema stream.
		/// * cid: CID of the incoming schema stream.
		/// * permissioned: schema type - permissioned or not.
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			creator: CordAccountOf<T>,
			hash: HashOf<T>,
			cid: Option<IdentifierOf>,
			permissioned: StatusOf,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<Schemas<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);
			ensure!(hash != identifier, Error::<T>::SameIdentifierAndHash);
			if let Some(ref cid) = cid {
				SchemaDetails::<T>::is_valid(cid)?;
			}
			let block_number = <frame_system::Pallet<T>>::block_number();

			SchemaCommit::<T>::store_tx(
				&identifier,
				SchemaCommit {
					hash: hash.clone(),
					cid: cid.clone(),
					block: block_number.clone(),
					commit: SchemaCommitOf::Genesis,
				},
			)?;
			if permissioned {
				Delegations::<T>::mutate(&identifier, |ref mut delegates| {
					delegates
						.try_push(creator.clone())
						.expect("delegates length checked above; qed");
				});
			}
			<Schemas<T>>::insert(
				&identifier,
				SchemaDetails {
					hash: hash.clone(),
					cid,
					parent_cid: None,
					creator: creator.clone(),
					block: block_number.clone(),
					permissioned,
					revoked: false,
				},
			);

			Self::deposit_event(Event::TxAdd(identifier, hash, creator));

			Ok(())
		}
		/// Update an existing schema.
		///
		///This transaction can only be performed by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming schema stream.
		/// * hash: hash of the incoming schema stream.
		/// * cid: CID of the incoming schema stream.
		#[pallet::weight(0)]
		pub fn update(
			origin: OriginFor<T>,
			identifier: IdOf<T>,
			updater: CordAccountOf<T>,
			hash: HashOf<T>,
			cid: Option<IdentifierOf>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(hash != identifier, Error::<T>::SameIdentifierAndHash);

			let schema_details =
				<Schemas<T>>::get(&identifier).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(hash != schema_details.hash, Error::<T>::HashAlreadyAnchored);

			if let Some(ref cid) = cid {
				ensure!(
					cid != schema_details.cid.as_ref().unwrap(),
					Error::<T>::CidAlreadyAnchored
				);
				SchemaDetails::<T>::is_valid(cid)?;
			}
			ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);
			ensure!(schema_details.creator == updater, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			SchemaCommit::<T>::store_tx(
				&identifier,
				SchemaCommit {
					hash: hash.clone(),
					cid: cid.clone(),
					block: block_number,
					commit: SchemaCommitOf::Update,
				},
			)?;

			<Schemas<T>>::insert(
				&identifier,
				SchemaDetails {
					hash,
					cid,
					parent_cid: schema_details.cid,
					block: block_number,
					..schema_details
				},
			);

			Self::deposit_event(Event::TxUpdate(identifier, hash, updater));

			Ok(())
		}
		/// Update the status of the schema - revoked or not
		///
		///This transaction can only be performed by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * status: status to be updated
		#[pallet::weight(0)]
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
					hash: schema_details.hash.clone(),
					cid: schema_details.cid.clone(),
					block: block_number,
					commit: SchemaCommitOf::StatusChange,
				},
			)?;

			<Schemas<T>>::insert(&identifier, SchemaDetails { revoked: status, ..schema_details });
			Self::deposit_event(Event::TxStatus(identifier, updater));

			Ok(())
		}
		/// Update the schema type - permissioned or not
		///
		/// This update can only be performed by by the schema controller
		///
		/// * origin: the identity of the schema controller.
		/// * identifier: unique identifier of the incoming stream.
		/// * status: status to be updated
		#[pallet::weight(0)]
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
					hash: schema_details.hash.clone(),
					cid: schema_details.cid.clone(),
					block: block_number,
					commit: SchemaCommitOf::Permission,
				},
			)?;

			<Schemas<T>>::insert(&identifier, SchemaDetails { permissioned, ..schema_details });
			Self::deposit_event(Event::TxPermission(identifier, updater));

			Ok(())
		}
	}
}
