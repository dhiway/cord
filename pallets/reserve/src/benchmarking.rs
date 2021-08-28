// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Reserve

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
