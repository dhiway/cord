// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Digest


#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks,whitelisted_caller};
use frame_system::RawOrigin;
use sp_std::{vec, vec::Vec, boxed::Box};
use sp_runtime::traits::Hash;
// use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use chrono::{DateTime, TimeZone, Utc};
use codec::alloc::string::String;

const SEED: u32 = 0;

benchmarks! {
	
	anchor {

		// let dt = Utc.timestamp(1_500_000_000, 0);
		// let val: &[u8] = dt.into();
		// now.to_rfc2822();
		// now.datetime_from_str();
		// let mut key = [0u8; 16];
		// OsRng.fill_bytes(&mut key);
		// let random_u64 = OsRng.next_u32()();
		// let ran_ud:&[u8] = random_u64 as &[u8];
		// // let x: u8 = rand::random();
		// // let val: &[u8] = (vec![2,4]*x);
		// let rand_string: String = thread_rng()
        // .sample_iter(&Alphanumeric)
        // .take(30)
        // .map(char::from)
        // .collect();

		// let mut someVex: &[u8] = [b"a",b"kk",b"c",b"d",b"e"];

		// let val = rand_string.into_bytes();
		// let s = str::from_utf8(val).expect("Found invalid UTF-8");
		// let fin:&[u8] = s.as_bytes();
		// let rand_string: String = thread_rng()
        // .sample_iter(&Alphanumeric)
        // .take(30)
        // .map(char::from)
        // .collect();


		let caller: T::AccountId = whitelisted_caller();
		let acc: T::AccountId = whitelisted_caller();

		let digest_hash: T::Hash = T::Hash::default();
		let content_hash: T::Hash = T::Hashing::hash(b"somedat");
		let hash = <T::Hash as Default>::default();
		let mtype_hash: T::Hash = T::Hash::default();
		
		pallet_mtype::Module::<T>::anchor(RawOrigin::Signed(caller.clone()).into(), mtype_hash)?;
		pallet_mark::Module::<T>::anchor(RawOrigin::Signed(acc.clone()).into(), content_hash,mtype_hash,None)?;

		<Digests<T>>::insert(digest_hash, Digest {content_hash, marker: caller.clone(), revoked: false});
		// let mark = <pallet_mark::Marks<T>>::get(content_hash).ok_or(pallet_mark::Error::<T>::MarkNotFound)?;
	}: _(RawOrigin::Signed(caller.clone()),digest_hash,content_hash)
	verify {
		 Digests::<T>::contains_key(digest_hash);
		
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