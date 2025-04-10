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
use cord_utilities::mock::{mock_origin, SubjectId};
use frame_support::{derive_impl, parameter_types};
use pallet_namespace::IsPermissioned;

use frame_system::EnsureRoot;
use sp_runtime::{
	traits::{IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSignature,
};

type Signature = MultiSignature;
type AccountPublic = <Signature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub(crate) type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Identifier: identifier,
		MockOrigin: mock_origin,
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
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Block = Block;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type SS58Prefix = SS58Prefix;
}

impl mock_origin::Config for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type AccountId = AccountId;
	type SubjectId = SubjectId;
}

parameter_types! {
	pub const MaxDataKeyLength: u8 = 128;
	pub const MaxDataValueLength: u32 = 1 * 1024; //1KB
}

impl pallet_profile::Config for Test {
	type MaxDataKeyLength = MaxDataKeyLength;
	type MaxDataValueLength = MaxDataValueLength;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl pallet_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
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

impl cord_uri::Config for Test {
	type BlockNumberProvider = frame_system::Pallet<Test>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| system::Pallet::<Test>::set_block_number(1));
	ext
}
