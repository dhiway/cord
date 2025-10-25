// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later
//! Authority membership management
#![warn(unused_extern_crates)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use crate::pallet::*;
use alloc::vec::Vec;
use frame_support::traits::Get;
use frame_support::{dispatch::DispatchResult, ensure, pallet_prelude::*, BoundedVec};
use sp_runtime::traits::Convert;
use sp_staking::SessionIndex;

// Aliases used outside the pallet module
type Session<T> = pallet_session::Pallet<T>;

#[cfg(test)]
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
		/// Never select fewer than this number.
		#[pallet::constant]
		type MinAuthorities: Get<u32>;
		/// Upper bound for storage collections.
		#[pallet::constant]
		type MaxAuthorities: Get<u32>;
		/// Max count of invulnerables.
		#[pallet::constant]
		type MaxInvulnerables: Get<u32>;
		/// Target active set size per session (must be >= MinAuthorities).
		#[pallet::constant]
		type TargetActive: Get<u32>;
		/// Kick after this many consecutive offline sessions (for ACTIVE validators).
		#[pallet::constant]
		type KickThresholdSessions: Get<u32>;
		/// Origin allowed to curate membership & invulnerables.
		type AuthorityMembershipOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Liveness Provider
		type Liveness: LivenessProvider;
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
	pub type Registered<T: Config> =
		StorageValue<_, BoundedVec<T::ValidatorId, T::MaxAuthorities>, ValueQuery>;

	/// Always included; never kicked by liveness.
	#[pallet::storage]
	pub type Invulnerables<T: Config> =
		StorageValue<_, BoundedVec<T::ValidatorId, T::MaxInvulnerables>, ValueQuery>;

	/// Pending manual adds/removes applied at next rotation.
	#[pallet::storage]
	pub type PendingAdditions<T: Config> =
		StorageValue<_, BoundedVec<T::ValidatorId, T::MaxAuthorities>, ValueQuery>;
	#[pallet::storage]
	pub type PendingRemovals<T: Config> =
		StorageValue<_, BoundedVec<T::ValidatorId, T::MaxAuthorities>, ValueQuery>;

	/// Consecutive missed sessions (updated only for ACTIVE validators).
	#[pallet::storage]
	pub type MissCount<T: Config> = StorageMap<_, Twox64Concat, T::ValidatorId, u32, ValueQuery>;

	/// Mirror of last enacted active set (for ops/fallback).
	#[pallet::storage]
	pub type LastActive<T: Config> =
		StorageValue<_, BoundedVec<T::ValidatorId, T::MaxAuthorities>, ValueQuery>;

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
		/// Invulnerables list has been updated (with the new count).
		InvulnerablesSet { count: u32 },
		/// A member was kicked for repeated liveness failures (misses >= threshold).
		Kicked { validator: T::ValidatorId, miss_count: u32 },
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
		/// Too many invulnerables were specified (exceeds MaxInvulnerables).
		TooManyInvulnerables,
		/// Candidate does not have session keys registered.
		SessionKeysNotQueued,
		/// Cannot remove an invulnerable authority.
		CannotRemoveInvulnerable,
		/// Candidate pool (`Registered`) has reached capacity.
		CandidatePoolFull,
		/// Pending list (additions/removals) has reached capacity.
		PendingListFull,
	}

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub initial_authorities: Vec<T::ValidatorId>,
		pub invulnerables: Vec<T::ValidatorId>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			assert!(
				T::TargetActive::get() >= T::MinAuthorities::get(),
				"TargetActive must be >= MinAuthorities"
			);
			assert!(
				T::TargetActive::get() <= T::MaxAuthorities::get(),
				"TargetActive must be <= MaxAuthorities"
			);
			assert!(<Registered<T>>::get().is_empty(), "Authorities are already initialized!");

			let mut reg = BoundedVec::<T::ValidatorId, T::MaxAuthorities>::default();
			for a in &self.initial_authorities {
				// Fail fast on duplicates and overflow.
				if reg.contains(a) {
					panic!("initial_authorities contains a duplicate entry");
				}
				reg.try_push(a.clone()).expect("initial_authorities exceeds MaxAuthorities");
			}

			let mut inv = BoundedVec::<T::ValidatorId, T::MaxInvulnerables>::default();
			for a in &self.invulnerables {
				if inv.contains(a) {
					panic!("invulnerables contains a duplicate entry");
				}
				inv.try_push(a.clone()).expect("invulnerables exceeds MaxInvulnerables");
			}

			Registered::<T>::mutate(|r| {
				for v in inv.iter() {
					let _ = super::push_unique_vec(r, v.clone());
				}
			});

			Invulnerables::<T>::put(inv.clone());
			Registered::<T>::put(reg.clone());
			LastActive::<T>::put(BoundedVec::truncate_from(reg.into_inner()));
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a candidate (requires session keys staged via `session.setKeys`).
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::nominate())]
		pub fn nominate(origin: T::RuntimeOrigin, who: T::AccountId) -> DispatchResult {
			T::AuthorityMembershipOrigin::ensure_origin(origin)?;

			let vid = T::ValidatorIdOf::convert(who.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;
			ensure!(Self::has_staged_keys(&vid), Error::<T>::SessionKeysNotQueued);
			ensure!(!Registered::<T>::get().contains(&vid), Error::<T>::AlreadyMember);

			PendingAdditions::<T>::try_mutate(|s| -> Result<(), Error<T>> {
				ensure!(!s.contains(&vid), Error::<T>::AlreadyQueued);
				super::push_unique_vec(s, vid.clone()).map_err(|_| Error::<T>::PendingListFull)
			})?;

			Registered::<T>::try_mutate(|r| -> Result<(), Error<T>> {
				super::push_unique_vec(r, vid.clone()).map_err(|_| Error::<T>::CandidatePoolFull)
			})?;

			Self::deposit_event(Event::QueuedAdd(who));
			Ok(())
		}

		/// Remove a candidate (applies next rotation).
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove())]
		pub fn remove(origin: T::RuntimeOrigin, who: T::AccountId) -> DispatchResult {
			T::AuthorityMembershipOrigin::ensure_origin(origin)?;

			let vid = T::ValidatorIdOf::convert(who.clone())
				.ok_or(pallet_session::Error::<T>::NoAssociatedValidatorId)?;

			let reg = Registered::<T>::get();
			ensure!(reg.contains(&vid), Error::<T>::NotMember);

			ensure!(
				!Invulnerables::<T>::get().contains(&vid),
				Error::<T>::CannotRemoveInvulnerable
			);

			let remaining = reg.len().saturating_sub(1) as u32;
			ensure!(remaining >= T::MinAuthorities::get(), Error::<T>::TooLowAuthorityCount);

			PendingRemovals::<T>::try_mutate(|s| -> Result<(), Error<T>> {
				ensure!(!s.contains(&vid), Error::<T>::AlreadyQueued);
				super::push_unique_vec(s, vid.clone()).map_err(|_| Error::<T>::PendingListFull)
			})?;

			Self::deposit_event(Event::QueuedRemoval(who));
			Ok(())
		}

		/// Set invulnerables (bounded) and ensure they are in `Registered`.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_invulnerables(list.len() as u32))]
		pub fn set_invulnerables(
			origin: T::RuntimeOrigin,
			list: Vec<T::ValidatorId>,
		) -> DispatchResult {
			T::AuthorityMembershipOrigin::ensure_origin(origin)?;
			let mut v = BoundedVec::<T::ValidatorId, T::MaxInvulnerables>::default();
			for id in list {
				ensure!(Self::has_staged_keys(&id), Error::<T>::SessionKeysNotQueued);
				v.try_push(id).map_err(|_| Error::<T>::TooManyInvulnerables)?;
			}
			// Keep invuls in the pool so they’re selectable.
			Registered::<T>::try_mutate(|r| -> Result<(), Error<T>> {
				for id in v.iter() {
					super::push_unique_vec(r, id.clone())
						.map_err(|_| Error::<T>::CandidatePoolFull)?;
				}
				Ok(())
			})?;

			Invulnerables::<T>::put(v.clone());
			Self::deposit_event(Event::InvulnerablesSet { count: v.len() as u32 });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	#[inline]
	fn has_staged_keys(vid: &T::ValidatorId) -> bool {
		pallet_session::NextKeys::<T>::contains_key(vid)
			|| pallet_session::QueuedKeys::<T>::get().iter().any(|(v, _)| v == vid)
	}

	/// Union of Registered and current active (so joiners aren’t ignored), unbounded Vec.
	fn union_with_current_unbounded() -> Vec<T::ValidatorId> {
		let mut pool: Vec<T::ValidatorId> = pallet::Registered::<T>::get().into_inner();
		for cur in Session::<T>::validators() {
			if !pool.contains(&cur) {
				pool.push(cur);
			}
		}
		pool
	}

	/// Best-effort fallback to ensure non-empty and >= MinAuthorities when possible.
	fn fallback_nonempty(current_active: &Vec<T::ValidatorId>) -> Vec<T::ValidatorId> {
		// 1) Current active
		if !current_active.is_empty() && (current_active.len() as u32) >= T::MinAuthorities::get() {
			return current_active.clone();
		}
		// 2) Last active
		let last = pallet::LastActive::<T>::get();
		if !last.is_empty() && (last.len() as u32) >= T::MinAuthorities::get() {
			return last.into_inner();
		}
		// 3) Invulnerables
		let inv = pallet::Invulnerables::<T>::get();
		if !inv.is_empty() && (inv.len() as u32) >= T::MinAuthorities::get() {
			return inv.into_inner();
		}
		// 4) Registered (best-effort)
		let reg_vec: Vec<_> = pallet::Registered::<T>::get().into_inner();
		if !reg_vec.is_empty() {
			return reg_vec;
		}
		// 5) Absolute last resort: return current_active (possibly empty) and log.
		log::error!(
			target: "runtime::authorities",
			"Invariant breach: no validators available; returning current_active"
		);
		current_active.clone()
	}

	/// Core rotation: update liveness, apply deltas, safe deterministic selection.
	fn on_new_session_rotate(_next_idx: SessionIndex) -> Vec<T::ValidatorId> {
		let current_active = Session::<T>::validators();

		// 1) Update liveness (only for ACTIVE). No map; just enumerate indices.
		for (i, v) in current_active.iter().enumerate() {
			let online = <T as pallet::Config>::Liveness::is_online(i as u32);
			pallet::MissCount::<T>::mutate(v, |c| {
				*c = if online { 0 } else { c.saturating_add(1) }
			});
		}

		// 2) Apply manual deltas
		let adds = pallet::PendingAdditions::<T>::take();
		let rems = pallet::PendingRemovals::<T>::take();
		pallet::Registered::<T>::mutate(|r| {
			for v in adds.iter() {
				// Attempt to insert; on capacity, log and continue (the candidate is still queued for future).
				if push_unique_vec(r, v.clone()).is_err() {
					log::warn!(
						target: "runtime::authorities",
						"Registered reached capacity while applying addition; dropping {:?}",
						v
					);
				}
			}
			for v in rems.iter() {
				r.retain(|x| x != v);
			}
		});
		pallet::Pallet::<T>::deposit_event(pallet::Event::Planned {
			add: adds.into_inner(),
			remove: rems.into_inner(),
		});

		// 3) Auto-kicks for repeated misses (ignore invuls; protect floor & union viability)
		{
			let thr = T::KickThresholdSessions::get();
			let inv = pallet::Invulnerables::<T>::get();
			let mut reg = pallet::Registered::<T>::get();

			// gather
			let mut to_kick: Vec<T::ValidatorId> = Vec::new();
			for v in reg.iter() {
				if inv.contains(v) {
					continue;
				}
				if pallet::MissCount::<T>::get(v) >= thr {
					to_kick.push(v.clone());
				}
			}

			// attempt with union viability guard (use unbounded vector to avoid BoundedVec capacity bias)
			for v in to_kick {
				let mut tentative = reg.clone();
				tentative.retain(|x| x != &v);

				// union size with current_active must still reach MinAuthorities
				let union_len = {
					let mut tmp: Vec<T::ValidatorId> = tentative.iter().cloned().collect();
					for cur in current_active.iter() {
						if !tmp.contains(cur) {
							tmp.push(cur.clone());
						}
					}
					tmp.len() as u32
				};

				if union_len >= T::MinAuthorities::get() {
					let miss = pallet::MissCount::<T>::get(&v);
					reg.retain(|x| x != &v);
					pallet::Pallet::<T>::deposit_event(pallet::Event::Kicked {
						validator: v.clone(),
						miss_count: miss,
					});
				} else {
					log::warn!(
						target: "runtime::authorities",
						"Kick of {:?} skipped to preserve MinAuthorities",
						v
					);
				}
			}
			pallet::Registered::<T>::put(reg);
		}

		// 4) Build candidate pool and select deterministically
		let pool: Vec<T::ValidatorId> = pallet::Pallet::<T>::union_with_current_unbounded();

		let min = T::MinAuthorities::get() as usize;
		let tgt = T::TargetActive::get() as usize;

		if pool.len() < min {
			return pallet::Pallet::<T>::fallback_nonempty(&current_active);
		}

		// Always include invulnerables first.
		let inv = pallet::Invulnerables::<T>::get();
		let mut active: Vec<T::ValidatorId> = Vec::new();
		for v in inv.iter() {
			if pallet::Pallet::<T>::has_staged_keys(v) && !active.contains(v) {
				active.push(v.clone());
			} else if !pallet::Pallet::<T>::has_staged_keys(v) {
				log::warn!(target: "runtime::authorities", "Invulnerable has no session keys; skipping in active set");
			}
		}
		if active.len() >= tgt {
			active.truncate(tgt);
			return active;
		}

		// ---- simple, fast ranking: precompute once, then sort by (miss asc, pos asc) ----
		let need = tgt.saturating_sub(active.len());

		// Build scored list (id, miss, pos). `pos` is a deterministic tie-breaker (order in pool).
		let mut scored: Vec<(T::ValidatorId, u32, usize)> = pool
			.into_iter()
			.enumerate()
			.filter(|(_, v)| !inv.contains(v)) // invulnerables already included
			.map(|(pos, id)| {
				let miss = pallet::MissCount::<T>::get(&id); // 1 storage read per candidate
				(id, miss, pos)
			})
			.collect();

		// Full sort is fine (pools are typically small). Constants are tiny.
		scored.sort_unstable_by_key(|(_, miss, pos)| (*miss, *pos));

		// Fill remaining slots
		for (id, _, _) in scored.into_iter().take(need) {
			if !active.contains(&id) {
				active.push(id);
			}
		}

		// Floor/fallback guard (shouldn't trigger)
		if active.len() < (T::MinAuthorities::get() as usize) || active.is_empty() {
			return pallet::Pallet::<T>::fallback_nonempty(&current_active);
		}

		active
	}
}

