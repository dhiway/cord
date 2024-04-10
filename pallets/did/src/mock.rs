// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

use crate as pallet_did;
use codec::{Decode, Encode};
use cord_utilities::mock::*;
use frame_support::{derive_impl, parameter_types};
use frame_system::EnsureRoot;
use scale_info::TypeInfo;
use sp_core::{ecdsa, ed25519, sr25519, Pair};
use sp_runtime::{
	testing::H256,
	traits::{IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSignature, MultiSigner,
};

#[cfg(feature = "runtime-benchmarks")]
use frame_system::EnsureSigned;

use crate::{
	did_details::{
		DeriveDidCallAuthorizationVerificationKeyRelationship, DeriveDidCallKeyRelationshipResult,
		DidAuthorizedCallOperation, DidAuthorizedCallOperationWithVerificationRelationship,
		DidDetails, DidEncryptionKey, DidPublicKey, DidPublicKeyDetails, DidVerificationKey,
		DidVerificationKeyRelationship, RelationshipDeriveError,
	},
	utils as crate_utils, Config, KeyIdOf,
};
#[cfg(not(feature = "runtime-benchmarks"))]
use crate::{DidRawOrigin, EnsureDidOrigin};

pub(crate) type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type Signature = MultiSignature;
pub(crate) type AccountPublic = <Signature as Verify>::Signer;
pub(crate) type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub(crate) type DidIdentifier = AccountId;

frame_support::construct_runtime!(
	pub enum Test
	{
		Did: pallet_did,
		System: frame_system,
		Space: pallet_chain_space,
		Identifier: identifier,
		MockOrigin: mock_origin,
	}
);

parameter_types! {
	pub const SS58Prefix: u8 = 29;
	pub const BlockHashCount: u64 = 250;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Block = Block;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type SS58Prefix = SS58Prefix;
}

parameter_types! {
	#[derive(Clone, TypeInfo, Debug, PartialEq, Eq, Encode, Decode)]
	pub const MaxNewKeyAgreementKeys: u32 = 10u32;
	#[derive(Debug, Clone, Eq, PartialEq)]
	pub const MaxTotalKeyAgreementKeys: u32 = 10u32;
	// IMPORTANT: Needs to be at least MaxTotalKeyAgreementKeys + 3 (auth, delegation, assertion keys) for benchmarks!
	#[derive(Debug, Clone)]
	pub const MaxPublicKeysPerDid: u32 = 13u32;
	pub const MaxBlocksTxValidity: u64 = 300u64;
	pub const MaxNumberOfServicesPerDid: u32 = 25u32;
	pub const MaxServiceIdLength: u32 = 50u32;
	pub const MaxServiceTypeLength: u32 = 50u32;
	pub const MaxServiceUrlLength: u32 = 100u32;
	pub const MaxNumberOfTypesPerService: u32 = 1u32;
	pub const MaxNumberOfUrlsPerService: u32 = 1u32;
}

impl Config for Test {
	#[cfg(feature = "runtime-benchmarks")]
	type EnsureOrigin = EnsureSigned<DidIdentifier>;
	#[cfg(feature = "runtime-benchmarks")]
	type OriginSuccess = AccountId;

	#[cfg(not(feature = "runtime-benchmarks"))]
	type EnsureOrigin = EnsureDidOrigin<DidIdentifier, AccountId>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type OriginSuccess = DidRawOrigin<AccountId, DidIdentifier>;

	type DidIdentifier = DidIdentifier;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MaxNewKeyAgreementKeys = MaxNewKeyAgreementKeys;
	type MaxTotalKeyAgreementKeys = MaxTotalKeyAgreementKeys;
	type MaxPublicKeysPerDid = MaxPublicKeysPerDid;
	type MaxBlocksTxValidity = MaxBlocksTxValidity;
	type WeightInfo = ();
	type MaxNumberOfServicesPerDid = MaxNumberOfServicesPerDid;
	type MaxServiceIdLength = MaxServiceIdLength;
	type MaxServiceTypeLength = MaxServiceTypeLength;
	type MaxServiceUrlLength = MaxServiceUrlLength;
	type MaxNumberOfTypesPerService = MaxNumberOfTypesPerService;
	type MaxNumberOfUrlsPerService = MaxNumberOfUrlsPerService;
}

impl mock_origin::Config for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type AccountId = AccountId;
	type SubjectId = SubjectId;
}

