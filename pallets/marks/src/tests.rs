// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Marks: Handles #MARKs on chain,
//! adding and revoking #MARKs.

use crate::*;

use codec::Encode;
use frame_support::{
	assert_err, assert_ok, impl_outer_origin, parameter_types,
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
		DispatchClass,
	},
};
use frame_system::limits::{BlockLength, BlockWeights};
use cord_runtime::{
	AccountId, BlockHashCount,
	Signature, Weight, WEIGHT_PER_SECOND,
};
use sp_core::{ed25519, Pair, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	MultiSignature, MultiSigner, Perbill,
};

impl_outer_origin! {
	pub enum Origin for Test {}
}

#[derive(Clone, Eq, PartialEq, Debug)]

pub struct Test;
/// We assume that ~10% of the block weight is consumed by `on_initalize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 4 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;

parameter_types! {
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u8 = 29;
}

impl frame_system::Config for Test {
	type Origin = Origin;
	type Call = ();
	type Index = u32;
	type BlockNumber = u32;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
	type Lookup = IdentityLookup<AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type DbWeight = RocksDbWeight;
	type Version = ();
	type PalletInfo = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = RuntimeBlockLength;
	type SS58Prefix = SS58Prefix;
}

impl mtypes::Trait for Test {
	type Event = ();
}

impl error::Trait for Test {
	type ErrorCode = u16;
	type Event = ();
}

impl delegation::Trait for Test {
	type Event = ();
	type Signature = Signature;
	type Signer = <Self::Signature as Verify>::Signer;
	type DelegateId = H256;
}

impl Trait for Test {
	type Event = ();
}

type Mark = Module<Test>;
type MType = mtype::Module<Test>;
type Delegation = delegation::Module<Test>;

fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

fn hash_to_u8<T: Encode>(hash: T) -> Vec<u8> {
	hash.encode()
}

#[test]
fn check_add_marks() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_ok!(MType::add(Origin::signed(account_hash.clone()), hash));
		assert_ok!(Mark::add(
			Origin::signed(account_hash.clone()),
			hash,
			hash,
			None
		));
		let existing_mark_for_claim = {
			let opt = Mark::marks(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(existing_mark_for_claim.0, hash);
		assert_eq!(existing_mark_for_claim.1, account_hash);
		assert_eq!(existing_mark_for_claim.3, false);
	});
}

#[test]
fn check_revoke_mark() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_ok!(MType::add(Origin::signed(account_hash.clone()), hash));
		assert_ok!(Mark::add(
			Origin::signed(account_hash.clone()),
			hash,
			hash,
			None
		));
		assert_ok!(Mark::revoke(
			Origin::signed(account_hash.clone()),
			hash
		));
		let existing_mark_for_claim = {
			let opt = Mark::marks(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(existing_mark_for_claim.0, hash);
		assert_eq!(existing_mark_for_claim.1, account_hash);
		assert_eq!(existing_mark_for_claim.3, true);
	});
}

#[test]
fn check_double_mark() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_ok!(MType::add(Origin::signed(account_hash.clone()), hash));
		assert_ok!(Mark::add(
			Origin::signed(account_hash.clone()),
			hash,
			hash,
			None
		));
		assert_err!(
			Mark::add(Origin::signed(account_hash), hash, hash, None),
			Mark::ERROR_ALREADY_ATTESTED.1
		);
	});
}

#[test]
fn check_double_revoke_mark() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_ok!(MType::add(Origin::signed(account_hash.clone()), hash));
		assert_ok!(Mark::add(
			Origin::signed(account_hash.clone()),
			hash,
			hash,
			None
		));
		assert_ok!(Mark::revoke(
			Origin::signed(account_hash.clone()),
			hash
		));
		assert_err!(
			Mark::revoke(Origin::signed(account_hash), hash),
			Mark::ERROR_ALREADY_REVOKED.1
		);
	});
}

#[test]
fn check_revoke_unknown() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_err!(
			Mark::revoke(Origin::signed(account_hash), hash),
			Mark::ERROR_MARK_NOT_FOUND.1
		);
	});
}

#[test]
fn check_revoke_not_permitted() {
	new_test_ext().execute_with(|| {
		let pair_alice = ed25519::Pair::from_seed(&*b"Alice                           ");
		let account_hash_alice = MultiSigner::from(pair_alice.public()).into_account();
		let pair_bob = ed25519::Pair::from_seed(&*b"Bob                             ");
		let account_hash_bob = MultiSigner::from(pair_bob.public()).into_account();
		let hash = H256::from_low_u64_be(1);
		assert_ok!(MType::add(Origin::signed(account_hash_alice.clone()), hash));
		assert_ok!(Mark::add(
			Origin::signed(account_hash_alice),
			hash,
			hash,
			None
		));
		assert_err!(
			Mark::revoke(Origin::signed(account_hash_bob), hash),
			Mark::ERROR_NOT_PERMITTED_TO_REVOKE_ATTESTATION.1
		);
	});
}

