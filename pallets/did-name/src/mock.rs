// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
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
use self::did_name::AsciiDidName;
use super::*;
use crate as pallet_did_name;
use crate::Config;
use cord_utilities::mock::{mock_origin, SubjectId};
use frame_support::{derive_impl, parameter_types};
use sp_runtime::{
	traits::{IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSignature,
};

use frame_system::EnsureRoot;

type Signature = MultiSignature;
type AccountPublic = <Signature as Verify>::Signer;
type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub(crate) type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test{
		System: frame_system,
		DidName: pallet_did_name,
		MockOrigin: mock_origin,
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

pub(crate) type TestDidName = AsciiDidName<Test>;
pub(crate) type TestDidNameOwner = SubjectId;
pub(crate) type TestDidNamePayer = AccountId;
pub(crate) type TestOwnerOrigin =
	mock_origin::EnsureDoubleOrigin<TestDidNamePayer, TestDidNameOwner>;
pub(crate) type TestOriginSuccess = mock_origin::DoubleOrigin<TestDidNamePayer, TestDidNameOwner>;
pub(crate) type TestBanOrigin = EnsureRoot<AccountId>;

parameter_types! {
	pub const MaxNameLength: u32 = 64;
	pub const MinNameLength: u32 = 3;
	pub const MaxPrefixLength: u32 = 54;
}

impl Config for Test {
	type BanOrigin = TestBanOrigin;
	type EnsureOrigin = TestOwnerOrigin;
	type OriginSuccess = TestOriginSuccess;
	type RuntimeEvent = RuntimeEvent;
	type MaxNameLength = MaxNameLength;
	type MinNameLength = MinNameLength;
	type MaxPrefixLength = MaxPrefixLength;
	type DidName = TestDidName;
	type DidNameOwner = TestDidNameOwner;
	type WeightInfo = ();
}

impl mock_origin::Config for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type AccountId = AccountId;
	type SubjectId = SubjectId;
}

#[allow(dead_code)]
pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	#[cfg(feature = "runtime-benchmarks")]
	let keystore = sp_keystore::testing::MemoryKeystore::new();
	#[cfg(feature = "runtime-benchmarks")]
	ext.register_extension(sp_keystore::KeystoreExt(sp_std::sync::Arc::new(keystore)));
	ext.execute_with(|| System::set_block_number(1));
	ext
}
