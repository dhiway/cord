#![allow(clippy::from_over_into)]

use crate as mark;
use crate::*;
use codec::Decode;
use pallet_mtype::mock as mtype_mock;

use frame_support::{ensure, parameter_types, weights::constants::RocksDbWeight};
use frame_system::EnsureSigned;
use sp_core::{ed25519, sr25519, Pair};

use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	MultiSignature, MultiSigner,
};

pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub type TestMtypeOwner = cord_primitives::AccountId;
pub type TestMtypeHash = cord_primitives::Hash;
pub type TestDelegationNodeId = cord_primitives::Hash;
pub type TestDelegatorId = cord_primitives::AccountId;
pub type TestContentHash = cord_primitives::Hash;
pub type TestAttester = TestDelegatorId;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Mark: mark::{Pallet, Call, Storage, Event<T>},
		Mtype: pallet_mtype::{Pallet, Call, Storage, Event<T>},
		Delegation: pallet_delegation::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const SS58Prefix: u8 = 29;
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = cord_primitives::Hash;
	type Hashing = BlakeTwo256;
	type AccountId = <<cord_primitives::Signature as Verify>::Signer as IdentifyAccount>::AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type DbWeight = RocksDbWeight;
	type Version = ();

	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

impl pallet_mtype::Config for Test {
	type MtypeCreatorId = TestMtypeOwner;
	type EnsureOrigin = EnsureSigned<TestMtypeOwner>;
	type Event = ();
}

parameter_types! {
	pub const MaxSignatureByteLength: u16 = 64;
}

impl pallet_delegation::Config for Test {
	type DelegationSignatureVerification = Self;
	type DelegationEntityId = TestDelegatorId;
	type DelegationNodeId = TestDelegationNodeId;
	type EnsureOrigin = EnsureSigned<TestDelegatorId>;
	type Event = ();
	type MaxSignatureByteLength = MaxSignatureByteLength;
}

impl Config for Test {
	type EnsureOrigin = EnsureSigned<TestAttester>;
	type Event = ();
}

impl pallet_delegation::VerifyDelegateSignature for Test {
	type DelegateId = TestDelegatorId;
	type Payload = Vec<u8>;
	type Signature = Vec<u8>;

	// No need to retrieve delegate details as it is simply an AccountId.
	fn verify(
		delegate: &Self::DelegateId,
		payload: &Self::Payload,
		signature: &Self::Signature,
	) -> pallet_delegation::SignatureVerificationResult {
		// Try to decode signature first.
		let decoded_signature = MultiSignature::decode(&mut &signature[..])
			.map_err(|_| pallet_delegation::SignatureVerificationError::SignatureInvalid)?;

		ensure!(
			decoded_signature.verify(&payload[..], delegate),
			pallet_delegation::SignatureVerificationError::SignatureInvalid
		);

		Ok(())
	}
}

const ALICE_SEED: [u8; 32] = [0u8; 32];
const BOB_SEED: [u8; 32] = [1u8; 32];

const DEFAULT_CLAIM_HASH_SEED: u64 = 1u64;
const ALTERNATIVE_CLAIM_HASH_SEED: u64 = 2u64;

pub fn get_origin(account: TestAttester) -> Origin {
	Origin::signed(account)
}

pub fn get_ed25519_account(public_key: ed25519::Public) -> TestDelegatorId {
	MultiSigner::from(public_key).into_account()
}

pub fn get_sr25519_account(public_key: sr25519::Public) -> TestDelegatorId {
	MultiSigner::from(public_key).into_account()
}

pub fn get_alice_ed25519() -> ed25519::Pair {
	ed25519::Pair::from_seed(&ALICE_SEED)
}

pub fn get_alice_sr25519() -> sr25519::Pair {
	sr25519::Pair::from_seed(&ALICE_SEED)
}

pub fn get_bob_ed25519() -> ed25519::Pair {
	ed25519::Pair::from_seed(&BOB_SEED)
}

pub fn get_bob_sr25519() -> sr25519::Pair {
	sr25519::Pair::from_seed(&BOB_SEED)
}

pub fn get_content_hash(default: bool) -> TestContentHash {
	if default {
		TestContentHash::from_low_u64_be(DEFAULT_CLAIM_HASH_SEED)
	} else {
		TestContentHash::from_low_u64_be(ALTERNATIVE_CLAIM_HASH_SEED)
	}
}

pub struct MarkCreationDetails {
	pub content_hash: TestContentHash,
	pub mtype_hash: TestMtypeHash,
	pub delegation_id: Option<TestDelegationNodeId>,
}

pub fn generate_base_mark_creation_details(
	content_hash: TestContentHash,
	mark: MarkDetails<Test>,
) -> MarkCreationDetails {
	MarkCreationDetails {
		content_hash,
		mtype_hash: mark.mtype_hash,
		delegation_id: mark.delegation_id,
	}
}

pub struct MarkRevocationDetails {
	pub content_hash: TestContentHash,
	pub max_parent_checks: u32,
}

pub fn generate_base_mark_revocation_details(content_hash: TestContentHash) -> MarkRevocationDetails {
	MarkRevocationDetails {
		content_hash,
		max_parent_checks: 0u32,
	}
}

pub fn generate_base_mark(marker: TestAttester) -> MarkDetails<Test> {
	MarkDetails {
		marker,
		delegation_id: None,
		mtype_hash: mtype_mock::get_mtype_hash(true),
		revoked: false,
	}
}

#[derive(Clone)]
pub struct ExtBuilder {
	marks_stored: Vec<(TestContentHash, MarkDetails<Test>)>,
	delegated_marks_stored: Vec<(TestDelegationNodeId, Vec<TestContentHash>)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			marks_stored: vec![],
			delegated_marks_stored: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_marks(mut self, marks: Vec<(TestContentHash, MarkDetails<Test>)>) -> Self {
		self.marks_stored = marks;
		self
	}

	pub fn with_delegated_marks(mut self, delegated_marks: Vec<(TestDelegationNodeId, Vec<TestContentHash>)>) -> Self {
		self.delegated_marks_stored = delegated_marks;
		self
	}

	pub fn build(self, ext: Option<sp_io::TestExternalities>) -> sp_io::TestExternalities {
		let mut ext = if let Some(ext) = ext {
			ext
		} else {
			let storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
			sp_io::TestExternalities::new(storage)
		};

		if !self.marks_stored.is_empty() {
			ext.execute_with(|| {
				self.marks_stored.iter().for_each(|mark| {
					mark::Marks::<Test>::insert(mark.0, mark.1.clone());
				})
			});
		}

		if !self.delegated_marks_stored.is_empty() {
			ext.execute_with(|| {
				self.delegated_marks_stored.iter().for_each(|delegated_mark| {
					mark::DelegatedMarks::<Test>::insert(delegated_mark.0, delegated_mark.1.clone());
				})
			});
		}

		ext
	}
}
