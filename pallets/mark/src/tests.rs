// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Marks: Handles #MARKs on chain,
//! adding and revoking #MARKs.

use crate::*;

use codec::Encode;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::Weight,
	impl_outer_origin, parameter_types,
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		DispatchClass,
	},
	StorageMap,
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
	pub const BlockHashCount: u32 = 250;

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

impl delegation::Trait for Test {
	type Event = ();
	type Signature = Signature;
	type Signer = <Self::Signature as Verify>::Signer;
	type DelegationNodeId = H256;
}

impl Trait for Test {
	type Event = ();
}

type MarkerModule = Module<Test>;
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
fn check_anchor_mark() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_ok!(MType::anchor(Origin::signed(account_hash.clone()), hash));
		assert_ok!(MarkerModule::add(
			Origin::signed(account_hash.clone()),
			hash,
			hash,
			None
		));
		let Mark {
			mtype_hash,
			owner,
			revoked,
			delegation_id,
		} = {
			let opt = MarkerModule::marks(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(mtype_hash, hash);
		assert_eq!(owner, account_hash);
		assert_eq!(delegation_id, None);
		assert_eq!(revoked, false);
	});
}

#[test]
fn check_revoke_mark() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_ok!(MType::anchor(Origin::signed(account_hash.clone()), hash));
		assert_ok!(MarkerModule::add(
			Origin::signed(account_hash.clone()),
			hash,
			hash,
			None
		));
		assert_ok!(MarkerModule::revoke(
			Origin::signed(account_hash.clone()),
			hash,
			10
		));
		let Mark {
			mtype_hash,
			owner,
			revoked,
			delegation_id,
		} = {
			let opt = MarkerModule::marks(hash);
			assert!(opt.is_some());
			opt.unwrap()
		};
		assert_eq!(mtype_hash, hash);
		assert_eq!(owner, account_hash);
		assert_eq!(delegation_id, None);
		assert_eq!(revoked, true);
	});
}

#[test]
fn check_double_mark() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_ok!(MType::anchor(Origin::signed(account_hash.clone()), hash));
		assert_ok!(MarkerModule::add(
			Origin::signed(account_hash.clone()),
			hash,
			hash,
			None
		));
		assert_noop!(
			MarkerModule::add(Origin::signed(account_hash), hash, hash, None),
			Error::<Test>::AlreadyAnchored
		);
	});
}

#[test]
fn check_double_revoke_mark() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_ok!(MType::anchor(Origin::signed(account_hash.clone()), hash));
		assert_ok!(MarkerModule::add(
			Origin::signed(account_hash.clone()),
			hash,
			hash,
			None
		));
		assert_ok!(MarkerModule::revoke(
			Origin::signed(account_hash.clone()),
			hash,
			10
		));
		assert_noop!(
			MarkerModule::revoke(Origin::signed(account_hash), hash, 10),
			Error::<Test>::AlreadyRevoked
		);
	});
}

