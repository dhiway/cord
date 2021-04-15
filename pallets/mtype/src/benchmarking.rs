// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Mtype


#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_std::{vec, vec::Vec, boxed::Box};

const SEED: u32 = 0;

benchmarks! {
	add {
		let caller = account("caller", 0, SEED);
		let hash = <T::Hash as Default>::default();

	}: _(RawOrigin::Signed(caller), hash)
	verify {
		MTYPEs::<T>::contains_key(hash)
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
			assert_ok!(test_benchmark_add::<Test>());
		});
	}
}