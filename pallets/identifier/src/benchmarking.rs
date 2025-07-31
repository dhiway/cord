// This file is part of CORD – https://cord.network
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::{
	traits::{ConstU32, Get},
	BoundedVec,
};
// use sp_std::prelude::*;
use frame_benchmarking::{v2::extrinsic_call, BenchmarkError};

benchmarks! {
	get_or_add_pallet_index {
		// Whitelisted caller (not used in function but for consistency with entry pallet)
		let caller: T::AccountId = whitelisted_caller();
		// Variable-length name up to 64 bytes
		let l in 1 .. 64;
		let name_bytes = vec![b'a'; l as usize];
		let pallet_name = String::from_utf8(name_bytes.clone()).expect("ASCII bytes should be valid UTF-8");
	}: {
		// Measured call
		// #[extrinsic_call]
		Pallet::<T>::get_or_add_pallet_index(&pallet_name)
			.map_err(|e| BenchmarkError::Stop(e.into()))?;
	}
	verify {
		// Verify the pallet index was created
		let bounded_name: BoundedVec<u8, ConstU32<64>> =
			name_bytes.try_into().expect("should fit within 64 bytes");
		assert!(PalletIndex::<T>::contains_key(&bounded_name));
		let index = PalletIndex::<T>::get(&bounded_name).expect("Index exists");
		let resolved_name = Pallet::<T>::resolve_pallet_name(index).expect("Name resolves");
		assert_eq!(resolved_name, pallet_name);
	}

	resolve_pallet_name {
		// Whitelisted caller (not used in function but for consistency with entry pallet)
		let caller: T::AccountId = whitelisted_caller();
		// Setup: create a pallet index
		let pallet_name = "TestPallet";
		let index = Pallet::<T>::get_or_add_pallet_index(pallet_name)
			.map_err(|e| BenchmarkError::Stop(e.into()))?;
	}: {
		// Measured call
		// #[extrinsic_call]
		Pallet::<T>::resolve_pallet_name(index)
			.map_err(|e| BenchmarkError::Stop(e.into()))?;
	}
	verify {
		// Verify the name was resolved
		let resolved_name = Pallet::<T>::resolve_pallet_name(index).expect("Name resolves");
		assert_eq!(resolved_name, pallet_name);
	}

	update_identifier_state {
		// Whitelisted caller (not used in function but for consistency with entry pallet)
		let caller: T::AccountId = whitelisted_caller();
		// Setup identifier and event payload
		let digest_bytes = vec![1u8; 32];
		let identifier = Ss58Identifier::to_encoded(
			digest_bytes.clone(),
			100,
			5,
			T::Ss58Prefix::get(),
			0,
		).map_err(|_| BenchmarkError::Stop("Failed to encode identifier"))?;
		let action: EventTypeOf = vec![2u8; 10].try_into().expect("bounded vec");
		let seal = EventBlock { height: 1, index: 0 };
		let digest: HashOf<T> = T::Hash::default();
	}: {
		// Measured call
		// #[extrinsic_call]
		Pallet::<T>::update_identifier_state(&identifier, digest, action.clone(), seal.clone())
			.map_err(|_| BenchmarkError::Stop("State update failed"))?;
	}
	verify {
		// Verify the event was recorded
		let version = StateVersion::<T>::get(&identifier);
		assert_eq!(version, 1);
		let record = StateHistory::<T>::get(&identifier, 0).expect("Record exists");
		assert_eq!(record.action, action);
		assert_eq!(record.digest, digest);
		assert_eq!(record.seal, seal);
	}

	impl_benchmark_test_suite!(
		Pallet,
		crate::mock::new_test_ext(),
		crate::mock::Test
	);
}
