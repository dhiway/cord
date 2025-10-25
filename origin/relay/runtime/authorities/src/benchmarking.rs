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

#![cfg(feature = "runtime-benchmarks")]

extern crate alloc;

use alloc::vec::Vec;
use codec::Encode;
use frame_benchmarking::v2::*;
use frame_support::traits::Get;
use frame_system::RawOrigin;

use crate::Config;

use crate::pallet::{
	self as pallet, Config as PalletConfig, Invulnerables, LastActive, MissCount, Pallet,
	PendingAdditions, PendingRemovals, Registered,
};
use sp_runtime::traits::Convert;

/// Stage session keys for an *account* by converting it into a ValidatorId.
/// Requires `T::Keys: Default` for a dummy key payload.
fn stage_keys_for_account<T: pallet::Config>(acc: &T::AccountId)
where
	T::Keys: Default,
{
	if let Some(vid) = <T as pallet_session::Config>::ValidatorIdOf::convert(acc.clone()) {
		if !pallet_session::NextKeys::<T>::contains_key(&vid) {
			pallet_session::NextKeys::<T>::insert(vid, T::Keys::default());
		}
	} else {
		panic!("cannot convert AccountId -> ValidatorId in benchmark setup");
	}
}

fn vid_at<T: pallet::Config>(prefix: &'static str, i: u32) -> T::ValidatorId
where
	T::ValidatorId: Clone + PartialEq + Encode,
{
	frame_benchmarking::account::<T::ValidatorId>(prefix, i, 0)
}

fn fill_registered<T: pallet::Config>(n: u32)
where
	T::ValidatorId: Clone + PartialEq + Encode,
{
	Registered::<T>::mutate(|r| {
		r.clear();
		for i in 0..n {
			let vid = vid_at::<T>("reg", i);
			let _ = crate::push_unique_vec(r, vid);
		}
	});
}

/// Fill PendingAdditions with `n` unique ids not in `Registered`.
fn fill_pending_additions<T: pallet::Config>(n: u32)
where
	T::ValidatorId: Clone + PartialEq + Encode,
{
	PendingAdditions::<T>::mutate(|p| {
		p.clear();
		for i in 0..n {
			let vid = vid_at::<T>("pend_add", i);
			let _ = crate::push_unique_vec(p, vid);
		}
	});
}

/// Fill PendingRemovals with `n` ids drawn from `Registered` (when possible).
fn fill_pending_removals<T: pallet::Config>(n: u32)
where
	T::ValidatorId: Clone + PartialEq + Encode,
{
	let pool = Registered::<T>::get().into_inner();
	PendingRemovals::<T>::mutate(|p| {
		p.clear();
		for id in pool.into_iter().take(n as usize) {
			let _ = crate::push_unique_vec(p, id);
		}
	});
}

fn fill_invulnerables<T: pallet::Config>(k: u32)
where
	T::ValidatorId: Clone + PartialEq + Encode,
{
	Invulnerables::<T>::mutate(|v| {
		v.clear();
		for i in 0..k {
			let id = vid_at::<T>("inv", i);
			let _ = v.try_push(id);
		}
	});
}

fn set_misscounts<T: pallet::Config>(k: u32, thr: u32)
where
	T::ValidatorId: Clone + PartialEq + Encode,
{
	let pool = Registered::<T>::get().into_inner();
	for (idx, vid) in pool.into_iter().enumerate() {
		if (idx as u32) >= k {
			break;
		}
		MissCount::<T>::insert(vid, thr);
	}
}

/// Stage session keys directly for a ValidatorId (used by set_invulnerables bench).
fn stage_keys_for_vid<T: pallet::Config>(vid: &T::ValidatorId)
where
	T::Keys: Default,
{
	if !pallet_session::NextKeys::<T>::contains_key(vid) {
		pallet_session::NextKeys::<T>::insert(vid.clone(), T::Keys::default());
	}
}

/// Fresh test externalities for benchmarks.
fn bench_ext() -> sp_io::TestExternalities {
	crate::mock::new_test_ext(8)
}

#[benchmarks(
    where
        T: PalletConfig + pallet_session::Config,
        T::ValidatorId: Clone + PartialEq + Encode,
        T::Keys: Default,
)]
mod benches {
	use super::*;
	use frame_benchmarking::v2::account;

