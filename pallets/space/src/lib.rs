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

pub mod spaces;
pub mod weights;

pub use crate::spaces::*;
pub use pallet::*;
pub mod utils;
use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// ID of a space.
	pub type SpaceIdOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a space controller.
	pub type ControllerOf<T> = pallet_entity::ControllerOf<T>;
	/// Type of an entity Id.
	pub type EntityIdOf<T> = pallet_entity::EntityIdOf<T>;
	/// Type of a schema hash.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// EntityTransaction Type Information
	pub type ActivityOf = Vec<u8>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// CID Information
	pub type CidOf = Vec<u8>;
	/// CID Information
	pub type StatusOf = bool;

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

	/// schemas stored on chain.
	/// It maps from a schema hash to its owner.
	#[pallet::storage]
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, SpaceIdOf<T>, SpaceDetails<T>>;

	/// schemas stored on chain.
	/// It maps from a schema hash to its owner.
	#[pallet::storage]
	#[pallet::getter(fn spaceactivities)]
	pub type SpaceActivities<T> =
		StorageMap<_, Blake2_128Concat, SpaceIdOf<T>, Vec<ActivityDetails<T>>>;
	/// Schema revisions stored on chain.
	/// It maps from a schema ID hash to a vector of schema hashes.
	#[pallet::storage]
	#[pallet::getter(fn entityspaces)]
	pub type EntitySpaceLinks<T> =
		StorageMap<_, Blake2_128Concat, EntityIdOf<T>, Vec<EntitySpaceLinkDetails<T>>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[owner identifier, schema hash, schema Id\]
		SpaceAdded(EntityIdOf<T>, ControllerOf<T>),
		/// A new schema has been created.
		/// \[owner identifier, schema hash, schema Id\]
		SpaceUpdated(EntityIdOf<T>, ControllerOf<T>),
		// EntityStatusUpdated(EntityIdOf<T>, ControllerOf<T>),
		/// A schema has been revoked.
		/// \[owner identifier, schema hash, schema Iid\]
		SpaceStatusUpdated(EntityIdOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no space with the given id.
		SpaceNotFound,
		/// The space already exists.
		SpaceAlreadyExists,
		/// Invalid CID encoding.
		InvalidCidEncoding,
		/// space actions not authorised.
		UnauthorizedUpdate,
		/// The schema hash does not match the schema specified
		SchemaMismatch,
		/// There is no schema with the given parent CID.
		CidAlreadyMapped,
		/// The space is marked inactive.
		SpaceNotActive,
		/// Only when the revoker is not the creator.
		UnauthorizedRevocation,
		/// Only when the restorer is not the creator.
		UnauthorizedRestore,
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
		pub fn create_space(
			origin: OriginFor<T>,
			space_id: SpaceIdOf<T>,
			space_cid: CidOf,
			entity_id: EntityIdOf<T>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<Spaces<T>>::contains_key(&space_id), Error::<T>::SpaceAlreadyExists);

			let entity = <pallet_entity::Entities<T>>::get(&entity_id)
				.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;
			ensure!(entity.active, pallet_entity::Error::<T>::EntityNotActive);

			let cid_base = str::from_utf8(&space_cid).unwrap();
			ensure!(
				cid_base.len() <= 62
					&& (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
				Error::<T>::InvalidCidEncoding
			);
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of space activities linked to space Id
			let mut space_activities = <SpaceActivities<T>>::get(space_id).unwrap_or_default();
			space_activities.push(ActivityDetails {
				space_cid: space_cid.clone(),
				block_number: block_number.clone(),
				activity: "Create".as_bytes().to_vec(),
			});
			<SpaceActivities<T>>::insert(space_id, space_activities);

			// vector of space activities linked to entity Id
			let mut entity_links = <EntitySpaceLinks<T>>::get(entity_id).unwrap_or_default();
			entity_links.push(EntitySpaceLinkDetails {
				space_id: space_id.clone(),
				block_number: block_number.clone(),
				activity: "Create".as_bytes().to_vec(),
			});
			<EntitySpaceLinks<T>>::insert(entity_id, entity_links);

			log::debug!(
				"Creating a new space with id {:?} and controller {:?}",
				&space_id,
				&controller
			);
			<Spaces<T>>::insert(
				&space_id,
				SpaceDetails {
					controller: controller.clone(),
					entity_id,
					space_cid,
					parent_cid: None,
					block_number,
					active: true,
				},
			);

			Self::deposit_event(Event::SpaceAdded(space_id, controller));

			Ok(())
		}

		/// Updates the entity information and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * entity_id: unique identifier of the entity.
		/// * entity_cid: cid of the entity profile
		#[pallet::weight(0)]
		pub fn update_space(
			origin: OriginFor<T>,
			space_id: SpaceIdOf<T>,
			space_cid: CidOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let space = <Spaces<T>>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(space.active, Error::<T>::SpaceNotActive);
			ensure!(space.controller == updater, Error::<T>::UnauthorizedUpdate);

			let entity = <pallet_entity::Entities<T>>::get(&space.entity_id)
				.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;
			ensure!(entity.active, pallet_entity::Error::<T>::EntityNotActive);

			let cid_base = str::from_utf8(&space_cid).unwrap();
			ensure!(
				str::from_utf8(&space.space_cid).unwrap() != cid_base,
				Error::<T>::CidAlreadyMapped
			);
			ensure!(
				cid_base.len() <= 62
					&& (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
				Error::<T>::InvalidCidEncoding
			);
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of space activities linked to space Id
			let mut space_activities = <SpaceActivities<T>>::get(space_id).unwrap();
			space_activities.push(ActivityDetails {
				space_cid: space_cid.clone(),
				block_number: block_number.clone(),
				activity: "Update".as_bytes().to_vec(),
			});
			<SpaceActivities<T>>::insert(space_id, space_activities);

			// vector of space activities linked to entity Id
			let mut entity_links = <EntitySpaceLinks<T>>::get(space.entity_id).unwrap();
			entity_links.push(EntitySpaceLinkDetails {
				space_id: space_id.clone(),
				block_number: block_number.clone(),
				activity: "Update".as_bytes().to_vec(),
			});
			<EntitySpaceLinks<T>>::insert(space.entity_id, entity_links);

			log::debug!("Updating space with id {:?} and cid {:?}", &space_id, &space_cid);
			<Spaces<T>>::insert(
				&space_id,
				SpaceDetails {
					controller: updater.clone(),
					space_cid,
					parent_cid: Some(space.space_cid),
					block_number,
					..space
				},
			);

			Self::deposit_event(Event::SpaceUpdated(space_id, updater));

			Ok(())
		}
		/// Update the status of the entity - active or not
		///
		/// This update can only be performed by a registrar account
		/// * origin: the identifier of the registrar
		/// * entity_id: unique identifier of the entity.
		/// * status: status to be updated
		#[pallet::weight(0)]
		pub fn update_space_status(
			origin: OriginFor<T>,
			space_id: EntityIdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let space = <Spaces<T>>::get(&space_id).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(space.controller == updater, Error::<T>::UnauthorizedUpdate);
			ensure!(space.active == status, Error::<T>::NoChangeRequired);

			let entity = <pallet_entity::Entities<T>>::get(&space.entity_id)
				.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;
			ensure!(entity.active, pallet_entity::Error::<T>::EntityNotActive);

			log::debug!("Changing Entity Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of space activities linked to space Id
			let mut space_activities = <SpaceActivities<T>>::get(space_id).unwrap();
			space_activities.push(ActivityDetails {
				space_cid: space.space_cid.clone(),
				block_number: block_number.clone(),
				activity: "Status".as_bytes().to_vec(),
			});
			<SpaceActivities<T>>::insert(space_id, space_activities);

			// vector of space activities linked to entity Id
			let mut entity_links = <EntitySpaceLinks<T>>::get(space.entity_id).unwrap();
			entity_links.push(EntitySpaceLinkDetails {
				space_id: space_id.clone(),
				block_number: block_number.clone(),
				activity: "Status".as_bytes().to_vec(),
			});
			<EntitySpaceLinks<T>>::insert(space.entity_id, entity_links);

			<Spaces<T>>::insert(&space_id, SpaceDetails { block_number, active: status, ..space });
			Self::deposit_event(Event::SpaceStatusUpdated(space_id));

			Ok(())
		}
	}
}
