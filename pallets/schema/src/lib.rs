// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};
pub mod schemas;
pub mod weights;

pub use crate::schemas::*;
use crate::weights::WeightInfo;
pub use pallet::*;
use pallet_entity::{RequestOf, TypeOf};

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
	pub type CordAccountOf<T> = pallet_entity::CordAccountOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// status Information
	pub type StatusOf = bool;
	/// CID type.
	pub type CidOf = Vec<u8>;
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_entity::Config + pallet_space::Config {
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// transactions stored on chain.
	/// It maps from a transaction Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn schemas)]
	pub type Schemas<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, SchemaDetails<T>>;

	/// transactions stored on chain.
	/// It maps from a transaction Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn schemahashes)]
	pub type SchemaHashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new entity has been created.
		/// \[entity identifier, controller\]
		TransactionAdded(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// An entity has been updated.
		/// \[entity identifier, controller\]
		TransactionUpdated(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// An entity transaction has been marked inactive.
		/// \[entity identifier, controller\]
		TransactionInactive(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// An entity has been revoked.
		/// \[entity identifier\]
		TransactionStatusUpdated(IdOf<T>, CordAccountOf<T>),
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
		/// Transaction ID is not unique
		SchemaIdAlreadyExists,
		/// Transaction hash is not unique
		SchemaHashAlreadyExists,
		/// Transaction hash not found
		SchemaHashNotFound,
		/// Invalid CID encoding.
		InvalidCidEncoding,
		/// CID already anchored
		CidAlreadyMapped,
		/// no status change required
		StatusChangeNotRequired,
		/// Only when the author is not the controller.
		UnauthorizedOperation,
		/// Only when the author is not the controller.
		SpaceSchemaError,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new entity and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * tx_id: unique identifier of the incoming stream.
		/// * tx_input: incoming stream details
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_hash: HashOf<T>,
			tx_cid: CidOf,
			tx_link: IdOf<T>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			//check hash and id
			ensure!(tx_hash != tx_id, Error::<T>::SameSchemaIdAndHash);
			//check cid encoding
			ensure!(
				pallet_entity::EntityDetails::<T>::check_cid(&tx_cid),
				Error::<T>::InvalidCidEncoding
			);
			//check transaction id
			ensure!(!<Schemas<T>>::contains_key(&tx_id), Error::<T>::SchemaAlreadyExists);

			let tx_entity = pallet_space::SpaceDetails::<T>::space_status(tx_link)
				.map_err(<pallet_space::Error<T>>::from)?;
			pallet_entity::EntityDetails::<T>::entity_status(tx_entity, controller.clone())
				.map_err(<pallet_entity::Error<T>>::from)?;

			let block_number = <frame_system::Pallet<T>>::block_number();

			pallet_entity::TxCommits::<T>::store_commit_tx(
				&tx_id,
				pallet_entity::TxCommits {
					tx_type: TypeOf::Schema,
					tx_hash: tx_hash.clone(),
					tx_cid: tx_cid.clone(),
					tx_link: Some(tx_link.clone()),
					block: block_number.clone(),
					commit: RequestOf::Create,
				},
			)?;

			<Schemas<T>>::insert(
				&tx_id,
				SchemaDetails {
					tx_hash: tx_hash.clone(),
					tx_cid,
					ptx_cid: None,
					tx_link,
					controller: controller.clone(),
					block: block_number,
				},
			);

			Self::deposit_event(Event::TransactionAdded(tx_id, tx_hash, controller));

			Ok(())
		}
		/// Updates the entity information and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * tx_id: unique identifier of the incoming stream.
		/// * tx_input: incoming stream details
		#[pallet::weight(0)]
		pub fn update(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_hash: HashOf<T>,
			tx_cid: CidOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			//check hash and id
			ensure!(tx_hash != tx_id, Error::<T>::SameSchemaIdAndHash);
			ensure!(
				pallet_entity::EntityDetails::<T>::check_cid(&tx_cid),
				Error::<T>::InvalidCidEncoding
			);

			let tx_prev = <Schemas<T>>::get(&tx_id).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(tx_prev.controller == updater, Error::<T>::UnauthorizedOperation);
			ensure!(tx_cid != tx_prev.tx_cid, Error::<T>::CidAlreadyMapped);

			let tx_entity = pallet_space::SpaceDetails::<T>::space_status(tx_prev.tx_link)
				.map_err(<pallet_space::Error<T>>::from)?;
			pallet_entity::EntityDetails::<T>::entity_status(tx_entity, updater.clone())
				.map_err(<pallet_entity::Error<T>>::from)?;

			let block_number = <frame_system::Pallet<T>>::block_number();

			pallet_entity::TxCommits::<T>::update_commit_tx(
				&tx_id,
				pallet_entity::TxCommits {
					tx_type: TypeOf::Schema,
					tx_hash: tx_hash.clone(),
					tx_cid: tx_cid.clone(),
					tx_link: Some(tx_prev.tx_link.clone()),
					block: block_number.clone(),
					commit: RequestOf::Update,
				},
			)?;

			<Schemas<T>>::insert(
				&tx_id,
				SchemaDetails {
					tx_hash,
					tx_cid,
					ptx_cid: Some(tx_prev.tx_cid),
					block: block_number,
					..tx_prev
				},
			);

			Self::deposit_event(Event::TransactionUpdated(tx_id, tx_hash, updater));

			Ok(())
		}
	}
}
