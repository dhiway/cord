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
use crate::{self as cord_authority_membership};

use alloc::vec::Vec;
use core::{cell::RefCell, cmp::max};

use frame_support::{derive_impl, parameter_types};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_session::historical as pallet_session_historical;
use sp_core::crypto::key_types::DUMMY;
use sp_io;
use sp_runtime::{
	impl_opaque_keys,
	testing::UintAuthorityId,
	traits::{ConvertInto, Dispatchable, OpaqueKeys, Zero},
	BuildStorage, KeyTypeId,
};
use sp_state_machine::BasicExternalities;

pub(crate) type AccountId = u64;
type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type Balance = u128;

parameter_types! {
	pub static ExistentialDeposit: Balance = 1;
}

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Session: pallet_session,
		Historical: pallet_session_historical,
		Balances: pallet_balances,
		AuthorityMembership: cord_authority_membership,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = sp_runtime::traits::IdentityLookup<AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type MaxLocks = frame_support::traits::ConstU32<1024>;
	type Balance = Balance;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
}

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

impl Default for MockSessionKeys {
	fn default() -> Self {
		// The macro generates `From<UintAuthorityId>` for MockSessionKeys,
		// and `UintAuthorityId(u64)` is constructible in tests.
		Self::from(UintAuthorityId(0))
	}
}

// Thread-local offline mask the tests can control.
thread_local! {
	static OFFLINE_MASK: RefCell<Vec<u32>> = RefCell::new(Vec::new());
}

/// Mark the given validator indices as offline for the next liveness check.
pub fn set_offline_idxs(idxs: &[u32]) {
	OFFLINE_MASK.with(|mask| {
		let mut m = mask.borrow_mut();
		m.clear();
		m.extend_from_slice(idxs);
	});
}

/// Clear the offline mask (everyone online).
pub fn clear_offline_mask() {
	OFFLINE_MASK.with(|mask| mask.borrow_mut().clear());
}

fn reset_test_globals() {
	clear_offline_mask();
}

/// Test liveness provider that consults the mask above.
pub struct TestLiveness;
impl crate::LivenessProvider for TestLiveness {
	fn is_online(idx: u32) -> bool {
		OFFLINE_MASK.with(|mask| !mask.borrow().contains(&idx))
	}
}

// ---------- Session config ----------
pub struct TestSessionHandler;
impl pallet_session::SessionHandler<AccountId> for TestSessionHandler {
	const KEY_TYPE_IDS: &'static [KeyTypeId] = &[DUMMY];

	fn on_new_session<Ks: OpaqueKeys>(
		_changed: bool,
		_validators: &[(AccountId, Ks)],
		_queued_validators: &[(AccountId, Ks)],
	) {
	}
	fn on_disabled(_validator_index: u32) {}
	fn on_genesis_session<Ks: OpaqueKeys>(_validators: &[(AccountId, Ks)]) {}
}

pub struct TestShouldEndSession;
impl pallet_session::ShouldEndSession<BlockNumberFor<Test>> for TestShouldEndSession {
	fn should_end_session(now: BlockNumberFor<Test>) -> bool {
		let period: BlockNumberFor<Test> = 5u32.into();
		(now % period).is_zero()
	}
}

impl pallet_session::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = AccountId;
	type ValidatorIdOf = ConvertInto;
	type ShouldEndSession = TestShouldEndSession;
	type NextSessionRotation = ();
	type SessionManager = AuthorityMembership; // use our pallet
	type SessionHandler = TestSessionHandler;
	type Keys = MockSessionKeys;
	type WeightInfo = ();
	type DisablingStrategy = ();
	type Currency = Balances;
	type KeyDeposit = ();
}

// Optional but fine to include for historical snapshots
pub struct FullIdentificationOfImpl;
impl sp_runtime::traits::Convert<AccountId, Option<()>> for FullIdentificationOfImpl {
	fn convert(_: AccountId) -> Option<()> {
		Some(())
	}
}
impl pallet_session_historical::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type FullIdentification = ();
	type FullIdentificationOf = FullIdentificationOfImpl;
}

// ---------- Pallet config ----------
parameter_types! {
	pub const MinAuthorities: u32 = 1;
	pub const MaxAuthorities: u32 = 64;
	pub const MaxInvulnerables: u32 = 8;
	pub const TargetActive: u32 = 4;
	pub const KickThresholdSessions: u32 = 2;
}

impl cord_authority_membership::Config for Test {
	// type RuntimeEvent = RuntimeEvent;
	type MinAuthorities = MinAuthorities;
	type MaxAuthorities = MaxAuthorities;
	type MaxInvulnerables = MaxInvulnerables;
	type TargetActive = TargetActive;
	type KickThresholdSessions = KickThresholdSessions;
	type AuthorityMembershipOrigin = frame_system::EnsureRoot<AccountId>;
	type Liveness = TestLiveness;
	type WeightInfo = ();
}

