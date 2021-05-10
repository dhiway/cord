// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Digest

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::Module as DigestModule;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
// use sp_core::{ed25519, ed25519::Signature, Pair, H256};
use sp_runtime::traits::Hash;
use sp_std::{boxed::Box, vec, vec::Vec};

// const MAX_DEPTH: u32 = 10;

benchmarks! {

	anchor {

		let caller: T::AccountId = whitelisted_caller();
		let acc: T::AccountId = whitelisted_caller();

		let content_hash: T::Hash = T::Hashing::hash(b"someContentHash");
		let hash = <T::Hash as Default>::default();
		let mtype_hash: T::Hash = T::Hash::default();

		pallet_mtype::Module::<T>::anchor(RawOrigin::Signed(caller.clone()).into(), mtype_hash)?;
		pallet_mark::Module::<T>::anchor(RawOrigin::Signed(acc.clone()).into(), content_hash,mtype_hash,None)?;

		let digest_hash: T::Hash = T::Hashing::hash(b"thisIsDigestHash");

	}: _(RawOrigin::Signed(caller.clone()),digest_hash,content_hash)
	verify {
		 assert!(Digests::<T>::contains_key(digest_hash));

	}

	revoke {

		// let d in 1 .. MAX_DEPTH;

		let content_hash: T::Hash = T::Hashing::hash(b"claim");
		let digest_hash: T::Hash = T::Hashing::hash(b"digest");
		let mtype_hash: T::Hash = T::Hash::default();

		let caller: T::AccountId = whitelisted_caller();
		let acc: T::AccountId = whitelisted_caller();

		pallet_mtype::Module::<T>::anchor(RawOrigin::Signed(caller.clone()).into(), mtype_hash)?;
		pallet_mark::Module::<T>::anchor(RawOrigin::Signed(acc.clone()).into(), content_hash,mtype_hash,None)?;
		DigestModule::<T>::anchor(RawOrigin::Signed(caller.clone()).into(), digest_hash,content_hash)?;
	}: _(RawOrigin::Signed(caller.clone()),digest_hash)
	verify {
		assert!(Digests::<T>::contains_key(digest_hash));

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
			assert_ok!(test_benchmark_revoke::<Test>());
		});
	}
}
