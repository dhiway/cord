// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.
//
//! A pallet for managing authorities on CORD.

use sp_staking::SessionIndex;
use sp_std::vec::Vec;

use frame_support::{dispatch::DispatchResult, ensure, pallet_prelude::*, traits::EnsureOrigin};
pub use pallet::*;
use sp_runtime::traits::Convert;
use sp_staking::offence::{Offence, OffenceError, ReportOffence};

type Session<T> = pallet_session::Pallet<T>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configuration for the proposer.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_session::Config {
		/// The overreaching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Privileged origin that can add or remove validators.
		type AuthorityOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;
		#[pallet::constant]
		type MaxProposalLength: Get<u16>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New Authorities were added to the set.
		RegistrationInitiated(Vec<T::ValidatorId>),
		/// Authorities were removed from the set.
		RemovalInitiated(Vec<T::ValidatorId>),
		/// An authority is marked offline.
		GoingOnlineInitiated(T::ValidatorId),
		/// An authority is marked online.
		GoingOfflineInitiated(T::ValidatorId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no authority with the given ID.
		AuthorityNotFound,
		/// The authority entry already exists.
		AuthorityAlreadyExists,
		/// No validator associated with the identity.
		NoAssociatedValidatorId,
		/// Not an authority owner.
		BadOrigin,
		/// Max authorities included in a proposal exceeds the limit.
		MaxProposalLimitExceeded,
	}

	/// Authority Membership.
	#[pallet::storage]
	pub(crate) type AuthorityMembers<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	/// Authorities that should be retired.
	#[pallet::storage]
	pub(crate) type AuthoritiesToRetire<T: Config> =
		StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	/// Authorities that should be added.
	#[pallet::storage]
	pub(crate) type AuthoritiesToAdd<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub authorities: Vec<T::ValidatorId>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { authorities: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let authorities = &self.authorities;

			if !authorities.is_empty() {
				AuthorityMembers::<T>::put(authorities);
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add new authorities to the set.
		/// The new authorities will be active from current session + 2.
		#[pallet::call_index(0)]
		#[pallet::weight({100_000})]
		pub fn register(origin: OriginFor<T>, authorities: Vec<T::ValidatorId>) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(
				authorities.len() <= T::MaxProposalLength::get() as usize,
				Error::<T>::MaxProposalLimitExceeded
			);

			Self::add_authority_member(authorities.clone())?;
			authorities.clone().into_iter().for_each(AuthoritiesToAdd::<T>::append);

			Self::deposit_event(Event::RegistrationInitiated(authorities));
			Ok(())
		}

		/// Remove authorities from the set.
		/// The removed authorities will be deactivated from current session + 2
		#[pallet::call_index(1)]
		#[pallet::weight({100_000})]
		pub fn remove(origin: OriginFor<T>, authorities: Vec<T::ValidatorId>) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(
				authorities.len() <= T::MaxProposalLength::get() as usize,
				Error::<T>::MaxProposalLimitExceeded
			);

			Self::remove_authority_member(authorities.clone())?;
			authorities.clone().into_iter().for_each(AuthoritiesToRetire::<T>::append);

			Self::deposit_event(Event::RemovalInitiated(authorities));
			Ok(())
		}

		/// Mark an authority offline.
		/// The authority will be deactivated from current session + 2.
		#[pallet::call_index(2)]
		#[pallet::weight({100_000})]
		pub fn go_offline(origin: OriginFor<T>, authority: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == authority, Error::<T>::BadOrigin);
			let validator_id =
				T::ValidatorIdOf::convert(who).ok_or(Error::<T>::NoAssociatedValidatorId)?;

			ensure!(
				<AuthorityMembers<T>>::get().contains(&validator_id),
				Error::<T>::NoAssociatedValidatorId
			);

			<AuthoritiesToRetire<T>>::mutate(|v| v.push(validator_id.clone()));

			Self::deposit_event(Event::GoingOfflineInitiated(validator_id));
			Ok(())
		}

		/// An extisting offline authority is going online.
		/// Authority will be activated from current session + 2.
		#[pallet::call_index(3)]
		#[pallet::weight({100_000})]
		pub fn go_online(origin: OriginFor<T>, authority: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == authority, Error::<T>::BadOrigin);
			let validator_id =
				T::ValidatorIdOf::convert(who).ok_or(Error::<T>::NoAssociatedValidatorId)?;

			ensure!(
				<AuthorityMembers<T>>::get().contains(&validator_id),
				Error::<T>::NoAssociatedValidatorId
			);

			<AuthoritiesToAdd<T>>::mutate(|v| v.push(validator_id.clone()));

			Self::deposit_event(Event::GoingOnlineInitiated(validator_id));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn add_authority_member(authorities: Vec<T::ValidatorId>) -> DispatchResult {
		for a in authorities {
			ensure!(!<AuthorityMembers<T>>::get().contains(&a), Error::<T>::AuthorityAlreadyExists);
			<AuthorityMembers<T>>::mutate(|v| v.push(a));
		}
		Ok(())
	}
	fn remove_authority_member(authorities: Vec<T::ValidatorId>) -> DispatchResult {
		for a in authorities {
			ensure!(<AuthorityMembers<T>>::get().contains(&a), Error::<T>::AuthorityNotFound);
			<AuthorityMembers<T>>::mutate(|vs| vs.retain(|v| *v != a));
		}
		Ok(())
	}
	// Adds offline authoritities to a local cache for removal at new session.
	fn mark_for_removal(authority: T::ValidatorId) {
		<AuthoritiesToRetire<T>>::mutate(|v| v.push(authority));
	}
}

