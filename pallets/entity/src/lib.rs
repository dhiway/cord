// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{fmt::Debug, prelude::Clone, str};

pub mod verifiers;
pub mod weights;

pub use crate::verifiers::*;
use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	// use codec::MaxEncodedLen;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a registrar.
	// pub type CordAccountOf<T> = <T as Config>::CordAccountId;
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;

	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// status Information
	pub type StatusOf = bool;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		// type CordAccountId: Parameter + Default + MaxEncodedLen;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// The origin which may forcibly set or remove a name. Root can always
		/// do this.
		/// Only for bootstrapping the network. Should be removed later.
		type ForceOrigin: EnsureOrigin<Self::Origin>;
		/// The origin which may add or remove registrars. Root can always do
		/// this.
		type EntityOrigin: EnsureOrigin<Self::Origin>;
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	// #[pallet::generate_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// registrars stored on-chain.
	#[pallet::storage]
	#[pallet::getter(fn verifiers)]
	pub type Verifiers<T> = StorageMap<_, Blake2_128Concat, CordAccountOf<T>, VerifierDetails<T>>;

	/// identity verification information stored on chain.
	/// It maps from an identity to its verification status.
	#[pallet::storage]
	#[pallet::getter(fn entities)]
	pub type Entities<T> = StorageMap<_, Blake2_128Concat, CordAccountOf<T>, StatusOf>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A registrar was added. \[registrar identifier\]
		TxAdd(CordAccountOf<T>),
		/// A registrar was added. \[registrar identifier\]
		TxRevoke(CordAccountOf<T>),
		/// An identity has been verified.
		/// \[identity, verifier\]
		TxVerify(CordAccountOf<T>, CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no registrar with the given ID.
		VerifierAccountNotFound,
		/// The registrar already exists.
		VerifierAlreadyExists,
		/// The registrar has already been revoked.
		AccountAlreadyRevoked,
		/// Only when the revoker is permitted.
		UnauthorizedRevocation,
		/// registrar account revoked
		VerifierAccountRevoked,
		/// current status matches proposed change
		NoChangeRequired,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new entity verifier.
		///
		/// * origin: Council or Root
		/// * account: registrar account
		#[pallet::weight(182_886_000)]
		pub fn add_entity_verifier(
			origin: OriginFor<T>,
			account: CordAccountOf<T>,
		) -> DispatchResult {
			T::EntityOrigin::ensure_origin(origin)?;

			ensure!(!<Verifiers<T>>::contains_key(&account), Error::<T>::VerifierAlreadyExists);

			let block = <frame_system::Pallet<T>>::block_number();

			<Verifiers<T>>::insert(&account, VerifierDetails { block, revoked: false });

			Self::deposit_event(Event::TxAdd(account));

			Ok(())
		}
		/// Revoke an existing entity verifier account.
		///
		/// * origin: Council or Root
		/// * account: registrar account
		#[pallet::weight(0)]
		pub fn revoke_entity_verifier(
			origin: OriginFor<T>,
			account: CordAccountOf<T>,
		) -> DispatchResult {
			T::EntityOrigin::ensure_origin(origin)?;

			let verifier =
				<Verifiers<T>>::get(&account).ok_or(Error::<T>::VerifierAccountNotFound)?;
			ensure!(!verifier.revoked, Error::<T>::AccountAlreadyRevoked);
			let block = <frame_system::Pallet<T>>::block_number();

			<Verifiers<T>>::insert(&account, VerifierDetails { block, revoked: true });

			Self::deposit_event(Event::TxRevoke(account));

			Ok(())
		}

		/// Update the verification status of an entity
		///
		/// This update can only be performed by a registrar
		/// * origin: the identifier of the registrar
		/// * tx_id: identity to verify.
		/// * status: status to be updated
		#[pallet::weight(182_886_000)]
		pub fn verify_entity(
			origin: OriginFor<T>,
			identity: CordAccountOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let requestor = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let verifier =
				<Verifiers<T>>::get(&requestor).ok_or(Error::<T>::VerifierAccountNotFound)?;
			ensure!(!verifier.revoked, Error::<T>::VerifierAccountRevoked);

			<Entities<T>>::insert(&identity, status);
			Self::deposit_event(Event::TxVerify(identity, requestor));

			Ok(())
		}
	}
}
