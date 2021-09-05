// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::{fmt::Debug, prelude::Clone};

pub mod registrars;
pub mod weights;

pub use crate::registrars::*;
use crate::weights::WeightInfo;
pub use pallet::*;
use pallet_entity::{CommitOf, TypeOf};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// ID of an entity.
	pub type IdOf<T> = pallet_entity::IdOf<T>;
	/// Type of a registrar.
	pub type CordAccountOf<T> = pallet_entity::CordAccountOf<T>;
	// pub type ControllerOf<T> = <T as Config>::CordAccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// status Information
	pub type StatusOf = bool;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_entity::Config + Debug {
		// type CordAccountId: Parameter + Default;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
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
	#[pallet::getter(fn registrars)]
	pub type Registrars<T> = StorageMap<_, Blake2_128Concat, CordAccountOf<T>, RegistrarDetails<T>>;

	/// entities verification information stored on chain.
	/// It maps from a entity Id to its verification status.
	#[pallet::storage]
	#[pallet::getter(fn verifiedentities)]
	pub type VerifiedEntities<T> = StorageMap<_, Blake2_128Concat, IdOf<T>, StatusOf>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A registrar was added. \[registrar identifier\]
		RegistrarAdded(CordAccountOf<T>),
		/// A registrar was added. \[registrar identifier\]
		RegistrarRevoked(CordAccountOf<T>),
		/// A entity has been restored.
		/// \[entity identifier\]
		EntityVerificationStatusUpdated(IdOf<T>, CordAccountOf<T>),
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
			let verifier = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let tx_verify = <pallet_entity::Entities<T>>::get(&tx_id)
				.ok_or(pallet_entity::Error::<T>::EntityNotFound)?;

			let registrar =
				<Registrars<T>>::get(&verifier).ok_or(Error::<T>::RegistrarAccountNotFound)?;
			ensure!(!registrar.revoked, Error::<T>::RegistrarAccountRevoked);

			log::debug!("Changing Entity Verification Status");
			let block_number = <frame_system::Pallet<T>>::block_number();

			// vector of entity activities linked to entity Id
			let mut commit = <pallet_entity::Commits<T>>::get(tx_id).unwrap_or_default();
			commit.push(pallet_entity::TxCommits {
				tx_type: TypeOf::Entity,
				tx_hash: tx_verify.tx_hash.clone(),
				tx_cid: tx_verify.tx_cid,
				tx_link: None,
				block: block_number,
				commit: CommitOf::Verify,
			});
			<pallet_entity::Commits<T>>::insert(&tx_id, commit);

			<VerifiedEntities<T>>::insert(&tx_id, status);
			Self::deposit_event(Event::EntityVerificationStatusUpdated(tx_id, verifier));

			Ok(())
		}
	}
}
