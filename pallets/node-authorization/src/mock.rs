// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
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

//! Test environment for node-authorization pallet.

use super::*;
use crate as pallet_node_authorization;
use crate::{Config, NodeId};

use frame_support::{
	construct_runtime, derive_impl, ord_parameter_types,
	traits::{ConstU32, ConstU64},
};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BoundedVec, BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
	pub enum Test
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		NodeAuthorization: pallet_node_authorization::{
			Pallet, Call, Storage, Config<T>, Event<T>,
		},
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type Nonce = u64;
	type Hash = H256;
	type RuntimeCall = RuntimeCall;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
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
	type MaxWellKnownNodes = ConstU32<4>;
	type MaxNodeIdLength = ConstU32<54>;
	type MaxPeerIdLength = ConstU32<128>;
	type NodeAuthorizationOrigin = EnsureSignedBy<One, u64>;
	type WeightInfo = ();
}

// Constants for test parameters
pub const TEST_NODE_1: &str = "12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2";
pub const TEST_NODE_2: &str = "12D3KooWQYV9dGMFoRzNStwpXztXaBUjtPqi6aU76ZgUriHhKust";
pub const TEST_NODE_3: &str = "12D3KooWJvyP3VJYymTqG7eH4PM5rN4T2agk5cdNCfNymAqwqcvZ";
pub const TEST_NODE_4: &str = "12D3KooWFMNXNsQtDX5CS9Fw25WcH8JRwQALmKm1XJ5dKEZgencc";
pub const TEST_NODE_5: &str = "12D3KooWCzvmH8ehWaPttWVzj1ERB1FvMqVcqfDRvwLqzJDAqBih";
pub const TEST_NODE_6: &str = "12D3KooW9tw9VUZkysjCwBpJo8ArH9TsgUWZhFqqUtii6VfTCvL6";
pub const TEST_NODE_LEN: &str = "12D3KooWCzvmH8ehWaPttWVzj1ERB1FvMqVcqfDRvwLqzJDAqBih8123w";
pub const TEST_NODE_7: &str = "13C3KooWCzVmH8ehW/PttWVzj1ERB1Fv0qVcqf1RvwLQzJDAqBih";

pub fn test_node(input: &str) -> NodeId {
	input.as_bytes().to_vec()
}

pub fn generate_peer(input: &str) -> PeerId {
	let node_id = test_node(input);
	Pallet::<Test>::generate_peer_id(&node_id).unwrap()
}

pub fn genesis_node(id: u8) -> PeerId {
	PeerId(vec![id])
}

pub fn test_node_id(id: &str) -> BoundedVec<u8, ConstU32<54>> {
	let node_id = id.as_bytes().to_vec();
	BoundedVec::try_from(node_id).unwrap()
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	pallet_node_authorization::GenesisConfig::<Test> {
		nodes: vec![
			(test_node(TEST_NODE_1), 10),
			(test_node(TEST_NODE_2), 20),
			(test_node(TEST_NODE_3), 30),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
}