parameter_types! {
	#[derive(Debug, Clone)]
	pub const MaxSpaceDelegates: u32 = 5u32;
}

impl pallet_chain_space::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type EnsureOrigin = mock_origin::EnsureDoubleOrigin<AccountId, SubjectId>;
	type OriginSuccess = mock_origin::DoubleOrigin<AccountId, SubjectId>;
	type SpaceCreatorId = SubjectId;
	type MaxSpaceDelegates = MaxSpaceDelegates;
	type ChainSpaceOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxEventsHistory: u32 = 6u32;
}

impl identifier::Config for Test {
	type MaxEventsHistory = MaxEventsHistory;
}

pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);
pub(crate) const ACCOUNT_01: AccountId = AccountId::new([2u8; 32]);

pub(crate) const AUTH_SEED_0: [u8; 32] = [4u8; 32];
pub(crate) const AUTH_SEED_1: [u8; 32] = [40u8; 32];
pub(crate) const ENC_SEED_0: [u8; 32] = [254u8; 32];
pub(crate) const ENC_SEED_1: [u8; 32] = [255u8; 32];
pub(crate) const ATT_SEED_0: [u8; 32] = [6u8; 32];
pub(crate) const ATT_SEED_1: [u8; 32] = [60u8; 32];
pub(crate) const DEL_SEED_0: [u8; 32] = [7u8; 32];
pub(crate) const DEL_SEED_1: [u8; 32] = [70u8; 32];

/// Solely used to fill public keys in unit tests to check for correct error
/// throws. Thus, it does not matter whether the correct key types get added
/// such that we can use the ed25519 for all key types per default.
pub(crate) fn fill_public_keys(mut did_details: DidDetails<Test>) -> DidDetails<Test> {
	while (did_details.public_keys.len() as u32) < <Test as Config>::MaxPublicKeysPerDid::get() {
		did_details
			.public_keys
			.try_insert(
				H256::random(),
				DidPublicKeyDetails {
					key: DidPublicKey::from(DidVerificationKey::from(ed25519::Public::from_h256(
						H256::random(),
					))),
					block_number: 0u64,
				},
			)
			.expect("Should not exceed BoundedBTreeMap size due to prior check");
	}
	did_details
}

pub fn get_did_identifier_from_ed25519_key(public_key: ed25519::Public) -> DidIdentifier {
	MultiSigner::from(public_key).into_account()
}

pub fn get_did_identifier_from_sr25519_key(public_key: sr25519::Public) -> DidIdentifier {
	MultiSigner::from(public_key).into_account()
}

pub fn get_did_identifier_from_ecdsa_key(public_key: ecdsa::Public) -> DidIdentifier {
	MultiSigner::from(public_key).into_account()
}

pub fn get_ed25519_authentication_key(seed: &[u8; 32]) -> ed25519::Pair {
	ed25519::Pair::from_seed(seed)
}

pub fn get_sr25519_authentication_key(seed: &[u8; 32]) -> sr25519::Pair {
	sr25519::Pair::from_seed(seed)
}

pub fn get_ecdsa_authentication_key(seed: &[u8; 32]) -> ecdsa::Pair {
	ecdsa::Pair::from_seed(seed)
}

pub fn get_x25519_encryption_key(seed: &[u8; 32]) -> DidEncryptionKey {
	DidEncryptionKey::X25519(*seed)
}

pub fn get_ed25519_assertion_key(seed: &[u8; 32]) -> ed25519::Pair {
	ed25519::Pair::from_seed(seed)
}

pub fn get_sr25519_assertion_key(seed: &[u8; 32]) -> sr25519::Pair {
	sr25519::Pair::from_seed(seed)
}

pub fn get_ecdsa_assertion_key(seed: &[u8; 32]) -> ecdsa::Pair {
	ecdsa::Pair::from_seed(seed)
}

//pub fn get_ed25519_delegation_key(seed: &[u8; 32]) -> ed25519::Pair {
//	ed25519::Pair::from_seed(seed)
//}

pub fn get_sr25519_delegation_key(seed: &[u8; 32]) -> sr25519::Pair {
	sr25519::Pair::from_seed(seed)
}

pub fn get_ecdsa_delegation_key(seed: &[u8; 32]) -> ecdsa::Pair {
	ecdsa::Pair::from_seed(seed)
}

