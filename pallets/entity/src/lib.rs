// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
// use sp_std::vec::Vec;
use sp_std::{fmt::Debug, prelude::Clone, vec::Vec};

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
	pub type EntityIdOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a entity controller.
	pub type ControllerOf<T> = pallet_registrar::CordAccountOf<T>;
	/// Entity Transaction Type Information
	pub type ActivityOf = Vec<u8>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// CID Information
	pub type CidOf = Vec<u8>;
	/// CID Information
	pub type StatusOf = bool;

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

	/// entities stored on chain.
	/// It maps from a entity Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn entities)]
	pub type Entities<T> = StorageMap<_, Blake2_128Concat, EntityIdOf<T>, EntityDetails<T>>;

	/// entity activities stored on chain.
	/// It maps from a entity Id to a vector of activity details.
	#[pallet::storage]
	#[pallet::getter(fn entityactivities)]
	pub type EntityActivities<T> =
		StorageMap<_, Blake2_128Concat, EntityIdOf<T>, Vec<ActivityDetails<T>>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new entity has been created.
		/// \[entity identifier, controller\]
		EntityAdded(EntityIdOf<T>, ControllerOf<T>),
		/// An entityhas been created.
		/// \[entity identifier, controller\]
		EntityUpdated(EntityIdOf<T>, ControllerOf<T>),
		/// An entity has been revoked.
		/// \[entity identifier\]
		EntityStatusUpdated(EntityIdOf<T>),
		/// A entity has been restored.
		/// \[entity identifier\]
		EntityVerificationStatusUpdated(EntityIdOf<T>),
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new entity and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * entity_id: unique identifier of the entity.
		/// * entity_cid: cid of the entity profile
		#[pallet::weight(0)]
		pub fn create_entity(
			origin: OriginFor<T>,
			entity_id: EntityIdOf<T>,
			entity_cid: CidOf,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<Entities<T>>::contains_key(&entity_id), Error::<T>::EntityAlreadyExists);

			let cid_base = str::from_utf8(&entity_cid).unwrap();
			ensure!(
				cid_base.len() <= 62
					&& (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
				Error::<T>::InvalidCidEncoding
			);
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut entity_activities = <EntityActivities<T>>::get(entity_id).unwrap_or_default();
			entity_activities.push(ActivityDetails {
				entity_cid: entity_cid.clone(),
				block_number: block_number.clone(),
				activity: "Create".as_bytes().to_vec(),
			});
			<EntityActivities<T>>::insert(entity_id, entity_activities);
			log::debug!(
				"Creating a new entity with id {:?} and controller {:?}",
				&entity_id,
				&controller
			);
			<Entities<T>>::insert(
				&entity_id,
				EntityDetails {
					controller: controller.clone(),
					entity_cid,
					parent_cid: None,
					block_number,
					verified: false,
					active: true,
				},
			);

			Self::deposit_event(Event::EntityAdded(entity_id, controller));

			Ok(())
		}

		/// Updates the entity information and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * entity_id: unique identifier of the entity.
		/// * entity_cid: cid of the entity profile
		#[pallet::weight(0)]
		pub fn update_entity(
			origin: OriginFor<T>,
			entity_id: EntityIdOf<T>,
			entity_cid: CidOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let entity = <Entities<T>>::get(&entity_id).ok_or(Error::<T>::EntityNotFound)?;
			// ensure!(entity.controller == updater, Error::<T>::UpdateNotAuthorised);
			ensure!(entity.active, Error::<T>::EntityNotActive);
			ensure!(entity.controller == updater, Error::<T>::UnauthorizedUpdate);

			let cid_base = str::from_utf8(&entity_cid).unwrap();
			ensure!(
				str::from_utf8(&entity.entity_cid).unwrap() != cid_base,
				Error::<T>::CidAlreadyMapped
			);
			ensure!(
				cid_base.len() <= 62
					&& (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
				Error::<T>::InvalidCidEncoding
			);
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut entity_activities = <EntityActivities<T>>::get(entity_id).unwrap();
			entity_activities.push(ActivityDetails {
				entity_cid: entity_cid.clone(),
				block_number: block_number.clone(),
				activity: "Update".as_bytes().to_vec(),
			});
			<EntityActivities<T>>::insert(entity_id, entity_activities);
			log::debug!("Creating entity with id {:?} and owner {:?}", &entity_id, &updater);
			<Entities<T>>::insert(
				&entity_id,
				EntityDetails {
					entity_cid,
					parent_cid: Some(entity.entity_cid),
					block_number,
					verified: false,
					..entity
				},
			);

			Self::deposit_event(Event::EntityUpdated(entity_id, updater));

			Ok(())
		}
		/// Update the status of the entity - active or not
		///
		/// This update can only be performed by a registrar account
		/// * origin: the identifier of the registrar
		/// * entity_id: unique identifier of the entity.
		/// * status: status to be updated
		#[pallet::weight(0)]
		pub fn update_entity_status(
			origin: OriginFor<T>,
			entity_id: EntityIdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let entity = <Entities<T>>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
			ensure!(entity.active == status, Error::<T>::NoChangeRequired);

			let registrar = <pallet_registrar::Registrars<T>>::get(&updater)
				.ok_or(pallet_registrar::Error::<T>::RegistrarAccountNotFound)?;
			ensure!(!registrar.revoked, pallet_registrar::Error::<T>::RegistrarAccountRevoked);

			log::debug!("Changing Entity Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut entity_activities = <EntityActivities<T>>::get(entity_id).unwrap();
			entity_activities.push(ActivityDetails {
				entity_cid: entity.entity_cid.clone(),
				block_number: block_number.clone(),
				activity: "Status".as_bytes().to_vec(),
			});
			<EntityActivities<T>>::insert(entity_id, entity_activities);

			<Entities<T>>::insert(
				&entity_id,
				EntityDetails { block_number, active: status, ..entity },
			);
			Self::deposit_event(Event::EntityStatusUpdated(entity_id));

			Ok(())
		}
		/// Update the verificationstatus of the entity
		///
		/// This update can only be performed by a registrar account
		/// * origin: the identifier of the registrar
		/// * entity_id: unique identifier of the entity.
		/// * status: status to be updated
		#[pallet::weight(0)]
		pub fn entity_verification(
			origin: OriginFor<T>,
			entity_id: EntityIdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let entity = <Entities<T>>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
			ensure!(entity.active, Error::<T>::EntityNotActive);
			ensure!(entity.verified == status, Error::<T>::NoChangeRequired);

			let registrar = <pallet_registrar::Registrars<T>>::get(&updater)
				.ok_or(pallet_registrar::Error::<T>::RegistrarAccountNotFound)?;
			ensure!(!registrar.revoked, pallet_registrar::Error::<T>::RegistrarAccountRevoked);

			log::debug!("Changing Entity Verification Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut entity_activities = <EntityActivities<T>>::get(entity_id).unwrap();
			entity_activities.push(ActivityDetails {
				entity_cid: entity.entity_cid.clone(),
				block_number: block_number.clone(),
				activity: "Verification".as_bytes().to_vec(),
			});
			<EntityActivities<T>>::insert(entity_id, entity_activities);

			<Entities<T>>::insert(
				&entity_id,
				EntityDetails { block_number, verified: status, ..entity },
			);
			Self::deposit_event(Event::EntityStatusUpdated(entity_id));

			Ok(())
		}
	}
}
