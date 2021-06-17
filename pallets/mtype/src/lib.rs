// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod weights;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

/// Test module for MTYPEs
#[cfg(test)]
mod tests;

pub use pallet::*;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a MTYPE hash.
	pub type MtypeHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a MTYPE creator.
	pub type MtypeCreatorOf<T> = <T as Config>::MtypeCreatorId;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type MtypeCreatorId: Parameter + Default;
		type EnsureOrigin: EnsureOrigin<Success = MtypeCreatorOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
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
	pub type Mtypes<T> = StorageMap<_, Blake2_128Concat, MtypeHashOf<T>, MtypeCreatorOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new MTYPE has been created.
		/// \[creator identifier, MTYPE hash\]
		MTypeCreated(MtypeCreatorOf<T>, MtypeHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no MTYPE with the given hash.
		MTypeNotFound,
		/// The MTYPE already exists.
		MTypeAlreadyExists,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new MTYPE and associates it with its creator.
		///
		/// * origin: the identifier of the MTYPE creator
		/// * hash: the MTYPE hash. It has to be unique.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add())]
		pub fn anchor(origin: OriginFor<T>, hash: MtypeHashOf<T>) -> DispatchResultWithPostInfo {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(!<Mtypes<T>>::contains_key(&hash), Error::<T>::MTypeAlreadyExists);

			log::debug!("Creating MTYPE with hash {:?} and creator {:?}", &hash, &creator);
			<Mtypes<T>>::insert(&hash, creator.clone());

			Self::deposit_event(Event::MTypeCreated(creator, hash));

			Ok(None.into())
		}
	}
}
