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
use frame_benchmarking::v1::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_std::vec::Vec;
const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
	where_clause { where <T::RuntimeOrigin as frame_support::traits::OriginTrait>::PalletsOrigin: Clone }
	add {
		let a in 1..T::MaxAuthorityProposals::get();

		let mut authorities = Vec::new();

		for i in 0..a {
			let authority = account("authority", i, SEED);
			authorities.push(authority);
		}
	}: _(RawOrigin::Root, authorities.clone())
	verify {
		assert_last_event::<T>(Event::AuthorsAdded { authors_added: authorities }.into());
	}
	remove {
		let a in 1..T::MaxAuthorityProposals::get();

		let mut authorities = Vec::new();

		for i in 0..a {
			let authority = account("authority", i, SEED);
			authorities.push(authority);
		}

		let auth_origin = RawOrigin::Root;
		Pallet::<T>::add(auth_origin.clone().into(), authorities.clone()).expect("Should add Authorities");
	}: _(auth_origin, authorities.clone())
	verify {
		assert_last_event::<T>(Event::AuthorsRemoved { authors_removed: authorities }.into());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);
