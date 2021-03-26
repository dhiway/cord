// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Marks: Handles #MARKs on chain,
//! adding and revoking #MARKs.

use super::*;
use crate as pallet_mark;

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
		PalletMark: pallet_mark::{Module, Call, Storage, Event<T>},
		Delegation: pallet_delegation::{Module, Call, Storage, Event<T>},
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

impl pallet_mtype::Config for Test {
	type Event = ();
}

impl pallet_delegation::Config for Test {
	type Event = ();
	type Signature = Signature;
	type Signer = <Self::Signature as Verify>::Signer;
	type DelegationNodeId = H256;
}

impl Config for Test {
	type Event = ();
}

fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

fn hash_to_u8<T: Encode>(hash: T) -> Vec<u8> {
	hash.encode()
}

pub fn account_pair(s: &str) -> ed25519::Pair {
	ed25519::Pair::from_string(&format!("//{}", s), None).expect("static values are valid")
}

#[test]
fn check_anchor_mark() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(Origin::signed(account.clone()), hash, hash, None));
		let Mark {
			mtype_hash,
			marker,
			revoked,
			delegation_id,
		} = {
			let opt = PalletMark::marks(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(mtype_hash, hash);
		assert_eq!(marker, account);
		assert_eq!(delegation_id, None);
		assert_eq!(revoked, false);
	});
}

#[test]
fn check_revoke_mark() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(Origin::signed(account.clone()), hash, hash, None));
		assert_ok!(PalletMark::revoke(Origin::signed(account.clone()), hash, 10));
		let Mark {
			mtype_hash,
			marker,
			revoked,
			delegation_id,
		} = {
			let opt = PalletMark::marks(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(mtype_hash, hash);
		assert_eq!(marker, account);
		assert_eq!(delegation_id, None);
		assert_eq!(revoked, true);
	});
}

#[test]
fn check_restore_mark() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(Origin::signed(account.clone()), hash, hash, None));
		assert_ok!(PalletMark::revoke(Origin::signed(account.clone()), hash, 10));
		let Mark {
			mtype_hash,
			marker,
			revoked,
			delegation_id,
		} = {
			let opt = PalletMark::marks(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(mtype_hash, hash);
		assert_eq!(marker, account);
		assert_eq!(delegation_id, None);
		assert_eq!(revoked, true);

		assert_ok!(PalletMark::restore(Origin::signed(account.clone()), hash, 10));
		let Mark {
			mtype_hash,
			marker,
			revoked,
			delegation_id,
		} = {
			let opt = PalletMark::marks(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(mtype_hash, hash);
		assert_eq!(marker, account);
		assert_eq!(delegation_id, None);
		assert_eq!(revoked, false);
	});
}

#[test]
fn check_double_mark() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(Origin::signed(account.clone()), hash, hash, None));
		assert_noop!(
			PalletMark::anchor(Origin::signed(account), hash, hash, None),
			Error::<Test>::AlreadyAnchored
		);
	});
}

#[test]
fn check_active_restore() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(Origin::signed(account.clone()), hash, hash, None));
		assert_noop!(
			PalletMark::restore(Origin::signed(account), hash, 10),
			Error::<Test>::MarkStillActive
		);
	});
}

#[test]
fn check_double_revoke_mark() {
	new_test_ext().execute_with(|| {
		let pair = account_pair("Alice");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_ok!(MType::anchor(Origin::signed(account.clone()), hash));
		assert_ok!(PalletMark::anchor(Origin::signed(account.clone()), hash, hash, None));
		assert_ok!(PalletMark::revoke(Origin::signed(account.clone()), hash, 10));
		assert_noop!(
			PalletMark::revoke(Origin::signed(account), hash, 10),
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
			PalletMark::revoke(Origin::signed(account), hash, 10),
			Error::<Test>::MarkNotFound
		);
	});
}

#[test]
fn check_restore_unknown() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account = pair.public();
		assert_noop!(
			PalletMark::restore(Origin::signed(account), hash, 10),
			Error::<Test>::MarkNotFound
		);
	});
}

#[test]
fn check_revoke_not_permitted() {
	new_test_ext().execute_with(|| {
		let pair_alice = account_pair("Alice");
		let account_hash_alice = pair_alice.public();
		let pair_bob = account_pair("Bob");
		let account_hash_bob = pair_bob.public();
		let hash = H256::from_low_u64_be(1);
		assert_ok!(MType::anchor(Origin::signed(account_hash_alice.clone()), hash));
		assert_ok!(PalletMark::anchor(Origin::signed(account_hash_alice), hash, hash, None));
		assert_noop!(
			PalletMark::revoke(Origin::signed(account_hash_bob), hash, 10),
			Error::<Test>::UnauthorizedRevocation
		);
	});
}

#[test]
fn check_restore_not_permitted() {
	new_test_ext().execute_with(|| {
		let pair_alice = account_pair("Alice");
		let account_hash_alice = pair_alice.public();
		let pair_bob = account_pair("Bob");
		let account_hash_bob = pair_bob.public();
		let hash = H256::from_low_u64_be(1);
		assert_ok!(MType::anchor(Origin::signed(account_hash_alice.clone()), hash));
		assert_ok!(PalletMark::anchor(Origin::signed(account_hash_alice), hash, hash, None));
		assert_ok!(PalletMark::revoke(Origin::signed(account_hash_alice), hash, 10));
		assert_noop!(
			PalletMark::restore(Origin::signed(account_hash_bob), hash, 10),
			Error::<Test>::UnauthorizedRestore
		);
	});
}

