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
//! Authority membership management
#![warn(unused_extern_crates)]
#![cfg_attr(not(feature = "std"), no_std)]

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
	use frame_support::traits::ValidatorRegistration;
	use frame_system::pallet_prelude::*;
	use network_membership::traits::IsMember;
	// use sp_runtime::traits::{Convert, IsMember};
	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configuration.
	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_session::Config + pallet_network_membership::Config
	{
		/// The overreaching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Privileged origin that can add or remove validators.
		type AuthorityMembershipOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A member will be added to the authority membership.
		MemberAdded(T::AccountId),
		/// A member will leave the set of authorities in 2 sessions.
		MemberGoOffline(T::AccountId),
		/// A member will enter the set of authorities in 2 sessions.
		MemberGoOnline(T::AccountId),
		/// this member will be removed from the authority set in 2 sessions.
		MemberRemoved(T::AccountId),
		/// A member has been removed from the blacklist.
		MemberWhiteList(T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Already incoming
		MemberAlreadyIncoming,
		/// The authority entry already exists.
		MemberAlreadyExists,
		/// Already outgoing
		MemberAlreadyOutgoing,
		/// Not found owner key
		/// There is no authority with the given ID.
		MemberNotFound,
		/// Member is blacklisted
		MemberBlackListed,
		/// Session keys not provided
		SessionKeysNotAdded,
		/// Member not blacklisted
		MemberNotBlackListed,
		/// Not a network member
		NetworkMembershipNotFound,
	}

	/// list incoming authorities
	#[pallet::storage]
	#[pallet::getter(fn incoming)]
	pub type IncomingAuthorities<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	/// list outgoing authorities
	#[pallet::storage]
	#[pallet::getter(fn outgoing)]
	pub type OutgoingAuthorities<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	/// maps member id to member data
	#[pallet::storage]
	#[pallet::getter(fn member)]
	pub type Members<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	// Blacklist.
	#[pallet::storage]
	#[pallet::getter(fn blacklist)]
	pub type BlackList<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub initial_authorities: Vec<T::ValidatorId>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { initial_authorities: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let members = &self.initial_authorities;

			if !members.is_empty() {
				Members::<T>::put(members);
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add new authorities to the set.
		/// The new authorities will be active from current session + 2.
		#[pallet::call_index(0)]
		#[pallet::weight({100_000})]
		pub fn nominate(origin: OriginFor<T>, candidate: T::AccountId) -> DispatchResult {
			T::AuthorityMembershipOrigin::ensure_origin(origin)?;

			if !pallet_network_membership::Pallet::<T>::is_member(&candidate) {
				return Err(Error::<T>::NetworkMembershipNotFound.into())
			}

			let member = T::ValidatorIdOf::convert(candidate.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;
			if !pallet_session::Pallet::<T>::is_registered(&member) {
				return Err(Error::<T>::SessionKeysNotAdded.into())
			}

			if Self::is_blacklisted(&member) {
				return Err(Error::<T>::MemberBlackListed.into())
			}

			if Self::is_incoming(&member) {
				return Err(Error::<T>::MemberAlreadyIncoming.into())
			}
			if Self::is_outgoing(&member) {
				return Err(Error::<T>::MemberAlreadyOutgoing.into())
			}

			Self::add_authority_member(&member)?;

			Self::deposit_event(Event::MemberAdded(candidate));
			Ok(())
		}

		/// Remove authorities from the set.
		/// The removed authorities will be deactivated from current session + 2
		#[pallet::call_index(1)]
		#[pallet::weight({100_000})]
		pub fn remove(origin: OriginFor<T>, candidate: T::AccountId) -> DispatchResult {
			T::AuthorityMembershipOrigin::ensure_origin(origin)?;

			let member = T::ValidatorIdOf::convert(candidate.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			Self::remove_authority_member(&member)?;

			// Purge session keys
			if let Err(e) = pallet_session::Pallet::<T>::purge_keys(
				frame_system::Origin::<T>::Signed(candidate.clone()).into(),
			) {
				log::error!(
					target: "runtime::authorities",
					"Logic error: fail to purge session keys : {:?}",
					e
				);
			}

			Self::deposit_event(Event::MemberRemoved(candidate));
			Ok(())
		}

		/// Remove members from blacklist.
		#[pallet::call_index(2)]
		#[pallet::weight({100_000})]
		pub fn remove_member_from_blacklist(
			origin: OriginFor<T>,
			candidate: T::AccountId,
		) -> DispatchResult {
			T::AuthorityMembershipOrigin::ensure_origin(origin)?;

			let member = T::ValidatorIdOf::convert(candidate.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			ensure!(!<BlackList<T>>::get().contains(&member), Error::<T>::MemberNotBlackListed);

			Self::remove_from_blacklist(&member)?;

			Self::deposit_event(Event::MemberWhiteList(candidate));
			Ok(())
		}

		/// Mark an authority member offline.
		/// The authority will be deactivated from current session + 2.
		#[pallet::call_index(3)]
		#[pallet::weight({100_000})]
		pub fn go_offline(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let member = T::ValidatorIdOf::convert(who.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			ensure!(<Members<T>>::get().contains(&member), Error::<T>::MemberNotFound);

			Self::mark_for_removal(member);
			Self::deposit_event(Event::MemberGoOffline(who));
			Ok(())
		}

		/// Mark an authority member going online.
		/// Authority will be activated from current session + 2.
		#[pallet::call_index(4)]
		#[pallet::weight({100_000})]
		pub fn go_online(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let member = T::ValidatorIdOf::convert(who.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			ensure!(<Members<T>>::get().contains(&member), Error::<T>::MemberNotFound);

			Self::mark_for_addition(member);

			Self::deposit_event(Event::MemberGoOnline(who));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn add_authority_member(authority: &T::ValidatorId) -> DispatchResult {
		ensure!(!<Members<T>>::get().contains(authority), Error::<T>::MemberAlreadyExists);
		Members::<T>::mutate(|v| v.push(authority.clone()));
		Self::mark_for_addition(authority.clone());
		Ok(())
	}
	fn remove_authority_member(authority: &T::ValidatorId) -> DispatchResult {
		ensure!(<Members<T>>::get().contains(authority), Error::<T>::MemberNotFound);
		Members::<T>::mutate(|vs| vs.retain(|v| *v != *authority));
		Self::mark_for_removal(authority.clone());
		Ok(())
	}
	fn remove_from_blacklist(authority: &T::ValidatorId) -> DispatchResult {
		BlackList::<T>::mutate(|vs| vs.retain(|v| *v != *authority));
		Ok(())
	}

	// Adds offline authorities to a local cache for removal.
	fn mark_for_removal(authority: T::ValidatorId) {
		OutgoingAuthorities::<T>::mutate(|v| v.push(authority));
	}
	// Adds offline authorities to a local cache for readdition.
	fn mark_for_addition(authority: T::ValidatorId) {
		IncomingAuthorities::<T>::mutate(|v| v.push(authority));
	}
	// Adds offline authorities to a local cache for removal and blacklist.
	fn mark_for_blacklist_removal(authority: T::ValidatorId) {
		BlackList::<T>::mutate(|v| v.push(authority.clone()));
		OutgoingAuthorities::<T>::mutate(|v| v.push(authority));
	}
	/// check if authority is incoming
	fn is_incoming(authority: &T::ValidatorId) -> bool {
		IncomingAuthorities::<T>::get().contains(authority)
	}
	/// check if authority is outgoing
	fn is_outgoing(authority: &T::ValidatorId) -> bool {
		OutgoingAuthorities::<T>::get().contains(authority)
	}
	/// check if authority is blacklisted
	fn is_blacklisted(authority: &T::ValidatorId) -> bool {
		BlackList::<T>::get().contains(authority)
	}
}

impl<T: Config> pallet_session::SessionManager<T::ValidatorId> for Pallet<T> {
	fn new_session(new_index: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		if new_index <= 1 {
			return None
		}

		let mut authorities = Session::<T>::validators();

		OutgoingAuthorities::<T>::take().iter().for_each(|v| {
			if let Some(pos) = authorities.iter().position(|r| r == v) {
				authorities.swap_remove(pos);
			}
		});

		IncomingAuthorities::<T>::take().into_iter().for_each(|v| {
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

impl<T: Config, O: Offence<(T::AccountId, T::AccountId)>>
	ReportOffence<T::AccountId, (T::AccountId, T::AccountId), O> for Pallet<T>
{
	fn report_offence(_reporters: Vec<T::AccountId>, offence: O) -> Result<(), OffenceError> {
		let offenders = offence.offenders();

		for (a, _) in offenders.into_iter() {
			let v = T::ValidatorIdOf::convert(a).ok_or(OffenceError::DuplicateReport)?;
			Self::mark_for_blacklist_removal(v);
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
