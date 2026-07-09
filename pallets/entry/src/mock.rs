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

use super::*;
use crate as pallet_entry;
use frame_support::{derive_impl, parameter_types};
use sp_runtime::BuildStorage;

pub type AccountId = u64;
pub(crate) type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Identifier: pallet_doken,
		Profile: pallet_profile,
		Registry: pallet_registry,
		Entry: pallet_entry,
	}
);

parameter_types! {
	pub const SS58Prefix: u8 = 29;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountId = AccountId;
	type SS58Prefix = SS58Prefix;
}

parameter_types! {
	pub const MaxDataKeyLength: u8 = 128;
	pub const MaxDataValueLength: u32 = 1 * 1024; //1KB
}

impl pallet_doken::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberProvider = frame_system::Pallet<Test>;
}

impl pallet_profile::Config for Test {
	type MaxDataKeyLength = MaxDataKeyLength;
	type MaxDataValueLength = MaxDataValueLength;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxRegistryBlobSize: u32 = 4 * 1024;
	pub const MaxEncodedInputLength: u32 = 256;
}

impl pallet_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxRegistryBlobSize = MaxRegistryBlobSize;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxRegistryEntryBlobSize: u32 = 4 * 1024; // 4KB in bytes
}

impl pallet_entry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxEncodedInputLength = MaxEncodedInputLength;
	type MaxRegistryEntryBlobSize = MaxRegistryEntryBlobSize;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| frame_system::Pallet::<Test>::set_block_number(1));
	ext
}
