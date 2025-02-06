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
#![allow(non_camel_case_types)]

use super::*;
use crate as pallet_registry;
use frame_support::{derive_impl, parameter_types};
use frame_system as system;
use sp_runtime::BuildStorage;

type Block = system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: system,
		Registry: pallet_registry,
		Identifier: cord_uri,
	}
);

parameter_types! {
	pub const SS58Prefix: u8 = 29;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountData = ();
	type SS58Prefix = SS58Prefix;
}

impl pallet_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl cord_uri::Config for Test {
	type BlockNumberProvider = frame_system::Pallet<Test>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| system::Pallet::<Test>::set_block_number(1));
	ext
}
