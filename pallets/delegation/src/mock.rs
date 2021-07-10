#![allow(clippy::from_over_into)]

use codec::Decode;
use frame_support::{parameter_types, weights::constants::RocksDbWeight};
use frame_system::EnsureSigned;
use sp_core::{ed25519, sr25519, Pair};
use sp_keystore::{testing::KeyStore, KeystoreExt};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	MultiSignature, MultiSigner,
};
use sp_std::sync::Arc;

#[cfg(test)]
use codec::Encode;

use crate as delegation;
use crate::*;
// use pallet_mtype::mock as mtype_mock;

pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub type TestMtypeOwner = cord_primitives::AccountId;
pub type TestMtypeHash = cord_primitives::Hash;
pub type TestDelegationNodeId = cord_primitives::Hash;
pub type TestDelegatorId = TestMtypeOwner;
pub type TestDelegateSignature = MultiSignature;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Mtype: pallet_mtype::{Pallet, Call, Storage, Event<T>},
		Delegation: delegation::{Pallet, Call, Storage, Event<T>},
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
	type MtypeOwnerId = TestMtypeOwner;
	type EnsureOrigin = EnsureSigned<TestMtypeOwner>;
	type Event = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxSignatureByteLength: u16 = 64;
	pub const MaxParentChecks: u32 = 5;
	pub const MaxRevocations: u32 = 5;
}

impl Config for Test {
	type DelegationSignatureVerification = Self;
	type DelegationEntityId = TestDelegatorId;
	type DelegationNodeId = TestDelegationNodeId;
	type EnsureOrigin = EnsureSigned<TestDelegatorId>;
	type Event = ();
	type MaxSignatureByteLength = MaxSignatureByteLength;
	type MaxParentChecks = MaxParentChecks;
	type MaxRevocations = MaxRevocations;
	type WeightInfo = ();
}

impl VerifyDelegateSignature for Test {
	type DelegateId = TestDelegatorId;
	type Payload = Vec<u8>;
	type Signature = Vec<u8>;

	// No need to retrieve delegate details as it is simply an AccountId.
	fn verify(
		delegate: &Self::DelegateId,
		payload: &Self::Payload,
		signature: &Self::Signature,
	) -> SignatureVerificationResult {
		// Try to decode signature first.
		let decoded_signature =
			MultiSignature::decode(&mut &signature[..]).map_err(|_| SignatureVerificationError::SignatureInvalid)?;

		ensure!(
			decoded_signature.verify(&payload[..], delegate),
			SignatureVerificationError::SignatureInvalid
		);

		Ok(())
	}
}

const ALICE_SEED: [u8; 32] = [0u8; 32];
const BOB_SEED: [u8; 32] = [1u8; 32];
const CHARLIE_SEED: [u8; 32] = [2u8; 32];

const DEFAULT_ROOT_ID_SEED: u64 = 1u64;
const ALTERNATIVE_ROOT_ID_SEED: u64 = 2u64;
const DEFAULT_DELEGATION_ID_SEED: u64 = 3u64;
const ALTERNATIVE_DELEGATION_ID_SEED: u64 = 4u64;
const DEFAULT_DELEGATION_ID_2_SEED: u64 = 3u64;
const ALTERNATIVE_DELEGATION_ID_2_SEED: u64 = 4u64;

