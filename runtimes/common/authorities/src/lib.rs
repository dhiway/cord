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

pub mod impls;

use frame_support::{dispatch::DispatchResult, ensure, pallet_prelude::*, traits::EnsureOrigin};
pub use pallet::*;
use sp_staking::SessionIndex;
use sp_std::{vec, vec::Vec};

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(test)]
pub mod tests;

type Session<T> = pallet_session::Pallet<T>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{Convert, IsMember};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configuration.
	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_session::Config + pallet_session::historical::Config
	{
		/// The overreaching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type IsMember: IsMember<Self::AccountId>;
		#[pallet::constant]
		type MinAuthorities: Get<u32>;
		/// Privileged origin that can add or remove validators.
		type AuthorityMembershipOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// List of members who will enter the set of authorities at the next
		/// session. [Vec<member_id>]
		IncomingAuthorities(Vec<T::ValidatorId>),
		/// List of members who will leave the set of authorities at the next
		/// session. [Vec<member_id>]
		OutgoingAuthorities(Vec<T::ValidatorId>),
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
		/// A member is scheduled for removal in 2 sessions due to non-availability.
		MemberDisconnected(T::ValidatorId),
		/// A member is added to the blacklist and is scheduled for removal in 2 sessions due to
		/// non-availability.
		MemberBlacklistedRemoved(T::ValidatorId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Already incoming
		MemberAlreadyIncoming,
		/// The authority entry already exists.
		MemberAlreadyExists,
		/// Already outgoing
		MemberAlreadyOutgoing,
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
		/// Authority count below threshold
		TooLowAuthorityCount,
	}

	/// list incoming authorities
	#[pallet::storage]
	pub type IncomingAuthorities<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	/// list outgoing authorities
	#[pallet::storage]
	pub type OutgoingAuthorities<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	/// maps member id to member data
	#[pallet::storage]
	pub type Members<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	// Blacklist.
	#[pallet::storage]
	pub type BlackList<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub initial_authorities: Vec<T::ValidatorId>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			assert!(<Members<T>>::get().is_empty(), "Authorities are already initialized!");
			<Members<T>>::put(&self.initial_authorities);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add new authorities to the set.
		/// The new authorities will be active from current session + 2.
		#[pallet::call_index(0)]
		#[pallet::weight({200_000})]
		pub fn nominate(origin: OriginFor<T>, candidate: T::AccountId) -> DispatchResult {
			T::AuthorityMembershipOrigin::ensure_origin(origin)?;

			if !T::IsMember::is_member(&candidate) {
				return Err(Error::<T>::NetworkMembershipNotFound.into());
			}

			let member = T::ValidatorIdOf::convert(candidate.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			if Self::is_blacklisted(&member) {
				return Err(Error::<T>::MemberBlackListed.into());
			}

			if Self::is_incoming(&member) {
				return Err(Error::<T>::MemberAlreadyIncoming.into());
			}
			if Self::is_outgoing(&member) {
				return Err(Error::<T>::MemberAlreadyOutgoing.into());
			}

			Self::add_authority_member(&member)?;

			Self::deposit_event(Event::MemberAdded(candidate));
			Ok(())
		}

		/// Remove authorities from the set.
		/// The removed authorities will be deactivated from current session + 2
		#[pallet::call_index(1)]
		#[pallet::weight({200_000})]
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
		#[pallet::weight({200_000})]
		pub fn remove_member_from_blacklist(
			origin: OriginFor<T>,
			candidate: T::AccountId,
		) -> DispatchResult {
			T::AuthorityMembershipOrigin::ensure_origin(origin)?;

			let member = T::ValidatorIdOf::convert(candidate.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			ensure!(<BlackList<T>>::get().contains(&member), Error::<T>::MemberNotBlackListed);

			Self::remove_from_blacklist(&member)?;

			Self::deposit_event(Event::MemberWhiteList(candidate));
			Ok(())
		}

		/// Mark an authority member offline.
		/// The authority will be deactivated from current session + 2.
		#[pallet::call_index(3)]
		#[pallet::weight({200_000})]
		pub fn go_offline(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let member = T::ValidatorIdOf::convert(who.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			let authorities = <Members<T>>::get();
			ensure!(authorities.contains(&member), Error::<T>::MemberNotFound);
			ensure!(
				authorities.len().saturating_sub(1) as u32 >= T::MinAuthorities::get(),
				Error::<T>::TooLowAuthorityCount
			);
			Self::mark_for_removal(member);
			Self::deposit_event(Event::MemberGoOffline(who));
			Ok(())
		}

		/// Mark an authority member going online.
		/// Authority will be activated from current session + 2.
		#[pallet::call_index(4)]
		#[pallet::weight({200_000})]
		pub fn go_online(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let member = T::ValidatorIdOf::convert(who.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			if Self::is_blacklisted(&member) {
				return Err(Error::<T>::MemberBlackListed.into());
			}

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
		let mut authorities = <Members<T>>::get();
		ensure!(authorities.contains(authority), Error::<T>::MemberNotFound);
		ensure!(
			authorities.len().saturating_sub(1) as u32 >= T::MinAuthorities::get(),
			Error::<T>::TooLowAuthorityCount
		);
		authorities.retain(|v| *v != *authority);
		<Members<T>>::put(&authorities);
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
	// Adds offline authorities reported by imOnline to a local cache for removal.
	fn mark_for_disconnect(authority: T::ValidatorId) {
		OutgoingAuthorities::<T>::mutate(|v| v.push(authority.clone()));
		Self::deposit_event(Event::MemberDisconnected(authority));
	}
	// Adds offline authorities to a local cache for readdition.
	fn mark_for_addition(authority: T::ValidatorId) {
		IncomingAuthorities::<T>::mutate(|v| v.push(authority));
	}
	// Adds offline authorities to a local cache for removal and blacklist.
	fn mark_for_blacklist_and_removal(authority: T::ValidatorId) {
		BlackList::<T>::mutate(|v| v.push(authority.clone()));
		OutgoingAuthorities::<T>::mutate(|v| v.push(authority.clone()));
		Self::deposit_event(Event::MemberBlacklistedRemoved(authority));
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
	fn new_session(_new_index: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		let members_to_add = IncomingAuthorities::<T>::take();
		let members_to_del = OutgoingAuthorities::<T>::take();

		if members_to_add.is_empty() {
			if members_to_del.is_empty() {
				// when no change to the set of autorities, return None
				return None;
			} else {
				Self::deposit_event(Event::OutgoingAuthorities(members_to_del.clone()));
			}
		} else {
			Self::deposit_event(Event::IncomingAuthorities(members_to_add.clone()));
		}

		let mut authorities = Session::<T>::validators();

		members_to_del.iter().for_each(|v| {
			if let Some(pos) = authorities.iter().position(|r| r == v) {
				authorities.swap_remove(pos);
			}
		});

		members_to_add.into_iter().for_each(|v| {
			if !authorities.contains(&v) {
				authorities.push(v);
			}
		});

		Some(authorities)
	}

	/// Same as `new_session`, but it this should only be called at genesis.
	fn new_session_genesis(_new_index: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		Some(Members::<T>::get().into_iter().collect())
	}

	fn end_session(_: SessionIndex) {}

	fn start_session(_start_index: SessionIndex) {}
}

// see substrate FullIdentification
fn add_full_identification<T: Config>(
	validator_id: T::ValidatorId,
) -> Option<(T::ValidatorId, T::FullIdentification)> {
	use sp_runtime::traits::Convert as _;
	T::FullIdentificationOf::convert(validator_id.clone())
		.map(|full_ident| (validator_id, full_ident))
}

impl<T: Config> pallet_session::historical::SessionManager<T::ValidatorId, T::FullIdentification>
	for Pallet<T>
{
	fn new_session(
		new_index: SessionIndex,
	) -> Option<Vec<(T::ValidatorId, T::FullIdentification)>> {
		<Self as pallet_session::SessionManager<_>>::new_session(new_index).map(|validators_ids| {
			validators_ids.into_iter().filter_map(add_full_identification::<T>).collect()
		})
	}
	fn new_session_genesis(
		new_index: SessionIndex,
	) -> Option<sp_std::vec::Vec<(T::ValidatorId, T::FullIdentification)>> {
		<Self as pallet_session::SessionManager<_>>::new_session_genesis(new_index).map(
			|validators_ids| {
				validators_ids.into_iter().filter_map(add_full_identification::<T>).collect()
			},
		)
	}

	fn start_session(start_index: SessionIndex) {
		<Self as pallet_session::SessionManager<_>>::start_session(start_index)
	}

	fn end_session(end_index: SessionIndex) {
		<Self as pallet_session::SessionManager<_>>::end_session(end_index)
	}
}
