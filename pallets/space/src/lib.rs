// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use cord_primitives::{CidOf, StatusOf};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};

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
	pub type SpaceHashOf<T> = <T as frame_system::Config>::Hash;
	/// EntityTransaction Type Information
	pub type ActionOf = pallet_entity::ActionOf;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

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

	/// spaces stored on chain.
	/// It maps from a space Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, SpaceIdOf<T>, SpaceDetails<T>>;

	/// space activities stored on chain.
	/// It maps from a space Id to activity details.
	#[pallet::storage]
	#[pallet::getter(fn spaceactions)]
	pub type SpaceActions<T> = StorageMap<_, Blake2_128Concat, SpaceIdOf<T>, Vec<ActionDetails<T>>>;

	/// Entity - Space links stored on chain.
	/// It maps from a entity Id to a vector of space links.
	#[pallet::storage]
	#[pallet::getter(fn entityspaceactions)]
	pub type EntityActionss<T> =
		StorageMap<_, Blake2_128Concat, EntityIdOf<T>, Vec<ActionDetails<T>>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new space has been created.
		/// \[space identifier, controller\]
		SpaceAdded(EntityIdOf<T>, ControllerOf<T>),
		/// A space has been updated.
		/// \[space identifier, controller\]
		SpaceUpdated(EntityIdOf<T>, ControllerOf<T>),
		/// A space status has been changed.
		/// \[space identifier\]
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
		/// entity actions not authorised.
		UnauthorizedOperation,
		/// There is no schema with the given parent CID.
		CidAlreadyMapped,
		/// The space is marked inactive.
		SpaceNotActive,
		/// no status change required
		NoChangeRequired,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new space and associates it with its controller.
		///
		/// * origin: the identifier of the space controller.
		/// * space_id: unique identifier of the space.
		/// * space_cid: cid of the space profile.
		/// * entity_id: unique identifier of the associated entity.
		#[pallet::weight(0)]
		pub fn create_space(
			origin: OriginFor<T>,
			tx_id: SpaceIdOf<T>,
			tx_cid: Option<CidOf>,
			entity_id: EntityIdOf<T>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<Spaces<T>>::contains_key(&tx_id), Error::<T>::SpaceAlreadyExists);

			let entity = <pallet_entity::Entities<T>>::get(&entity_id)
				.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;
			ensure!(entity.active, pallet_entity::Error::<T>::EntityNotActive);
			ensure!(entity.controller == controller, Error::<T>::UnauthorizedOperation);

			if let Some(ref tx_cid) = tx_cid {
				let cid_base = str::from_utf8(&tx_cid).unwrap();
				ensure!(
					cid_base.len() <= 62
						&& (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
					Error::<T>::InvalidCidEncoding
				);
			}

			let block = <frame_system::Pallet<T>>::block_number();

			// vector of space activities linked to space Id
			let mut actions = <SpaceActions<T>>::get(tx_id).unwrap_or_default();
			actions.push(SpaceActions {
				tx_hash: tx_hash.clone(),
				tx_cid: tx_cid.clone(),
				block: block.clone(),
				action: ActionOf::Create,
			});
			<SpaceActions<T>>::insert(tx_id, actions);
			<EntityActions<T>>::insert(entity_id, actions);

			log::debug!(
				"Creating a new space with id {:?} and controller {:?}",
				&tx_id,
				&controller
			);
			<Spaces<T>>::insert(
				&tx_id,
				SpaceDetails {
					controller: controller.clone(),
					tx_hash,
					tx_cid,
					ptx_cid: None,
					block,
					active: true,
				},
			);

			Self::deposit_event(Event::SpaceAdded(tx_id, controller));

			Ok(())
		}

		/// Updates the space information and associates it with its controller.
		///
		/// * origin: the identifier of the space controller.
		/// * space_id: unique identifier of the space.
		/// * space_cid: cid of the entity profile.
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
					space_cid,
					parent_cid: Some(space.space_cid),
					block_number,
					..space
				},
			);

			Self::deposit_event(Event::SpaceUpdated(space_id, updater));

			Ok(())
		}
		/// Update the status of the space - active or not
		///
		/// * origin: the identifier of the space controller.
		/// * space_id: unique identifier of the space.
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
