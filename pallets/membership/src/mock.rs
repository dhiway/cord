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
use crate as pallet_membership;

use frame_support::{derive_impl, ord_parameter_types, parameter_types, traits::ConstU32};
use frame_system::{pallet_prelude::BlockNumberFor, EnsureRoot, EnsureSignedBy};
use sp_runtime::{bounded_vec, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Membership: pallet_membership,
		NetworkMembership: pallet_network_membership,
	}
);

parameter_types! {
	pub static Members: Vec<u64> = vec![];
	pub static Prime: Option<u64> = None;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
}
ord_parameter_types! {
	pub const One: u64 = 1;
	pub const Two: u64 = 2;
	pub const Three: u64 = 3;
	pub const Four: u64 = 4;
	pub const Five: u64 = 5;
}

pub struct TestChangeMembers;
impl ChangeMembers<u64> for TestChangeMembers {
	fn change_members_sorted(incoming: &[u64], outgoing: &[u64], new: &[u64]) {
		let mut old_plus_incoming = Members::get();
		old_plus_incoming.extend_from_slice(incoming);
		old_plus_incoming.sort();
		let mut new_plus_outgoing = new.to_vec();
		new_plus_outgoing.extend_from_slice(outgoing);
		new_plus_outgoing.sort();
		assert_eq!(old_plus_incoming, new_plus_outgoing);

		Members::set(new.to_vec());
		Prime::set(None);
	}
	fn set_prime(who: Option<u64>) {
		Prime::set(who);
	}
	fn get_prime() -> Option<u64> {
		Prime::get()
	}
}

impl InitializeMembers<u64> for TestChangeMembers {
	fn initialize_members(members: &[u64]) {
		MEMBERS.with(|m| *m.borrow_mut() = members.to_vec());
	}
}
parameter_types! {
	pub const MembershipPeriod: BlockNumberFor<Test> = 5;
	pub const MaxMembersPerBlock: u32 = 5;
}

impl pallet_network_membership::Config for Test {
	type NetworkMembershipOrigin = EnsureRoot<u64>;
	type RuntimeEvent = RuntimeEvent;
	type MembershipPeriod = MembershipPeriod;
	type MaxMembersPerBlock = MaxMembersPerBlock;
	type WeightInfo = ();
}

pub struct TestIsNetworkMember;
#[cfg(not(feature = "runtime-benchmarks"))]
impl IsMember<u64> for TestIsNetworkMember {
	fn is_member(member_id: &u64) -> bool {
		(1..=40).contains(member_id)
	}
}

#[cfg(feature = "runtime-benchmarks")]
//#[cfg(feature = "tests")]
impl IsMember<<Test as frame_system::Config>::AccountId> for TestIsNetworkMember {
	fn is_member(_account_id: &<Test as frame_system::Config>::AccountId) -> bool {
		// For benchmarking, assume all generated accounts are members
		true
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type IsMember = TestIsNetworkMember;
	type AddOrigin = EnsureSignedBy<One, u64>;
	type RemoveOrigin = EnsureSignedBy<Two, u64>;
	type SwapOrigin = EnsureSignedBy<Three, u64>;
	type ResetOrigin = EnsureSignedBy<Four, u64>;
	type PrimeOrigin = EnsureSignedBy<Five, u64>;
	type MembershipInitialized = TestChangeMembers;
	type MembershipChanged = TestChangeMembers;
	type MaxMembers = ConstU32<10>;
	type WeightInfo = ();
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	// We use default for brevity, but you can configure as desired if needed.
	pallet_membership::GenesisConfig::<Test> {
		members: bounded_vec![10, 20, 30],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
}

#[cfg(feature = "runtime-benchmarks")]
pub(crate) fn new_bench_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}

#[cfg(feature = "runtime-benchmarks")]
pub(crate) fn clean() {
	Members::set(vec![]);
	Prime::set(None);
}
