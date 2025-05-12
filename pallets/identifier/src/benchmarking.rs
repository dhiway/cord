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

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::pallet;
use frame_benchmarking::{benchmarks, v2::*};

benchmarks! {
	get_or_add_pallet_index {
		let pallet_name: &str = "TestPallet";
	}: {
		let _ = Pallet::<T>::get_or_add_pallet_index(pallet_name)
			.map_err(|e| BenchmarkError::from(sp_runtime::DispatchError::from(e)))?;
	}
	verify {
		let bounded_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
			pallet_name.as_bytes().to_vec().try_into().expect("BoundedVec creation should not fail");
		assert!(pallet::PalletIndex::<T>::contains_key(&bounded_name));
	}

	resolve_pallet_name {
		let pallet_name: &str = "TestPallet";
		let index = Pallet::<T>::get_or_add_pallet_index(pallet_name)
			.map_err(|e| BenchmarkError::from(sp_runtime::DispatchError::from(e)))?;
	}: {
		let _ = Pallet::<T>::resolve_pallet_name(index)
			.map_err(|e| BenchmarkError::from(sp_runtime::DispatchError::from(e)))?;
	}

	// Benchmark the new update_state functionality.
	update_identifier_state {
		let digest_bytes: Vec<u8> = vec![1u8; 32];
		let identifier = Ss58Identifier::to_encoded(digest_bytes, 100, 5)
			.expect("Identifier encoding should succeed");
		let event: EventTypeOf = vec![2u8; 10]
			.try_into()
			.expect("Should create valid bounded vector");
		let stamp = EventBlock { height: 1, index: 0 };
		let new_digest: HashOf<T> = T::Hash::default();
	}: {
		Pallet::<T>::update_identifier_state(&identifier, new_digest, event.clone(), stamp.clone())?;
	}
}
