// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Mtype

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_std::{boxed::Box, vec, vec::Vec};

const SEED: u32 = 0;

benchmarks! {

	anchor {
		let caller :T::AccountId = account("sender", 0, SEED);
		let sign_key = <T::PublicSigningKey as Default>::default();
		let box_key = <T::PublicBoxKey as Default>::default();
		// let doc_ref = Option<Vec<u8>>
		let doc_ref = Some(b"http://dway.io/submit".to_vec());

	}: _(RawOrigin::Signed(caller.clone()), sign_key,box_key,doc_ref)
	verify {
		DIDs::<T>::contains_key(caller)
	}

	remove {
		let caller :T::AccountId = account("sender", 0, SEED);
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		assert_eq!(<DIDs<T>>::contains_key(caller), false);
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
			assert_ok!(test_benchmark_anchor::<Test>());
			assert_ok!(test_benchmark_remove::<Test>());
		});
	}
}
