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

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_author::WeightInfo for WeightInfo<T> {
	fn add(l: u32, ) -> Weight {
		Weight::from_ref_time(50_978_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(5_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(l as u64))
			.saturating_add(T::DbWeight::get().writes(l as u64))
	}
	fn remove(l: u32,) -> Weight {
		Weight::from_ref_time(50_978_000 as u64)
			.saturating_add(Weight::from_ref_time(5_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(l as u64))
			.saturating_add(T::DbWeight::get().writes(l as u64))
	}
}

