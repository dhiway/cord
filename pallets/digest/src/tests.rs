// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Marks: Handles #MARKs on chain,
//! adding and revoking #MARKs.

use super::*;
use crate as pallet_digest;

use frame_support::{assert_noop, assert_ok, parameter_types};

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
		PalletMark: pallet_mark::{Module, Call, Storage, Event<T>},
		Delegation: pallet_delegation::{Module, Call, Storage, Event<T>},
		MType: pallet_mtype::{Module, Call, Storage, Event<T>},
		PalletDigest: pallet_digest::{Module, Call, Storage, Event<T>},
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

impl pallet_mark::Config for Test {
	type Event = ();
	type WeightInfo = ();
}

// impl pallet_delegation::Config for Test {
// 	type Event = ();
// }
impl pallet_delegation::Config for Test {
	type Event = ();
	type WeightInfo = ();
	type Signature = Signature;
	type Signer = <Self::Signature as Verify>::Signer;
	type DelegationNodeId = H256;
}

impl pallet_mtype::Config for Test {
	type Event = ();
	type WeightInfo = ();
}

impl Config for Test {
	type Event = ();
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

// fn hash_to_u8<T: Encode>(hash: T) -> Vec<u8> {
// 	hash.encode()
// }

pub fn account_pair(s: &str) -> ed25519::Pair {
	ed25519::Pair::from_string(&format!("//{}", s), None).expect("static values are valid")
}

#[test]
fn check_anchor_digest() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
			None
		));
		assert_ok!(PalletDigest::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
		));
		let Digest {
			stream_hash,
			issuer,
			revoked,
		} = {
			let opt = PalletDigest::digests(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(stream_hash, hash);
		assert_eq!(issuer, account);
		assert_eq!(revoked, false);
	});
}

#[test]
fn check_revoke_digest() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
			None
		));
		assert_ok!(PalletDigest::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
		));
		assert_ok!(PalletDigest::revoke(
			Origin::signed(account.clone()),
			hash,
			10
		));
		let Digest {
			stream_hash,
			issuer,
			revoked,
		} = {
			let opt = PalletDigest::digests(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(stream_hash, hash);
		assert_eq!(issuer, account);
		assert_eq!(revoked, true);
	});
}

#[test]
fn check_double_digest() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
			None
		));
		assert_ok!(PalletDigest::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
		));
		assert_noop!(
			PalletDigest::anchor(Origin::signed(account), hash, hash),
			Error::<Test>::AlreadyAnchoredDigest
		);
	});
}

#[test]
fn check_double_revoke_digest() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
			None
		));
		assert_ok!(PalletDigest::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
		));
		assert_ok!(PalletDigest::revoke(
			Origin::signed(account.clone()),
			hash,
			10
		));
		assert_noop!(
			PalletDigest::revoke(Origin::signed(account), hash),
			Error::<Test>::AlreadyRevoked
		);
	});
}

#[test]
fn check_revoke_unknown() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_noop!(
			PalletDigest::revoke(Origin::signed(account), hash),
			Error::<Test>::NotFound
		);
	});
}

#[test]
fn check_revoke_not_permitted() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		let pair_bob = account_pair("Bob");
		let account_hash_bob = pair_bob.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
			None
		));
		assert_ok!(PalletDigest::anchor(
			Origin::signed(account.clone()),
			hash,
			hash,
		));
		assert_noop!(
			PalletDigest::revoke(Origin::signed(account_hash_bob), hash),
			Error::<Test>::NotOwner
		);
	});
}
