#![allow(clippy::from_over_into)]
#![allow(unused_must_use)]

use codec::{Decode, Encode};
use frame_support::{parameter_types, weights::constants::RocksDbWeight};
#[cfg(feature = "runtime-benchmarks")]
use frame_system::EnsureSigned;
use sp_core::{ed25519, sr25519, Pair};
use sp_keystore::{testing::KeyStore, KeystoreExt};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
};
use sp_std::{collections::btree_set::BTreeSet, convert::TryInto, sync::Arc};

use crate as did;
use crate::*;

pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub type TestDidIdentifier = cord_primitives::DidIdentifier;
pub type TestKeyId = did::KeyIdOf<Test>;
pub type TestBlockNumber = cord_primitives::BlockNumber;
pub type TestMtypeOwner = TestDidIdentifier;
pub type TestMtypeHash = cord_primitives::Hash;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		Did: did::{Pallet, Call, Storage, Event<T>, Origin<T>},
		Mtype: pallet_mtype::{Pallet, Call, Storage, Event<T>},
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
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

parameter_types! {
	pub const MaxNewKeyAgreementKeys: u32 = 10u32;
	pub const MaxVerificationKeysToRevoke: u32 = 10u32;
	pub const MaxUrlLength: u32 = 200u32;
}

impl Config for Test {
	type DidIdentifier = TestDidIdentifier;
	type Origin = Origin;
	type Call = Call;
	type Event = ();
	type MaxNewKeyAgreementKeys = MaxNewKeyAgreementKeys;
	type MaxUrlLength = MaxUrlLength;
	type MaxVerificationKeysToRevoke = MaxVerificationKeysToRevoke;
	type WeightInfo = ();
}

impl pallet_mtype::Config for Test {
	type MtypeCreatorId = TestMtypeOwner;
	#[cfg(feature = "runtime-benchmarks")]
	type EnsureOrigin = EnsureSigned<TestDidIdentifier>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type EnsureOrigin = did::EnsureDidOrigin<TestMtypeOwner>;
	type Event = ();
	type WeightInfo = ();
}

#[cfg(test)]
pub(crate) const DEFAULT_ACCOUNT: cord_primitives::AccountId = cord_primitives::AccountId::new([0u8; 32]);

pub const ALICE_DID: TestDidIdentifier = TestDidIdentifier::new([1u8; 32]);
pub const BOB_DID: TestDidIdentifier = TestDidIdentifier::new([2u8; 32]);
const DEFAULT_AUTH_SEED: [u8; 32] = [4u8; 32];
const ALTERNATIVE_AUTH_SEED: [u8; 32] = [40u8; 32];
const DEFAULT_ENC_SEED: [u8; 32] = [5u8; 32];
const ALTERNATIVE_ENC_SEED: [u8; 32] = [50u8; 32];
const DEFAULT_ATT_SEED: [u8; 32] = [6u8; 32];
const ALTERNATIVE_ATT_SEED: [u8; 32] = [60u8; 32];
const DEFAULT_DEL_SEED: [u8; 32] = [7u8; 32];
const ALTERNATIVE_DEL_SEED: [u8; 32] = [70u8; 32];
const DEFAULT_URL_SCHEME: [u8; 8] = *b"https://";

pub fn get_ed25519_authentication_key(default: bool) -> ed25519::Pair {
	if default {
		ed25519::Pair::from_seed(&DEFAULT_AUTH_SEED)
	} else {
		ed25519::Pair::from_seed(&ALTERNATIVE_AUTH_SEED)
	}
}

pub fn get_sr25519_authentication_key(default: bool) -> sr25519::Pair {
	if default {
		sr25519::Pair::from_seed(&DEFAULT_AUTH_SEED)
	} else {
		sr25519::Pair::from_seed(&ALTERNATIVE_AUTH_SEED)
	}
}

pub fn get_x25519_encryption_key(default: bool) -> DidEncryptionKey {
	if default {
		DidEncryptionKey::X25519(DEFAULT_ENC_SEED)
	} else {
		DidEncryptionKey::X25519(ALTERNATIVE_ENC_SEED)
	}
}

pub fn get_ed25519_mark_key(default: bool) -> ed25519::Pair {
	if default {
		ed25519::Pair::from_seed(&DEFAULT_ATT_SEED)
	} else {
		ed25519::Pair::from_seed(&ALTERNATIVE_ATT_SEED)
	}
}

