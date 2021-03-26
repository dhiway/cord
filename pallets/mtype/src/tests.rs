// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! testing #MARK Types.

use super::*;
use crate as pallet_mtype;

use frame_support::{assert_noop, assert_ok, parameter_types};

use sp_core::ed25519::Signature;
use sp_core::{ed25519, Pair, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
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
		MType: pallet_mtype::{Module, Call, Storage, Event<T>},
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
	type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

impl Config for Test {
	type Event = ();
}

pub fn account_pair(s: &str) -> ed25519::Pair {
	ed25519::Pair::from_string(&format!("//{}", s), None).expect("static values are valid")
}

fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

#[test]
fn check_mtype_with_default_values() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let mtype_hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), mtype_hash));
		assert_eq!(<MTYPEs<Test>>::contains_key(mtype_hash), true);
		assert_eq!(MType::mtypes(mtype_hash), Some(account.clone()));
		assert_noop!(
			MType::anchor(Origin::signed(account), mtype_hash),
			Error::<Test>::AlreadyExists
		);
	});
}
