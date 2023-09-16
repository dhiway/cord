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

// use super::*;
use crate::{self as pallet_network_membership};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64, OnFinalize, OnInitialize},
};
use frame_system::EnsureRoot;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSignature,
};

use maplit::btreemap;
use network_membership::MemberData;
type Hash = sp_core::H256;
type Balance = u128;
type BlockNumber = u64;
type Signature = MultiSignature;
type AccountPublic = <Signature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

construct_runtime!(
	pub enum Test where
	Block = frame_system::mocking::MockBlock<Test>,
	NodeBlock = frame_system::mocking::MockBlock<Test>,
	UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		NetworkMembership: pallet_network_membership::{Pallet, Call, Storage, Event<T>, Config<T>},
	}
);

parameter_types! {
	pub const SS58Prefix: u8 = 29;
}
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<2>;
}

parameter_types! {
	pub const MembershipPeriod: BlockNumber = 5;
	pub const MaxMembersPerBlock: u32 = 5;
}

impl pallet_network_membership::Config for Test {
	type NetworkMembershipOrigin = EnsureRoot<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type MembershipPeriod = MembershipPeriod;
	type MaxMembersPerBlock = MaxMembersPerBlock;
	type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext(
	gen_conf: pallet_network_membership::GenesisConfig<Test>,
) -> sp_io::TestExternalities {
	GenesisConfig { system: SystemConfig::default(), network_membership: gen_conf }
		.build_storage()
		.unwrap()
		.into()
}

pub fn run_to_block(n: u64) {
	while System::block_number() < n {
		NetworkMembership::on_finalize(System::block_number());
		System::on_finalize(System::block_number());
		System::reset_events();
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
		NetworkMembership::on_initialize(System::block_number());
	}
}

pub(crate) fn default_gen_conf() -> NetworkMembershipConfig {
	NetworkMembershipConfig {
		members: btreemap![
			 AccountId::new([11u8; 32]) => MemberData {
				expire_on: 3,
			}
		],
	}
}