#[inline]
fn push_unique_vec<Ty, S>(v: &mut BoundedVec<Ty, S>, x: Ty) -> Result<(), ()>
where
	Ty: PartialEq + Clone,
	S: Get<u32>,
{
	if v.contains(&x) {
		Ok(())
	} else {
		v.try_push(x).map_err(|_| ())
	}
}

impl<T: Config> pallet_session::SessionManager<T::ValidatorId> for Pallet<T> {
	fn new_session(next: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		let next_set = <pallet::Pallet<T>>::on_new_session_rotate(next);
		let cur = pallet_session::Pallet::<T>::validators();
		if next_set == cur {
			None
		} else {
			LastActive::<T>::put(BoundedVec::truncate_from(next_set.clone()));
			pallet::Pallet::<T>::deposit_event(pallet::Event::Enacted { active: next_set.clone() });
			Some(next_set)
		}
	}

	/// Hand the initial validator set to Session at genesis.
	fn new_session_genesis(_index: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		Some(pallet::Registered::<T>::get().into_inner())
	}

	fn end_session(_: SessionIndex) {}
	fn start_session(_: SessionIndex) {}
}

fn map_full_id<T: pallet::Config>(
	vid: T::ValidatorId,
) -> Option<(T::ValidatorId, T::FullIdentification)> {
	T::FullIdentificationOf::convert(vid.clone()).map(|fi| (vid, fi))
}

impl<T: Config> pallet_session::historical::SessionManager<T::ValidatorId, T::FullIdentification>
	for Pallet<T>
{
	fn new_session(idx: SessionIndex) -> Option<Vec<(T::ValidatorId, T::FullIdentification)>> {
		<Self as pallet_session::SessionManager<_>>::new_session(idx)
			.map(|ids| ids.into_iter().filter_map(map_full_id::<T>).collect())
	}

	/// Provide the historical snapshot at genesis as well.
	fn new_session_genesis(
		_idx: SessionIndex,
	) -> Option<Vec<(T::ValidatorId, T::FullIdentification)>> {
		let initial: Vec<T::ValidatorId> = pallet::Registered::<T>::get().into_inner();
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

pub trait LivenessProvider {
	fn is_online(idx: u32) -> bool;
}

pub struct ImOnlineAdapter<T>(core::marker::PhantomData<T>);
impl<T: pallet_im_online::Config> LivenessProvider for ImOnlineAdapter<T> {
	#[inline]
	fn is_online(idx: u32) -> bool {
		pallet_im_online::Pallet::<T>::is_online(idx)
	}
}