	// Worst-case: Registered and PendingAdditions near full; candidate not present.
	// NOTE: nominate() takes AccountId; convert to ValidatorId only for storage checks.
	#[benchmark]
	fn nominate() -> Result<(), BenchmarkError> {
		let m = T::MaxAuthorities::get();
		fill_registered::<T>(m.saturating_sub(1));
		fill_pending_additions::<T>(m.saturating_sub(1));

		// Build an AccountId (not a ValidatorId!)
		let who_acc = account::<T::AccountId>("who", m.saturating_add(77), 0);
		stage_keys_for_account::<T>(&who_acc);

		// Fully-qualify Convert to avoid inference issues and local type aliases.
		let who_vid = <<T as pallet_session::Config>::ValidatorIdOf as Convert<
			<T as frame_system::Config>::AccountId,
			Option<<T as pallet_session::Config>::ValidatorId>,
		>>::convert(who_acc.clone())
		.expect("account -> validator id");

		#[block]
		{
			Pallet::<T>::nominate(RawOrigin::Root.into(), who_acc.clone())?;
		}

		assert!(Registered::<T>::get().contains(&who_vid));
		assert!(PendingAdditions::<T>::get().contains(&who_vid));
		Ok(())
	}

	// Worst-case: Registered full; PendingRemovals near full; remove an id present in Registered.
	// remove() takes AccountId; construct an AccountId that maps to a known Registered vid.
	#[benchmark]
	fn remove() -> Result<(), BenchmarkError> {
		let m = T::MaxAuthorities::get();

		// Build the target *account*, convert to ValidatorId for storage assertions.
		let who_acc = account::<T::AccountId>("rm", 9_001, 0);
		let who_vid = <<T as pallet_session::Config>::ValidatorIdOf as Convert<
			<T as frame_system::Config>::AccountId,
			Option<<T as pallet_session::Config>::ValidatorId>,
		>>::convert(who_acc.clone())
		.expect("account -> validator id");

		// Fill Registered to m-1, then insert the target vid as the m-th element.
		fill_registered::<T>(m.saturating_sub(1));
		Registered::<T>::mutate(|r| {
			// Guaranteed to fit: r.len() == m-1
			let _ = crate::push_unique_vec(r, who_vid.clone());
		});
		debug_assert_eq!(Registered::<T>::get().len() as u32, m);

		// Prefill PendingRemovals heavily, but do NOT include the target
		// to exercise the O(n) uniqueness scan before pushing it.
		let prefill = m.saturating_sub(1);
		fill_pending_removals::<T>(prefill);

		#[block]
		{
			Pallet::<T>::remove(RawOrigin::Root.into(), who_acc.clone())?;
		}

		assert!(
			PendingRemovals::<T>::get().contains(&who_vid),
			"target vid should be queued for removal"
		);
		Ok(())
	}

	// Worst-case: set invulnerables to the declared maximum, verify stored and in pool.
	#[benchmark]
	fn set_invulnerables() -> Result<(), BenchmarkError> {
		let k = T::MaxInvulnerables::get();
		let mut list: Vec<T::ValidatorId> = Vec::new();
		for i in 0..k {
			let id = account::<T::ValidatorId>("inv", i, 0);
			stage_keys_for_vid::<T>(&id);
			list.push(id);
		}

		#[block]
		{
			Pallet::<T>::set_invulnerables(RawOrigin::Root.into(), list.clone())?;
		}

		let inv = Invulnerables::<T>::get();
		assert_eq!(inv.len() as u32, k, "all invulnerables should be stored");

		for id in inv.into_inner() {
			assert!(Registered::<T>::get().contains(&id));
		}
		Ok(())
	}

	// Exercise liveness update + manual deltas + auto-kicks.
	#[benchmark]
	fn bench_rotate() -> Result<(), BenchmarkError> {
		let m = T::MaxAuthorities::get();
		let floor = T::MinAuthorities::get();
		let thr = T::KickThresholdSessions::get();

		fill_registered::<T>(m);

		// Prepare sizable manual deltas to exercise step (2)
		let adds = m / 2;
		let rems = m / 2;
		fill_pending_additions::<T>(adds);
		fill_pending_removals::<T>(rems);

		// Many candidates at/over threshold for kick path
		let to_mark = m.saturating_sub(floor);
		set_misscounts::<T>(to_mark, thr);

		// Keep at least the floor as invulnerables
		let keep = core::cmp::min(T::MaxInvulnerables::get(), floor);
		fill_invulnerables::<T>(keep);

		#[block]
		{
			Pallet::<T>::bench_rotate(); // internal call
		}

		let pool = Registered::<T>::get();
		assert!((pool.len() as u32) >= T::MinAuthorities::get());
		assert!(!LastActive::<T>::get().is_empty());
		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, bench_ext(), crate::mock::Test);
}
