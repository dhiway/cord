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
	/// Schema of the transaction.
	// pub type SchemaIdOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a entity controller.
	pub type ControllerOf<T> = pallet_registrar::CordAccountOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// status Information
	pub type StatusOf = bool;
	/// CID type.
	pub type CidOf = Vec<u8>;
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_registrar::Config {
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
	pub type Transactions<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, TxDetails<T>>;

	/// transactions stored on chain.
	/// It maps from a transaction Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn transactionids)]
	pub type TransactionIds<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	/// transaction details stored on chain.
	/// It maps from a transaction Id to a vector of transaction details.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<TxCommits<T>>>;

	/// entities verification information stored on chain.
	/// It maps from a entity Id to its verification status.
	#[pallet::storage]
	#[pallet::getter(fn entities)]
	pub type Entities<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<HashOf<T>>>;
	/// entities verification information stored on chain.
	/// It maps from a entity Id to its verification status.
	#[pallet::storage]
	#[pallet::getter(fn verifiedentities)]
	pub type VerifiedEntities<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, TxVerifiedOf<T>>;

	/// space links stored on chain.
	/// It maps from a space Id to the linked Id.
	#[pallet::storage]
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<HashOf<T>>>;

	/// space links stored on chain.
	/// It maps from a space Id to the linked Id.
	#[pallet::storage]
	#[pallet::getter(fn schemahashes)]
	pub type SchemaHashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdOf<T>>;

	/// space links stored on chain.
	/// It maps from a space Id to the linked Id.
	#[pallet::storage]
	#[pallet::getter(fn schemas)]
	pub type Schemas<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<HashOf<T>>>;
	// /// space links stored on chain.
	// /// It maps from a space Id to the linked Id.
	// #[pallet::storage]
	// #[pallet::getter(fn schemaids)]
	// pub type SchemaIds<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, HashOf<T>>;

	/// space links stored on chain.
	/// It maps from a space Id to the linked Id.
	#[pallet::storage]
	#[pallet::getter(fn journals)]
	pub type Journals<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<HashOf<T>>>;

	// /// space links stored on chain.
	// /// It maps from a space Id to the linked Id.
	// #[pallet::storage]
	// #[pallet::getter(fn journalhashes)]
	// pub type JournalHashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdOf<T>>;

	/// space links stored on chain.
	/// It maps from a space Id to the linked Id.
	#[pallet::storage]
	#[pallet::getter(fn streams)]
	pub type Streams<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<HashOf<T>>>;

	/// space links stored on chain.
	/// It maps from a space Id to the linked Id.
	// #[pallet::storage]
	// #[pallet::getter(fn streamhashes)]
	// pub type StreamHashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdOf<T>>;

	/// space links stored on chain.
	/// It maps from a space Id to the linked Id.
	#[pallet::storage]
	#[pallet::getter(fn links)]
	pub type Links<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<HashOf<T>>>;

	/// space links stored on chain.
	/// It maps from a space Id to the linked Id.
	// #[pallet::storage]
	// #[pallet::getter(fn linkhashes)]
	// pub type LinkHashes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdOf<T>>;

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
		TransactionStatusUpdated(IdOf<T>),
		/// A entity has been restored.
		/// \[entity identifier\]
		EntityVerificationStatusUpdated(IdOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no entity with the given ID.
		EntityNotFound,
		/// The entity already exists.
		EntityAlreadyExists,
		/// Invalid CID encoding.
		InvalidCidEncoding,
		/// Only when the author is not the controller.
		UnauthorizedUpdate,
		/// There is no schema with the given parent CID.
		CidAlreadyMapped,
		/// The space is marked inactive.
		EntityNotActive,
		/// no status change required
		NoChangeRequired,
		TransactionAlreadyExists,
		LinkNotFound,
		LinkNotActive,
		UnauthorizedOperation,
		MissingTransactionLink,
		TransactionNotFound,
		EntryNotActive,
		InvalidTransactionRequest,
		MissingSchemaDetails,
		SchemaNotFound,
		SchemaNotActive,
		LinkMissing,
		SchemaNotLinked,
		JournalNotFound,
		JournalNotLinked,
		StreamlNotFound,
		StreamNotLinked,
		StreamNotFound,
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

			ensure!(TypeOf::is_valid(&tx_input.tx_type), Error::<T>::InvalidTransactionRequest);
			if &tx_input.tx_type != &TypeOf::Entity {
				ensure!(
					(tx_input.tx_link_id.is_some() || tx_input.tx_link_hash.is_some()),
					Error::<T>::MissingTransactionLink
				);
			}
			if &tx_input.tx_type != &TypeOf::Entity || &tx_input.tx_type != &TypeOf::Space {
				ensure!(
					(tx_input.tx_schema_id.is_some() || tx_input.tx_schema_hash.is_some()),
					Error::<T>::MissingTransactionLink
				);
			}

			if let Some(tx_cid) = &tx_input.tx_cid {
				ensure!(TxDetails::<T>::check_cid(&tx_cid), Error::<T>::InvalidCidEncoding);
			}

			ensure!(
				!<Transactions<T>>::contains_key(&tx_hash),
				Error::<T>::TransactionAlreadyExists
			);

			// store entitty verification status
			// store space link
			//store commit log
			let commit = RequestOf::Create;
			// let block_number = <frame_system::Pallet<T>>::block_number();
			let tx_details = TxDetails::<T>::validate_tx(tx_input, controller.clone())?;
			TxCommits::<T>::store_tx(tx_hash, &tx_details, commit)?;
			Pallet::<T>::deposit_event(Event::TransactionAdded(tx_hash, controller.clone()));

			log::debug!(
				"Creating a new entity with ID {:?} and controller {:?}",
				&tx_hash,
				&controller
			);
			<TransactionIds<T>>::insert(&tx_details.tx_id, &tx_hash);
			<Transactions<T>>::insert(&tx_hash, tx_details);

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
			tx_hash: HashOf<T>,
			tx_input: TxInput<T>,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(TypeOf::is_valid(&tx_input.tx_type), Error::<T>::InvalidTransactionRequest);

			let tx_update =
				<Transactions<T>>::get(&tx_hash).ok_or(Error::<T>::TransactionNotFound)?;
			ensure!(tx_update.active, Error::<T>::EntryNotActive);
			ensure!(tx_update.controller == updater, Error::<T>::UnauthorizedUpdate);

			let update_tx = TxDetails::<T>::map_update_tx(&tx_input, &tx_update)?;

			if &update_tx.tx_type != &TypeOf::Entity {
				ensure!(
					(update_tx.tx_link.tx_id.is_some() || update_tx.tx_link.tx_hash.is_some()),
					Error::<T>::MissingTransactionLink
				);
			}
			if &tx_input.tx_type != &TypeOf::Entity || &tx_input.tx_type != &TypeOf::Space {
				ensure!(
					(update_tx.tx_schema.tx_id.is_some() || update_tx.tx_schema.tx_id.is_some()),
					Error::<T>::MissingTransactionLink
				);
			}
			let commit = RequestOf::Update;
			// let tx_details = TxDetails::<T>::validate_tx(tx_input, controller)?;
			TxCommits::<T>::store_tx(tx_hash, &update_tx, commit)?;
			// Pallet::<T>::deposit_event(Event::TransactionAdded(tx_hash, &updater));

			log::debug!("Creating entity with id {:?} and owner {:?}", &tx_hash, &updater);
			<TransactionIds<T>>::insert(&tx_update.tx_id, &tx_hash);
			<Transactions<T>>::insert(&tx_hash, tx_update);
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
			tx_type: TypeOf,
			tx_hash: HashOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(TypeOf::is_valid(&tx_type), Error::<T>::InvalidTransactionRequest);

			let tx_status =
				<Transactions<T>>::get(&tx_hash).ok_or(Error::<T>::TransactionNotFound)?;
			ensure!(tx_status.active == status, Error::<T>::NoChangeRequired);
			ensure!(tx_status.controller == updater, Error::<T>::UnauthorizedUpdate);

			log::debug!("Changing Transaction Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut commit = <Commits<T>>::get(tx_status.tx_id).unwrap_or_default();
			commit.push(TxCommits {
				tx_type,
				tx_hash,
				tx_cid: tx_status.tx_storage.tx_cid.clone(),
				tx_link: tx_status.tx_link.tx_hash.clone(),
				block: block_number,
				commit: RequestOf::Status,
			});
			<Commits<T>>::insert(tx_status.tx_id, commit);

			log::debug!(
				"Updating transaction status with id {:?} and owner {:?}",
				&tx_status.tx_id,
				&updater
			);
			<Transactions<T>>::insert(
				&tx_hash,
				TxDetails { block: block_number, active: status, ..tx_status },
			);
			Self::deposit_event(Event::TransactionStatusUpdated(tx_hash));

			Ok(())
		}
		/// Update the verificationstatus of the entity
		///
		/// This update can only be performed by a registrar account
		/// * origin: the identifier of the registrar
		/// * entity_id: unique identifier of the entity.
		/// * status: status to be updated
		#[pallet::weight(0)]
		pub fn verify_entity(
			origin: OriginFor<T>,
			tx_id: IdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(<Entities<T>>::contains_key(tx_id), Error::<T>::EntityNotFound);

			let entity_hash =
				<VerifiedEntities<T>>::get(&tx_id).ok_or(Error::<T>::EntityNotFound)?;
			ensure!(entity_hash.tx_verified == status, Error::<T>::NoChangeRequired);

			let tx_verify = <Transactions<T>>::get(entity_hash.tx_hash)
				.ok_or(Error::<T>::TransactionNotFound)?;

			let registrar = <pallet_registrar::Registrars<T>>::get(&updater)
				.ok_or(pallet_registrar::Error::<T>::RegistrarAccountNotFound)?;
			ensure!(!registrar.revoked, pallet_registrar::Error::<T>::RegistrarAccountRevoked);

			log::debug!("Changing Entity Verification Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut commit = <Commits<T>>::get(tx_id).unwrap_or_default();
			commit.push(TxCommits {
				tx_type: TypeOf::Entity,
				tx_hash: entity_hash.tx_hash,
				tx_cid: tx_verify.tx_storage.tx_cid,
				tx_link: tx_verify.tx_link.tx_hash,
				block: block_number,
				commit: RequestOf::Verify,
			});
			<Commits<T>>::insert(tx_id, commit);

			<VerifiedEntities<T>>::insert(
				&tx_id,
				TxVerifiedOf { tx_verified: status, ..entity_hash },
			);
			Self::deposit_event(Event::EntityVerificationStatusUpdated(tx_id));

			Ok(())
		}
	}
}
