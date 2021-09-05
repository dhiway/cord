// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};
pub mod entities;
pub mod weights;

pub use crate::entities::*;
pub use pallet::*;
pub mod utils;
use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// ID of an entity.
	pub type IdOf<T> = <T as frame_system::Config>::Hash;
	/// Hash of the transaction.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of an entity account.
	pub type CordAccountOf<T> = <T as Config>::CordAccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// status Information
	pub type StatusOf = bool;
	/// CID type.
	pub type CidOf = Vec<u8>;
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type CordAccountId: Parameter + Default;
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
	#[pallet::getter(fn entities)]
	pub type Entities<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, EntityDetails<T>>;

	/// transaction details stored on chain.
	/// It maps from a transaction Id to a vector of transaction details.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<TxCommits<T>>>;

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
		SameEntityIdAndHash,
		/// Transaction idenfier is not unique
		EntityAlreadyExists,
		/// Transaction idenfier not found
		EntityNotFound,
		/// Transaction idenfier marked inactive
		EntityNotActive,
		/// Invalid CID encoding.
		InvalidCidEncoding,
		/// CID already anchored
		CidAlreadyMapped,
		/// no status change required
		StatusChangeNotRequired,
		/// Only when the author is not the controller.
		UnauthorizedOperation,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new entity and associates it with its owner.
		///
		/// * origin: the identifier of the entity owner
		/// * tx_id: unique identifier of the incoming entity stream.
		/// * tx_hash: hash of the incoming entity stream.
		/// * tx_cid: incoming entity stream Cid
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_hash: HashOf<T>,
			tx_cid: CidOf,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			//check incoming Id and hash
			ensure!(tx_hash != tx_id, Error::<T>::SameEntityIdAndHash);
			//check cid encoding
			ensure!(EntityDetails::<T>::check_cid(&tx_cid), Error::<T>::InvalidCidEncoding);
			//check entity Id
			ensure!(!<Entities<T>>::contains_key(&tx_id), Error::<T>::EntityAlreadyExists);
			let block_number = <frame_system::Pallet<T>>::block_number();

			TxCommits::<T>::store_commit_tx(
				&tx_id,
				TxCommits {
					tx_type: TypeOf::Entity,
					tx_hash: tx_hash.clone(),
					tx_cid: tx_cid.clone(),
					tx_link: None,
					block: block_number.clone(),
					commit: CommitOf::Genesis,
				},
			)?;

			<Entities<T>>::insert(
				&tx_id,
				EntityDetails {
					tx_hash: tx_hash.clone(),
					tx_cid,
					ptx_cid: None,
					controller: controller.clone(),
					block: block_number,
					active: true,
				},
			);
			Self::deposit_event(Event::TransactionAdded(tx_id, tx_hash, controller));

			Ok(())
		}
		/// Updates the entity information and associates it with its owner.
		///
		/// * origin: the identifier of the entity owner
		/// * tx_id: unique identifier of the incoming entity stream.
		/// * tx_id: hash of the incoming entity stream.
		/// * tx_cid: incoming entity stream Cid
		#[pallet::weight(0)]
		pub fn update(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_hash: HashOf<T>,
			tx_cid: CidOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(EntityDetails::<T>::check_cid(&tx_cid), Error::<T>::InvalidCidEncoding);

			let tx_prev = <Entities<T>>::get(&tx_id).ok_or(Error::<T>::EntityNotFound)?;
			ensure!(tx_prev.active, Error::<T>::EntityNotActive);
			ensure!(tx_prev.controller == updater, Error::<T>::UnauthorizedOperation);
			ensure!(tx_cid != tx_prev.tx_cid, Error::<T>::CidAlreadyMapped);

			let block_number = <frame_system::Pallet<T>>::block_number();

			TxCommits::<T>::update_commit_tx(
				&tx_id,
				TxCommits {
					tx_type: TypeOf::Entity,
					tx_hash: tx_hash.clone(),
					tx_cid: tx_cid.clone(),
					tx_link: None,
					block: block_number.clone(),
					commit: CommitOf::Update,
				},
			)?;

			<Entities<T>>::insert(
				&tx_id,
				EntityDetails {
					tx_hash: tx_hash.clone(),
					tx_cid,
					ptx_cid: Some(tx_prev.tx_cid),
					block: block_number,
					..tx_prev
				},
			);

			Self::deposit_event(Event::TransactionUpdated(tx_id, tx_hash, updater));

			Ok(())
		}
		/// Update the status of the entity - active or not
		///
		/// This update can only be performed by a registrar account
		/// * origin: the identifier of the registrar
		/// * tx_type: type of the request - entity or space
		/// * tx_id: unique identifier of the incoming stream.
		/// * status: status to be updated
		#[pallet::weight(0)]
		pub fn set_status(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let tx_status = <Entities<T>>::get(&tx_id).ok_or(Error::<T>::EntityNotFound)?;
			ensure!(tx_status.active != status, Error::<T>::StatusChangeNotRequired);
			ensure!(tx_status.controller == updater, Error::<T>::UnauthorizedOperation);

			log::debug!("Changing Transaction Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			TxCommits::<T>::update_commit_tx(
				&tx_id,
				TxCommits {
					tx_type: TypeOf::Entity,
					tx_hash: tx_status.tx_hash.clone(),
					tx_cid: tx_status.tx_cid.clone(),
					tx_link: None,
					block: block_number.clone(),
					commit: CommitOf::Status,
				},
			)?;

			<Entities<T>>::insert(
				&tx_id,
				EntityDetails { block: block_number, active: status, ..tx_status },
			);

			Self::deposit_event(Event::TransactionStatusUpdated(tx_id, updater));

			Ok(())
		}
	}
}
