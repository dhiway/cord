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

//! Authority membership management

#![warn(unused_extern_crates)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use crate::pallet::*;
use alloc::vec::Vec;
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::*,
	traits::{EnsureOrigin, Get},
};
use sp_runtime::traits::Convert;
use sp_staking::SessionIndex;

// Aliases used outside the pallet module
type Session<T> = pallet_session::Pallet<T>;

#[cfg(any(test, feature = "runtime-benchmarks"))]
pub mod mock;
#[cfg(test)]
pub mod tests;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// Configuration.
	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_session::Config + pallet_session::historical::Config
	{
		/// The overarching event type.
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Never select fewer than this number.
		#[pallet::constant]
		type MinAuthorities: Get<u32>;

		/// Origin allowed to curate membership & invulnerables.
		type AuthorityManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Candidate pool (registered authorities).
	#[pallet::storage]
	pub type Registered<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	/// Pending manual adds/removes applied at next rotation.
	#[pallet::storage]
	pub type PendingAdditions<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	#[pallet::storage]
	pub type PendingRemovals<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	/// Last enacted active set (ops/fallback).
	#[pallet::storage]
	pub type LastActive<T: Config> = StorageValue<_, Vec<T::ValidatorId>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Membership changes planned for next session (adds + removes).
		Planned { add: Vec<T::ValidatorId>, remove: Vec<T::ValidatorId> },
		/// New active validator set enacted for the current session.
		Enacted { active: Vec<T::ValidatorId> },
		/// A candidate has been queued to join in the next rotation.
		QueuedAdd(T::AccountId),
		/// A member has been queued to be removed in the next rotation.
		QueuedRemoval(T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The candidate is already a registered member.
		AlreadyMember,
		/// The given account is not a registered member.
		NotMember,
		/// The candidate is already queued for addition/removal.
		AlreadyQueued,
		/// Removing this member would drop below MinAuthorities.
		TooLowAuthorityCount,
		/// Candidate does not have session keys registered.
		SessionKeysNotQueued,
	}

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub initial_authorities: Vec<T::AccountId>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			assert!(<Registered<T>>::get().is_empty(), "Authorities are already initialized!");

			let mut reg: Vec<T::ValidatorId> = Vec::new();
			for acc in &self.initial_authorities {
				let vid = T::ValidatorIdOf::convert(acc.clone())
					.expect("initial authorities AccountId has no associated ValidatorId");
				if !reg.contains(&vid) {
					reg.push(vid);
				}
			}

			Registered::<T>::put(reg.clone());
			LastActive::<T>::put(reg);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a candidate (requires session keys staged via `session.setKeys`).
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::nominate())]
		pub fn nominate(origin: T::RuntimeOrigin, who: T::AccountId) -> DispatchResult {
			T::AuthorityManagerOrigin::ensure_origin(origin)?;

			let vid = T::ValidatorIdOf::convert(who.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			ensure!(Self::has_staged_keys(&vid), Error::<T>::SessionKeysNotQueued);
			ensure!(!Registered::<T>::get().contains(&vid), Error::<T>::AlreadyMember);
			ensure!(!PendingAdditions::<T>::get().contains(&vid), Error::<T>::AlreadyQueued);

			PendingAdditions::<T>::mutate(|s| s.push(vid.clone()));

			Registered::<T>::mutate(|r| {
				if !r.contains(&vid) {
					r.push(vid.clone());
				}
			});

			Self::deposit_event(Event::QueuedAdd(who));
			Ok(())
		}

		/// Remove a candidate (applies next rotation).
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove())]
		pub fn remove(origin: T::RuntimeOrigin, who: T::AccountId) -> DispatchResult {
			T::AuthorityManagerOrigin::ensure_origin(origin)?;

			let vid = T::ValidatorIdOf::convert(who.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			let reg = Registered::<T>::get();
			ensure!(reg.contains(&vid), Error::<T>::NotMember);

			let pending = PendingRemovals::<T>::get();
			ensure!(!pending.contains(&vid), Error::<T>::AlreadyQueued);

			let mut seen: Vec<T::ValidatorId> = Vec::new();
			let mut queued_effective = 0usize;
			for id in pending.iter() {
				if reg.contains(id) && !seen.contains(id) {
					queued_effective = queued_effective.saturating_add(1);
					seen.push(id.clone());
				}
			}

			ensure!(
				reg.len().saturating_sub(queued_effective.saturating_add(1))
					>= T::MinAuthorities::get() as usize,
				Error::<T>::TooLowAuthorityCount
			);

			PendingRemovals::<T>::mutate(|s| s.push(vid.clone()));

			Self::deposit_event(Event::QueuedRemoval(who));
			Ok(())
		}
	}
}

