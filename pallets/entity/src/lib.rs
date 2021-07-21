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
	/// ID of a space.
	pub type SpaceIdOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema owner.
	pub type ControllerOf<T> = <T as Config>::CordAccountId;
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
	/// Type of a schema hash.
	pub type RegistrarIdOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema owner.
	pub type RegistrarAccountOf<T> = <T as Config>::CordAccountId;
	/// An identifier for a single name registrar/identity verification service.
	pub type RegistrarIndex = u32;
	#[pallet::config]
	pub trait Config: frame_system::Config + Debug {
		type CordAccountId: Parameter + Default;

		/// Maxmimum number of registrars allowed in the system.
		#[pallet::constant]
		type MaxRegistrars: Get<u32>;

		type EnsureOrigin: EnsureOrigin<
			Success = ControllerOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;

		/// The origin which may forcibly set or remove a name. Root can always do this.
		type ForceOrigin: EnsureOrigin<Self::Origin>;

		/// The origin which may add or remove registrars. Root can always do this.
		type RegistrarOrigin: EnsureOrigin<Self::Origin>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for extrinsics in this pallet.
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
	#[pallet::getter(fn entities)]
	pub type Entities<T> = StorageMap<_, Blake2_128Concat, EntityIdOf<T>, EntityDetails<T>>;

	/// schemas stored on chain.
	/// It maps from a schema hash to its owner.
	#[pallet::storage]
	#[pallet::getter(fn entityactivities)]
	pub type EntityActivities<T> =
		StorageMap<_, Blake2_128Concat, EntityIdOf<T>, Vec<ActivityDetails<T>>>;

	/// Schema revisions stored on chain.
	/// It maps from a schema ID hash to a vector of schema hashes.
	#[pallet::storage]
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, SpaceDetails<T>>;

	/// Schema revisions stored on chain.
	/// It maps from a schema ID hash to a vector of schema hashes.
	#[pallet::storage]
	#[pallet::getter(fn entityspaces)]
	pub type EntitySpaces<T> =
		StorageMap<_, Blake2_128Concat, EntityIdOf<T>, Vec<EntitySpaceDetails<T>>>;

	// Schema revisions stored on chain.
	/// It maps from a schema ID hash to a vector of schema hashes.
	#[pallet::storage]
	#[pallet::getter(fn registrars)]
	pub type Registrars<T> = StorageValue<_, Vec<RegistrarAccountOf<T>>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[owner identifier, schema hash, schema Id\]
		EntityAdded(EntityIdOf<T>, ControllerOf<T>),
		/// A new schema has been created.
		/// \[owner identifier, schema hash, schema Id\]
		EntityUpdated(EntityIdOf<T>, ControllerOf<T>),
		// EntityStatusUpdated(EntityIdOf<T>, ControllerOf<T>),
		/// A schema has been revoked.
		/// \[owner identifier, schema hash, schema Iid\]
		EntityStatusUpdated(EntityIdOf<T>),
		/// A schema has been restored.
		/// \[owner identifier, schema hash, schema Iid\]
		EntityVerificationStatusUpdated(EntityIdOf<T>),
		/// A registrar was added. \[registrar_index\]
		RegistrarAdded(RegistrarIndex),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no schema with the given ID.
		SchemaIdNotFound,
		/// There is no schema with the given hash.
		SchemaNotFound,
		/// The schema already exists.
		SchemaAlreadyExists,
		/// Invalid Schema CID encoding.
		InvalidCidEncoding,
		/// schema not authorised.
		SchemaNotDelegated,
		/// The schema hash does not match the schema specified
		SchemaMismatch,
		/// There is no schema with the given parent CID.
		CidAlreadyMapped,
		/// The schema has already been revoked.
		SchemaAlreadyRevoked,
		/// Only when the revoker is not the creator.
		UnauthorizedRevocation,
		/// Only when the restorer is not the creator.
		UnauthorizedRestore,
		/// only when trying to restore an active stream.
		SchemaStillActive,
		/// schema revoked
		SchemaRevoked,
		TooManyRegistrars,
		EntityAlreadyExists,
		EntityNotFound,
		EntityNotActive,
		NoChangeRequired,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new schema and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * hash: the schema hash. It has to be unique.
		/// * schema_details: schema details information
		#[pallet::weight(0)]
		pub fn add_registrar(origin: OriginFor<T>, account: T::CordAccountId) -> DispatchResult {
			T::RegistrarOrigin::ensure_origin(origin)?;

			let (i, _registrar_count) = <Registrars<T>>::try_mutate(
				|registrars| -> Result<(RegistrarIndex, usize), DispatchError> {
					ensure!(
						registrars.len() < T::MaxRegistrars::get() as usize,
						Error::<T>::TooManyRegistrars
					);
					registrars.push(account);
					Ok(((registrars.len() - 1) as RegistrarIndex, registrars.len()))
				},
			)?;

			Self::deposit_event(Event::RegistrarAdded(i));

			Ok(())
		}
		/// Create a new schema and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * hash: the schema hash. It has to be unique.
		/// * schema_details: schema details information
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

			// vector of schema hashes linked to schema Id
			let mut entity_activities = <EntityActivities<T>>::get(entity_id).unwrap_or_default();
			entity_activities.push(ActivityDetails {
				entity_cid: entity_cid.clone(),
				block_number: block_number.clone(),
				activity: "Create".as_bytes().to_vec(),
			});
			<EntityActivities<T>>::insert(entity_id, entity_activities);

			// schema id to schema hash storage
			// <EntityUpdates<T>>::insert(entity_details.schema_id, entity_hash);

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

		/// Updates the latest version of stored schema
		/// and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * hash: the schema hash. It has to be unique.
		/// * schema_details: schema details information
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

			// vector of schema hashes linked to schema Id
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
					controller: updater.clone(),
					entity_cid,
					parent_cid: Some(entity.entity_cid),
					block_number,
					verified: false,
					active: true,
				},
			);

			Self::deposit_event(Event::EntityUpdated(entity_id, updater));

			Ok(())
		}
		/// Revoke an existing schema
		///
		/// The revoker must be the creator of the schema
		/// * origin: the identifier of the revoker
		/// * schema_hash: the hash of the schema to revoke
		#[pallet::weight(0)]
		pub fn update_entity_status(
			origin: OriginFor<T>,
			entity_id: EntityIdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			T::RegistrarOrigin::ensure_origin(origin)?;

			// let updater = T::RegistrarOrigin::ensure_origin(origin)?;

			let entity = <Entities<T>>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
			ensure!(entity.active == status, Error::<T>::NoChangeRequired);

			// let schema = <Schemas<T>>::get(&schema_hash).ok_or(Error::<T>::SchemaNotFound)?;
			// ensure!(schema.owner == revoker, Error::<T>::UnauthorizedRevocation);
			// ensure!(!schema.revoked, Error::<T>::SchemaAlreadyRevoked);

			log::debug!("Changing Entity Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of schema hashes linked to schema Id
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
		// Restore a revoked schema.
		///
		/// The restorer must be the creator of the schema being restored
		/// * origin: the identifier of the restorer
		/// * schema_hash: the hash of the schema to restore
		#[pallet::weight(0)]
		pub fn verification(
			origin: OriginFor<T>,
			entity_id: EntityIdOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			T::RegistrarOrigin::ensure_origin(origin)?;

			// let updater = T::RegistrarOrigin::ensure_origin(origin)?;

			let entity = <Entities<T>>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
			ensure!(entity.active, Error::<T>::EntityNotActive);
			ensure!(entity.verified == status, Error::<T>::NoChangeRequired);

			// let schema = <Schemas<T>>::get(&schema_hash).ok_or(Error::<T>::SchemaNotFound)?;
			// ensure!(schema.owner == revoker, Error::<T>::UnauthorizedRevocation);
			// ensure!(!schema.revoked, Error::<T>::SchemaAlreadyRevoked);

			log::debug!("Changing Entity Verification Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of schema hashes linked to schema Id
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
