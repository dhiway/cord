// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_indices`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_indices::WeightInfo for WeightInfo<T> {
	/// Storage: Indices Accounts (r:1 w:1)
	/// Proof: Indices Accounts (max_values: None, max_size: Some(69), added: 2544, mode: MaxEncodedLen)
	fn claim() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `142`
		//  Estimated: `3534`
		// Minimum execution time: 19_803_000 picoseconds.
		Weight::from_parts(20_268_000, 0)
			.saturating_add(Weight::from_parts(0, 3534))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: Indices Accounts (r:1 w:1)
	/// Proof: Indices Accounts (max_values: None, max_size: Some(69), added: 2544, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn transfer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `341`
		//  Estimated: `7127`
		// Minimum execution time: 25_909_000 picoseconds.
		Weight::from_parts(26_506_000, 0)
			.saturating_add(Weight::from_parts(0, 7127))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: Indices Accounts (r:1 w:1)
	/// Proof: Indices Accounts (max_values: None, max_size: Some(69), added: 2544, mode: MaxEncodedLen)
	fn free() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `238`
		//  Estimated: `3534`
		// Minimum execution time: 21_587_000 picoseconds.
		Weight::from_parts(21_867_000, 0)
			.saturating_add(Weight::from_parts(0, 3534))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: Indices Accounts (r:1 w:1)
	/// Proof: Indices Accounts (max_values: None, max_size: Some(69), added: 2544, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn force_transfer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `341`
		//  Estimated: `7127`
		// Minimum execution time: 24_681_000 picoseconds.
		Weight::from_parts(25_015_000, 0)
			.saturating_add(Weight::from_parts(0, 7127))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: Indices Accounts (r:1 w:1)
	/// Proof: Indices Accounts (max_values: None, max_size: Some(69), added: 2544, mode: MaxEncodedLen)
	fn freeze() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `238`
		//  Estimated: `3534`
		// Minimum execution time: 23_779_000 picoseconds.
		Weight::from_parts(24_063_000, 0)
			.saturating_add(Weight::from_parts(0, 3534))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}