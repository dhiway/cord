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

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;
use frame_system::Config;

/// Weights: TBD.
/// The `()` impl provides zero weights for tests/dev.
pub trait WeightInfo {
	fn nominate() -> Weight;
	fn remove() -> Weight;
	fn set_invulnerables(_n: u32) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: Config> WeightInfo for SubstrateWeight<T> {
	fn nominate() -> Weight {
		Weight::from_parts(0, 0)
	}
	fn remove() -> Weight {
		Weight::from_parts(0, 0)
	}
	fn set_invulnerables(_n: u32) -> Weight {
		Weight::from_parts(0, 0)
	}
}

impl WeightInfo for () {
	fn nominate() -> Weight {
		Weight::from_parts(0, 0)
	}
	fn remove() -> Weight {
		Weight::from_parts(0, 0)
	}
	fn set_invulnerables(_n: u32) -> Weight {
		Weight::from_parts(0, 0)
	}
}
