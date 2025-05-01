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

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_indices`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_indices::WeightInfo for WeightInfo<T> {
	/// Storage: `Indices::Accounts` (r:1 w:1)
	/// Proof: `Indices::Accounts` (`max_values`: None, `max_size`: Some(69), added: 2544, mode: `MaxEncodedLen`)
	fn claim() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4`
		//  Estimated: `3534`
		// Minimum execution time: 23_190_000 picoseconds.
		Weight::from_parts(23_720_000, 0)
			.saturating_add(Weight::from_parts(0, 3534))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Indices::Accounts` (r:1 w:1)
	/// Proof: `Indices::Accounts` (`max_values`: None, `max_size`: Some(69), added: 2544, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn transfer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `203`
		//  Estimated: `3593`
		// Minimum execution time: 38_650_000 picoseconds.
		Weight::from_parts(39_480_000, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `Indices::Accounts` (r:1 w:1)
	/// Proof: `Indices::Accounts` (`max_values`: None, `max_size`: Some(69), added: 2544, mode: `MaxEncodedLen`)
	fn free() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `100`
		//  Estimated: `3534`
		// Minimum execution time: 23_671_000 picoseconds.
		Weight::from_parts(24_110_000, 0)
			.saturating_add(Weight::from_parts(0, 3534))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Indices::Accounts` (r:1 w:1)
	/// Proof: `Indices::Accounts` (`max_values`: None, `max_size`: Some(69), added: 2544, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn force_transfer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `203`
		//  Estimated: `3593`
		// Minimum execution time: 25_870_000 picoseconds.
		Weight::from_parts(26_350_000, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `Indices::Accounts` (r:1 w:1)
	/// Proof: `Indices::Accounts` (`max_values`: None, `max_size`: Some(69), added: 2544, mode: `MaxEncodedLen`)
	fn freeze() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `100`
		//  Estimated: `3534`
		// Minimum execution time: 24_820_000 picoseconds.
		Weight::from_parts(25_160_000, 0)
			.saturating_add(Weight::from_parts(0, 3534))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Indices::Accounts` (r:1 w:1)
	/// Proof: `Indices::Accounts` (`max_values`: None, `max_size`: Some(69), added: 2544, mode: `MaxEncodedLen`)
	fn poke_deposit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `100`
		//  Estimated: `3534`
		// Minimum execution time: 23_290_000 picoseconds.
		Weight::from_parts(24_217_000, 0)
			.saturating_add(Weight::from_parts(0, 3534))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
