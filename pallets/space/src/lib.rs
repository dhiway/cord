// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};
pub mod spaces;
pub mod weights;

pub use crate::spaces::*;
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
	/// Type of a entity creator.
	pub type CordAccountOf<T> = pallet_entity::CordAccountOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// status Information
	pub type StatusOf = bool;
	/// CID type.
	pub type CidOf = Vec<u8>;
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_entity::Config {
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
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, SpaceDetails<T>>;

	// /// transactions stored on chain.
	// /// It maps from a transaction Id to its details.
	// #[pallet::storage]
	// #[pallet::getter(fn spacehashes)]
	// pub type SpaceHashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new entity has been created.
		/// \[entity identifier, creator\]
		TransactionAdded(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// An entity has been updated.
		/// \[entity identifier, creator\]
		TransactionUpdated(IdOf<T>, HashOf<T>, CordAccountOf<T>),
		/// An entity has been revoked.
		/// \[entity identifier\]
		TransactionStatusUpdated(IdOf<T>, CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid request
		InvalidRequest,
		/// Hash and ID are the same
		SameSpaceIdAndHash,
		/// Transaction idenfier is not unique
		SpaceAlreadyExists,
		/// Transaction idenfier not found
		SpaceNotFound,
		/// Transaction idenfier marked inactive
		SpaceNotActive,
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
			ensure!(tx_hash != tx_id, Error::<T>::SameSpaceIdAndHash);
			//check cid encoding
			ensure!(
				pallet_entity::EntityDetails::<T>::check_cid(&tx_cid),
				pallet_entity::Error::<T>::InvalidCidEncoding
			);
			//check transaction id
			ensure!(!<Spaces<T>>::contains_key(&tx_id), Error::<T>::SpaceAlreadyExists);
			//check transaction link
			let tx_link_details = <pallet_entity::Entities<T>>::get(&tx_link)
				.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;
			ensure!(tx_link_details.active, pallet_entity::Error::<T>::EntityNotActive);
			ensure!(
				tx_link_details.controller == controller,
				pallet_entity::Error::<T>::UnauthorizedOperation
			);

			let block_number = <frame_system::Pallet<T>>::block_number();

			pallet_entity::TxCommits::<T>::store_commit_tx(
				&tx_id,
				pallet_entity::TxCommits {
					tx_type: TypeOf::Space,
					tx_hash: tx_hash.clone(),
					tx_cid: tx_cid.clone(),
					tx_link: Some(tx_link.clone()),
					block: block_number.clone(),
					commit: RequestOf::Create,
				},
			)?;

			<Spaces<T>>::insert(
				&tx_id,
				SpaceDetails {
					tx_hash: tx_hash.clone(),
					tx_cid,
					ptx_cid: None,
					tx_link,
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
		/// * origin: the identifier of the schema owner
		/// * tx_id: unique identifier of the incoming stream.
		/// * tx_input: incoming stream details
		#[pallet::weight(0)]
		pub fn update(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			tx_hash: HashOf<T>,
			tx_cid: CidOf,
			tx_link: Option<IdOf<T>>,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			//check hash and id
			ensure!(tx_hash != tx_id, Error::<T>::SameSpaceIdAndHash);
			ensure!(
				pallet_entity::EntityDetails::<T>::check_cid(&tx_cid),
				pallet_entity::Error::<T>::InvalidCidEncoding
			);

			let mut tx_prev = <Spaces<T>>::get(&tx_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(tx_prev.active, Error::<T>::SpaceNotActive);
			ensure!(tx_prev.controller == updater, Error::<T>::UnauthorizedOperation);
			ensure!(tx_cid != tx_prev.tx_cid, Error::<T>::CidAlreadyMapped);

			if let Some(tx_link_id) = &tx_link {
				let tx_link_details = <pallet_entity::Entities<T>>::get(&tx_link_id)
					.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;
				ensure!(tx_link_details.active, pallet_entity::Error::<T>::EntityNotActive);
				ensure!(
					tx_link_details.controller == updater,
					pallet_entity::Error::<T>::UnauthorizedOperation
				);
				tx_prev.tx_link = *tx_link_id;
			}

			let block_number = <frame_system::Pallet<T>>::block_number();

			pallet_entity::TxCommits::<T>::update_commit_tx(
				&tx_id,
				pallet_entity::TxCommits {
					tx_type: TypeOf::Space,
					tx_hash: tx_hash.clone(),
					tx_cid: tx_cid.clone(),
					tx_link: Some(tx_prev.tx_link.clone()),
					block: block_number.clone(),
					commit: RequestOf::Update,
				},
			)?;

			<Spaces<T>>::insert(
				&tx_id,
				SpaceDetails {
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

			let tx_status = <Spaces<T>>::get(&tx_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(tx_status.active != status, Error::<T>::StatusChangeNotRequired);
			ensure!(tx_status.controller == updater, Error::<T>::UnauthorizedOperation);

			log::debug!("Changing Transaction Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			pallet_entity::TxCommits::<T>::update_commit_tx(
				&tx_id,
				pallet_entity::TxCommits {
					tx_type: TypeOf::Space,
					tx_hash: tx_status.tx_hash.clone(),
					tx_cid: tx_status.tx_cid.clone(),
					tx_link: Some(tx_status.tx_link.clone()),
					block: block_number.clone(),
					commit: RequestOf::Status,
				},
			)?;

			<Spaces<T>>::insert(
				&tx_id,
				SpaceDetails { block: block_number, active: status, ..tx_status },
			);

			Self::deposit_event(Event::TransactionStatusUpdated(tx_id, updater));

			Ok(())
		}
	}
}
