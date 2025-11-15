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

use crate::Config;
use codec::Encode;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_runtime::traits::Convert;

use crate::pallet::{
	self as pallet, Config as PalletConfig, LastActive, Pallet, PendingAdditions, PendingRemovals,
	Registered,
};

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
		// in benches we assume the mock runtime maps AccountId -> ValidatorId
		panic!("cannot convert AccountId -> ValidatorId in benchmark setup");
	}
}

fn vid_at<T: pallet::Config>(prefix: &'static str, i: u32) -> T::ValidatorId
where
	T::ValidatorId: Clone + PartialEq + Encode,
{
	frame_benchmarking::account::<T::ValidatorId>(prefix, i, 0)
}

fn fill_registered<T: pallet::Config>(n: u32, also_stage_keys: bool)
where
	T::ValidatorId: Clone + PartialEq + Encode,
	T::Keys: Default,
{
	Registered::<T>::mutate(|r| {
		r.clear();
		for i in 0..n {
			let vid = vid_at::<T>("reg", i);
			if also_stage_keys {
				if !pallet_session::NextKeys::<T>::contains_key(&vid) {
					pallet_session::NextKeys::<T>::insert(vid.clone(), T::Keys::default());
				}
			}
			if !r.contains(&vid) {
				r.push(vid);
			}
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
			if !p.contains(&vid) {
				p.push(vid);
			}
		}
	});
}

/// Fill PendingRemovals with `n` ids drawn from `Registered` (when possible).
fn fill_pending_removals<T: pallet::Config>(n: u32)
where
	T::ValidatorId: Clone + PartialEq + Encode,
{
	let pool = Registered::<T>::get();
	PendingRemovals::<T>::mutate(|p| {
		p.clear();
		for id in pool.into_iter().take(n as usize) {
			if !p.contains(&id) {
				p.push(id);
			}
		}
	});
}

/// Fresh test externalities for benchmarks (use your mock’s helper).
fn bench_ext() -> sp_io::TestExternalities {
	crate::mock::new_test_ext(8)
}

#[benchmarks(
    where
        T: PalletConfig + pallet_session::Config,
        T::ValidatorId: Clone + PartialEq + Encode,
        T::AccountId: Clone + PartialEq + Encode,
        T::Keys: Default,
)]
mod benches {
	use super::*;
	use frame_benchmarking::v2::account;

	/// `nominate`: heavy case — prefill Registered and PendingAdditions, then add a new account
	/// with staged keys.
	#[benchmark]
	fn nominate(n: Linear<100, 1000>) -> Result<(), BenchmarkError> {
		let n: u32 = n;

		// Fill registered & pending with n elements (and stage keys for registered so selection can
		// work)
		fill_registered::<T>(n, /* also_stage_keys= */ true);
		fill_pending_additions::<T>(n);

		// Build a fresh AccountId not in the pools
		let who_acc = account::<T::AccountId>("who", n.saturating_add(7777), 0);
		stage_keys_for_account::<T>(&who_acc);

		// `nominate` takes AccountId
		#[block]
		{
			Pallet::<T>::nominate(RawOrigin::Root.into(), who_acc.clone())?;
		}

		// Convert to ValidatorId to assert storages
		let who_vid = <<T as pallet_session::Config>::ValidatorIdOf as Convert<
			T::AccountId,
			Option<T::ValidatorId>,
		>>::convert(who_acc)
		.expect("account -> validator id");

		assert!(Registered::<T>::get().contains(&who_vid));
		assert!(PendingAdditions::<T>::get().contains(&who_vid));
		Ok(())
	}

	/// `remove`: heavy case — register n+1 members, queue many for removal, then remove target.
	#[benchmark]
	fn remove(n: Linear<100, 1000>) -> Result<(), BenchmarkError> {
		let n: u32 = n;

		// Fill Registered with n+1 entries and stage keys for all (not strictly required for remove
		// path)
		fill_registered::<T>(n.saturating_add(1), /* also_stage_keys= */ true);

		// Pick a target account that maps to a known ValidatorId
		let who_acc = account::<T::AccountId>("rm", 9_001, 0);
		let who_vid = <<T as pallet_session::Config>::ValidatorIdOf as Convert<
			T::AccountId,
			Option<T::ValidatorId>,
		>>::convert(who_acc.clone())
		.expect("account -> validator id");

		// Ensure it exists in Registered (in case the random acc isn't in the n+1 set)
		Registered::<T>::mutate(|r| {
			if !r.contains(&who_vid) {
				r.push(who_vid.clone())
			}
		});

		// Pre-fill PendingRemovals heavily but NOT with the target
		fill_pending_removals::<T>(n);
		PendingRemovals::<T>::mutate(|p| p.retain(|v| v != &who_vid));

		#[block]
		{
			Pallet::<T>::remove(RawOrigin::Root.into(), who_acc.clone())?;
		}

		assert!(PendingRemovals::<T>::get().contains(&who_vid));
		Ok(())
	}

	/// `bench_rotate`: fill registered, queue adds/removes, rotate once.
	#[benchmark]
	fn bench_rotate(n: Linear<100, 1000>) -> Result<(), BenchmarkError> {
		let n: u32 = n;

		// Seed registered with n and stage keys so selection won't be empty
		fill_registered::<T>(n, /* also_stage_keys= */ true);

		// Adds: n/2 fresh ids; stage keys for them so they can be selected on next sessions
		let adds = n / 2;
		PendingAdditions::<T>::mutate(|p| {
			p.clear();
			for i in 0..adds {
				let vid = vid_at::<T>("add", i);
				if !pallet_session::NextKeys::<T>::contains_key(&vid) {
					pallet_session::NextKeys::<T>::insert(vid.clone(), T::Keys::default());
				}
				p.push(vid);
			}
		});

		// Removals: n/2 from currently registered (if available)
		fill_pending_removals::<T>(n / 2);

		// Call the internal rotate hook exposed for benches
		#[block]
		{
			Pallet::<T>::bench_rotate();
		}

		// Postconditions: active snapshot exists; queues were applied (best-effort check)
		assert!(!LastActive::<T>::get().is_empty());

		// Either additions were applied or removals were applied (can't assert exact diff without
		// reading previous set) Keep a light check: at least one queue should be empty now
		// (consumed).
		let pa = PendingAdditions::<T>::get();
		let pr = PendingRemovals::<T>::get();
		assert!(pa.is_empty() || pr.is_empty(), "queues should be applied on rotation");

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, bench_ext(), crate::mock::Test);
}
