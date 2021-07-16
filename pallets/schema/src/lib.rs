// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::vec::Vec;

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

	/// Type of a schema hash.
	pub type SchemaHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema owner.
	pub type SchemaOwnerOf<T> = <T as Config>::CordAccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// CID Information
	pub type SchemaCidOf = Vec<u8>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
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

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[owner identifier, schema hash, schema cid\]
		SchemaAnchored(SchemaOwnerOf<T>, SchemaHashOf<T>, SchemaCidOf),
	}

	#[pallet::error]
	pub enum Error<T> {
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new schema and associates it with its owner.
		///
		/// * origin: the identifier of the schema owner
		/// * hash: the schema hash. It has to be unique.
		/// * schema_cid: CID of the schema
		#[pallet::weight(0)]
		pub fn anchor(origin: OriginFor<T>, schema_hash: SchemaHashOf<T>, schema_cid: SchemaCidOf) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				!<Schemas<T>>::contains_key(&schema_hash),
				Error::<T>::SchemaAlreadyExists
			);

			let cid_base = str::from_utf8(&schema_cid).unwrap();
			ensure!(
				cid_base.len() <= 62 && (utils::is_base_32(cid_base) || utils::is_base_58(cid_base)),
				Error::<T>::InvalidCidEncoding
			);
			let block_number = <frame_system::Pallet<T>>::block_number();

			log::debug!("Creating schema with hash {:?} and owner {:?}", &schema_hash, &owner);
			<Schemas<T>>::insert(
				&schema_hash,
				SchemaDetails {
					owner: owner.clone(),
					schema_cid: schema_cid.clone(),
					block_number,
				},
			);

			Self::deposit_event(Event::SchemaAnchored(owner, schema_hash, schema_cid));

			Ok(())
		}
	}
}