impl<T: pallet::Config> Pallet<T> {
	#[inline]
	fn has_staged_keys(vid: &T::ValidatorId) -> bool {
		pallet_session::NextKeys::<T>::contains_key(vid)
			|| pallet_session::QueuedKeys::<T>::get().iter().any(|(v, _)| v == vid)
	}

	/// Deterministic selection: take all registered with staged keys.
	fn select_active() -> Vec<T::ValidatorId> {
		let with_keys: Vec<T::ValidatorId> = pallet::Registered::<T>::get()
			.into_iter()
			.filter(|id| Self::has_staged_keys(id))
			.collect();

		let min = T::MinAuthorities::get() as usize;
		if with_keys.len() >= min {
			return with_keys;
		}
		// Fallbacks to avoid empty sets:
		let last = pallet::LastActive::<T>::get();
		if last.len() >= min as usize {
			return last;
		}
		let cur = Session::<T>::validators();
		if cur.len() >= min as usize {
			return cur;
		}
		// As a last resort, return whatever registered we have (even if < min).
		pallet::Registered::<T>::get()
	}

	/// Apply deltas (if any) and select next set.
	fn on_new_session_rotate(_next_idx: SessionIndex) -> Vec<T::ValidatorId> {
		let adds = pallet::PendingAdditions::<T>::take();
		let rems = pallet::PendingRemovals::<T>::take();

		let has_change = !adds.is_empty() || !rems.is_empty();

		if has_change {
			pallet::Registered::<T>::mutate(|r| {
				for v in adds.iter() {
					if !r.contains(v) {
						r.push(v.clone());
					}
				}
				for v in rems.iter() {
					r.retain(|x| x != v);
				}
			});

			pallet::Pallet::<T>::deposit_event(pallet::Event::Planned { add: adds, remove: rems });
		}

		Self::select_active()
	}
}

impl<T: pallet::Config> pallet_session::SessionManager<T::ValidatorId> for Pallet<T> {
	fn new_session(next: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		let next_set = Pallet::<T>::on_new_session_rotate(next);
		let cur = pallet_session::Pallet::<T>::validators();
		if next_set == cur {
			None
		} else {
			pallet::LastActive::<T>::put(next_set.clone());
			Pallet::<T>::deposit_event(pallet::Event::Enacted { active: next_set.clone() });
			Some(next_set)
		}
	}

	fn new_session_genesis(_index: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		Some(pallet::Registered::<T>::get())
	}

	fn end_session(_: SessionIndex) {}
	fn start_session(_: SessionIndex) {}
}

fn map_full_id<T: pallet::Config>(
	vid: T::ValidatorId,
) -> Option<(T::ValidatorId, T::FullIdentification)> {
	T::FullIdentificationOf::convert(vid.clone()).map(|fi| (vid, fi))
}

impl<T: pallet::Config>
	pallet_session::historical::SessionManager<T::ValidatorId, T::FullIdentification> for Pallet<T>
{
	fn new_session(idx: SessionIndex) -> Option<Vec<(T::ValidatorId, T::FullIdentification)>> {
		<Self as pallet_session::SessionManager<_>>::new_session(idx)
			.map(|ids| ids.into_iter().filter_map(map_full_id::<T>).collect())
	}

	fn new_session_genesis(
		_idx: SessionIndex,
	) -> Option<Vec<(T::ValidatorId, T::FullIdentification)>> {
		let initial = pallet::Registered::<T>::get();
		Some(initial.into_iter().filter_map(map_full_id::<T>).collect())
	}

	fn start_session(i: SessionIndex) {
		<Self as pallet_session::SessionManager<_>>::start_session(i)
	}
	fn end_session(i: SessionIndex) {
		<Self as pallet_session::SessionManager<_>>::end_session(i)
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<T: pallet::Config> Pallet<T> {
	pub fn bench_rotate() {
		let _ = Self::on_new_session_rotate(0u32.into());
	}
}
