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

use crate as pallet_namespace;
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
		NameSpace: pallet_namespace,
		Identifier: identifier,
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

impl mock_origin::Config for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type AccountId = AccountId;
	type SubjectId = SubjectId;
}

pub struct NetworkPermission;
impl IsPermissioned for NetworkPermission {
	fn is_permissioned() -> bool {
		true
	}
}

parameter_types! {
	#[derive(Debug, Clone)]
	pub const MaxNameSpaceDelegates: u32 = 5u32;
	pub const MaxNameSpaceBlobSize: u32 = 4u32 * 1024;
}

impl pallet_namespace::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ChainSpaceOrigin = EnsureRoot<AccountId>;
	type NetworkPermission = NetworkPermission;
	type MaxNameSpaceDelegates = MaxNameSpaceDelegates;
	type MaxNameSpaceBlobSize = MaxNameSpaceBlobSize;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxEventsHistory: u32 = 6u32;
}

impl identifier::Config for Test {
	type MaxEventsHistory = MaxEventsHistory;
}

parameter_types! {
	storage NameSpaceEvents: u32 = 0;
}

/// All events of this pallet.
pub fn space_events_since_last_call() -> Vec<super::Event<Test>> {
	let events = System::events()
		.into_iter()
		.map(|r| r.event)
		.filter_map(|e| if let RuntimeEvent::NameSpace(inner) = e { Some(inner) } else { None })
		.collect::<Vec<_>>();
	let already_seen = NameSpaceEvents::get();
	NameSpaceEvents::set(&(events.len() as u32));
	events.into_iter().skip(already_seen as usize).collect()
}

#[allow(dead_code)]
pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let t: sp_runtime::Storage =
		frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	#[cfg(feature = "runtime-benchmarks")]
	let keystore = sp_keystore::testing::MemoryKeystore::new();
	#[cfg(feature = "runtime-benchmarks")]
	ext.register_extension(sp_keystore::KeystoreExt(sync::Arc::new(keystore)));
	ext.execute_with(|| System::set_block_number(1));
	ext
}
