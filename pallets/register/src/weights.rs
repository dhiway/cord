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
	fn revoke_registry() -> Weight;
	fn restore_registry() -> Weight;
	fn delete_registry() -> Weight;
	fn create_packet(attribute_count: u32) -> Weight;
	fn update_packet(attribute_count: u32) -> Weight;
	fn revoke_packet() -> Weight;
	fn restore_packet() -> Weight;
	fn remove_packet() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: Config> WeightInfo for SubstrateWeight<T> {
	fn create_registry(payload: u32) -> Weight {
		Weight::from_parts(55_000_000, 0)
			.saturating_add(Weight::from_parts(payload as u64 * 1_500, 0))
			.saturating_add(T::DbWeight::get().reads_writes(2, 3))
	}

	fn set_registry_delegate(count: u32) -> Weight {
		Weight::from_parts(30_000_000, 0)
			.saturating_add(Weight::from_parts(count as u64 * 2_000, 0))
			.saturating_add(T::DbWeight::get().reads_writes(3, 2))
	}

	fn remove_registry_delegate() -> Weight {
		Weight::from_parts(24_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 2))
	}

	fn update_registry_info(size: u32) -> Weight {
		Weight::from_parts(26_000_000, 0)
			.saturating_add(Weight::from_parts(size as u64 * 1_000, 0))
			.saturating_add(T::DbWeight::get().reads_writes(2, 2))
	}

	fn revoke_registry() -> Weight {
		Weight::from_parts(20_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 2))
	}

	fn restore_registry() -> Weight {
		Weight::from_parts(20_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 2))
	}

	fn delete_registry() -> Weight {
		Weight::from_parts(22_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 2))
	}

	fn create_packet(attribute_count: u32) -> Weight {
		Weight::from_parts(70_000_000, 0)
			.saturating_add(Weight::from_parts(attribute_count as u64 * 3_000, 0))
			.saturating_add(T::DbWeight::get().reads_writes(5, 4))
	}

	fn update_packet(attribute_count: u32) -> Weight {
		Weight::from_parts(85_000_000, 0)
			.saturating_add(Weight::from_parts(attribute_count as u64 * 3_500, 0))
			.saturating_add(T::DbWeight::get().reads_writes(6, 6))
	}

	fn revoke_packet() -> Weight {
		Weight::from_parts(40_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(4, 4))
	}

	fn restore_packet() -> Weight {
		Weight::from_parts(40_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(4, 4))
	}

	fn remove_packet() -> Weight {
		Weight::from_parts(45_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(4, 3))
	}
}

impl WeightInfo for () {
	fn create_registry(payload: u32) -> Weight {
		Weight::from_parts(55_000_000, 0)
			.saturating_add(Weight::from_parts(payload as u64 * 1_500, 0))
			.saturating_add(RocksDbWeight::get().reads_writes(2, 3))
	}

	fn set_registry_delegate(count: u32) -> Weight {
		Weight::from_parts(30_000_000, 0)
			.saturating_add(Weight::from_parts(count as u64 * 2_000, 0))
			.saturating_add(RocksDbWeight::get().reads_writes(3, 2))
	}

	fn remove_registry_delegate() -> Weight {
		Weight::from_parts(24_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(2, 2))
	}

	fn update_registry_info(size: u32) -> Weight {
		Weight::from_parts(26_000_000, 0)
			.saturating_add(Weight::from_parts(size as u64 * 1_000, 0))
			.saturating_add(RocksDbWeight::get().reads_writes(2, 2))
	}

	fn revoke_registry() -> Weight {
		Weight::from_parts(20_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 2))
	}

	fn restore_registry() -> Weight {
		Weight::from_parts(20_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 2))
	}

	fn delete_registry() -> Weight {
		Weight::from_parts(22_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 2))
	}

	fn create_packet(attribute_count: u32) -> Weight {
		Weight::from_parts(70_000_000, 0)
			.saturating_add(Weight::from_parts(attribute_count as u64 * 3_000, 0))
			.saturating_add(RocksDbWeight::get().reads_writes(5, 4))
	}

	fn update_packet(attribute_count: u32) -> Weight {
		Weight::from_parts(85_000_000, 0)
			.saturating_add(Weight::from_parts(attribute_count as u64 * 3_500, 0))
			.saturating_add(RocksDbWeight::get().reads_writes(6, 6))
	}

	fn revoke_packet() -> Weight {
		Weight::from_parts(40_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(4, 4))
	}

	fn restore_packet() -> Weight {
		Weight::from_parts(40_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(4, 4))
	}

	fn remove_packet() -> Weight {
		Weight::from_parts(45_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(4, 3))
	}
}
