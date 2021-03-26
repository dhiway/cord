// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.
// Test module for reserve pallet

#![cfg(test)]

use super::*;
use crate as pallet_reserve;

use frame_support::{assert_noop, assert_ok, ord_parameter_types, parameter_types};
use std::cell::RefCell;

use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	DispatchError::BadOrigin,
	ModuleId,
};
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
		CordReserve: pallet_reserve::{Module, Call, Storage, Config, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(1024);
}

impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Test {
	type Balance = u64;
	type Event = ();
	type DustRemoval = ();
	type MaxLocks = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
}

thread_local! {
	static TEN_TO_FOURTEEN: RefCell<Vec<u128>> = RefCell::new(vec![10,11,12,13,14]);
}

ord_parameter_types! {
	pub const Admin: u64 = 1;
}

parameter_types! {
	pub const ReserveModuleId: ModuleId = ModuleId(*b"py/resrv");
}

impl Config for Test {
	type ModuleId = ReserveModuleId;
	type Event = ();
	type Currency = pallet_balances::Module<Test>;
	type ExternalOrigin = EnsureSignedBy<Admin, u64>;
	type Call = Call;
}

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

#[test]
fn spend_error_if_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_noop!(CordReserve::transfer(Origin::signed(0), 1, 1), BadOrigin);
	})
}

#[test]
fn spend_funds_to_target() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&CordReserve::account_id(), 100);

		assert_eq!(Balances::free_balance(CordReserve::account_id()), 100);
		assert_eq!(Balances::free_balance(3), 0);
		assert_ok!(CordReserve::transfer(Origin::signed(Admin::get()), 3, 90));
		assert_eq!(Balances::free_balance(3), 90);
		assert_eq!(Balances::free_balance(CordReserve::account_id()), 10);
	})
}

#[test]
fn receive() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&999, 100);

		assert_ok!(CordReserve::receive(Origin::signed(999), 50));
		assert_eq!(Balances::free_balance(999), 50);
		assert_eq!(Balances::free_balance(CordReserve::account_id()), 50);
	})
}

fn make_call(value: u8) -> Box<Call> {
	Box::new(Call::System(frame_system::Call::<Test>::remark(vec![value])))
}

#[test]
fn apply_as_error_if_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_noop!(CordReserve::apply_as(Origin::signed(0), make_call(1)), BadOrigin);
	})
}

#[test]
fn apply_as_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(CordReserve::apply_as(Origin::signed(Admin::get()), make_call(1)));
	})
}

#[test]
fn try_root_if_not_admin() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&CordReserve::account_id(), 100);

		assert_ok!(CordReserve::transfer(Origin::signed(1), 3, 100));
		assert_ok!(CordReserve::apply_as(Origin::signed(1), make_call(1)));
	})
}
