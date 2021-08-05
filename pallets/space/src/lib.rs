// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
// pub use cord_primitives::{CidOf, StatusOf};
// use frame_support::traits::Len;
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};
pub mod spaces;
pub mod weights;

pub use crate::spaces::*;
pub use pallet::*;
pub mod utils;
use crate::weights::WeightInfo;
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
	pub type ControllerOf<T> = pallet_entity::ControllerOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// status Information
	pub type StatusOf = bool;
	/// CID type.
	pub type CidOf = Vec<u8>;
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_entity::Config {
		type EnsureOrigin: EnsureOrigin<
			Success = ControllerOf<Self>,
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
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, SpaceDetails<T>>;

	/// transactions stored on chain.
	/// It maps from a transaction Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn spaceids)]
	pub type SpaceIds<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new entity has been created.
		/// \[entity identifier, controller\]
		TransactionAdded(IdOf<T>, HashOf<T>, ControllerOf<T>),
		/// An entity has been updated.
		/// \[entity identifier, controller\]
		TransactionUpdated(IdOf<T>, HashOf<T>, ControllerOf<T>),
		/// An entity transaction has been marked inactive.
		/// \[entity identifier, controller\]
		TransactionInactive(IdOf<T>, HashOf<T>, ControllerOf<T>),
		/// An entity has been revoked.
		/// \[entity identifier\]
		TransactionStatusUpdated(IdOf<T>, ControllerOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid request
		InvalidRequest,
		/// Not all required inputs
		MissingInputDetails,
		/// Hash and ID are the same
		SameHashAndId,
		/// Transaction idenfier is not unique
		SpaceAlreadyExists,
		/// Transaction idenfier not found
		SpaceNotFound,
		/// Transaction idenfier marked inactive
		SpaceNotActive,
		/// Transaction ID is not unique
		SpaceIdAlreadyExists,
		/// Transaction hash is not unique
		SpaceHashAlreadyExists,
		/// Transaction hash not found
		SpaceHashNotFound,
		/// Invalid CID encoding.
		InvalidCidEncoding,
		/// CID already anchored
		CidAlreadyMapped,
		/// Only when the author is not the controller.
		UnauthorizedUpdate,
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
			ensure!(tx_hash != tx_id, Error::<T>::SameHashAndId);
			//check cid encoding
			ensure!(
				pallet_entity::EntityDetails::<T>::check_cid(&tx_cid),
				Error::<T>::InvalidCidEncoding
			);
			//check transaction
			ensure!(!<Spaces<T>>::contains_key(&tx_hash), Error::<T>::SpaceAlreadyExists);
			//check transaction id
			ensure!(!<SpaceIds<T>>::contains_key(&tx_id), Error::<T>::SpaceIdAlreadyExists);
			//check transaction link
			let tx_link_hash = <pallet_entity::EntityIds<T>>::get(&tx_link)
				.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;
			let tx_link_details = <pallet_entity::Entities<T>>::get(&tx_link_hash)
				.ok_or(pallet_entity::Error::<T>::EntityHashNotFound)?;
			ensure!(tx_link_details.active, pallet_entity::Error::<T>::EntityNotActive);
			ensure!(
				tx_link_details.controller == controller,
				pallet_entity::Error::<T>::UnauthorizedOperation
			);

			let block_number = <frame_system::Pallet<T>>::block_number();

			let tx_details = SpaceDetails {
				tx_id: tx_id.clone(),
				tx_cid,
				ptx_cid: None,
				tx_link,
				controller: controller.clone(),
				block: block_number.clone(),
				active: true,
			};
			SpaceDetails::<T>::store_commit_tx(tx_hash, &tx_details, RequestOf::Create)?;
			<SpaceIds<T>>::insert(&tx_id, &tx_hash);
			<Spaces<T>>::insert(&tx_hash, tx_details);
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
			ensure!(
				pallet_entity::EntityDetails::<T>::check_cid(&tx_cid),
				Error::<T>::InvalidCidEncoding
			);

			let tx_prev_hash = <SpaceIds<T>>::get(&tx_id).ok_or(Error::<T>::SpaceNotFound)?;
			let tx_prev = <Spaces<T>>::get(&tx_prev_hash).ok_or(Error::<T>::SpaceHashNotFound)?;
			ensure!(tx_prev.active, Error::<T>::SpaceNotActive);
			ensure!(tx_prev.controller == updater, Error::<T>::UnauthorizedUpdate);
			ensure!(tx_cid != tx_prev.tx_cid, Error::<T>::CidAlreadyMapped);

			if let Some(tx_link_id) = &tx_link {
				let tx_link_hash = <pallet_entity::EntityIds<T>>::get(&tx_link_id)
					.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;
				let tx_link_details = <pallet_entity::Entities<T>>::get(&tx_link_hash)
					.ok_or(pallet_entity::Error::<T>::EntityHashNotFound)?;
				ensure!(tx_link_details.active, pallet_entity::Error::<T>::EntityNotActive);
				ensure!(
					tx_link_details.controller == updater,
					pallet_entity::Error::<T>::UnauthorizedOperation
				);
			}

			let block_number = <frame_system::Pallet<T>>::block_number();

			let update_tx = SpaceDetails {
				tx_cid,
				ptx_cid: Some(tx_prev.tx_cid.clone()),
				block: block_number,
				..tx_prev.clone()
			};

			SpaceDetails::<T>::update_commit_tx(tx_hash, &update_tx, RequestOf::Update)?;

			<Spaces<T>>::insert(
				&tx_prev_hash,
				SpaceDetails { block: block_number, active: false, ..tx_prev },
			);
			Self::deposit_event(Event::TransactionInactive(
				tx_id.clone(),
				tx_prev_hash,
				updater.clone(),
			));
			<Spaces<T>>::insert(&tx_hash, update_tx);
			<SpaceIds<T>>::insert(&tx_id, &tx_hash);

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

			let tx_status_hash = <SpaceIds<T>>::get(&tx_id).ok_or(Error::<T>::SpaceNotFound)?;
			let tx_status =
				<Spaces<T>>::get(&tx_status_hash).ok_or(Error::<T>::SpaceHashNotFound)?;
			ensure!(tx_status.active == status, Error::<T>::StatusChangeNotRequired);
			ensure!(tx_status.controller == updater, Error::<T>::UnauthorizedUpdate);

			log::debug!("Changing Transaction Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			let update_tx =
				SpaceDetails { block: block_number, active: status, ..tx_status.clone() };
			SpaceDetails::<T>::update_commit_tx(tx_status_hash, &update_tx, RequestOf::Status)?;
			<Spaces<T>>::insert(
				&tx_status_hash,
				SpaceDetails { block: block_number, active: status, ..tx_status },
			);
			Self::deposit_event(Event::TransactionStatusUpdated(tx_status_hash, updater));

			Ok(())
		}
	}
}
