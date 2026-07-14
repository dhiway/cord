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

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

/// Initial executable weights. P7 replaces these with generated benchmark output.
pub trait WeightInfo {
	fn create_drive() -> Weight;
	fn update_root() -> Weight;
	fn set_controller() -> Weight;
	fn transfer_drive() -> Weight;
	fn archive_drive() -> Weight;
}

impl WeightInfo for () {
	fn create_drive() -> Weight {
		Weight::from_parts(40_000_000, 10_000)
	}
	fn update_root() -> Weight {
		Weight::from_parts(25_000_000, 7_000)
	}
	fn set_controller() -> Weight {
		Weight::from_parts(30_000_000, 8_000)
	}
	fn transfer_drive() -> Weight {
		Weight::from_parts(40_000_000, 11_000)
	}
	fn archive_drive() -> Weight {
		Weight::from_parts(20_000_000, 6_000)
	}
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn create_drive() -> Weight {
		Weight::from_parts(40_000_000, 10_000)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	fn update_root() -> Weight {
		Weight::from_parts(25_000_000, 7_000)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn set_controller() -> Weight {
		Weight::from_parts(30_000_000, 8_000)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn transfer_drive() -> Weight {
		Weight::from_parts(40_000_000, 11_000)
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(5))
	}
	fn archive_drive() -> Weight {
		Weight::from_parts(20_000_000, 6_000)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
