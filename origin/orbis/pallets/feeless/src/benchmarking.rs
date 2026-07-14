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
use crate::Pallet as FeelessPallet;
use frame_benchmarking::{v2::*, BenchmarkError};
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn add_feeless_account() -> Result<(), BenchmarkError> {
		let account: T::AccountId = whitelisted_caller();

		#[extrinsic_call]
		_(RawOrigin::Root, account.clone());

		assert!(FeelessPallet::<T>::is_feeless_account(&account));
		Ok(())
	}

	#[benchmark]
	fn remove_feeless_account() -> Result<(), BenchmarkError> {
		let account: T::AccountId = whitelisted_caller();
		FeelessPallet::<T>::add_feeless_account(RawOrigin::Root.into(), account.clone())?;

		#[extrinsic_call]
		_(RawOrigin::Root, account.clone());

		assert!(!FeelessPallet::<T>::is_feeless_account(&account));
		Ok(())
	}

	impl_benchmark_test_suite!(FeelessPallet, crate::mock::new_test_ext(), crate::mock::Test);
}
