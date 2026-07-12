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

// Dummy Weights

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;
use frame_system::Config;

/// Weights for `pallet_orbis_entity` using the Substrate node and recommended hardware.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_orbis_entity::WeightInfo for WeightInfo<T> {
	fn set_info(info_size: u32) -> Weight {
        // base + per-byte fee + DB ops
        Weight::from_parts(50_000_000, 0)                // base
            .saturating_add(Weight::from_parts(info_size as u64 * 1_000, 0))
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn rotate_attributes(ops_size: u32) -> Weight {
        Weight::from_parts(30_000_000, 0)
            .saturating_add(Weight::from_parts(ops_size as u64 * 500, 0))
            // assume each history insert is an extra write
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn add_attributes(kv_size: u32) -> Weight {
        Weight::from_parts(20_000_000, 0)
            .saturating_add(Weight::from_parts(kv_size as u64 * 500, 0))
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn remove_attribute(k_size: u32) -> Weight {
        Weight::from_parts(20_000_000, 0)
            .saturating_add(Weight::from_parts(k_size as u64 * 500, 0))
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn rotate_attribute(kv_size: u32) -> Weight {
        Weight::from_parts(20_000_000, 0)
            .saturating_add(Weight::from_parts(kv_size as u64 * 500, 0))
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn set_linked_account(r: u32) -> Weight {
        Weight::from_parts(15_000_000, 0)
       		.saturating_add(Weight::from_parts(r as u64 * 500, 0))
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn revoke_linked_account(r: u32) -> Weight {
        Weight::from_parts(15_000_000, 0)
        	.saturating_add(Weight::from_parts(r as u64 * 500, 0))
        	.saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn revoke_linked_account_for(r: u32) -> Weight {
        Weight::from_parts(15_000_000, 0)
        	.saturating_add(Weight::from_parts(r as u64 * 500, 0))
        	.saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn rotate_controller(r: u32) -> Weight {
        Weight::from_parts(10_000_000, 0)
       		.saturating_add(Weight::from_parts(r as u64 * 500, 0))
            .saturating_add(T::DbWeight::get().reads(0))
            .saturating_add(T::DbWeight::get().writes(5))
    }
    fn rotate_controller_for(r: u32) -> Weight {
  Weight::from_parts(10_000_000, 0)
 		.saturating_add(Weight::from_parts(r as u64 * 500, 0))
      .saturating_add(T::DbWeight::get().reads(0))
      .saturating_add(T::DbWeight::get().writes(5))
    }
    fn clear_everything(num_links: u32) -> Weight {
        Weight::from_parts(40_000_000, 0)
            // each linked-account cleanup is one read+one write
            .saturating_add(T::DbWeight::get().reads(2 + num_links as u64))
            .saturating_add(T::DbWeight::get().writes(7 + num_links as u64))
    }
    fn clear_everything_for(num_links: u32) -> Weight { Self::clear_everything(num_links) }
    fn set_entity_nym(prefix_len: u32) -> Weight {
        Weight::from_parts(20_000_000, 0)
            .saturating_add(Weight::from_parts(prefix_len as u64 * 100, 0))
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn remove_entity_nym() -> Weight {
        Weight::from_parts(15_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(3))
    }
}
