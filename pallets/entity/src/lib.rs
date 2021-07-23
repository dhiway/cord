// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use cord_primitives::{CidOf, StatusOf};
use frame_support::traits::Len;
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
	/// Type of a entity controller.
	pub type ControllerOf<T> = pallet_registrar::CordAccountOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

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
	pub type Transactions<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, TxDetails<T>>;

	/// transaction details stored on chain.
	/// It maps from a transaction Id to a vector of transaction details.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Vec<TxCommits<T>>>;

	/// entities verification information stored on chain.
	/// It maps from a entity Id to its verification status.
	#[pallet::storage]
	#[pallet::getter(fn entities)]
	pub type Entities<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, bool>;

	/// space links stored on chain.
	/// It maps from a space Id to the linked Id.
	#[pallet::storage]
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, Option<IdOf<T>>>;

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
			tx_input: TxInput<T>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				(tx_input.tx_req == TypeOf::Entity || tx_input.tx_req == TypeOf::Space),
				Error::<T>::InvalidTransactionRequest
			);
			ensure!(!<Transactions<T>>::contains_key(&tx_id), Error::<T>::TransactionAlreadyExists);

			if tx_input.tx_req == TypeOf::Space {
				ensure!(tx_input.tx_link.len() > 0, Error::<T>::MissingTransactionLink);
			}

			if let Some(ref tx_link) = tx_input.tx_link {
				let link = <Transactions<T>>::get(tx_link).ok_or(Error::<T>::LinkNotFound)?;
				ensure!(link.active, Error::<T>::LinkNotActive);
				ensure!(link.controller == controller, Error::<T>::UnauthorizedOperation);
			}
			if let Some(ref tx_cid) = tx_input.tx_cid {
				let cid_base = str::from_utf8(&tx_cid).unwrap();
				ensure!(
					cid_base.len() <= 62
						&& (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
					Error::<T>::InvalidCidEncoding
				);
			}
			let block_number = <frame_system::Pallet<T>>::block_number();

			//store commit log
			let mut commits = <Commits<T>>::get(tx_id).unwrap_or_default();
			commits.push(TxCommits {
				tx_type: tx_input.tx_type.clone(),
				tx_hash: tx_input.tx_hash.clone(),
				tx_cid: tx_input.tx_cid.clone(),
				tx_link: tx_input.tx_link.clone(),
				block: block_number.clone(),
				commit: RequestOf::Create,
			});
			<Commits<T>>::insert(tx_id, commits);

			// store entitty verification status
			// store space link
			match tx_input.tx_req {
				TypeOf::Entity => {
					<Entities<T>>::insert(tx_id, false);
				}
				TypeOf::Space => {
					<Spaces<T>>::insert(tx_id, tx_input.tx_link);
				}
			}
			log::debug!(
				"Creating a new entity with ID {:?} and controller {:?}",
				&tx_id,
				&controller
			);
			<Transactions<T>>::insert(
				&tx_id,
				TxDetails {
					controller: controller.clone(),
					tx_hash: tx_input.tx_hash,
					tx_cid: tx_input.tx_cid,
					ptx_cid: None,
					tx_link: tx_input.tx_link,
					block: block_number,
					active: true,
				},
			);

			Self::deposit_event(Event::TransactionAdded(tx_id, controller));

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
			tx_input: TxInput<T>,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				(tx_input.tx_req == TypeOf::Entity || tx_input.tx_req == TypeOf::Space),
				Error::<T>::InvalidTransactionRequest
			);
			let tx_update =
				<Transactions<T>>::get(&tx_id).ok_or(Error::<T>::TransactionNotFound)?;
			ensure!(tx_update.active, Error::<T>::EntryNotActive);
			ensure!(tx_update.controller == updater, Error::<T>::UnauthorizedUpdate);

			if tx_input.tx_req == TypeOf::Space {
				ensure!(tx_input.tx_link.len() > 0, Error::<T>::MissingTransactionLink);
			}

			if let Some(ref tx_link) = tx_input.tx_link {
				let link = <Transactions<T>>::get(tx_link).ok_or(Error::<T>::LinkNotFound)?;
				ensure!(link.active, Error::<T>::LinkNotActive);
				ensure!(link.controller == updater, Error::<T>::UnauthorizedOperation);
			}

			if let Some(ref tx_cid) = tx_input.tx_cid {
				let cid_base = str::from_utf8(&tx_cid).unwrap();
				ensure!(
					cid_base.len() <= 62
						&& (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
					Error::<T>::InvalidCidEncoding
				);
			}

			let block_number = <frame_system::Pallet<T>>::block_number();
			let mut commits = <Commits<T>>::get(tx_id).unwrap_or_default();
			commits.push(TxCommits {
				tx_type: tx_input.tx_type.clone(),
				tx_hash: tx_input.tx_hash.clone(),
				tx_cid: tx_input.tx_cid.clone(),
				tx_link: tx_input.tx_link.clone(),
				block: block_number.clone(),
				commit: RequestOf::Update,
			});
			<Commits<T>>::insert(tx_id, commits);
			if tx_input.tx_req != TypeOf::Space {
				<Spaces<T>>::insert(tx_id, tx_input.tx_link);
			}

			log::debug!("Creating entity with id {:?} and owner {:?}", &tx_id, &updater);
			<Transactions<T>>::insert(
				&tx_id,
				TxDetails {
					tx_hash: tx_input.tx_hash,
					tx_cid: tx_input.tx_cid,
					ptx_cid: tx_update.tx_cid,
					tx_link: tx_input.tx_link,
					block: block_number,
					..tx_update
				},
			);

			Self::deposit_event(Event::TransactionUpdated(tx_id, updater));

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
			tx_id: IdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				(tx_type == TypeOf::Entity || tx_type == TypeOf::Space),
				Error::<T>::InvalidTransactionRequest
			);

			let tx_status =
				<Transactions<T>>::get(&tx_id).ok_or(Error::<T>::TransactionNotFound)?;
			ensure!(tx_status.active == status, Error::<T>::NoChangeRequired);

			if tx_type == TypeOf::Space {
				ensure!(tx_status.controller == updater, Error::<T>::UnauthorizedUpdate);
			} else {
				let registrar = <pallet_registrar::Registrars<T>>::get(&updater)
					.ok_or(pallet_registrar::Error::<T>::RegistrarAccountNotFound)?;
				ensure!(!registrar.revoked, pallet_registrar::Error::<T>::RegistrarAccountRevoked);
			}

			log::debug!("Changing Transaction Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut commits = <Commits<T>>::get(tx_id).unwrap_or_default();
			commits.push(TxCommits {
				tx_type,
				tx_hash: tx_status.tx_hash.clone(),
				tx_cid: tx_status.tx_cid.clone(),
				tx_link: tx_status.tx_link.clone(),
				block: block_number.clone(),
				commit: RequestOf::Status,
			});
			<Commits<T>>::insert(tx_id, commits);

			log::debug!(
				"Updating transaction status with id {:?} and owner {:?}",
				&tx_id,
				&updater
			);
			<Transactions<T>>::insert(
				&tx_id,
				TxDetails { block: block_number, active: status, ..tx_status },
			);
			Self::deposit_event(Event::TransactionStatusUpdated(tx_id));

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

			let tx_ver_status = <Entities<T>>::get(&tx_id).ok_or(Error::<T>::EntityNotFound)?;
			ensure!(tx_ver_status == status, Error::<T>::NoChangeRequired);

			let tx_verify =
				<Transactions<T>>::get(&tx_id).ok_or(Error::<T>::TransactionNotFound)?;

			let registrar = <pallet_registrar::Registrars<T>>::get(&updater)
				.ok_or(pallet_registrar::Error::<T>::RegistrarAccountNotFound)?;
			ensure!(!registrar.revoked, pallet_registrar::Error::<T>::RegistrarAccountRevoked);

			log::debug!("Changing Entity Verification Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut commits = <Commits<T>>::get(tx_id).unwrap_or_default();
			commits.push(TxCommits {
				tx_type: TypeOf::Entity,
				tx_hash: tx_verify.tx_hash.clone(),
				tx_cid: tx_verify.tx_cid.clone(),
				tx_link: tx_verify.tx_link.clone(),
				block: block_number.clone(),
				commit: RequestOf::Status,
			});
			<Commits<T>>::insert(tx_id, commits);

			<Entities<T>>::insert(&tx_id, status);
			Self::deposit_event(Event::EntityVerificationStatusUpdated(tx_id));

			Ok(())
		}
	}
}
