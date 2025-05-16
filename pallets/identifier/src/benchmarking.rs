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

// benchmarking.rs
// benchmarking.rs
#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::vec;
use frame_benchmarking::v2::*;
use frame_support::traits::ConstU32;
use frame_system::{EventRecord, Pallet as System, RawOrigin};
use sp_runtime::DispatchError;

#[cfg(test)]
use crate::Pallet as Identifier;

/// Assert that the last deposited event matches `generic_event`.
fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	let events = System::<T>::events();
	let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
	let EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

#[benchmarks]
mod benchmarks {
	use super::*;

	/// 1) Benchmark get_or_add_pallet_index for names up to 64 bytes
	#[benchmark]
	fn get_or_add_pallet_index(l: Linear<1, { 64 }>) -> Result<(), BenchmarkError> {
		// --- setup a variable-length name ---
		let name_bytes = vec![b'a'; l as usize];
		let pallet_name =
			String::from_utf8(name_bytes.clone()).expect("ASCII bytes should be valid UTF-8");

		// --- measured call as an “extrinsic” ---
		#[extrinsic_call]
		let index = Identifier::<T>::get_or_add_pallet_index(&pallet_name)
			.map_err(|e| BenchmarkError::from(DispatchError::from(e)))?;

		// --- verify storage and round-trip resolve ---
		let bounded_name: BoundedVec<u8, ConstU32<64>> =
			name_bytes.try_into().expect("should fit within 64 bytes");
		assert!(PalletIndex::<T>::contains_key(&bounded_name));
		let _ = Identifier::<T>::resolve_pallet_name(index)?;

		Ok(())
	}

	/// 2) Benchmark resolve_pallet_name (cold hit)
	#[benchmark]
	fn resolve_pallet_name() -> Result<(), BenchmarkError> {
		// --- setup: ensure one mapping exists ---
		let pallet_name = "TestPallet";

		#[extrinsic_call]
		let index = Identifier::<T>::get_or_add_pallet_index(pallet_name)
			.map_err(|e| BenchmarkError::from(DispatchError::from(e)))?;

		// --- measured call as an “extrinsic” ---
		#[extrinsic_call]
		let _ = Identifier::<T>::resolve_pallet_name(index)
			.map_err(|e| BenchmarkError::from(DispatchError::from(e)))?;

		Ok(())
	}

	/// 3) Benchmark update_identifier_state and assert the event
	#[benchmark]
	fn update_identifier_state() -> Result<(), BenchmarkError> {
		// --- setup identifier and event payload ---
		let digest_bytes = vec![1u8; 32];
		let identifier = Ss58Identifier::to_encoded(digest_bytes.clone(), 100, 5)
			.expect("Identifier encoding must succeed");
		let action: EventTypeOf = vec![2u8; 10].try_into().expect("bounded vec");
		let seal = EventBlock { height: 1, index: 0 };
		let new_digest: HashOf<T> = T::Hash::default();

		// --- measured call as an “extrinsic” ---
		#[extrinsic_call]
		Identifier::<T>::update_identifier_state(
			&identifier,
			new_digest,
			action.clone(),
			seal.clone(),
		)
		.map_err(|e| BenchmarkError::from(DispatchError::from(e)))?;

		// --- verify that the event was emitted ---
		assert_last_event::<T>(
			Event::StateChange { identifier: identifier.clone(), version: 0, action }.into(),
		);

		Ok(())
	}

	// Mock‐based test suite integration
	impl_benchmark_test_suite!(
		Identifier,
		crate::mock::new_test_ext::<crate::mock::Test>(),
		crate::mock::Test
	);
}