#[test]
fn check_anchor_mark_with_delegation() {
	new_test_ext().execute_with(|| {
		let pair_alice = account_pair("Alice");
		let account_hash_alice = pair_alice.public();
		let pair_bob = account_pair("Bob");
		let account_hash_bob = pair_bob.public();
		let pair_charlie = account_pair("Charlie");
		let account_hash_charlie = pair_charlie.public();

		let mtype_hash = H256::from_low_u64_be(1);
		let other_mtype_hash = H256::from_low_u64_be(2);
		let stream_hash = H256::from_low_u64_be(1);

		let delegation_root = H256::from_low_u64_be(0);
		let delegation_1 = H256::from_low_u64_be(1);
		let delegation_2 = H256::from_low_u64_be(2);

		assert_ok!(MType::anchor(Origin::signed(account_hash_alice.clone()), mtype_hash));

		// cannot anchor #MARK based on a missing Delegation
		assert_noop!(
			PalletMark::anchor(
				Origin::signed(account_hash_alice.clone()),
				stream_hash,
				mtype_hash,
				Some(delegation_root)
			),
			pallet_delegation::Error::<Test>::DelegationNotFound
		);

		// add root Delegation
		assert_ok!(Delegation::create_root(
			Origin::signed(account_hash_alice.clone()),
			delegation_root,
			mtype_hash
		));

		// add delegation_1 as child of root
		assert_ok!(Delegation::add_delegation(
			Origin::signed(account_hash_alice.clone()),
			delegation_1,
			delegation_root,
			None,
			account_hash_bob.clone(),
			pallet_delegation::Permissions::DELEGATE,
			pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
				delegation_1,
				delegation_root,
				None,
				pallet_delegation::Permissions::DELEGATE
			)))
		));

		// add delegation_2 as child of root
		assert_ok!(Delegation::add_delegation(
			Origin::signed(account_hash_alice.clone()),
			delegation_2,
			delegation_root,
			None,
			account_hash_bob.clone(),
			pallet_delegation::Permissions::ANCHOR,
			pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
				delegation_2,
				delegation_root,
				None,
				pallet_delegation::Permissions::ANCHOR
			)))
		));

		// cannot anchor #MARK for missing mtype
		assert_noop!(
			PalletMark::anchor(
				Origin::signed(account_hash_bob.clone()),
				stream_hash,
				other_mtype_hash,
				Some(delegation_2)
			),
			pallet_mtype::Error::<Test>::NotFound
		);

		// add missing mtype
		assert_ok!(MType::anchor(
			Origin::signed(account_hash_alice.clone()),
			other_mtype_hash
		));

		// cannot add attestation with different ctype than in root
		assert_noop!(
			PalletMark::anchor(
				Origin::signed(account_hash_bob.clone()),
				stream_hash,
				other_mtype_hash,
				Some(delegation_2)
			),
			Error::<Test>::MTypeMismatch
		);
		// cannot add Delegation if not marker (bob is marker of delegation_2)
		assert_noop!(
			PalletMark::anchor(
				Origin::signed(account_hash_alice.clone()),
				stream_hash,
				mtype_hash,
				Some(delegation_2)
			),
			Error::<Test>::NotDelegatedToMarker
		);

		// cannot add Delegation if not owner (alice is owner of delegation_1)
		assert_noop!(
			PalletMark::anchor(
				Origin::signed(account_hash_bob.clone()),
				stream_hash,
				mtype_hash,
				Some(delegation_1)
			),
			Error::<Test>::DelegationUnauthorisedToAnchor
		);

		// anchor attestation for delegation_2
		assert_ok!(PalletMark::anchor(
			Origin::signed(account_hash_bob.clone()),
			stream_hash,
			mtype_hash,
			Some(delegation_2)
		));

		let existing_markers_for_delegation = PalletMark::delegated_marks(delegation_2);
		assert_eq!(existing_markers_for_delegation.len(), 1);
		assert_eq!(existing_markers_for_delegation[0], stream_hash);

		// revoke root Delegation
		assert_ok!(Delegation::revoke_root(
			Origin::signed(account_hash_alice.clone()),
			delegation_root,
			2
		));

		// cannot revoke attestation if not owner (alice is owner of attestation)
		assert_noop!(
			PalletMark::revoke(Origin::signed(account_hash_charlie), stream_hash, 10),
			Error::<Test>::UnauthorizedRevocation
		);
		assert_ok!(PalletMark::revoke(Origin::signed(account_hash_alice), stream_hash, 10));

		// remove attestation to catch for revoked Delegation
		Marks::<Test>::remove(stream_hash);
		assert_noop!(
			PalletMark::anchor(
				Origin::signed(account_hash_bob),
				stream_hash,
				mtype_hash,
				Some(delegation_2)
			),
			Error::<Test>::DelegationRevoked
		);
	});
}
