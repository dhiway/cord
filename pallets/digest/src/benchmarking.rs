// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Digest


#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_std::{vec, vec::Vec, boxed::Box};
use sp_runtime::traits::Hash;



const SEED: u32 = 0;

benchmarks! {
	
	anchor {
		let caller :T::AccountId = account("sender", 0, SEED);
		let acc :T::AccountId = account("accr", 1, SEED);

		let digest_hash: T::Hash = T::Hash::default();
		let content_hash: T::Hash = T::Hash::default();
		let hash = <T::Hash as Default>::default();
		let mtype_hash: T::Hash = T::Hash::default();


		pallet_mtype::Module::<T>::anchor(RawOrigin::Signed(caller.clone()).into(), hash)?;
		pallet_mark::Module::<T>::anchor(RawOrigin::Signed(acc.clone()).into(), content_hash,mtype_hash,None)?;

		<Digests<T>>::insert(digest_hash, Digest {content_hash, marker: caller.clone(), revoked: false});

		// let mark = <pallet_mark::Marks<T>>::get(content_hash).ok_or(pallet_mark::Error::<T>::MarkNotFound)?;

	}: _(RawOrigin::Signed(caller.clone()),digest_hash,content_hash)
	verify {
		 Digests::<T>::contains_key(digest_hash)
	}

	// revoke {
	// 	let caller :T::AccountId = account("sender", 0, SEED);
	// 	let digest_hash = <T::Hash as Default>::default();
	// 	let max_depth: u64 = 10;

	// }: _(RawOrigin::Signed(caller.clone()))
	// verify {
	// 	Digests::<T>::contains_key(digest_hash)
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
