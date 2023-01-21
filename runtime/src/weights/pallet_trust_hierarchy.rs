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
#![allow(clippy::unnecessary_cast)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_trust_hierarchy::WeightInfo for WeightInfo<T> {
	fn create_hierarchy() -> Weight {
		Weight::from_ref_time(37_014_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	fn add_delegation() -> Weight {
		Weight::from_ref_time(44_986_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	fn revoke_delegation_root_child(r: u32, c: u32, ) -> Weight {
		Weight::from_ref_time(19_082_000 as u64)
			// Standard Error: 51_000
			.saturating_add(Weight::from_ref_time(13_360_000 as u64).saturating_mul(r as u64))
			// Standard Error: 51_000
			.saturating_add(Weight::from_ref_time(169_000 as u64).saturating_mul(c as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(r as u64)))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(r as u64)))
	}
	fn revoke_delegation_leaf(r: u32, c: u32, ) -> Weight {
		Weight::from_ref_time(32_789_000 as u64)
			// Standard Error: 41_000
			.saturating_add(Weight::from_ref_time(13_000 as u64).saturating_mul(r as u64))
			// Standard Error: 41_000
			.saturating_add(Weight::from_ref_time(5_095_000 as u64).saturating_mul(c as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(c as u64)))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	/// The range of component `r` is `[1, 5]`.
	fn remove_delegation(r: u32, ) -> Weight {
		Weight::from_ref_time(51_863_000 as u64)
			// Standard Error: 59_000
			.saturating_add(Weight::from_ref_time(23_235_000 as u64).saturating_mul(r as u64))
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(r as u64)))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(r as u64)))
	}
	fn reclaim_deposit(r: u32, ) -> Weight {
		Weight::from_ref_time(44_669_000 as u64)
			// Standard Error: 56_000
			.saturating_add(Weight::from_ref_time(23_133_000 as u64).saturating_mul(r as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(r as u64)))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(r as u64)))
	}
	fn can_attest() -> Weight {
		Weight::from_ref_time(12_484_000 as u64)
			.saturating_add(T::DbWeight::get().reads(2 as u64))
	}
	fn can_revoke(c: u32, ) -> Weight {
		Weight::from_ref_time(8_127_000 as u64)
			// Standard Error: 38_000
			.saturating_add(Weight::from_ref_time(5_164_000 as u64).saturating_mul(c as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(c as u64)))
	}
	fn can_remove(c: u32, ) -> Weight {
		Weight::from_ref_time(7_991_000 as u64)
			// Standard Error: 35_000
			.saturating_add(Weight::from_ref_time(5_193_000 as u64).saturating_mul(c as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(c as u64)))
	}
	fn transfer_deposit( ) -> Weight {
		Weight::from_ref_time(7_991_000 as u64)
			// Standard Error: 35_000
			.saturating_add(T::DbWeight::get().reads(1 as u64))
	}
}

