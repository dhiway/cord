// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! DID: Handles decentralized identifiers on chain,
//! test adding and removing DIDs.

use super::*;
use crate as pallet_did;

use frame_support::{assert_ok, parameter_types};

use sp_core::{ed25519, ed25519::Signature, Pair, H256};
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
		DID: pallet_did::{Module, Call, Storage, Event<T>},
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
	type WeightInfo = ();
	type PublicSigningKey = H256;
	type PublicBoxKey = H256;
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
fn add_did() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let signing_key = H256::from_low_u64_be(1);
		let box_key = H256::from_low_u64_be(2);
		let account = pair.public();
		assert_ok!(DID::anchor(
			Origin::signed(account),
			signing_key,
			box_key,
			Some(b"http://dway.io/submit".to_vec())
		));

		assert_eq!(<DIDs<Test>>::contains_key(account), true);
		let did = {
			let opt = DID::dids(account);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(did.sign_key, signing_key);
		assert_eq!(did.box_key, box_key);
		assert_eq!(did.doc_ref, Some(b"http://dway.io/submit".to_vec()));
	});
}

#[test]
fn remove_did() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let account = pair.public();
		assert_ok!(DID::remove(Origin::signed(account.clone())));
		assert_eq!(<DIDs<Test>>::contains_key(account), false);
	});
}
