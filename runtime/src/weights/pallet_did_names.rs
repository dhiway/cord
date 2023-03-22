// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) 2023 Dhiway.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;


/// Weights for pallet_web3_names using the Substrate node and recommended hardware.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_did_names::WeightInfo for WeightInfo<T> {
	// Storage: DidNames Names (r:1 w:1)
	// Storage: DidNames Owner (r:1 w:1)
	// Storage: DidNames Banned (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	/// The range of component `n` is `[3, 32]`.
	fn register(_n: u32, ) -> Weight {
		Weight::from_ref_time(79_918_651 as u64)
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: DidNames Names (r:1 w:1)
	// Storage: DidNames Owner (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	fn release() -> Weight {
		Weight::from_ref_time(115_743_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: DidNames Banned (r:1 w:1)
	// Storage: DidNames Owner (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: DidNames Names (r:0 w:1)
	/// The range of component `n` is `[3, 32]`.
	fn ban(_n: u32, ) -> Weight {
		Weight::from_ref_time(65_086_706 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: DidNames Banned (r:1 w:1)
	/// The range of component `n` is `[3, 32]`.
	fn unban(_n: u32, ) -> Weight {
		Weight::from_ref_time(38_246_474 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}