/// Build genesis storage with `n_authorities` (clamped to >= MinAuthorities and >= 1).
pub fn new_test_ext(n_authorities: u64) -> sp_io::TestExternalities {
	new_test_ext_with(n_authorities, &[])
}

pub fn new_test_ext_with(n_authorities: u64, invuls: &[AccountId]) -> sp_io::TestExternalities {
	// Respect the pallet's own invariant at genesis.
	let min = <Test as crate::pallet::Config>::MinAuthorities::get() as u64;
	let n = max(3, max(n_authorities, min));

	// Build session keys: (validator_id, controller_id, OpaqueKeys)
	let keys: Vec<(AccountId, AccountId, MockSessionKeys)> = (1..=n)
		.map(|i| {
			let who: AccountId = i * 3; // 3, 6, 9, ...
			(who, who, UintAuthorityId(who).into())
		})
		.collect();

	// Base system storage
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	// Ensure providers exist for those accounts
	BasicExternalities::execute_with_storage(&mut t, || {
		for (ref k, ..) in &keys {
			frame_system::Pallet::<Test>::inc_providers(k);
		}
		// (Optional) extra accounts used by tests
		frame_system::Pallet::<Test>::inc_providers(&12);
		frame_system::Pallet::<Test>::inc_providers(&15);
	});

	// Compute invulnerables = (invuls ∩ authority_ids)[..MaxInvulnerables]
	let max_inv = <Test as crate::pallet::Config>::MaxInvulnerables::get() as usize;
	let auth_ids: Vec<AccountId> = keys.iter().map(|x| x.0).collect();
	let mut inv_filtered: Vec<AccountId> =
		invuls.iter().copied().filter(|id| auth_ids.contains(id)).collect();
	inv_filtered.truncate(max_inv);

	// Pallet genesis — NON-EMPTY initial_authorities, optional invulnerables
	crate::pallet::GenesisConfig::<Test> {
		initial_authorities: auth_ids.clone(),
		invulnerables: inv_filtered.clone(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	// Session genesis — modern API wants non_authority_keys too
	pallet_session::GenesisConfig::<Test> { keys, non_authority_keys: vec![] }
		.assimilate_storage(&mut t)
		.unwrap();

	reset_test_globals();

	sp_io::TestExternalities::new(t)
}

/// Advance to block `n`, running hooks in the correct order.
pub fn run_to_block(n: BlockNumberFor<Test>) {
	while System::block_number() < n {
		Session::on_finalize(System::block_number());
		System::on_finalize(System::block_number());
		System::reset_events();

		let next = System::block_number().saturating_add(One::one());
		System::set_block_number(next);

		System::on_initialize(next);
		Session::on_initialize(next);
	}
}

/// Advance to next session boundary (triggers selection/kicks).
pub fn run_to_next_session() {
	let now: BlockNumberFor<Test> = System::block_number();
	let period: BlockNumberFor<Test> = 5u32.into();
	let one: BlockNumberFor<Test> = 2u32.into();

	// Move to the start of the next session boundary.
	// ((now / period) + 1) * period
	let next_boundary = (now / period).saturating_add(one) * period;

	run_to_block(next_boundary);
}

/// Return the current session’s validator id at `idx`.
pub fn current_validator_id_at(idx: usize) -> AccountId {
	Session::validators()[idx]
}

/// Dispatch helpers (optional)
pub fn nominate_ok(id: AccountId) {
	let call = crate::pallet::Call::<Test>::nominate { who: id };
	let rc: <Test as frame_system::Config>::RuntimeCall = call.into();
	assert!(rc.dispatch(frame_system::RawOrigin::Root.into()).is_ok());
}
pub fn remove_ok(id: AccountId) {
	let call = crate::pallet::Call::<Test>::remove { who: id };
	let rc: <Test as frame_system::Config>::RuntimeCall = call.into();
	assert!(rc.dispatch(frame_system::RawOrigin::Root.into()).is_ok());
}
pub fn set_invulnerables(ids: Vec<AccountId>) {
	let call = crate::pallet::Call::<Test>::set_invulnerables { list: ids };
	let rc: <Test as frame_system::Config>::RuntimeCall = call.into();
	assert!(rc.dispatch(frame_system::RawOrigin::Root.into()).is_ok());
}

/// Force MissCount directly (useful for some negative-path tests).
pub fn set_miss_count(id: AccountId, count: u32) {
	crate::pallet::MissCount::<Test>::insert(id, count);
}
