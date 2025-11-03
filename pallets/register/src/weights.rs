// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use frame_system::Config;

pub trait WeightInfo {
	fn create_registry(payload: u32) -> Weight;
	fn set_registry_delegate(count: u32) -> Weight;
	fn remove_registry_delegate() -> Weight;
	fn update_registry_info(size: u32) -> Weight;
	fn set_registry_status() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: Config> WeightInfo for SubstrateWeight<T> {
	fn create_registry(payload: u32) -> Weight {
		Weight::from_parts(60_000_000, 0)
			.saturating_add(Weight::from_parts(payload as u64 * 1_000, 0))
			.saturating_add(T::DbWeight::get().reads_writes(1, 2))
	}

	fn set_registry_delegate(_count: u32) -> Weight {
		Weight::from_parts(25_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 1))
	}

	fn remove_registry_delegate() -> Weight {
		Weight::from_parts(20_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 1))
	}

	fn update_registry_info(size: u32) -> Weight {
		Weight::from_parts(22_000_000, 0)
			.saturating_add(Weight::from_parts(size as u64 * 500, 0))
			.saturating_add(T::DbWeight::get().reads_writes(1, 1))
	}

	fn set_registry_status() -> Weight {
		Weight::from_parts(18_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 1))
	}
}

impl WeightInfo for () {
	fn create_registry(payload: u32) -> Weight {
		Weight::from_parts(60_000_000, 0)
			.saturating_add(Weight::from_parts(payload as u64 * 1_000, 0))
			.saturating_add(RocksDbWeight::get().reads_writes(1, 2))
	}

	fn set_registry_delegate(_count: u32) -> Weight {
		Weight::from_parts(25_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(2, 1))
	}

	fn remove_registry_delegate() -> Weight {
		Weight::from_parts(20_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(2, 1))
	}

	fn update_registry_info(size: u32) -> Weight {
		Weight::from_parts(22_000_000, 0)
			.saturating_add(Weight::from_parts(size as u64 * 500, 0))
			.saturating_add(RocksDbWeight::get().reads_writes(1, 1))
	}

	fn set_registry_status() -> Weight {
		Weight::from_parts(18_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 1))
	}
}
