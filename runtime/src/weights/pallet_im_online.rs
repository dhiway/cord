// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
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

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_im_online`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_im_online::WeightInfo for WeightInfo<T> {
	// Storage: Session Validators (r:1 w:0)
	// Storage: Session CurrentIndex (r:1 w:0)
	// Storage: ImOnline ReceivedHeartbeats (r:1 w:1)
	// Storage: ImOnline AuthoredBlocks (r:1 w:0)
	// Storage: ImOnline Keys (r:1 w:0)
	// Storage: unknown [0x39e295d143ed41353167609a3d816584] (r:1 w:0)
	/// The range of component `k` is `[1, 1000]`.
	/// The range of component `e` is `[1, 100]`.
	fn validate_unsigned_and_then_heartbeat(k: u32, e: u32, ) -> Weight {
		Weight::from_ref_time(76_995_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(23_000 as u64).saturating_mul(k as u64))
			// Standard Error: 2_000
			.saturating_add(Weight::from_ref_time(299_000 as u64).saturating_mul(e as u64))
			.saturating_add(T::DbWeight::get().reads(5 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