#[test]
fn check_add_mark_with_delegation() {
	new_test_ext().execute_with(|| {
		let pair_alice = ed25519::Pair::from_seed(&*b"Alice                           ");
		let account_hash_alice = MultiSigner::from(pair_alice.public()).into_account();
		let pair_bob = ed25519::Pair::from_seed(&*b"Bob                             ");
		let account_hash_bob = MultiSigner::from(pair_bob.public()).into_account();
		let pair_charlie = ed25519::Pair::from_seed(&*b"Charlie                         ");
		let account_hash_charlie = MultiSigner::from(pair_charlie.public()).into_account();

		let mtype = H256::from_low_u64_be(1);
		let other_mtype = H256::from_low_u64_be(2);
		let mark = H256::from_low_u64_be(1);

		let delegation_root = H256::from_low_u64_be(0);
		let delegation_1 = H256::from_low_u64_be(1);
		let delegation_2 = H256::from_low_u64_be(2);

		assert_ok!(MType::add(
			Origin::signed(account_hash_alice.clone()),
			mtype
		));

		assert_err!(
			Mark::add(
				Origin::signed(account_hash_alice.clone()),
				mark,
				mtype,
				Some(delegation_1)
			),
			Delegation::ERROR_DELEGATION_NOT_FOUND.1
		);

		assert_ok!(Delegation::create_root(
			Origin::signed(account_hash_alice.clone()),
			delegation_root,
			mtype
		));
		assert_ok!(Delegation::add_delegation(
			Origin::signed(account_hash_alice.clone()),
			delegation_1,
			delegation_root,
			None,
			account_hash_bob.clone(),
			delegation::Permissions::DELEGATE,
			MultiSignature::from(pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
				delegation_1,
				delegation_root,
				None,
				delegation::Permissions::DELEGATE
			))))
		));
		assert_ok!(Delegation::add_delegation(
			Origin::signed(account_hash_alice.clone()),
			delegation_2,
			delegation_root,
			None,
			account_hash_bob.clone(),
			delegation::Permissions::ATTEST,
			MultiSignature::from(pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
				delegation_2,
				delegation_root,
				None,
				delegation::Permissions::ATTEST
			))))
		));

		assert_err!(
			Mark::add(
				Origin::signed(account_hash_bob.clone()),
				mark,
				other_mtype,
				Some(delegation_2)
			),
			MType::ERROR_MTYPE_NOT_FOUND.1
		);
		assert_ok!(MType::add(
			Origin::signed(account_hash_alice.clone()),
			other_mtype
		));
		assert_err!(
			Mark::add(
				Origin::signed(account_hash_bob.clone()),
				mark,
				other_mtype,
				Some(delegation_2)
			),
			Mark::ERROR_MTYPE_OF_DELEGATION_NOT_MATCHING.1
		);
		assert_err!(
			Mark::add(
				Origin::signed(account_hash_alice.clone()),
				mark,
				mtype,
				Some(delegation_2)
			),
			Mark::ERROR_NOT_DELEGATED_TO_ATTESTER.1
		);
		assert_err!(
			Mark::add(
				Origin::signed(account_hash_bob.clone()),
				mark,
				mtype,
				Some(delegation_1)
			),
			Mark::ERROR_DELEGATION_NOT_AUTHORIZED_TO_ATTEST.1
		);
		assert_ok!(Mark::add(
			Origin::signed(account_hash_bob.clone()),
			mark,
			mtype,
			Some(delegation_2)
		));

		let existing_mark_for_delegation =
			Mark::delegated_marks(delegation_2);
		assert_eq!(existing_mark_for_delegation.len(), 1);
		assert_eq!(existing_mark_for_delegation[0], mark);

		assert_ok!(Delegation::revoke_root(
			Origin::signed(account_hash_alice.clone()),
			delegation_root
		));
		assert_err!(
			Mark::add(
				Origin::signed(account_hash_bob),
				mark,
				mtype,
				Some(delegation_2)
			),
			Mark::ERROR_DELEGATION_REVOKED.1
		);

		assert_err!(
			Mark::revoke(Origin::signed(account_hash_charlie), mark),
			Mark::ERROR_NOT_PERMITTED_TO_REVOKE_ATTESTATION.1
		);
		assert_ok!(Mark::revoke(
			Origin::signed(account_hash_alice),
			mark
		));
	});
}
