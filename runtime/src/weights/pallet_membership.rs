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

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_membership`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_membership::WeightInfo for WeightInfo<T> {
	// Storage: Membership Members (r:1 w:1)
	// Storage: Collective Proposals (r:1 w:0)
	// Storage: Collective Members (r:0 w:1)
	// Storage: Collective Prime (r:0 w:1)
	/// The range of component `m` is `[1, 99]`.
	fn add_member(m: u32, ) -> Weight {
		Weight::from_ref_time(19_637_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(32_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Membership Members (r:1 w:1)
	// Storage: Collective Proposals (r:1 w:0)
	// Storage: Membership Prime (r:1 w:0)
	// Storage: Collective Members (r:0 w:1)
	// Storage: Collective Prime (r:0 w:1)
	/// The range of component `m` is `[2, 100]`.
	fn remove_member(m: u32, ) -> Weight {
		Weight::from_ref_time(21_565_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(31_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Membership Members (r:1 w:1)
	// Storage: Collective Proposals (r:1 w:0)
	// Storage: Membership Prime (r:1 w:0)
	// Storage: Collective Members (r:0 w:1)
	// Storage: Collective Prime (r:0 w:1)
	/// The range of component `m` is `[2, 100]`.
	fn swap_member(m: u32, ) -> Weight {
		Weight::from_ref_time(21_637_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(46_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Membership Members (r:1 w:1)
	// Storage: Collective Proposals (r:1 w:0)
	// Storage: Membership Prime (r:1 w:0)
	// Storage: Collective Members (r:0 w:1)
	// Storage: Collective Prime (r:0 w:1)
	/// The range of component `m` is `[1, 100]`.
	fn reset_member(m: u32, ) -> Weight {
		Weight::from_ref_time(21_551_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(149_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Membership Members (r:1 w:1)
	// Storage: Collective Proposals (r:1 w:0)
	// Storage: Membership Prime (r:1 w:1)
	// Storage: Collective Members (r:0 w:1)
	// Storage: Collective Prime (r:0 w:1)
	/// The range of component `m` is `[1, 100]`.
	fn change_key(m: u32, ) -> Weight {
		Weight::from_ref_time(22_510_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(41_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: Membership Members (r:1 w:0)
	// Storage: Membership Prime (r:0 w:1)
	// Storage: Collective Prime (r:0 w:1)
	/// The range of component `m` is `[1, 100]`.
	fn set_prime(m: u32, ) -> Weight {
		Weight::from_ref_time(8_828_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(9_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Membership Prime (r:0 w:1)
	// Storage: Collective Prime (r:0 w:1)
	/// The range of component `m` is `[1, 100]`.
	fn clear_prime(m: u32, ) -> Weight {
		Weight::from_ref_time(5_084_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(1_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
}