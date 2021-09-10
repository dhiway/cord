// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};
pub mod schemas;
pub mod weights;

// #[cfg(any(feature = "mock", test))]
// pub mod mock;

pub use crate::schemas::*;
pub mod utils;
use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// ID of an entity.
	pub type IdOf<T> = <T as frame_system::Config>::Hash;
	/// Hash of the transaction.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a entity controller.
	pub type CordAccountOf<T> = <T as Config>::CordAccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// status Information
	pub type StatusOf = bool;
	/// CID type.
	pub type CidOf = Vec<u8>;
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
	/// It maps from a schema Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn schemas)]
	pub type Schemas<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, SchemaDetails<T>>;

	/// schema commits stored on chain.
	/// It maps from a schema Id to a vector of commit details.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<SchemaCommit<T>>>;

	/// transactions stored on chain.
	/// It maps from a transaction Id to its details.
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
		TxAddDelegate(IdOf<T>, CordAccountOf<T>),
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
		/// Hash and ID are the same
		SameSchemaIdAndHash,
		/// Transaction idenfier is not unique
		SchemaAlreadyExists,
		/// Transaction idenfier not found
		SchemaNotFound,
		/// Transaction idenfier marked inactive
		SchemaNotActive,
		/// Invalid CID encoding.
		InvalidCidEncoding,
		/// CID already anchored
		CidAlreadyMapped,
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new schema delegate.
		///
		///This transaction can only be performed by the schema controller account
		/// * origin: the identifier of the schema owner
		/// * tx_schema: unique identifier of the schema.
		/// * tx_delegate: schema delegate to add
		#[pallet::weight(0)]
		pub fn add_delegate(
			origin: OriginFor<T>,
			tx_schema: IdOf<T>,
			tx_delegate: CordAccountOf<T>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema = <Schemas<T>>::get(&tx_schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema.permissioned, Error::<T>::SchemaNotPermissioned);
			ensure!(schema.controller == controller, Error::<T>::UnauthorizedOperation);
			let block_number = <frame_system::Pallet<T>>::block_number();

			Delegations::<T>::try_mutate(&tx_schema, |ref mut delegates| {
				ensure!(
					delegates.len() < T::MaxDelegates::get() as usize,
					Error::<T>::TooManyDelegates
				);
				delegates
					.try_push(tx_delegate.clone())
					.expect("delegates length checked above; qed");
				SchemaCommit::<T>::store_tx(
					&tx_schema,
					SchemaCommit {
						tx_hash: schema.tx_hash,
						tx_cid: schema.tx_cid,
						block: block_number,
						commit: SchemaCommitOf::Delegate,
					},
				)?;
				Self::deposit_event(Event::TxAddDelegate(tx_schema, tx_delegate));
				Ok(())
			})
		}
		/// Remove a schema delegate.
		///
		///This transaction can only be performed by the schema controller account
		/// * origin: the identifier of the schema owner
		/// * tx_schema: unique identifier of the schema.
		/// * tx_delegate: schema delegate to be removed
		#[pallet::weight(0)]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			tx_schema: IdOf<T>,
			tx_delegate: CordAccountOf<T>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let schema = <Schemas<T>>::get(&tx_schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema.permissioned, Error::<T>::SchemaNotPermissioned);
			ensure!(schema.controller == controller, Error::<T>::UnauthorizedOperation);
			let block_number = <frame_system::Pallet<T>>::block_number();

			Delegations::<T>::try_mutate(&tx_schema, |ref mut delegates| {
				delegates.retain(|x| x != &tx_delegate);
				SchemaCommit::<T>::store_tx(
					&tx_schema,
					SchemaCommit {
						tx_hash: schema.tx_hash,
						tx_cid: schema.tx_cid,
						block: block_number,
						commit: SchemaCommitOf::Delegate,
					},
				)?;
				Self::deposit_event(Event::TxRemoveDelegate(tx_schema, tx_delegate));
				Ok(())
			})
		}

		/// Create a new schema and associates it with its controller.
		///
		/// * origin: the identifier of the schema owner
		/// * tx_id: unique identifier of the incoming schema stream.
		/// * tx_hash: hash of the incoming schema stream.
		/// * tx_cid: CID of the incoming schema stream.
		/// * tx_perm: schema type - permissioned or not.
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_hash: HashOf<T>,
			tx_cid: Option<CidOf>,
			tx_perm: StatusOf,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			//check transaction id
			ensure!(!<Schemas<T>>::contains_key(&tx_id), Error::<T>::SchemaAlreadyExists);
			//check hash and id
			ensure!(tx_hash != tx_id, Error::<T>::SameSchemaIdAndHash);
			//check cid encoding
			if let Some(ref tx_cid) = tx_cid {
				ensure!(SchemaDetails::<T>::check_cid(&tx_cid), Error::<T>::InvalidCidEncoding);
			}
			let block_number = <frame_system::Pallet<T>>::block_number();

			SchemaCommit::<T>::store_tx(
				&tx_id,
				SchemaCommit {
					tx_hash: tx_hash.clone(),
					tx_cid: tx_cid.clone(),
					block: block_number.clone(),
					commit: SchemaCommitOf::Genesis,
				},
			)?;
			if tx_perm {
				Delegations::<T>::mutate(&tx_id, |ref mut delegates| {
					delegates
						.try_push(controller.clone())
						.expect("delegates length checked above; qed");
				});
			}
			<Schemas<T>>::insert(
				&tx_id,
				SchemaDetails {
					tx_hash: tx_hash.clone(),
					tx_cid,
					ptx_cid: None,
					controller: controller.clone(),
					block: block_number.clone(),
					permissioned: tx_perm,
					revoked: false,
				},
			);

			Self::deposit_event(Event::TxAdd(tx_id, tx_hash, controller));

			Ok(())
		}
		/// Update an existing schema.
		///
		///This transaction can only be performed by the schema controller account
		/// * origin: the identifier of the schema owner
		/// * tx_id: unique identifier of the incoming schema stream.
		/// * tx_hash: hash of the incoming schema stream.
		/// * tx_cid: CID of the incoming schema stream.
		#[pallet::weight(0)]
		pub fn update(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_hash: HashOf<T>,
			tx_cid: Option<CidOf>,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			//check hash and id
			ensure!(tx_hash != tx_id, Error::<T>::SameSchemaIdAndHash);
			//check cid encoding
			if let Some(ref tx_cid) = tx_cid {
				ensure!(SchemaDetails::<T>::check_cid(&tx_cid), Error::<T>::InvalidCidEncoding);
			}
			let schema_details = <Schemas<T>>::get(&tx_id).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaNotActive);
			ensure!(schema_details.controller == updater, Error::<T>::UnauthorizedOperation);

			let block_number = <frame_system::Pallet<T>>::block_number();

			SchemaCommit::<T>::store_tx(
				&tx_id,
				SchemaCommit {
					tx_hash: tx_hash.clone(),
					tx_cid: tx_cid.clone(),
					block: block_number,
					commit: SchemaCommitOf::Update,
				},
			)?;

			<Schemas<T>>::insert(
				&tx_id,
				SchemaDetails {
					tx_hash,
					tx_cid,
					ptx_cid: schema_details.tx_cid,
					block: block_number,
					..schema_details
				},
			);

			Self::deposit_event(Event::TxUpdate(tx_id, tx_hash, updater));

			Ok(())
		}
		/// Update the status of the schema - revoked or not
		///
		///This transaction can only be performed by the schema controller account
		/// * origin: the identifier of the registrar
		/// * tx_id: unique identifier of the incoming stream.
		/// * status: status to be updated
		#[pallet::weight(0)]
		pub fn set_status(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let schema_details = <Schemas<T>>::get(&tx_id).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked != status, Error::<T>::StatusChangeNotRequired);
			ensure!(schema_details.controller == updater, Error::<T>::UnauthorizedOperation);
			let block_number = <frame_system::Pallet<T>>::block_number();

			SchemaCommit::<T>::store_tx(
				&tx_id,
				SchemaCommit {
					tx_hash: schema_details.tx_hash.clone(),
					tx_cid: schema_details.tx_cid.clone(),
					block: block_number,
					commit: SchemaCommitOf::Status,
				},
			)?;

			<Schemas<T>>::insert(&tx_id, SchemaDetails { revoked: status, ..schema_details });
			Self::deposit_event(Event::TxStatus(tx_id, updater));

			Ok(())
		}
		/// Update the schema type - permissioned or not
		///
		/// This update can only be performed by a registrar account
		/// * origin: the identifier of the registrar
		/// * tx_id: unique identifier of the incoming stream.
		/// * status: status to be updated
		#[pallet::weight(0)]
		pub fn set_permission(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_perm: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let schema_details = <Schemas<T>>::get(&tx_id).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema_details.revoked, Error::<T>::SchemaNotActive);
			ensure!(schema_details.permissioned != tx_perm, Error::<T>::NoPermissionChangeRequired);
			ensure!(schema_details.controller == updater, Error::<T>::UnauthorizedOperation);
			let block_number = <frame_system::Pallet<T>>::block_number();

			SchemaCommit::<T>::store_tx(
				&tx_id,
				SchemaCommit {
					tx_hash: schema_details.tx_hash.clone(),
					tx_cid: schema_details.tx_cid.clone(),
					block: block_number,
					commit: SchemaCommitOf::Permission,
				},
			)?;

			<Schemas<T>>::insert(&tx_id, SchemaDetails { permissioned: tx_perm, ..schema_details });
			Self::deposit_event(Event::TxPermission(tx_id, updater));

			Ok(())
		}
	}
}
