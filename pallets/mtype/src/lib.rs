// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod weights;
use crate::weights::WeightInfo;
use frame_support::{ensure, storage::types::StorageMap};
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Hash of the schema.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;

	/// The #MARK TYPE trait
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// #MARK TYPE specific event type
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// MTYPEs stored on chain.
	///
	/// It maps from a MTYPE hash to its creator.
	#[pallet::storage]
	#[pallet::getter(fn mtypes)]
	pub type Mtypes<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, CordAccountOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new #MARK schema has been anchored
		/// \[creator identifier, Schema hash\]
		Anchored(CordAccountOf<T>, HashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no MTYPE with the given hash.
		NotFound,
		/// The MTYPE already exists.
		AlreadyExists,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new MTYPE and associates it with its creator.
		///
		/// * origin: the identifier of the MTYPE creator
		/// * hash: the MTYPE hash. It has to be unique.
		#[pallet::weight(25_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn anchor(origin: OriginFor<T>, mtype_hash: HashOf<T>) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			// check if #MARK TYPE already exists
			ensure!(!<Mtypes<T>>::contains_key(mtype_hash), Error::<T>::AlreadyExists);

			// Anchors a new #MARK schema
			<Mtypes<T>>::insert(mtype_hash, creator.clone());
			// deposit event - #MARK TYPE has been anchored
			Self::deposit_event(Event::Anchored(creator, mtype_hash));
			Ok(())
		}
	}
}
