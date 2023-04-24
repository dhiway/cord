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
use codec::Encode;
use frame_benchmarking::v1::{account, benchmarks, impl_benchmark_test_suite};
use sp_std::{
	convert::{TryFrom, TryInto},
	vec::Vec,
};
use frame_system::RawOrigin;
use sp_core::Get;
const SEED: u32 = 0;

benchmarks! {
	
	add {
		let authors_count: u32 = T::MaxAuthorityProposals::get();
		let mut authors = Vec::new();

		for i in 0..authors_count {
			let user: T::AccountId = account("user", i, SEED);
			authors.push(user.clone());
            ExtrinsicAuthors::<T>::insert(user.clone(), ());
		}
        let caller: T::AccountId = account("caller", 0, SEED);
	}: _(RawOrigin::Root, authors.clone())
	verify {
		for author in authors {
			assert!(ExtrinsicAuthors::<T>::contains_key(author));
		}
	}
    remove {
		let authors_count: u32 = T::MaxAuthorityProposals::get();
		let mut authors = Vec::new();

		for i in 0..authors_count {
			let user: T::AccountId = account("user", i, SEED);
			authors.push(user.clone());
			ExtrinsicAuthors::<T>::insert(user.clone(), ());
		}

		let caller: T::AccountId = account("caller", 0, SEED);
	}: _(RawOrigin::Root, authors.clone())
	verify {
		for author in authors {
			assert!(!ExtrinsicAuthors::<T>::contains_key(author));
		}
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::new_test_ext(),
	crate::mock::Test,
);