pub fn generate_key_id(key: &DidPublicKey<AccountId>) -> KeyIdOf<Test> {
	crate_utils::calculate_key_id::<Test>(key)
}

pub(crate) fn get_assertion_key_test_input() -> H256 {
	H256::try_from([1u8; 32]).unwrap()
}
pub(crate) fn get_assertion_key_call() -> RuntimeCall {
	RuntimeCall::Space(pallet_chain_space::Call::create {
		space_code: get_assertion_key_test_input(),
	})
}
pub(crate) fn get_authentication_key_test_input() -> H256 {
	H256::try_from([2u8; 32]).unwrap()
}
pub(crate) fn get_authentication_key_call() -> RuntimeCall {
	RuntimeCall::Space(pallet_chain_space::Call::create {
		space_code: get_authentication_key_test_input(),
	})
}
pub(crate) fn get_delegation_key_test_input() -> H256 {
	H256::try_from([3u8; 32]).unwrap()
}
pub(crate) fn get_delegation_key_call() -> RuntimeCall {
	RuntimeCall::Space(pallet_chain_space::Call::create {
		space_code: get_delegation_key_test_input(),
	})
}
pub(crate) fn get_none_key_test_input() -> H256 {
	H256::try_from([4u8; 32]).unwrap()
}
pub(crate) fn get_none_key_call() -> RuntimeCall {
	RuntimeCall::Space(pallet_chain_space::Call::create { space_code: get_none_key_test_input() })
}

#[cfg(not(feature = "runtime-benchmarks"))]
pub(crate) fn build_test_origin(account: AccountId, did: DidIdentifier) -> RuntimeOrigin {
	crate::DidRawOrigin::new(account, did).into()
}

#[cfg(feature = "runtime-benchmarks")]
pub(crate) fn build_test_origin(account: AccountId, _did: DidIdentifier) -> RuntimeOrigin {
	RuntimeOrigin::signed(account)
}
impl DeriveDidCallAuthorizationVerificationKeyRelationship for RuntimeCall {
	fn derive_verification_key_relationship(&self) -> DeriveDidCallKeyRelationshipResult {
		if *self == get_assertion_key_call() {
			Ok(DidVerificationKeyRelationship::AssertionMethod)
		} else if *self == get_authentication_key_call() {
			Ok(DidVerificationKeyRelationship::Authentication)
		} else if *self == get_delegation_key_call() {
			Ok(DidVerificationKeyRelationship::CapabilityDelegation)
		} else {
			#[cfg(feature = "runtime-benchmarks")]
			if *self == Self::get_call_for_did_call_benchmark() {
				// Always require an authentication key to dispatch calls during benchmarking
				return Ok(DidVerificationKeyRelationship::Authentication);
			}
			Err(RelationshipDeriveError::NotCallableByDid)
		}
	}

	// Always return a System::remark() extrinsic call
	#[cfg(feature = "runtime-benchmarks")]
	fn get_call_for_did_call_benchmark() -> Self {
		RuntimeCall::System(frame_system::Call::remark { remark: sp_std::vec![] })
	}
}

pub fn generate_test_did_call(
	verification_key_required: DidVerificationKeyRelationship,
	caller: DidIdentifier,
	submitter: AccountId,
) -> DidAuthorizedCallOperationWithVerificationRelationship<Test> {
	let call = match verification_key_required {
		DidVerificationKeyRelationship::AssertionMethod => get_assertion_key_call(),
		DidVerificationKeyRelationship::Authentication => get_authentication_key_call(),
		DidVerificationKeyRelationship::CapabilityDelegation => get_delegation_key_call(),
		_ => get_none_key_call(),
	};
	DidAuthorizedCallOperationWithVerificationRelationship {
		operation: DidAuthorizedCallOperation {
			did: caller,
			call,
			tx_counter: 1u64,
			block_number: 0u64,
			submitter,
		},
		verification_key_relationship: verification_key_required,
	}
}
pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	#[cfg(feature = "runtime-benchmarks")]
	let keystore = sp_keystore::testing::MemoryKeystore::new();
	#[cfg(feature = "runtime-benchmarks")]
	ext.register_extension(sp_keystore::KeystoreExt(sp_std::sync::Arc::new(keystore)));
	ext.execute_with(|| System::set_block_number(1));
	ext
}