pub fn get_sr25519_mark_key(default: bool) -> sr25519::Pair {
	if default {
		sr25519::Pair::from_seed(&DEFAULT_ATT_SEED)
	} else {
		sr25519::Pair::from_seed(&ALTERNATIVE_ATT_SEED)
	}
}

pub fn get_ed25519_delegation_key(default: bool) -> ed25519::Pair {
	if default {
		ed25519::Pair::from_seed(&DEFAULT_DEL_SEED)
	} else {
		ed25519::Pair::from_seed(&ALTERNATIVE_DEL_SEED)
	}
}

pub fn get_sr25519_delegation_key(default: bool) -> sr25519::Pair {
	if default {
		sr25519::Pair::from_seed(&DEFAULT_DEL_SEED)
	} else {
		sr25519::Pair::from_seed(&ALTERNATIVE_DEL_SEED)
	}
}

pub fn get_key_agreement_keys(n_keys: u32) -> BTreeSet<DidEncryptionKey> {
	(1..=n_keys)
		.map(|i| {
			// Converts the loop index to a 32-byte array;
			let mut seed_vec = i.to_be_bytes().to_vec();
			seed_vec.resize(32, 0u8);
			let seed: [u8; 32] = seed_vec
				.try_into()
				.expect("Failed to create encryption key from raw seed.");
			DidEncryptionKey::X25519(seed)
		})
		.collect::<BTreeSet<DidEncryptionKey>>()
}

pub fn get_public_keys_to_remove(n_keys: u32) -> BTreeSet<TestKeyId> {
	(1..=n_keys)
		.map(|i| {
			// Converts the loop index to a 32-byte array;
			let mut seed_vec = i.to_be_bytes().to_vec();
			seed_vec.resize(32, 0u8);
			let seed: [u8; 32] = seed_vec
				.try_into()
				.expect("Failed to create encryption key from raw seed.");
			let key = DidEncryptionKey::X25519(seed);
			generate_key_id(&key.into())
		})
		.collect::<BTreeSet<TestKeyId>>()
}

// Assumes that the length of the URL is larger than 8 (length of the prefix https://)
pub fn get_url_endpoint(length: u32) -> Url {
	let total_length = usize::try_from(length).expect("Failed to convert URL max length value to usize value.");
	let mut url_encoded_string = DEFAULT_URL_SCHEME.to_vec();
	url_encoded_string.resize(total_length, b'0');
	Url::Http(
		HttpUrl::try_from(url_encoded_string.as_ref()).expect("Failed to create default URL with provided length."),
	)
}

pub fn generate_base_did_creation_operation(
	did: TestDidIdentifier,
	new_auth_key: did::DidVerificationKey,
) -> did::DidCreationOperation<Test> {
	DidCreationOperation {
		did,
		new_authentication_key: new_auth_key,
		new_key_agreement_keys: BTreeSet::new(),
		new_mark_key: None,
		new_delegation_key: None,
		new_endpoint_url: None,
	}
}

pub fn generate_base_did_update_operation(did: TestDidIdentifier) -> did::DidUpdateOperation<Test> {
	DidUpdateOperation {
		did,
		new_authentication_key: None,
		new_key_agreement_keys: BTreeSet::new(),
		mark_key_update: DidVerificationKeyUpdateAction::default(),
		delegation_key_update: DidVerificationKeyUpdateAction::default(),
		new_endpoint_url: None,
		public_keys_to_remove: BTreeSet::new(),
		tx_counter: 1u64,
	}
}

pub fn generate_base_did_delete_operation(did: TestDidIdentifier) -> did::DidDeletionOperation<Test> {
	DidDeletionOperation { did, tx_counter: 1u64 }
}

pub fn generate_base_did_details(authentication_key: did::DidVerificationKey) -> did::DidDetails<Test> {
	did::DidDetails::new(authentication_key, 0u64)
}

pub fn generate_key_id(key: &did::DidPublicKey) -> TestKeyId {
	utils::calculate_key_id::<Test>(key)
}

