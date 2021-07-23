// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::{fmt::Debug, prelude::Clone};

pub mod registrars;
pub mod weights;

pub use crate::registrars::*;
use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a registrar.
	pub type CordAccountOf<T> = <T as Config>::CordAccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	#[pallet::config]
	pub trait Config: frame_system::Config + Debug {
		type CordAccountId: Parameter + Default;
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
	#[pallet::getter(fn registrars)]
	pub type Registrars<T> = StorageMap<_, Blake2_128Concat, CordAccountOf<T>, RegistrarDetails<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A registrar was added. \[registrar identifier\]
		RegistrarAdded(CordAccountOf<T>),
		/// A registrar was added. \[registrar identifier\]
		RegistrarRevoked(CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no registrar with the given ID.
		RegistrarAccountNotFound,
		/// The registrar already exists.
		RegistrarAlreadyExists,
		/// The registrar has already been revoked.
		AccountAlreadyRevoked,
		/// Only when the revoker is permitted.
		UnauthorizedRevocation,
		/// registrar account revoked
		RegistrarAccountRevoked,
		/// current status matches proposed change
		NoChangeRequired,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new registrar.
		///
		/// * origin: Council or Root
		/// * account: registrar account
		#[pallet::weight(0)]
		pub fn add_registrar(origin: OriginFor<T>, account: CordAccountOf<T>) -> DispatchResult {
			T::RegistrarOrigin::ensure_origin(origin)?;

			ensure!(!<Registrars<T>>::contains_key(&account), Error::<T>::RegistrarAlreadyExists);

			let block = <frame_system::Pallet<T>>::block_number();

			log::debug!("Creating a new registrar account with id {:?} ", &account);

			<Registrars<T>>::insert(&account, RegistrarDetails { block, revoked: false });

			Self::deposit_event(Event::RegistrarAdded(account));

			Ok(())
		}
		/// Revoke an existing registrar account.
		///
		/// * origin: Council or Root
		/// * account: registrar account
		#[pallet::weight(0)]
		pub fn revoke_registrar(origin: OriginFor<T>, account: CordAccountOf<T>) -> DispatchResult {
			T::RegistrarOrigin::ensure_origin(origin)?;

			let registrar =
				<Registrars<T>>::get(&account).ok_or(Error::<T>::RegistrarAccountNotFound)?;
			ensure!(!registrar.revoked, Error::<T>::AccountAlreadyRevoked);
			let block = <frame_system::Pallet<T>>::block_number();

			log::debug!("Revoking a new registrar account with id {:?} ", &account);

			<Registrars<T>>::insert(&account, RegistrarDetails { block, revoked: true });

			Self::deposit_event(Event::RegistrarRevoked(account));

			Ok(())
		}
	}
}
