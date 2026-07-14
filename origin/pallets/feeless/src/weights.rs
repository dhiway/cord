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
use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use frame_system::Config;

pub trait WeightInfo {
	fn add_feeless_account() -> Weight;
	fn remove_feeless_account() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: Config> WeightInfo for SubstrateWeight<T> {
	fn add_feeless_account() -> Weight {
		Weight::from_parts(12_000_000, 4_000).saturating_add(T::DbWeight::get().reads_writes(1, 1))
	}

	fn remove_feeless_account() -> Weight {
		Weight::from_parts(15_000_000, 4_000).saturating_add(T::DbWeight::get().reads_writes(1, 2))
	}
}

impl WeightInfo for () {
	fn add_feeless_account() -> Weight {
		Weight::from_parts(12_000_000, 4_000)
			.saturating_add(RocksDbWeight::get().reads_writes(1, 1))
	}

	fn remove_feeless_account() -> Weight {
		Weight::from_parts(15_000_000, 4_000)
			.saturating_add(RocksDbWeight::get().reads_writes(1, 2))
	}
}
