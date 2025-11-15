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
use crate::{self as pallet_entity, entity::EntityInfo};
use alloc::collections::BTreeMap;
use core::cell::RefCell;
use frame_support::{derive_impl, parameter_types};
use frame_system::EnsureRoot;
use sp_core::{sr25519, Pair};
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use sp_runtime::{
	traits::{IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSignature, MultiSigner,
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
		Token: pallet_token
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
	pub const MaxRawDataLength: u32 = 4096;
	pub const MaxAdditionalAttributes: u32 = 32;
	pub const MaxLinkedAccounts: u32 = 2;
	pub const MaxEntityNymLength: u32 = 20;
	pub const MaxTokenAuthorizationLen: u32 = 256;
	pub const MaxTokenTimelineViewResults: u32 = 32;
	pub const DefaultTokenTimelineViewResults: u32 = 16;
	pub const MaxTokenAuthorizationTTL: u32 = 30;
	pub const MaxEntityAuthorizationLen: u32 = 256;
	pub const MaxEntityAuthorizationTTL: u32 = 30;
}

thread_local! {
	pub(crate) static ACCOUNT_KEYS: RefCell<BTreeMap<AccountId, sr25519::Pair>> =
		RefCell::new(BTreeMap::new());
}

impl pallet_entity::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Token = Token;
	type MaxLinkedAccounts = MaxLinkedAccounts;
	type EntityInfoPacket = EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>;
	type MaxRawDataLength = MaxRawDataLength;
	type MaxAdditionalAttributes = MaxAdditionalAttributes;
	type MaxEntityNymLength = MaxEntityNymLength;
	type MaxAuthorizationLen = MaxEntityAuthorizationLen;
	type MaxAuthorizationTTL = MaxEntityAuthorizationTTL;
	type Feeless = ();
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

impl pallet_token::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberProvider = System;
	type MaxAuthorizationLen = MaxTokenAuthorizationLen;
	type MaxTimelineViewResults = MaxTokenTimelineViewResults;
	type DefaultTimelineViewResults = DefaultTokenTimelineViewResults;
	type MaxAuthorizationTTL = MaxTokenAuthorizationTTL;
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
	let pair = sr25519::Pair::from_seed(&[n; 32]);
	let signer = MultiSigner::from(pair.public());
	let account: AccountId = signer.into_account();
	store_account_pair(account.clone(), pair);
	account
}

pub fn store_account_pair(account: AccountId, pair: sr25519::Pair) {
	ACCOUNT_KEYS.with(|keys| {
		keys.borrow_mut().insert(account, pair);
	});
}
