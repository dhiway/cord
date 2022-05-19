// This file is part of Cord – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// Cord is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cord is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cord. If not, see <https://www.gnu.org/licenses/>.
//
//! A pallet for managing authorities on CORD.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

use sp_std::prelude::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use cord_primitives::CORD_SESSION_PERIOD;
	use frame_support::{pallet_prelude::*, sp_std};
	use frame_system::pallet_prelude::*;
	use pallet_session::{Pallet as Session, SessionManager};

	#[pallet::storage]
	#[pallet::getter(fn authorities)]
	pub type Authorities<T: Config> = StorageValue<_, Vec<T::AccountId>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn session_for_authorities_change)]
	pub type SessionForAuthoritiesChange<T: Config> = StorageValue<_, u32>;

	/// Configuration for the proposer.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_session::Config {
		/// The overreaching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Privileged origin that can add or remove validators.
		type AuthorityOrigin: EnsureOrigin<Self::Origin>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new authority candidate added to the set.
		AuthorityRegistered(T::AccountId, u32),
		/// An authority scheduled to be removed from the set.
		AuthorityDeregistered(T::AccountId, u32),
	}
	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		NoAuthorities,
	}

	pub struct CordSessionManager<T>(sp_std::marker::PhantomData<T>);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(sp_std::marker::PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// Add new authorities to the set.
		///
		/// The new authorities will be active from current session + 2.
		#[pallet::weight(100_000)]
		pub fn register_authorities(
			origin: OriginFor<T>,
			authority: T::AccountId,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;
			let mut authorities: Vec<T::AccountId>;
			if <Authorities<T>>::get().is_none() {
				authorities = vec![authority.clone()];
			} else {
				authorities = <Authorities<T>>::get().unwrap();
				authorities.push(authority.clone());
			}

			let current_session = Session::<T>::current_index();

			Authorities::<T>::put(authorities);
			SessionForAuthoritiesChange::<T>::put(current_session + 2);

			Self::deposit_event(Event::AuthorityRegistered(authority, current_session + 2));
			Ok(())
		}

		/// Remove authorities from the set.
		///
		/// The removed authorities will be deactivated from current session + 2.
		#[pallet::weight(100_000)]
		pub fn deregister_authorities(
			origin: OriginFor<T>,
			authority: T::AccountId,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;
			let ref mut authorities = <Authorities<T>>::get().ok_or(Error::<T>::NoAuthorities)?;
			authorities.retain(|x| x != &authority);

			let current_session = Session::<T>::current_index();

			Authorities::<T>::put(authorities);
			SessionForAuthoritiesChange::<T>::put(current_session + 2);

			Self::deposit_event(Event::AuthorityDeregistered(authority, current_session + 2));
			Ok(())
		}
	}

	#[pallet::type_value]
	pub(super) fn DefaultForSessionPeriod() -> u32 {
		CORD_SESSION_PERIOD
	}

	#[pallet::storage]
	#[pallet::getter(fn session_period)]
	pub(super) type SessionPeriod<T: Config> =
		StorageValue<_, u32, ValueQuery, DefaultForSessionPeriod>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub authorities: Vec<T::AccountId>,
		pub session_period: u32,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { authorities: Vec::new(), session_period: CORD_SESSION_PERIOD }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize_authorities(&self.authorities);
			<SessionPeriod<T>>::put(&self.session_period);
		}
	}

	impl<T: Config> Pallet<T> {
		fn initialize_authorities(authorities: &[T::AccountId]) {
			if !authorities.is_empty() {
				assert!(<Authorities<T>>::get().is_none(), "Authorities are already initialized!");
				<Authorities<T>>::put(authorities);
			}
		}
	}

	impl<T: Config> SessionManager<T::AccountId> for CordSessionManager<T> {
		fn new_session(session: u32) -> Option<Vec<T::AccountId>> {
			if let Some(session_for_authorities_change) =
				Pallet::<T>::session_for_authorities_change()
			{
				if session_for_authorities_change == session {
					let authorities = Pallet::<T>::authorities().expect(
						"Authorities also should be Some(), when session_for_authorities_change is",
					);
					return Some(authorities);
				}
			}
			None
		}

		fn start_session(_: u32) {}

		fn end_session(_: u32) {}
	}
}
