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

#[cfg(feature = "runtime-benchmarks")]
use super::{Pallet as Membership, *};
use frame_benchmarking::v1::{account, benchmarks_instance_pallet, whitelist, BenchmarkError};
use frame_support::{assert_ok, traits::EnsureOrigin};
use frame_system::RawOrigin;

const SEED: u32 = 0;

fn set_members<T: Config<I>, I: 'static>(members: Vec<T::AccountId>, prime: Option<usize>) {
	let reset_origin = T::ResetOrigin::try_successful_origin()
		.expect("ResetOrigin has no successful origin required for the benchmark");
	let prime_origin = T::PrimeOrigin::try_successful_origin()
		.expect("PrimeOrigin has no successful origin required for the benchmark");

	assert_ok!(<Membership<T, I>>::reset_members(reset_origin, members.clone()));
	if let Some(prime) = prime.map(|i| members[i].clone()) {
		let prime_lookup = T::Lookup::unlookup(prime);
		assert_ok!(<Membership<T, I>>::set_prime(prime_origin, prime_lookup));
	} else {
		assert_ok!(<Membership<T, I>>::clear_prime(prime_origin));
	}
}

benchmarks_instance_pallet! {
	add_member {
		let m in 1 .. (T::MaxMembers::get() - 1);

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members, None);
		let new_member = account::<T::AccountId>("add", m, SEED);
		let new_member_lookup = T::Lookup::unlookup(new_member.clone());
	}: {
		assert_ok!(<Membership<T, I>>::add_member(
			T::AddOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?,
			new_member_lookup,
		));
	} verify {
		assert!(<Members<T, I>>::get().contains(&new_member));
		#[cfg(test)] crate::mock::clean();
	}

	// the case of no prime or the prime being removed is surely cheaper than the case of
	// reporting a new prime via `MembershipChanged`.
	remove_member {
		let m in 2 .. T::MaxMembers::get();

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members.clone(), Some(members.len() - 1));

		let to_remove = members.first().cloned().unwrap();
		let to_remove_lookup = T::Lookup::unlookup(to_remove.clone());
	}: {
		assert_ok!(<Membership<T, I>>::remove_member(
			T::RemoveOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?,
			to_remove_lookup,
		));
	} verify {
		assert!(!<Members<T, I>>::get().contains(&to_remove));
		// prime is rejigged
		assert!(<Prime<T, I>>::get().is_some() && T::MembershipChanged::get_prime().is_some());
		#[cfg(test)] crate::mock::clean();
	}

	// we remove a non-prime to make sure it needs to be set again.
	swap_member {
		let m in 2 .. T::MaxMembers::get();

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members.clone(), Some(members.len() - 1));
		let add = account::<T::AccountId>("member", m, SEED);
		let add_lookup = T::Lookup::unlookup(add.clone());
		let remove = members.first().cloned().unwrap();
		let remove_lookup = T::Lookup::unlookup(remove.clone());
	}: {
		assert_ok!(<Membership<T, I>>::swap_member(
			T::SwapOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?,
			remove_lookup,
			add_lookup,
		));
	} verify {
		assert!(!<Members<T, I>>::get().contains(&remove));
		assert!(<Members<T, I>>::get().contains(&add));
		// prime is rejigged
		assert!(<Prime<T, I>>::get().is_some() && T::MembershipChanged::get_prime().is_some());
		#[cfg(test)] crate::mock::clean();
	}

	// er keep the prime common between incoming and outgoing to make sure it is rejigged.
	reset_members {
		let m in 1 .. T::MaxMembers::get();

		let members = (1..m+1).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members.clone(), Some(members.len() - 1));
		let mut new_members = (m..2*m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
	}: {
		assert_ok!(<Membership<T, I>>::reset_members(
			T::ResetOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?,
			new_members.clone(),
		));
	} verify {
		new_members.sort();
		assert_eq!(<Members<T, I>>::get(), new_members);
		// prime is rejigged
		assert!(<Prime<T, I>>::get().is_some() && T::MembershipChanged::get_prime().is_some());
		#[cfg(test)] crate::mock::clean();
	}

	change_key {
		let m in 1 .. T::MaxMembers::get();

		// worse case would be to change the prime
		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		let prime = members.last().cloned().unwrap();
		set_members::<T, I>(members.clone(), Some(members.len() - 1));

		let add = account::<T::AccountId>("member", m, SEED);
		let add_lookup = T::Lookup::unlookup(add.clone());
		whitelist!(prime);
	}: {
		assert_ok!(<Membership<T, I>>::change_key(RawOrigin::Signed(prime.clone()).into(), add_lookup));
	} verify {
		assert!(!<Members<T, I>>::get().contains(&prime));
		assert!(<Members<T, I>>::get().contains(&add));
		// prime is rejigged
		assert_eq!(<Prime<T, I>>::get().unwrap(), add);
		#[cfg(test)] crate::mock::clean();
	}

	set_prime {
		let m in 1 .. T::MaxMembers::get();
		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		let prime = members.last().cloned().unwrap();
		let prime_lookup = T::Lookup::unlookup(prime);
		set_members::<T, I>(members, None);
	}: {
		assert_ok!(<Membership<T, I>>::set_prime(
			T::PrimeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?,
			prime_lookup,
		));
	} verify {
		assert!(<Prime<T, I>>::get().is_some());
		assert!(<T::MembershipChanged>::get_prime().is_some());
		#[cfg(test)] crate::mock::clean();
	}

	clear_prime {
		let members = (0..T::MaxMembers::get()).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		let prime = members.last().cloned().unwrap();
		set_members::<T, I>(members, None);
	}: {
		assert_ok!(<Membership<T, I>>::clear_prime(
			T::PrimeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?,
		));
	} verify {
		assert!(<Prime<T, I>>::get().is_none());
		assert!(<T::MembershipChanged>::get_prime().is_none());
		#[cfg(test)] crate::mock::clean();
	}

	impl_benchmark_test_suite!(Membership, crate::mock::new_bench_ext(), crate::mock::Test);
}
