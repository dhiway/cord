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
use crate as pallet_stream;
use cord_utilities::mock::{mock_origin, SubjectId};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64},
};

//use sp_core::{ecdsa, ed25519, sr25519, Pair};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	MultiSignature,
};

//use sp_std::vec::Vec;

type Hash = sp_core::H256;
type Balance = u128;
type Signature = MultiSignature;
type AccountPublic = <Signature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

pub(crate) type BlockNumber = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
	pub enum Test where
	Block = Block,
	NodeBlock = Block,
	UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Schema:pallet_schema::{Pallet, Call, Storage, Event<T>},
		Registry: pallet_registry::{Pallet, Storage, Call,Event<T>},
		Stream: pallet_stream::{Pallet, Storage, Call,Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
		MockOrigin: mock_origin::{Pallet, Origin<T>},

	}
);

parameter_types! {
	pub const SS58Prefix: u8 = 29;
	pub const BlockHashCount: u64 = 250;
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
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<2>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 500;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
}

impl mock_origin::Config for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type AccountId = AccountId;
	type SubjectId = SubjectId;
}

parameter_types! {
	pub const Fee: Balance = 500;
	#[derive(Debug, Clone)]
	pub const MaxStreamCommits: u32 = 5u32;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type EnsureOrigin = mock_origin::EnsureDoubleOrigin<AccountId, SubjectId>;
	type OriginSuccess = mock_origin::DoubleOrigin<AccountId, SubjectId>;
	type MaxStreamCommits = MaxStreamCommits;
	type WeightInfo = weights::SubstrateWeight<Test>;
}

parameter_types! {
	#[derive(Debug, Clone)]
	pub const MaxEncodedRegistryLength: u32 = 1024u32;
	pub const MaxRegistryAuthorities: u32 = 3u32;
	#[derive(Debug, Clone)]
	pub const MaxRegistryCommitActions: u32 = 5u32;
}

impl pallet_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type EnsureOrigin = mock_origin::EnsureDoubleOrigin<AccountId, SubjectId>;
	type OriginSuccess = mock_origin::DoubleOrigin<AccountId, SubjectId>;
	type RegistryCreatorId = SubjectId;
	type MaxEncodedRegistryLength = MaxEncodedRegistryLength;
	type MaxRegistryAuthorities = MaxRegistryAuthorities;
	type MaxRegistryCommitActions = MaxRegistryCommitActions;
	type WeightInfo = pallet_registry::weights::SubstrateWeight<Test>;
}

parameter_types! {
	pub const MaxEncodedSchemaLength: u32 = 102_400;
}

impl pallet_schema::Config for Test {
	type SchemaCreatorId = SubjectId;
	type EnsureOrigin = mock_origin::EnsureDoubleOrigin<AccountId, SubjectId>;
	type OriginSuccess = mock_origin::DoubleOrigin<AccountId, SubjectId>;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type MaxEncodedSchemaLength = MaxEncodedSchemaLength;
}

// parameter_types! {
// 	storage StreamEvents: u32 = 0;
// }

/// All events of this pallet.
// pub fn stream_events_since_last_call() -> Vec<super::Event<Test>> {
// 	let events = System::events()
// 		.into_iter()
// 		.map(|r| r.event)
// 		.filter_map(|e| if let RuntimeEvent::Registry(inner) = e { Some(inner) } else { None })
// 		.collect::<Vec<_>>();
// 	let already_seen = StreamEvents::get();
// 	StreamEvents::set(&(events.len() as u32));
// 	events.into_iter().skip(already_seen as usize).collect()
// }

#[allow(dead_code)]
pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	#[cfg(feature = "runtime-benchmarks")]
	let keystore = sp_keystore::testing::KeyStore::new();
	#[cfg(feature = "runtime-benchmarks")]
	ext.register_extension(sp_keystore::KeystoreExt(sp_std::sync::Arc::new(keystore)));
	ext.execute_with(|| System::set_block_number(1));
	ext
}
