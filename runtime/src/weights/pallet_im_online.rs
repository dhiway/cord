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

/// Weight functions for `pallet_im_online`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_im_online::WeightInfo for WeightInfo<T> {
	/// Storage: Session Validators (r:1 w:0)
	/// Proof Skipped: Session Validators (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Session CurrentIndex (r:1 w:0)
	/// Proof Skipped: Session CurrentIndex (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: ImOnline Keys (r:1 w:0)
	/// Proof: ImOnline Keys (max_values: Some(1), max_size: Some(320002), added: 320497, mode: MaxEncodedLen)
	/// Storage: ImOnline ReceivedHeartbeats (r:1 w:1)
	/// Proof: ImOnline ReceivedHeartbeats (max_values: None, max_size: Some(10021032), added: 10023507, mode: MaxEncodedLen)
	/// Storage: ImOnline AuthoredBlocks (r:1 w:0)
	/// Proof: ImOnline AuthoredBlocks (max_values: None, max_size: Some(56), added: 2531, mode: MaxEncodedLen)
	/// The range of component `k` is `[1, 1000]`.
	/// The range of component `e` is `[1, 100]`.
	fn validate_unsigned_and_then_heartbeat(k: u32, e: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `361 + k * (32 ±0)`
		//  Estimated: `10349676 + e * (35 ±0) + k * (64 ±0)`
		// Minimum execution time: 93_081_000 picoseconds.
		Weight::from_parts(73_987_584, 0)
			.saturating_add(Weight::from_parts(0, 10349676))
			// Standard Error: 265
			.saturating_add(Weight::from_parts(23_186, 0).saturating_mul(k.into()))
			// Standard Error: 2_677
			.saturating_add(Weight::from_parts(385_390, 0).saturating_mul(e.into()))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(1))
			.saturating_add(Weight::from_parts(0, 35).saturating_mul(e.into()))
			.saturating_add(Weight::from_parts(0, 64).saturating_mul(k.into()))
	}
}