#[test]
fn check_revoke_unknown() {
	new_test_ext().execute_with(|| {
		let pair = ed25519::Pair::from_seed(&*b"Alice                           ");
		let hash = H256::from_low_u64_be(1);
		let account_hash = MultiSigner::from(pair.public()).into_account();
		assert_noop!(
			MarkerModule::revoke(Origin::signed(account_hash), hash, 10),
			Error::<Test>::MarkNotFound
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
		assert_ok!(MType::anchor(Origin::signed(account_hash_alice.clone()), hash));
		assert_ok!(MarkerModule::add(
			Origin::signed(account_hash_alice),
			hash,
			hash,
			None
		));
		assert_noop!(
			MarkerModule::revoke(Origin::signed(account_hash_bob), hash, 10),
			Error::<Test>::UnauthorizedRevocation
		);
	});
}

#[test]
fn check_anchor_mark_with_delegation() {
	new_test_ext().execute_with(|| {
		let pair_alice = ed25519::Pair::from_seed(&*b"Alice                           ");
		let account_hash_alice = MultiSigner::from(pair_alice.public()).into_account();
		let pair_bob = ed25519::Pair::from_seed(&*b"Bob                             ");
		let account_hash_bob = MultiSigner::from(pair_bob.public()).into_account();
		let pair_charlie = ed25519::Pair::from_seed(&*b"Charlie                         ");
		let account_hash_charlie = MultiSigner::from(pair_charlie.public()).into_account();

		let mtype_hash = H256::from_low_u64_be(1);
		let other_mtype_hash = H256::from_low_u64_be(2);
		let stream_hash = H256::from_low_u64_be(1);

		let delegation_root = H256::from_low_u64_be(0);
		let delegation_1 = H256::from_low_u64_be(1);
		let delegation_2 = H256::from_low_u64_be(2);

		assert_ok!(MType::anchor(
			Origin::signed(account_hash_alice.clone()),
			mtype_hash
		));

		// cannot anchor #MARK based on a missing delegation
		assert_noop!(
			MarkerModule::add(
				Origin::signed(account_hash_alice.clone()),
				stream_hash,
				mtype_hash,
				Some(delegation_root)
			),
			delegation::Error::<Test>::DelegationNotFound
		);

		// add root delegation
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
			delegation::Permissions::DELEGATE,
			MultiSignature::from(pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
				delegation_1,
				delegation_root,
				None,
				delegation::Permissions::DELEGATE
			))))
		));

		// add delegation_2 as child of root
		assert_ok!(Delegation::add_delegation(
			Origin::signed(account_hash_alice.clone()),
			delegation_2,
			delegation_root,
			None,
			account_hash_bob.clone(),
			delegation::Permissions::ANCHOR,
			MultiSignature::from(pair_bob.sign(&hash_to_u8(Delegation::calculate_hash(
				delegation_2,
				delegation_root,
				None,
				delegation::Permissions::ANCHOR
			))))
		));

		// cannot anchor #MARK for missing mtype
		assert_noop!(
			MarkerModule::add(
				Origin::signed(account_hash_bob.clone()),
				stream_hash,
				other_mtype_hash,
				Some(delegation_2)
			),
			mtype::Error::<Test>::NotFound
		);

		// add missing mtype
		assert_ok!(MType::anchor(
			Origin::signed(account_hash_alice.clone()),
			other_mtype_hash
		));

		// cannot add attestation with different ctype than in root
		assert_noop!(
			MarkerModule::add(
				Origin::signed(account_hash_bob.clone()),
				stream_hash,
				other_mtype_hash,
				Some(delegation_2)
			),
			Error::<Test>::MTypeMismatch
		);
		// cannot add delegation if not owner (bob is owner of delegation_2)
		assert_noop!(
			MarkerModule::add(
				Origin::signed(account_hash_alice.clone()),
				stream_hash,
				mtype_hash,
				Some(delegation_2)
			),
			Error::<Test>::NotDelegatedToMarker
		);

		// cannot add delegation if not owner (alice is owner of delegation_1)
		assert_noop!(
			MarkerModule::add(
				Origin::signed(account_hash_bob.clone()),
				stream_hash,
				mtype_hash,
				Some(delegation_1)
			),
			Error::<Test>::DelegationUnauthorizedToAnchor
		);

		// add attestation for delegation_2
		assert_ok!(MarkerModule::add(
			Origin::signed(account_hash_bob.clone()),
			stream_hash,
			mtype_hash,
			Some(delegation_2)
		));

		let existing_markers_for_delegation =
			MarkerModule::delegated_marks(delegation_2);
		assert_eq!(existing_markers_for_delegation.len(), 1);
		assert_eq!(existing_markers_for_delegation[0], stream_hash);

		// revoke root delegation
		assert_ok!(Delegation::revoke_root(
			Origin::signed(account_hash_alice.clone()),
			delegation_root
		));

		// cannot revoke attestation if not owner (alice is owner of attestation)
		assert_noop!(
			MarkerModule::revoke(Origin::signed(account_hash_charlie), stream_hash, 10),
			Error::<Test>::UnauthorizedRevocation
		);
		assert_ok!(MarkerModule::revoke(
			Origin::signed(account_hash_alice),
			stream_hash,
			10
		));		

		// remove attestation to catch for revoked delegation
		Marks::<Test>::remove(stream_hash);
		assert_noop!(
			MarkerModule::add(
				Origin::signed(account_hash_bob),
				stream_hash,
				mtype_hash,
				Some(delegation_2)
			),
			Error::<Test>::DelegationRevoked
		);
	});
}