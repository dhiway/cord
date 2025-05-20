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

// pallets/entity/src/mock.rs

use super::*;
use crate::{self as pallet_entity, entity::EntityInfo};
use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstU16, ConstU32},
};
use frame_system::EnsureRoot;

use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use sp_runtime::{
	traits::{IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSignature,
};

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Entity: pallet_entity,
		Identifier: pallet_identifier
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type AccountStore = System;
}

// our new runtime constants
parameter_types! {
	pub const MaxAdditionalFields: u32 = 5;
	pub const MaxDataLength: u32 = 128;
	pub const MaxSubAccounts: u32 = 2;
	pub const MaxUsernameLength: u32 = 20;
}

impl pallet_entity::Config for Test {
	/// must match the generated enum from `construct_runtime!`
	type RuntimeEvent = RuntimeEvent;
	type MaxSubAccounts = MaxSubAccounts;
	type EntityInformation = EntityInfo<MaxAdditionalFields, MaxDataLength>;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxDataLength = MaxDataLength;
	type MaxUsernameLength = MaxUsernameLength;
	/// only the superuser may force‐set or -clear
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

impl pallet_identifier::Config for Test {
	type RuntimeEvent = Event;
	type Ss58Prefix = ConstU16<29>;
	type OriginChainId = ConstU32<0>;
	type BlockNumberProvider = System;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(account(1), 100),
			(account(2), 100),
			(account(3), 100),
			(account(10), 1000),
			(account(20), 1000),
			(account(30), 1000),
		],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.register_extension(KeystoreExt::new(MemoryKeystore::new()));
	ext.execute_with(|| System::set_block_number(1));
	ext
}

/// Helper to generate AccountIds from a byte
pub fn account(n: u8) -> <Test as frame_system::Config>::AccountId {
	[n; 32].into()
}
