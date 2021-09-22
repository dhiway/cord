// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Benchmarking of Dhi-Treasury

#![cfg(feature = "runtime-benchmarks")]

use super::*;

pub use cord_primitives::{AccountId, Balance};
use frame_benchmarking::{account, benchmarks_instance, whitelisted_caller};
use frame_system::RawOrigin;
use sp_std::{boxed::Box, vec, vec::Vec};

const SEED: u32 = 0;

benchmarks_instance! {

	transfer {
		let origin = T::ExternalOrigin::successful_origin();
		let receiver = account("receiver", 0, SEED);
	}: _(RawOrigin::Root, receiver, 1u32.into())
	verify{

	}

	receive {
		let caller :T::AccountId = account("sender", 0, SEED);
		let acc: T::AccountId = whitelisted_caller();
		let balance = T::Currency::free_balance(&caller);

	}: _(RawOrigin::Signed(acc.clone()),balance)
	verify {
	}

}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::{new_test_ext, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer::<Test>());
			assert_ok!(test_benchmark_receive::<Test>());
		});
	}
}
