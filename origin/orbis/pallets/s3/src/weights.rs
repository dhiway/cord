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

//! Weight interface for the native Orbis S3 registry.

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

pub trait WeightInfo {
	fn create_bucket(name_bytes: u32) -> Weight;
	fn set_controller() -> Weight;
	fn transfer_bucket() -> Weight;
	fn set_archived() -> Weight;
	fn set_versioning() -> Weight;
	fn put_object(key_bytes: u32, history_items: u32) -> Weight;
	fn delete_object(key_bytes: u32, history_items: u32) -> Weight;
	fn delete_bucket(indexed_keys: u32) -> Weight;
}

impl WeightInfo for () {
	fn create_bucket(name: u32) -> Weight {
		Weight::from_parts(35_000_000 + 5_000 * name as u64, 7_000)
	}
	fn set_controller() -> Weight {
		Weight::from_parts(25_000_000, 6_000)
	}
	fn transfer_bucket() -> Weight {
		Weight::from_parts(45_000_000, 10_000)
	}
	fn set_archived() -> Weight {
		Weight::from_parts(20_000_000, 5_000)
	}
	fn set_versioning() -> Weight {
		Weight::from_parts(20_000_000, 5_000)
	}
	fn put_object(key: u32, history: u32) -> Weight {
		Weight::from_parts(55_000_000 + 5_000 * key as u64 + 200_000 * history as u64, 12_000)
	}
	fn delete_object(key: u32, history: u32) -> Weight {
		Weight::from_parts(48_000_000 + 5_000 * key as u64 + 200_000 * history as u64, 11_000)
	}
	fn delete_bucket(keys: u32) -> Weight {
		Weight::from_parts(45_000_000 + 2_000_000 * keys as u64, 10_000)
	}
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn create_bucket(name: u32) -> Weight {
		<() as WeightInfo>::create_bucket(name)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	fn set_controller() -> Weight {
		<() as WeightInfo>::set_controller()
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn transfer_bucket() -> Weight {
		<() as WeightInfo>::transfer_bucket()
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(5))
	}
	fn set_archived() -> Weight {
		<() as WeightInfo>::set_archived()
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn set_versioning() -> Weight {
		<() as WeightInfo>::set_versioning()
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn put_object(key: u32, history: u32) -> Weight {
		<() as WeightInfo>::put_object(key, history)
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	fn delete_object(key: u32, history: u32) -> Weight {
		<() as WeightInfo>::delete_object(key, history)
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	fn delete_bucket(keys: u32) -> Weight {
		<() as WeightInfo>::delete_bucket(keys)
			.saturating_add(T::DbWeight::get().reads(5 + keys as u64))
			.saturating_add(T::DbWeight::get().writes(5 + keys as u64))
	}
}
