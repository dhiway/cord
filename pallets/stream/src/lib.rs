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
pub mod streams;
pub mod weights;

pub use crate::streams::*;
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
	#[pallet::getter(fn transactions)]
	pub type Streams<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, TxDetails<T>>;

	/// transaction details stored on chain.
	/// It maps from a transaction Id to a vector of transaction details.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<TxCommits<T>>>;

	/// transactions stored on chain.
	/// It maps from a transaction Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn transactionids)]
	pub type StreamIds<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	/// entities verification information stored on chain.
	/// It maps from a entity Id to its verification status.
	#[pallet::storage]
	#[pallet::getter(fn entities)]
	pub type Entities<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	/// space links stored on chain.
	/// It maps from a space Id to a vector of space hashes.
	#[pallet::storage]
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	/// schema links stored on chain.
	/// It maps from a schema Id to a vector of schema hashes.
	#[pallet::storage]
	#[pallet::getter(fn schemas)]
	pub type Schemas<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	/// journal links stored on chain.
	/// It maps from a journal Id to a vector of journal hashes.
	#[pallet::storage]
	#[pallet::getter(fn journals)]
	pub type Journals<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	/// stream links stored on chain.
	/// It maps from a stream Id to a vector of stream hashes.
	#[pallet::storage]
	#[pallet::getter(fn streams)]
	pub type Documents<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	/// links stored on chain.
	/// It maps from a link Id to a vector of link hashes.
	#[pallet::storage]
	#[pallet::getter(fn links)]
	pub type Links<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	/// entities verification information stored on chain.
	/// It maps from a entity Id to its verification status.
	#[pallet::storage]
	#[pallet::getter(fn verifiedentities)]
	pub type VerifiedEntities<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, StatusOf>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new entity has been created.
		/// \[entity identifier, controller\]
		TransactionAdded(IdOf<T>, ControllerOf<T>),
		/// An entityhas been created.
		/// \[entity identifier, controller\]
		TransactionUpdated(IdOf<T>, ControllerOf<T>),
		/// An entity has been revoked.
		/// \[entity identifier\]
		TransactionStatusUpdated(IdOf<T>, ControllerOf<T>),
		/// A entity has been restored.
		/// \[entity identifier\]
		EntityVerificationStatusUpdated(IdOf<T>, ControllerOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid request
		InvalidRequest,
		/// Not all required inputs
		MissingInputDetails,
		/// Hash and ID are the same
		CheckHashAndId,
		/// Transaction idenfier is not unique
		IdAlreadyExists,
		/// Transaction idenfier not found
		IdNotFound,
		/// Transaction idenfier marked inactive
		IdNotActive,
		/// Transaction hash is not unique
		HashAlreadyExists,
		/// Transaction hash not found
		HashNotFound,
		/// Invalid CID encoding.
		InvalidCidEncoding,
		/// CID already anchored
		CidAlreadyMapped,
		/// Transaction Link identifier not found
		LinkIdNotFound,
		/// Transaction Link hash not found
		LinkHashNotFound,
		/// Transaction Link marked inactive
		LinkNotActive,
		/// Only when the author is not the controller.
		UnauthorizedUpdate,
		/// There is no entity with the given ID
		EntityNotFound,
		/// The entity already exists.
		EntityAlreadyExists,
		/// Entity Link not found
		EntityLinkNotFound,
		/// Transaction Link not found
		LinkNotFound,
		/// Entity or Space Link not found
		InvalidSpaceLink,
		/// Schema Link not found
		SchemaLinkNotFound,
		/// Space Link not found
		SpaceLinkNotFound,
		/// Journal Link not found
		JournalLinkNotFound,
		/// Document Link not found
		DocumentLinkNotFound,
		// Linked Schema not found
		SchemaNotFound,
		/// Schema Link marked inactive
		SchemaNotActive,
		/// Schema idenfier not found
		SchemaIdNotFound,
		/// Schema hash not found
		SchemaHashNotFound,
		/// Schema parent link is not matcing with transaction
		SchemaLinkMisMatch,
		/// The space is marked inactive.
		// EntityNotActive,
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
			tx_hash: HashOf<T>,
			tx_input: TxInput<T>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			//check request type
			ensure!(TypeOf::is_valid(&tx_input.tx_type), Error::<T>::InvalidRequest);
			//check input parameters
			ensure!(TxInput::<T>::is_valid(&tx_input), Error::<T>::MissingInputDetails);
			//check hash and id
			ensure!(tx_hash != tx_input.tx_id, Error::<T>::CheckHashAndId);
			//check cid encoding
			ensure!(TxInput::<T>::check_cid(&tx_input.tx_cid), Error::<T>::InvalidCidEncoding);
			//check transaction id
			ensure!(!<StreamIds<T>>::contains_key(&tx_input.tx_id), Error::<T>::IdAlreadyExists);
			//check transaction
			ensure!(!<Streams<T>>::contains_key(&tx_hash), Error::<T>::HashAlreadyExists);

			let tx_details = TxDetails::<T>::validate_tx(tx_input, controller.clone())?;
			TxCommits::<T>::store_tx(tx_hash, &tx_details, RequestOf::Create)?;

			log::debug!(
				"Creating a new entity with ID {:?} and controller {:?}",
				&tx_hash,
				&controller
			);
			<StreamIds<T>>::insert(&tx_details.tx_id, &tx_hash);
			<Streams<T>>::insert(&tx_hash, tx_details);
			Self::deposit_event(Event::TransactionAdded(tx_hash, controller));

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
			tx_cid: CidOf,
			tx_hash: HashOf<T>,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(TxInput::<T>::check_cid(&tx_cid), Error::<T>::InvalidCidEncoding);

			let tx_last_hash = <StreamIds<T>>::get(&tx_id).ok_or(Error::<T>::IdNotFound)?;
			let tx_last = <Streams<T>>::get(&tx_last_hash).ok_or(Error::<T>::HashNotFound)?;
			ensure!(tx_last.active, Error::<T>::IdNotActive);
			ensure!(tx_last.controller == updater, Error::<T>::UnauthorizedUpdate);
			ensure!(tx_cid != tx_last.tx_cid, Error::<T>::CidAlreadyMapped);

			if let Some(tx_last_link) = tx_last.tx_link {
				let tx_last_link_hash =
					<StreamIds<T>>::get(tx_last_link).ok_or(Error::<T>::LinkIdNotFound)?;
				let tx_last_link =
					<Streams<T>>::get(&tx_last_link_hash).ok_or(Error::<T>::LinkHashNotFound)?;
				ensure!(tx_last_link.active, Error::<T>::LinkNotActive);
			}
			let block_number = <frame_system::Pallet<T>>::block_number();

			let update_tx = TxDetails {
				tx_cid,
				ptx_cid: Some(tx_last.tx_cid.clone()),
				block: block_number,
				..tx_last.clone()
			};

			TxCommits::<T>::update_tx(tx_hash, &update_tx, RequestOf::Update)?;

			<Streams<T>>::insert(
				&tx_last_hash,
				TxDetails { block: block_number, active: false, ..tx_last },
			);
			log::debug!("Updating entity with id {:?} and owner {:?}", &tx_hash, &updater);
			<Streams<T>>::insert(&tx_hash, update_tx);
			<StreamIds<T>>::insert(&tx_id, &tx_hash);

			Self::deposit_event(Event::TransactionUpdated(tx_hash, updater));

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

			let tx_status_hash = <StreamIds<T>>::get(&tx_id).ok_or(Error::<T>::IdNotFound)?;
			let tx_status = <Streams<T>>::get(&tx_status_hash).ok_or(Error::<T>::HashNotFound)?;
			ensure!(tx_status.active == status, Error::<T>::StatusChangeNotRequired);
			ensure!(tx_status.controller == updater, Error::<T>::UnauthorizedUpdate);

			log::debug!("Changing Transaction Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut commit = <Commits<T>>::get(tx_status.tx_id).unwrap();
			commit.push(TxCommits {
				tx_type: tx_status.tx_type.clone(),
				tx_hash: tx_status_hash.clone(),
				tx_cid: tx_status.tx_cid.clone(),
				tx_link: tx_status.tx_link.clone(),
				block: block_number.clone(),
				commit: RequestOf::Status,
			});
			<Commits<T>>::insert(tx_status.tx_id, commit);

			log::debug!(
				"Updating transaction status with id {:?} and owner {:?}",
				&tx_status.tx_id,
				&updater
			);
			<Streams<T>>::insert(
				&tx_status_hash,
				TxDetails { block: block_number, active: status, ..tx_status },
			);
			Self::deposit_event(Event::TransactionStatusUpdated(tx_status_hash, updater));

			Ok(())
		}
		/// Update the verificationstatus of the entity
		///
		/// This update can only be performed by a registrar account
		/// * origin: the identifier of the registrar
		/// * entity_id: unique identifier of the entity.
		/// * status: status to be updated
		// #[pallet::weight(0)]
		// pub fn verify_entity(
		// 	origin: OriginFor<T>,
		// 	tx_id: IdOf<T>,
		// 	status: StatusOf,
		// ) -> DispatchResult {
		// 	let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
		// 	ensure!(<Entities<T>>::contains_key(tx_id), Error::<T>::EntityNotFound);

		// 	let entity_hash = <StreamIds<T>>::get(&tx_id).ok_or(Error::<T>::IdNotFound)?;

		// 	let tx_verify = <Streams<T>>::get(&entity_hash).ok_or(Error::<T>::HashNotFound)?;
		// 	let registrar = <pallet_registrar::Registrars<T>>::get(&updater)
		// 		.ok_or(pallet_registrar::Error::<T>::RegistrarAccountNotFound)?;
		// 	ensure!(!registrar.revoked, pallet_registrar::Error::<T>::RegistrarAccountRevoked);

		// 	log::debug!("Changing Entity Verification Status");
		// 	let block_number = <frame_system::Pallet<T>>::block_number();

		// 	// vector of entity activities linked to entity Id
		// 	let mut commit = <Commits<T>>::get(tx_id).unwrap_or_default();
		// 	commit.push(TxCommits {
		// 		tx_type: TypeOf::Entity,
		// 		tx_hash: entity_hash.clone(),
		// 		tx_cid: tx_verify.tx_cid,
		// 		tx_link: tx_verify.tx_link,
		// 		block: block_number,
		// 		commit: RequestOf::Verify,
		// 	});
		// 	<Commits<T>>::insert(&tx_id, commit);

		// 	<VerifiedEntities<T>>::insert(&tx_id, status);
		// 	Self::deposit_event(Event::EntityVerificationStatusUpdated(tx_id, updater));

		// 	Ok(())
		// }
	}
}