impl<T: Config> pallet_session::SessionManager<T::ValidatorId> for Pallet<T> {
	fn new_session(new_index: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		if new_index <= 1 {
			return None
		}

		let mut authorities = Session::<T>::validators();

		AuthoritiesToRetire::<T>::take().iter().for_each(|v| {
			if let Some(pos) = authorities.iter().position(|r| r == v) {
				authorities.swap_remove(pos);
			}
		});

		AuthoritiesToAdd::<T>::take().into_iter().for_each(|v| {
			if !authorities.contains(&v) {
				authorities.push(v);
			}
		});

		Some(authorities)
	}

	fn end_session(_: SessionIndex) {}

	fn start_session(_start_index: SessionIndex) {}
}

impl<T: Config> pallet_session::historical::SessionManager<T::ValidatorId, ()> for Pallet<T> {
	fn new_session(new_index: SessionIndex) -> Option<Vec<(T::ValidatorId, ())>> {
		<Self as pallet_session::SessionManager<_>>::new_session(new_index)
			.map(|r| r.into_iter().map(|v| (v, Default::default())).collect())
	}

	fn start_session(start_index: SessionIndex) {
		<Self as pallet_session::SessionManager<_>>::start_session(start_index)
	}

	fn end_session(end_index: SessionIndex) {
		<Self as pallet_session::SessionManager<_>>::end_session(end_index)
	}
}

// TODO Imporve Offence reporting and unresponsiveness management.
impl<T: Config, O: Offence<(T::AccountId, T::AccountId)>>
	ReportOffence<T::AccountId, (T::AccountId, T::AccountId), O> for Pallet<T>
{
	fn report_offence(_reporters: Vec<T::AccountId>, offence: O) -> Result<(), OffenceError> {
		let offenders = offence.offenders();

		for (a, _) in offenders.into_iter() {
			let v = T::ValidatorIdOf::convert(a).ok_or(OffenceError::DuplicateReport)?;
			Self::mark_for_removal(v);
		}

		Ok(())
	}

	fn is_known_offence(
		_offenders: &[(T::AccountId, T::AccountId)],
		_time_slot: &O::TimeSlot,
	) -> bool {
		false
	}
}
