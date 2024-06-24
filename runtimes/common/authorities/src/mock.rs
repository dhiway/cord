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
//
use super::*;
use crate::{self as cord_authority_membership};
use frame_support::{derive_impl, parameter_types};
use sp_state_machine::BasicExternalities;
use std::collections::BTreeMap;

use frame_system::{pallet_prelude::BlockNumberFor, EnsureRoot};
use pallet_offences::{traits::OnOffenceHandler, SlashStrategy};
use pallet_session::ShouldEndSession;
use sp_core::crypto::key_types::DUMMY;
use sp_runtime::{
	impl_opaque_keys,
	testing::UintAuthorityId,
	traits::{ConvertInto, IsMember, OpaqueKeys},
	BuildStorage, KeyTypeId,
};
use sp_staking::offence::OffenceDetails;

type AccountId = u64;
type Block = frame_system::mocking::MockBlock<Test>;

impl_opaque_keys! {
	pub struct MockSessionKeys {
		pub dummy: UintAuthorityId,
	}
}

impl From<UintAuthorityId> for MockSessionKeys {
	fn from(dummy: UintAuthorityId) -> Self {
		Self { dummy }
	}
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Session: pallet_session,
		NetworkMembership: pallet_network_membership,
		AuthorityMembership: cord_authority_membership,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = u64;
	type Block = Block;
	type AccountData = ();
}

pub struct TestSessionHandler;
impl pallet_session::SessionHandler<u64> for TestSessionHandler {
	const KEY_TYPE_IDS: &'static [KeyTypeId] = &[DUMMY];

	fn on_new_session<Ks: OpaqueKeys>(
		_changed: bool,
		_validators: &[(u64, Ks)],
		_queued_validators: &[(u64, Ks)],
	) {
	}

	fn on_disabled(_validator_index: u32) {}

	fn on_genesis_session<Ks: OpaqueKeys>(_validators: &[(u64, Ks)]) {}
}

const SESSION_LENGTH: u64 = 5;
pub struct TestShouldEndSession;
impl ShouldEndSession<u64> for TestShouldEndSession {
	fn should_end_session(now: u64) -> bool {
		now % SESSION_LENGTH == 0
	}
}

impl pallet_session::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = u64;
	type ValidatorIdOf = ConvertInto;
	type ShouldEndSession = TestShouldEndSession;
	type NextSessionRotation = ();
	type SessionManager = AuthorityMembership;
	type SessionHandler = TestSessionHandler;
	type Keys = MockSessionKeys;
	type WeightInfo = ();
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

pub struct FullIdentificationOfImpl;
impl sp_runtime::traits::Convert<AccountId, Option<()>> for FullIdentificationOfImpl {
	fn convert(_: AccountId) -> Option<()> {
		Some(())
	}
}
impl pallet_session::historical::Config for Test {
	type FullIdentification = ();
	type FullIdentificationOf = FullIdentificationOfImpl;
}

pub struct TestIsNetworkMember;
#[cfg(not(feature = "runtime-benchmarks"))]
impl IsMember<u64> for TestIsNetworkMember {
	fn is_member(member_id: &u64) -> bool {
		member_id % 3 == 0
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl IsMember<<Test as frame_system::Config>::AccountId> for TestIsNetworkMember {
	fn is_member(_account_id: &<Test as frame_system::Config>::AccountId) -> bool {
		// For benchmarking, assume all generated accounts are members
		true
	}
}

impl cord_authority_membership::Config for Test {
	type AuthorityMembershipOrigin = EnsureRoot<u64>;
	type RuntimeEvent = RuntimeEvent;
	type MinAuthorities = ConstU32<1>;
	type IsMember = TestIsNetworkMember;
}

parameter_types! {
	pub static Validators: Vec<u64> = vec![3,6,9];
	pub static NextValidators: Vec<u64> = vec![3,6,9];
	pub static Authorities: Vec<UintAuthorityId> =
		vec![UintAuthorityId(3), UintAuthorityId(6), UintAuthorityId(9)];
	pub static ValidatorAccounts: BTreeMap<u64, u64> = BTreeMap::new();

}

pub fn authorities() -> Vec<UintAuthorityId> {
	Authorities::get().to_vec()
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext(authorities_len: u64) -> sp_io::TestExternalities {
	let keys: Vec<_> = (1..=authorities_len)
		.map(|i| (i * 3, i * 3, UintAuthorityId(i * 3).into()))
		.collect();

	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	BasicExternalities::execute_with_storage(&mut t, || {
		for (ref k, ..) in &keys {
			frame_system::Pallet::<Test>::inc_providers(k);
		}
		// Some dedicated test account
		frame_system::Pallet::<Test>::inc_providers(&12);
		frame_system::Pallet::<Test>::inc_providers(&15);
	});
	cord_authority_membership::GenesisConfig::<Test> {
		initial_authorities: keys.iter().map(|x| x.0).collect::<Vec<_>>(),
	}
	.assimilate_storage(&mut t)
	.unwrap();
	pallet_session::GenesisConfig::<Test> { keys }
		.assimilate_storage(&mut t)
		.unwrap();
	let v = NextValidators::get().iter().map(|&i| (i, i)).collect();
	ValidatorAccounts::mutate(|m| *m = v);
	sp_io::TestExternalities::new(t)
}

pub fn run_to_block(n: u64) {
	while System::block_number() < n {
		Session::on_finalize(System::block_number());
		AuthorityMembership::on_initialize(System::block_number());
		System::on_finalize(System::block_number());
		System::reset_events();
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
		AuthorityMembership::on_initialize(System::block_number());
		Session::on_initialize(System::block_number());
	}
}

pub(crate) fn on_offence(
	offenders: &[OffenceDetails<
		AccountId,
		pallet_session::historical::IdentificationTuple<Test>,
	>],
	slash_strategy: SlashStrategy,
) {
	AuthorityMembership::on_offence(offenders, slash_strategy, 0);
}
