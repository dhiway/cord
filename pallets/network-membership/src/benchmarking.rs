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

use super::*;
use crate::Pallet;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
	where_clause { where <T::RuntimeOrigin as frame_support::traits::OriginTrait>::PalletsOrigin: Clone }
	nominate {

	   let authority: T::AccountId = account("authority", 0, SEED);

	}: _(RawOrigin::Root, authority.clone(), true)
	verify {
		assert_last_event::<T>(Event::MembershipAcquired { member:  authority }.into());
	}

	renew {
	   let authority: T::AccountId = account("authority", 1, SEED);

		let auth_origin = RawOrigin::Root;
		Pallet::<T>::nominate(auth_origin.clone().into(), authority.clone(), true ).expect("Should add authority");
	}: _(auth_origin, authority.clone())
	verify {
		assert_last_event::<T>(Event::MembershipRenewalRequested { member: authority }.into());
	}

	revoke {
	   let authority: T::AccountId = account("authority", 1, SEED);

		let auth_origin = RawOrigin::Root;
		Pallet::<T>::nominate(auth_origin.clone().into(), authority.clone(), true ).expect("Should add authority");
	}: _(auth_origin, authority.clone())
	verify {
		assert_last_event::<T>(Event::MembershipRevoked { member: authority }.into());
	}

	impl_benchmark_test_suite! (
		Pallet,
		crate::mock::new_test_ext(),
		crate::mock::Test
	)
}
