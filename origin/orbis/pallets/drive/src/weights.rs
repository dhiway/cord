// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

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
