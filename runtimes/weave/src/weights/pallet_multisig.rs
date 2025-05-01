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

/// Weight functions for `pallet_multisig`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_multisig::WeightInfo for WeightInfo<T> {
	/// The range of component `z` is `[0, 10000]`.
	fn as_multi_threshold_1(z: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 12_630_000 picoseconds.
		Weight::from_parts(13_096_228, 0)
			.saturating_add(Weight::from_parts(0, 0))
			// Standard Error: 1
			.saturating_add(Weight::from_parts(476, 0).saturating_mul(z.into()))
	}
	/// Storage: `Multisig::Multisigs` (r:1 w:1)
	/// Proof: `Multisig::Multisigs` (`max_values`: None, `max_size`: Some(3346), added: 5821, mode: `MaxEncodedLen`)
	/// The range of component `s` is `[2, 100]`.
	/// The range of component `z` is `[0, 10000]`.
	fn as_multi_create(s: u32, z: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `301 + s * (2 ±0)`
		//  Estimated: `6811`
		// Minimum execution time: 44_881_000 picoseconds.
		Weight::from_parts(37_726_672, 0)
			.saturating_add(Weight::from_parts(0, 6811))
			// Standard Error: 245
			.saturating_add(Weight::from_parts(78_397, 0).saturating_mul(s.into()))
			// Standard Error: 2
			.saturating_add(Weight::from_parts(1_456, 0).saturating_mul(z.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Multisig::Multisigs` (r:1 w:1)
	/// Proof: `Multisig::Multisigs` (`max_values`: None, `max_size`: Some(3346), added: 5821, mode: `MaxEncodedLen`)
	/// The range of component `s` is `[3, 100]`.
	/// The range of component `z` is `[0, 10000]`.
	fn as_multi_approve(s: u32, z: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `320`
		//  Estimated: `6811`
		// Minimum execution time: 29_131_000 picoseconds.
		Weight::from_parts(21_706_792, 0)
			.saturating_add(Weight::from_parts(0, 6811))
			// Standard Error: 568
			.saturating_add(Weight::from_parts(81_496, 0).saturating_mul(s.into()))
			// Standard Error: 5
			.saturating_add(Weight::from_parts(1_493, 0).saturating_mul(z.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Multisig::Multisigs` (r:1 w:1)
	/// Proof: `Multisig::Multisigs` (`max_values`: None, `max_size`: Some(3346), added: 5821, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// The range of component `s` is `[2, 100]`.
	/// The range of component `z` is `[0, 10000]`.
	fn as_multi_complete(s: u32, z: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `426 + s * (33 ±0)`
		//  Estimated: `6811`
		// Minimum execution time: 49_730_000 picoseconds.
		Weight::from_parts(40_635_743, 0)
			.saturating_add(Weight::from_parts(0, 6811))
			// Standard Error: 339
			.saturating_add(Weight::from_parts(97_706, 0).saturating_mul(s.into()))
			// Standard Error: 3
			.saturating_add(Weight::from_parts(1_460, 0).saturating_mul(z.into()))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `Multisig::Multisigs` (r:1 w:1)
	/// Proof: `Multisig::Multisigs` (`max_values`: None, `max_size`: Some(3346), added: 5821, mode: `MaxEncodedLen`)
	/// The range of component `s` is `[2, 100]`.
	fn approve_as_multi_create(s: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `301 + s * (2 ±0)`
		//  Estimated: `6811`
		// Minimum execution time: 34_691_000 picoseconds.
		Weight::from_parts(35_728_647, 0)
			.saturating_add(Weight::from_parts(0, 6811))
			// Standard Error: 343
			.saturating_add(Weight::from_parts(77_434, 0).saturating_mul(s.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Multisig::Multisigs` (r:1 w:1)
	/// Proof: `Multisig::Multisigs` (`max_values`: None, `max_size`: Some(3346), added: 5821, mode: `MaxEncodedLen`)
	/// The range of component `s` is `[2, 100]`.
	fn approve_as_multi_approve(s: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `320`
		//  Estimated: `6811`
		// Minimum execution time: 17_560_000 picoseconds.
		Weight::from_parts(20_486_339, 0)
			.saturating_add(Weight::from_parts(0, 6811))
			// Standard Error: 916
			.saturating_add(Weight::from_parts(81_075, 0).saturating_mul(s.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Multisig::Multisigs` (r:1 w:1)
	/// Proof: `Multisig::Multisigs` (`max_values`: None, `max_size`: Some(3346), added: 5821, mode: `MaxEncodedLen`)
	/// The range of component `s` is `[2, 100]`.
	fn cancel_as_multi(s: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `492 + s * (1 ±0)`
		//  Estimated: `6811`
		// Minimum execution time: 35_130_000 picoseconds.
		Weight::from_parts(36_228_612, 0)
			.saturating_add(Weight::from_parts(0, 6811))
			// Standard Error: 409
			.saturating_add(Weight::from_parts(79_777, 0).saturating_mul(s.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Multisig::Multisigs` (r:1 w:1)
	/// Proof: `Multisig::Multisigs` (`max_values`: None, `max_size`: Some(3346), added: 5821, mode: `MaxEncodedLen`)
	/// The range of component `s` is `[2, 100]`.
	fn poke_deposit(s: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `420 + s * (1 ±0)`
		//  Estimated: `6811`
		// Minimum execution time: 30_328_000 picoseconds.
		Weight::from_parts(34_135_440, 0)
			.saturating_add(Weight::from_parts(0, 6811))
			// Standard Error: 4_398
			.saturating_add(Weight::from_parts(161_074, 0).saturating_mul(s.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