pub(crate) fn get_mark_key_test_input() -> TestMtypeHash {
	TestMtypeHash::from_slice(&[0u8; 32])
}
pub(crate) fn get_mark_key_call() -> Call {
	Call::Mtype(pallet_mtype::Call::anchor(get_mark_key_test_input()))
}
pub(crate) fn get_authentication_key_test_input() -> TestMtypeHash {
	TestMtypeHash::from_slice(&[1u8; 32])
}
pub(crate) fn get_authentication_key_call() -> Call {
	Call::Mtype(pallet_mtype::Call::anchor(get_authentication_key_test_input()))
}
pub(crate) fn get_delegation_key_test_input() -> TestMtypeHash {
	TestMtypeHash::from_slice(&[2u8; 32])
}
pub(crate) fn get_delegation_key_call() -> Call {
	Call::Mtype(pallet_mtype::Call::anchor(get_delegation_key_test_input()))
}
pub(crate) fn get_no_key_test_input() -> TestMtypeHash {
	TestMtypeHash::from_slice(&[3u8; 32])
}
pub(crate) fn get_no_key_call() -> Call {
	Call::Mtype(pallet_mtype::Call::anchor(get_no_key_test_input()))
}

impl did::DeriveDidCallAuthorizationVerificationKeyRelationship for Call {
	fn derive_verification_key_relationship(&self) -> Option<did::DidVerificationKeyRelationship> {
		if *self == get_mark_key_call() {
			Some(did::DidVerificationKeyRelationship::AssertionMethod)
		} else if *self == get_authentication_key_call() {
			Some(did::DidVerificationKeyRelationship::Authentication)
		} else if *self == get_delegation_key_call() {
			Some(did::DidVerificationKeyRelationship::CapabilityDelegation)
		} else {
			#[cfg(feature = "runtime-benchmarks")]
			if *self == Self::get_call_for_did_call_benchmark() {
				// Always require an authentication key to dispatch calls during benchmarking
				return Some(did::DidVerificationKeyRelationship::Authentication);
			}
			None
		}
	}

	// Always return a System::remark() extrinsic call
	#[cfg(feature = "runtime-benchmarks")]
	fn get_call_for_did_call_benchmark() -> Self {
		Call::System(frame_system::Call::remark(vec![]))
	}
}

pub fn generate_test_did_call(
	verification_key_required: did::DidVerificationKeyRelationship,
	caller: TestDidIdentifier,
) -> did::DidAuthorizedCallOperation<Test> {
	let call = match verification_key_required {
		DidVerificationKeyRelationship::AssertionMethod => get_mark_key_call(),
		DidVerificationKeyRelationship::Authentication => get_authentication_key_call(),
		DidVerificationKeyRelationship::CapabilityDelegation => get_delegation_key_call(),
		_ => get_no_key_call(),
	};
	did::DidAuthorizedCallOperation {
		did: caller,
		call,
		tx_counter: 1u64,
	}
}

// A test DID operation which can be crated to require any DID verification key
// type.
#[derive(Clone, Decode, Debug, Encode, PartialEq)]
pub struct TestDidOperation {
	pub did: TestDidIdentifier,
	pub verification_key_type: DidVerificationKeyRelationship,
	pub tx_counter: u64,
}

impl DidOperation<Test> for TestDidOperation {
	fn get_verification_key_relationship(&self) -> DidVerificationKeyRelationship {
		self.verification_key_type
	}

	fn get_did(&self) -> &TestDidIdentifier {
		&self.did
	}

	fn get_tx_counter(&self) -> u64 {
		self.tx_counter
	}
}

#[allow(dead_code)]
pub fn initialize_logger() {
	env_logger::builder().is_test(true).try_init();
}

#[derive(Clone)]
pub struct ExtBuilder {
	dids_stored: Vec<(TestDidIdentifier, did::DidDetails<Test>)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { dids_stored: vec![] }
	}
}

impl ExtBuilder {
	pub fn with_dids(mut self, dids: Vec<(TestDidIdentifier, did::DidDetails<Test>)>) -> Self {
		self.dids_stored = dids;
		self
	}

	pub fn build(self, ext: Option<sp_io::TestExternalities>) -> sp_io::TestExternalities {
		let mut ext = if let Some(ext) = ext {
			ext
		} else {
			let storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
			sp_io::TestExternalities::new(storage)
		};

		if !self.dids_stored.is_empty() {
			ext.execute_with(|| {
				self.dids_stored.iter().for_each(|did| {
					did::Did::<Test>::insert(did.0.clone(), did.1.clone());
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
