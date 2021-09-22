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
use sp_std::{fmt::Debug, prelude::Clone, str};

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
	/// status Information
	pub type StatusOf = bool;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type CordAccountId: Parameter + Default;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// The origin which may forcibly set or remove a name. Root can always
		/// do this.
		type ForceOrigin: EnsureOrigin<Self::Origin>;
		/// The origin which may add or remove registrars. Root can always do
		/// this.
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

	/// registrars stored on chain.
	/// It maps from a schema hash to its owner.
	#[pallet::storage]
	#[pallet::getter(fn registrars)]
	pub type Registrars<T> = StorageMap<_, Blake2_128Concat, CordAccountOf<T>, RegistrarDetails<T>>;

	/// identity verification information stored on chain.
	/// It maps from an identity to its verification status.
	#[pallet::storage]
	#[pallet::getter(fn verifiedentities)]
	pub type VerifiedIdentities<T> = StorageMap<_, Blake2_128Concat, CordAccountOf<T>, StatusOf>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A registrar was added. \[registrar identifier\]
		TxAdd(CordAccountOf<T>),
		/// A registrar was added. \[registrar identifier\]
		TxRevoke(CordAccountOf<T>),
		/// A identity has been verified.
		/// \[identity, verifier\]
		TxVerify(CordAccountOf<T>, CordAccountOf<T>),
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

			<Registrars<T>>::insert(&account, RegistrarDetails { block, revoked: false });

			Self::deposit_event(Event::TxAdd(account));

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

			<Registrars<T>>::insert(&account, RegistrarDetails { block, revoked: true });

			Self::deposit_event(Event::TxRevoke(account));

			Ok(())
		}

		/// Update the verification status of an identity
		///
		/// This update can only be performed by a registrar
		/// * origin: the identifier of the registrar
		/// * tx_id: identity to verify.
		/// * status: status to be updated
		#[pallet::weight(0)]
		pub fn verify_identity(
			origin: OriginFor<T>,
			tx_id: CordAccountOf<T>,
			status: StatusOf,
		) -> DispatchResult {
			let verifier = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let registrar =
				<Registrars<T>>::get(&verifier).ok_or(Error::<T>::RegistrarAccountNotFound)?;
			ensure!(!registrar.revoked, Error::<T>::RegistrarAccountRevoked);

			<VerifiedIdentities<T>>::insert(&tx_id, status);
			Self::deposit_event(Event::TxVerify(tx_id, verifier));

			Ok(())
		}
	}
}
