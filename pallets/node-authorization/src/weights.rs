// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Autogenerated weights for pallet_node_authorization

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn add_well_known_node() -> Weight;
	fn remove_well_known_node() -> Weight;
	fn swap_well_known_node() -> Weight;
	fn transfer_node() -> Weight;
	fn add_connection() -> Weight;
	fn remove_connection() -> Weight;
}

impl WeightInfo for () {
	fn add_well_known_node() -> Weight { Weight::from_parts(50_000_000, 0) }
	fn remove_well_known_node() -> Weight { Weight::from_parts(50_000_000, 0) }
	fn swap_well_known_node() -> Weight { Weight::from_parts(50_000_000, 0) }
	fn transfer_node() -> Weight { Weight::from_parts(50_000_000, 0) }
	fn add_connection() -> Weight { Weight::from_parts(50_000_000, 0) }
	fn remove_connection() -> Weight { Weight::from_parts(50_000_000, 0) }
}