pub fn get_origin(account: TestDelegatorId) -> Origin {
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

pub fn get_charlie_ed25519() -> ed25519::Pair {
	ed25519::Pair::from_seed(&CHARLIE_SEED)
}

pub fn get_charlie_sr25519() -> sr25519::Pair {
	sr25519::Pair::from_seed(&CHARLIE_SEED)
}

pub fn get_delegation_root_id(default: bool) -> TestDelegationNodeId {
	if default {
		TestMtypeHash::from_low_u64_be(DEFAULT_ROOT_ID_SEED)
	} else {
		TestMtypeHash::from_low_u64_be(ALTERNATIVE_ROOT_ID_SEED)
	}
}

pub fn get_delegation_id(default: bool) -> TestDelegationNodeId {
	if default {
		TestMtypeHash::from_low_u64_be(DEFAULT_DELEGATION_ID_SEED)
	} else {
		TestMtypeHash::from_low_u64_be(ALTERNATIVE_DELEGATION_ID_SEED)
	}
}

pub fn get_delegation_id_2(default: bool) -> TestDelegationNodeId {
	if default {
		TestMtypeHash::from_low_u64_be(DEFAULT_DELEGATION_ID_2_SEED)
	} else {
		TestMtypeHash::from_low_u64_be(ALTERNATIVE_DELEGATION_ID_2_SEED)
	}
}

#[cfg(test)]
pub(crate) fn hash_to_u8<T: Encode>(hash: T) -> Vec<u8> {
	hash.encode()
}

pub struct DelegationRootCreationDetails {
	pub root_id: TestDelegationNodeId,
	pub mtype_hash: TestMtypeHash,
}

pub fn generate_base_delegation_root_creation_details(
	root_id: TestDelegationNodeId,
	root_node: DelegationRoot<Test>,
) -> DelegationRootCreationDetails {
	DelegationRootCreationDetails {
		mtype_hash: root_node.mtype_hash,
		root_id,
	}
}

pub struct DelegationCreationDetails {
	pub delegation_id: TestDelegationNodeId,
	pub root_id: TestDelegationNodeId,
	pub parent_id: Option<TestDelegationNodeId>,
	pub delegate: TestDelegatorId,
	pub permissions: Permissions,
	pub delegate_signature: TestDelegateSignature,
}

pub fn generate_base_delegation_creation_details(
	delegation_id: TestDelegationNodeId,
	delegate_signature: TestDelegateSignature,
	delegation_node: DelegationNode<Test>,
) -> DelegationCreationDetails {
	DelegationCreationDetails {
		delegation_id,
		parent_id: delegation_node.parent,
		root_id: delegation_node.root_id,
		delegate: delegation_node.owner,
		delegate_signature,
		permissions: delegation_node.permissions,
	}
}

pub struct DelegationRootRevocationDetails {
	pub root_id: TestDelegationNodeId,
	pub max_children: u32,
}

pub fn generate_base_delegation_root_revocation_details(
	root_id: TestDelegationNodeId,
) -> DelegationRootRevocationDetails {
	DelegationRootRevocationDetails {
		root_id,
		max_children: 0u32,
	}
}

pub struct DelegationRevocationDetails {
	pub delegation_id: TestDelegationNodeId,
	pub max_parent_checks: u32,
	pub max_revocations: u32,
}

pub fn generate_base_delegation_revocation_details(delegation_id: TestDelegationNodeId) -> DelegationRevocationDetails {
	DelegationRevocationDetails {
		delegation_id,
		max_parent_checks: 0u32,
		max_revocations: 0u32,
	}
}

pub fn generate_base_delegation_root(owner: TestDelegatorId) -> DelegationRoot<Test> {
	DelegationRoot {
		owner,
		mtype_hash: mtype_mock::get_mtype_hash(true),
		revoked: false,
	}
}

pub fn generate_base_delegation_node(root_id: TestDelegationNodeId, owner: TestDelegatorId) -> DelegationNode<Test> {
	DelegationNode {
		owner,
		parent: None,
		root_id,
		permissions: Permissions::DELEGATE,
		revoked: false,
	}
}

#[derive(Clone)]
pub struct ExtBuilder {
	mtype_builder: Option<mtype_mock::ExtBuilder>,
	root_delegations_stored: Vec<(TestDelegationNodeId, DelegationRoot<Test>)>,
	delegations_stored: Vec<(TestDelegationNodeId, DelegationNode<Test>)>,
	children_stored: Vec<(TestDelegationNodeId, Vec<TestDelegationNodeId>)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			mtype_builder: None,
			root_delegations_stored: vec![],
			delegations_stored: vec![],
			children_stored: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_root_delegations(
		mut self,
		root_delegations: Vec<(TestDelegationNodeId, DelegationRoot<Test>)>,
	) -> Self {
		self.root_delegations_stored = root_delegations;
		self
	}

	pub fn with_delegations(mut self, delegations: Vec<(TestDelegationNodeId, DelegationNode<Test>)>) -> Self {
		self.delegations_stored = delegations;
		self
	}

	pub fn with_children(mut self, children: Vec<(TestDelegationNodeId, Vec<TestDelegationNodeId>)>) -> Self {
		self.children_stored = children;
		self
	}

	pub fn build(self, ext: Option<sp_io::TestExternalities>) -> sp_io::TestExternalities {
		let mut ext = if let Some(ext) = ext {
			ext
		} else {
			let storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
			sp_io::TestExternalities::new(storage)
		};

		if !self.root_delegations_stored.is_empty() {
			ext.execute_with(|| {
				self.root_delegations_stored.iter().for_each(|root_delegation| {
					delegation::Roots::<Test>::insert(root_delegation.0, root_delegation.1.clone());
				})
			});
		}

		if !self.delegations_stored.is_empty() {
			ext.execute_with(|| {
				self.delegations_stored.iter().for_each(|del| {
					delegation::Delegations::<Test>::insert(del.0, del.1.clone());
				})
			});
		}

		if !self.children_stored.is_empty() {
			ext.execute_with(|| {
				self.children_stored.iter().for_each(|child| {
					delegation::Children::<Test>::insert(child.0, child.1.clone());
				})
			});
		}
		ext
	}

	pub fn build_with_keystore(self, ext: Option<sp_io::TestExternalities>) -> sp_io::TestExternalities {
		let mut ext = self.build(ext);

		let keystore = KeyStore::new();
		ext.register_extension(KeystoreExt(Arc::new(keystore)));

		ext
	}
}
