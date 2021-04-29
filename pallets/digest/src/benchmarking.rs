// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Digest


#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_std::{vec, vec::Vec, boxed::Box};

const SEED: u32 = 0;

benchmarks! {
	
	anchor {
		let caller :T::AccountId = account("sender", 0, SEED);
		let digest_hash = <T::Hash as Default>::default();
		let content_hash = <T::Hash as Default>::default();
        let mark = <pallet_mark::Marks<T>>::get(content_hash);
        assert_ok!(MType::anchor(Origin::signed(caller.clone()), content_hash));

	}: _(RawOrigin::Signed(caller.clone()),digest_hash,content_hash)
	verify {
		// DIDs::<T>::contains_key(caller)
	}

	// remove {
	// 	let caller :T::AccountId = account("sender", 0, SEED);
	// }: _(RawOrigin::Signed(caller.clone()))
	// verify {
	// 	assert_eq!(<DIDs<T>>::contains_key(caller), false);
	// }
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