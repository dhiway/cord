// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Mtype

use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

use crate::*;

const SEED: u32 = 0;

benchmarks! {
	add {
		let caller = account("caller", 0, SEED);
		let hash = <T::Hash as Default>::default();

	}: _(RawOrigin::Signed(caller), hash)
	verify {
		Mtypes::<T>::contains_key(hash)
	}
}

impl_benchmark_test_suite! {
	Pallet,
	crate::mock::ExtBuilder::default().build_with_keystore(None),
	crate::mock::Test
}
