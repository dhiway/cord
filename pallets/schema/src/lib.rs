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

pub mod schemas;
pub mod weights;

// #[cfg(any(feature = "mock", test))]
// // pub mod mock;
// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

// /// Test module for schemas
// #[cfg(test)]
// mod tests;

pub use crate::schemas::*;
pub use pallet::*;
pub mod utils;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// ID of a schema.
	pub type SchemaIdOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema hash.
	pub type SchemaHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema owner.
	pub type SchemaOwnerOf<T> = <T as Config>::CordAccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// CID Information
	pub type SchemaCidOf = Vec<u8>;
	/// Transaction Type Information
	pub type SchemaTransOf = Vec<u8>;
	#[pallet::config]
	pub trait Config: frame_system::Config + Debug {
		type CordAccountId: Parameter + Default;
		type EnsureOrigin: EnsureOrigin<Success = SchemaOwnerOf<Self>, <Self as frame_system::Config>::Origin>;
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
	#[pallet::getter(fn schemas)]
	pub type Schemas<T> = StorageMap<_, Blake2_128Concat, SchemaHashOf<T>, SchemaDetails<T>>;

	/// Schema revisions stored on chain.
	/// It maps from a schema ID hash to a vector of schema hashes.
	#[pallet::storage]
	#[pallet::getter(fn schemaid)]
	pub type SchemaIds<T> = StorageMap<_, Blake2_128Concat, SchemaIdOf<T>, SchemaHashOf<T>>;

	/// Schema revisions stored on chain.
	/// It maps from a schema ID hash to a vector of schema hashes.
	#[pallet::storage]
	#[pallet::getter(fn schemalinks)]
	pub type SchemaLinks<T> = StorageMap<_, Blake2_128Concat, SchemaIdOf<T>, Vec<SchemaIdLinks<T>>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[owner identifier, schema hash, schema Id\]
		SchemaCreated(SchemaOwnerOf<T>, SchemaHashOf<T>, SchemaIdOf<T>),
		/// A schema has been updated.
		/// \[owner identifier, schema hash, schema Iid\]
		SchemaUpdated(SchemaOwnerOf<T>, SchemaHashOf<T>, SchemaIdOf<T>),
		/// A schema has been revoked.
		/// \[owner identifier, schema hash, schema Iid\]
		SchemaRevoked(SchemaOwnerOf<T>, SchemaHashOf<T>),
		/// A schema has been restored.
		/// \[owner identifier, schema hash, schema Iid\]
		SchemaRestored(SchemaOwnerOf<T>, SchemaHashOf<T>),
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
		SchemaCidAlreadyExists,
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new schema and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * hash: the schema hash. It has to be unique.
		/// * schema_details: schema details information
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			schema_hash: SchemaHashOf<T>,
			schema_details: SchemaInput<T>,
		) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				!<Schemas<T>>::contains_key(&schema_hash),
				Error::<T>::SchemaAlreadyExists
			);

			let cid_base = str::from_utf8(&schema_details.schema_cid).unwrap();
			ensure!(
				cid_base.len() <= 62 && (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
				Error::<T>::InvalidCidEncoding
			);
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of schema hashes linked to schema Id
			let mut schema_versions = <SchemaLinks<T>>::get(schema_details.schema_id).unwrap_or_default();
			schema_versions.push(SchemaIdLinks {
				schema_hash: schema_hash.clone(),
				block_number: block_number.clone(),
				trans_type: "Create".as_bytes().to_vec(),
			});
			// schema_versions.push(schema_hash);
			<SchemaLinks<T>>::insert(schema_details.schema_id, schema_versions);

			// schema id to schema hash storage
			<SchemaIds<T>>::insert(schema_details.schema_id, schema_hash);

			log::debug!("Creating schema with hash {:?} and owner {:?}", &schema_hash, &owner);
			<Schemas<T>>::insert(
				&schema_hash,
				SchemaDetails {
					schema_id: schema_details.schema_id.clone(),
					owner: owner.clone(),
					schema_cid: schema_details.schema_cid,
					parent_cid: None,
					block_number,
					revoked: false,
				},
			);

			Self::deposit_event(Event::SchemaCreated(owner, schema_hash, schema_details.schema_id));

			Ok(())
		}

		/// Updates the latest version of stored schema
		/// and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * hash: the schema hash. It has to be unique.
		/// * schema_details: schema details information
		#[pallet::weight(0)]
		pub fn update(
			origin: OriginFor<T>,
			schema_hash: SchemaHashOf<T>,
			schema_details: SchemaInput<T>,
		) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<Schemas<T>>::contains_key(&schema_hash),
				Error::<T>::SchemaAlreadyExists
			);

			let schema_last_version =
				<SchemaIds<T>>::get(schema_details.schema_id).ok_or(Error::<T>::SchemaIdNotFound)?;
			let existing_schema = <Schemas<T>>::get(schema_last_version).ok_or(Error::<T>::SchemaNotFound)?;

			ensure!(existing_schema.owner == owner, Error::<T>::SchemaNotDelegated);
			ensure!(!existing_schema.revoked, Error::<T>::SchemaRevoked);
			let cid_base = str::from_utf8(&schema_details.schema_cid).unwrap();
			ensure!(
				str::from_utf8(&existing_schema.schema_cid).unwrap() != cid_base,
				Error::<T>::SchemaCidAlreadyExists
			);
			ensure!(
				cid_base.len() <= 62 && (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
				Error::<T>::InvalidCidEncoding
			);
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of schema hashes linked to schema Id
			let mut schema_links = <SchemaLinks<T>>::get(schema_details.schema_id).unwrap();
			schema_links.push(SchemaIdLinks {
				schema_hash: schema_hash.clone(),
				block_number: block_number.clone(),
				trans_type: "Update".as_bytes().to_vec(),
			});
			<SchemaLinks<T>>::insert(schema_details.schema_id, schema_links);

			// schema id to schema hash storage
			<SchemaIds<T>>::insert(schema_details.schema_id, schema_hash);

			log::debug!("Creating schema with hash {:?} and owner {:?}", &schema_hash, &owner);
			<Schemas<T>>::insert(
				&schema_hash,
				SchemaDetails {
					schema_id: schema_details.schema_id.clone(),
					owner: owner.clone(),
					schema_cid: schema_details.schema_cid,
					parent_cid: Some(existing_schema.schema_cid),
					block_number,
					revoked: false,
				},
			);

			Self::deposit_event(Event::SchemaCreated(owner, schema_hash, schema_details.schema_id));

			Ok(())
		}
		/// Revoke an existing schema
		///
		/// The revoker must be the creator of the schema
		/// * origin: the identifier of the revoker
		/// * schema_hash: the hash of the schema to revoke
		#[pallet::weight(0)]
		pub fn revoke(origin: OriginFor<T>, schema_hash: SchemaHashOf<T>) -> DispatchResult {
			let revoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let schema = <Schemas<T>>::get(&schema_hash).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema.owner == revoker, Error::<T>::UnauthorizedRevocation);
			ensure!(!schema.revoked, Error::<T>::SchemaAlreadyRevoked);

			log::debug!("Revoking Schema");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of schema hashes linked to schema Id
			let mut schema_links = <SchemaLinks<T>>::get(schema.schema_id).unwrap();
			schema_links.push(SchemaIdLinks {
				schema_hash: schema_hash.clone(),
				block_number: block_number.clone(),
				trans_type: "Revoke".as_bytes().to_vec(),
			});
			<SchemaLinks<T>>::insert(schema.schema_id, schema_links);

			<Schemas<T>>::insert(
				&schema_hash,
				SchemaDetails {
					block_number,
					revoked: true,
					..schema
				},
			);
			Self::deposit_event(Event::SchemaRevoked(revoker, schema_hash));

			Ok(())
		}
		// Restore a revoked schema.
		///
		/// The restorer must be the creator of the schema being restored
		/// * origin: the identifier of the restorer
		/// * schema_hash: the hash of the schema to restore
		#[pallet::weight(0)]
		pub fn restore(origin: OriginFor<T>, schema_hash: SchemaHashOf<T>) -> DispatchResult {
			let restorer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let schema = <Schemas<T>>::get(&schema_hash).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema.revoked, Error::<T>::SchemaStillActive);
			ensure!(schema.owner == restorer, Error::<T>::UnauthorizedRestore);

			log::debug!("Restoring Schema");
			let block_number = <frame_system::Pallet<T>>::block_number();
			// vector of schema hashes linked to schema Id
			let mut schema_links = <SchemaLinks<T>>::get(schema.schema_id).unwrap();
			schema_links.push(SchemaIdLinks {
				schema_hash: schema_hash.clone(),
				block_number: block_number.clone(),
				trans_type: "Restore".as_bytes().to_vec(),
			});
			<SchemaLinks<T>>::insert(schema.schema_id, schema_links);

			<Schemas<T>>::insert(
				&schema_hash,
				SchemaDetails {
					block_number,
					revoked: false,
					..schema
				},
			);
			Self::deposit_event(Event::SchemaRestored(restorer, schema_hash));

			Ok(())
		}
	}
}
