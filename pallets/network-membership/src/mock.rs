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

use crate::{self as pallet_network_membership};
use frame_support::{
	derive_impl, parameter_types,
	traits::{OnFinalize, OnInitialize},
};
use frame_system::{pallet_prelude::BlockNumberFor, EnsureRoot};
use sp_runtime::{
	traits::{IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSignature,
};

use maplit::btreemap;
type Signature = MultiSignature;
type AccountPublic = <Signature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub(crate) type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		NetworkMembership: pallet_network_membership,
	}
);

parameter_types! {
	pub const SS58Prefix: u8 = 29;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
}

parameter_types! {
	pub const MembershipPeriod: BlockNumberFor<Test> = 5;
	pub const MaxMembersPerBlock: u32 = 5;
}

impl pallet_network_membership::Config for Test {
	type NetworkMembershipOrigin = EnsureRoot<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type MembershipPeriod = MembershipPeriod;
	type MaxMembersPerBlock = MaxMembersPerBlock;
	type WeightInfo = ();
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	// We use default for brevity, but you can configure as desired if needed.
	pallet_network_membership::GenesisConfig::<Test> {
		members: btreemap![
			 AccountId::new([11u8; 32]) => true,
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
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
