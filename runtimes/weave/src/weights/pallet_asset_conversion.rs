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
// along with CORD. If not, see <https://www.gnu.org/licenses/>. ANY KIND, either express or implied.

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

/// Weight functions for `pallet_asset_conversion`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_asset_conversion::WeightInfo for WeightInfo<T> {
	/// Storage: `AssetConversion::Pools` (r:1 w:1)
	/// Proof: `AssetConversion::Pools` (`max_values`: None, `max_size`: Some(1224), added: 3699,
	/// mode: `MaxEncodedLen`) Storage: `System::Account` (r:2 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode:
	/// `MaxEncodedLen`) Storage: `ForeignAssets::Asset` (r:1 w:0)
	/// Proof: `ForeignAssets::Asset` (`max_values`: None, `max_size`: Some(808), added: 3283, mode:
	/// `MaxEncodedLen`) Storage: `AssetConversion::NextPoolAssetId` (r:1 w:1)
	/// Proof: `AssetConversion::NextPoolAssetId` (`max_values`: Some(1), `max_size`: Some(4),
	/// added: 499, mode: `MaxEncodedLen`) Storage: `PoolAssets::Asset` (r:1 w:1)
	/// Proof: `PoolAssets::Asset` (`max_values`: None, `max_size`: Some(210), added: 2685, mode:
	/// `MaxEncodedLen`) Storage: `PoolAssets::NextAssetId` (r:1 w:0)
	/// Proof: `PoolAssets::NextAssetId` (`max_values`: Some(1), `max_size`: Some(4), added: 499,
	/// mode: `MaxEncodedLen`) Storage: `PoolAssets::Account` (r:1 w:1)
	/// Proof: `PoolAssets::Account` (`max_values`: None, `max_size`: Some(134), added: 2609, mode:
	/// `MaxEncodedLen`)
	fn create_pool() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `469`
		//  Estimated: `6196`
		// Minimum execution time: 103_161_000 picoseconds.
		Weight::from_parts(104_441_000, 0)
			.saturating_add(Weight::from_parts(0, 6196))
			.saturating_add(T::DbWeight::get().reads(8))
			.saturating_add(T::DbWeight::get().writes(5))
	}
	/// Storage: `AssetConversion::Pools` (r:1 w:0)
	/// Proof: `AssetConversion::Pools` (`max_values`: None, `max_size`: Some(1224), added: 3699,
	/// mode: `MaxEncodedLen`) Storage: `ForeignAssets::Asset` (r:1 w:1)
	/// Proof: `ForeignAssets::Asset` (`max_values`: None, `max_size`: Some(808), added: 3283, mode:
	/// `MaxEncodedLen`) Storage: `ForeignAssets::Account` (r:2 w:2)
	/// Proof: `ForeignAssets::Account` (`max_values`: None, `max_size`: Some(732), added: 3207,
	/// mode: `MaxEncodedLen`) Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode:
	/// `MaxEncodedLen`) Storage: `PoolAssets::Asset` (r:1 w:1)
	/// Proof: `PoolAssets::Asset` (`max_values`: None, `max_size`: Some(210), added: 2685, mode:
	/// `MaxEncodedLen`) Storage: `PoolAssets::Account` (r:2 w:2)
	/// Proof: `PoolAssets::Account` (`max_values`: None, `max_size`: Some(134), added: 2609, mode:
	/// `MaxEncodedLen`)
	fn add_liquidity() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `975`
		//  Estimated: `7404`
		// Minimum execution time: 166_212_000 picoseconds.
		Weight::from_parts(166_921_000, 0)
			.saturating_add(Weight::from_parts(0, 7404))
			.saturating_add(T::DbWeight::get().reads(8))
			.saturating_add(T::DbWeight::get().writes(7))
	}
	/// Storage: `AssetConversion::Pools` (r:1 w:0)
	/// Proof: `AssetConversion::Pools` (`max_values`: None, `max_size`: Some(1224), added: 3699,
	/// mode: `MaxEncodedLen`) Storage: `ForeignAssets::Asset` (r:1 w:1)
	/// Proof: `ForeignAssets::Asset` (`max_values`: None, `max_size`: Some(808), added: 3283, mode:
	/// `MaxEncodedLen`) Storage: `ForeignAssets::Account` (r:2 w:2)
	/// Proof: `ForeignAssets::Account` (`max_values`: None, `max_size`: Some(732), added: 3207,
	/// mode: `MaxEncodedLen`) Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode:
	/// `MaxEncodedLen`) Storage: `PoolAssets::Asset` (r:1 w:1)
	/// Proof: `PoolAssets::Asset` (`max_values`: None, `max_size`: Some(210), added: 2685, mode:
	/// `MaxEncodedLen`) Storage: `PoolAssets::Account` (r:1 w:1)
	/// Proof: `PoolAssets::Account` (`max_values`: None, `max_size`: Some(134), added: 2609, mode:
	/// `MaxEncodedLen`)
	fn remove_liquidity() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1130`
		//  Estimated: `7404`
		// Minimum execution time: 153_761_000 picoseconds.
		Weight::from_parts(154_922_000, 0)
			.saturating_add(Weight::from_parts(0, 7404))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	/// Storage: `ForeignAssets::Asset` (r:2 w:2)
	/// Proof: `ForeignAssets::Asset` (`max_values`: None, `max_size`: Some(808), added: 3283, mode:
	/// `MaxEncodedLen`) Storage: `ForeignAssets::Account` (r:4 w:4)
	/// Proof: `ForeignAssets::Account` (`max_values`: None, `max_size`: Some(732), added: 3207,
	/// mode: `MaxEncodedLen`) Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode:
	/// `MaxEncodedLen`) The range of component `n` is `[2, 3]`.
	fn swap_exact_tokens_for_tokens(n: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0 + n * (507 ±0)`
		//  Estimated: `7404 + n * (94 ±7)`
		// Minimum execution time: 108_631_000 picoseconds.
		Weight::from_parts(109_631_000, 0)
			.saturating_add(Weight::from_parts(0, 7404))
			// Standard Error: 202_949
			.saturating_add(Weight::from_parts(1_129_814, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(4))
			.saturating_add(Weight::from_parts(0, 94).saturating_mul(n.into()))
	}
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode:
	/// `MaxEncodedLen`) Storage: `ForeignAssets::Asset` (r:2 w:2)
	/// Proof: `ForeignAssets::Asset` (`max_values`: None, `max_size`: Some(808), added: 3283, mode:
	/// `MaxEncodedLen`) Storage: `ForeignAssets::Account` (r:4 w:4)
	/// Proof: `ForeignAssets::Account` (`max_values`: None, `max_size`: Some(732), added: 3207,
	/// mode: `MaxEncodedLen`) The range of component `n` is `[2, 3]`.
	fn swap_tokens_for_exact_tokens(n: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0 + n * (507 ±0)`
		//  Estimated: `7404 + n * (94 ±10)`
		// Minimum execution time: 108_641_000 picoseconds.
		Weight::from_parts(109_642_000, 0)
			.saturating_add(Weight::from_parts(0, 7404))
			// Standard Error: 205_180
			.saturating_add(Weight::from_parts(1_146_492, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(4))
			.saturating_add(Weight::from_parts(0, 94).saturating_mul(n.into()))
	}
	/// Storage: `AssetConversion::Pools` (r:1 w:0)
	/// Proof: `AssetConversion::Pools` (`max_values`: None, `max_size`: Some(1224), added: 3699,
	/// mode: `MaxEncodedLen`) Storage: `ForeignAssets::Asset` (r:1 w:1)
	/// Proof: `ForeignAssets::Asset` (`max_values`: None, `max_size`: Some(808), added: 3283, mode:
	/// `MaxEncodedLen`) Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode:
	/// `MaxEncodedLen`) Storage: `ForeignAssets::Account` (r:1 w:1)
	/// Proof: `ForeignAssets::Account` (`max_values`: None, `max_size`: Some(732), added: 3207,
	/// mode: `MaxEncodedLen`) Storage: `PoolAssets::Asset` (r:1 w:1)
	/// Proof: `PoolAssets::Asset` (`max_values`: None, `max_size`: Some(210), added: 2685, mode:
	/// `MaxEncodedLen`) Storage: `PoolAssets::Account` (r:1 w:1)
	/// Proof: `PoolAssets::Account` (`max_values`: None, `max_size`: Some(134), added: 2609, mode:
	/// `MaxEncodedLen`) The range of component `n` is `[0, 3]`.
	fn touch(n: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `990`
		//  Estimated: `4689`
		// Minimum execution time: 45_640_000 picoseconds.
		Weight::from_parts(50_614_873, 0)
			.saturating_add(Weight::from_parts(0, 4689))
			// Standard Error: 306_166
			.saturating_add(Weight::from_parts(12_889_745, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(n.into())))
	}
}
