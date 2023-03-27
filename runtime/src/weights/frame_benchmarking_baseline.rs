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

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `frame_benchmarking::baseline`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> frame_benchmarking::baseline::WeightInfo for WeightInfo<T> {
	/// The range of component `i` is `[0, 1000000]`.
	fn addition(_i: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 158_000 picoseconds.
		Weight::from_parts(190_632, 0)
			.saturating_add(Weight::from_parts(0, 0))
	}
	/// The range of component `i` is `[0, 1000000]`.
	fn subtraction(_i: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 149_000 picoseconds.
		Weight::from_parts(183_147, 0)
			.saturating_add(Weight::from_parts(0, 0))
	}
	/// The range of component `i` is `[0, 1000000]`.
	fn multiplication(_i: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 155_000 picoseconds.
		Weight::from_parts(184_117, 0)
			.saturating_add(Weight::from_parts(0, 0))
	}
	/// The range of component `i` is `[0, 1000000]`.
	fn division(_i: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 147_000 picoseconds.
		Weight::from_parts(184_827, 0)
			.saturating_add(Weight::from_parts(0, 0))
	}
	fn hashing() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 19_094_522_000 picoseconds.
		Weight::from_parts(19_201_969_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
	}
	/// The range of component `i` is `[0, 100]`.
	fn sr25519_verification(i: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 183_000 picoseconds.
		Weight::from_parts(208_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
			// Standard Error: 15_974
			.saturating_add(Weight::from_parts(47_488_580, 0).saturating_mul(i.into()))
	}
}
