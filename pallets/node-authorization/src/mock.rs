// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Test environment for node-authorization pallet.

use super::*;
use crate as pallet_node_authorization;

use frame_support::{
	ord_parameter_types,
	traits::{ConstU32, ConstU64, GenesisBuild},
};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	BoundedVec,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		NodeAuthorization: pallet_node_authorization::{
			Pallet, Call, Storage, Config<T>, Event<T>,
		},
	}
);

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type RuntimeCall = RuntimeCall;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

ord_parameter_types! {
	pub const One: u64 = 1;
	pub const Two: u64 = 2;
	pub const Three: u64 = 3;
	pub const Four: u64 = 4;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxWellKnownNodes = ConstU32<5>;
	type MaxNodeIdLength = ConstU32<3>;
	type MaxPeerIdLength = ConstU32<3>;
	type NodeAuthorizationOrigin = EnsureSignedBy<One, u64>;
	type WeightInfo = ();
}

pub fn test_node(input: &str) -> Vec<u8> {
	let base58_encoded = bs58::encode(input).into_string();

	base58_encoded.as_bytes().to_vec()
}

pub fn genesis_node(id: u8) -> PeerId {
	PeerId(vec![id])
}

pub fn test_node_id(id: u8) -> BoundedVec<u8, ConstU32<3>> {
	BoundedVec::try_from(vec![id]).unwrap()
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	pallet_node_authorization::GenesisConfig::<Test> {
		nodes: vec![(genesis_node(10), 10), (genesis_node(20), 20), (genesis_node(30), 30)],
		// nodes: vec![(PeerId(bs58::decode("10").into_vec().unwrap()), 10)],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
}
