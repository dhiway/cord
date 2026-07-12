// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
	extension::{AsPerson, AsPersonInfo},
	*,
};
use core::time::Duration;
use frame_support::{
	assert_ok, derive_impl, dispatch::DispatchErrorWithPostInfo, parameter_types,
	storage::with_transaction, traits::UnixTime, weights::RuntimeDbWeight,
};
use frame_system::{
	offchain::{CreateAuthorizedTransaction, CreateTransaction, CreateTransactionBase},
	ChainContext,
};
use indiv_support::traits::RingExponent;
use sp_core::{ConstU16, ConstU32, ConstU64, H256};
use sp_runtime::{
	testing::UintAuthorityId,
	traits::{Applyable, BlakeTwo256, Checkable, IdentityLookup},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidityError},
	BuildStorage, DispatchError, Weight,
};
use verifiable::{mock::Mock, Alias, AliasVec, Error as VerifiableError, GenerateVerifiable};

// First ring, used in testing.
pub const RI_ZERO: RingIndex = 0;

pub struct MockTime;
impl UnixTime for MockTime {
	fn now() -> Duration {
		Duration::from_secs(1_000_000)
	}
}

const EXTENSION_VERSION: u8 = 0;
pub type TransactionExtension = (AsPerson<Test>, frame_system::CheckNonce<Test>);
pub type Header = sp_runtime::generic::Header<u64, sp_runtime::traits::BlakeTwo256>;
pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<
	u64,
	RuntimeCall,
	sp_runtime::testing::UintAuthorityId,
	TransactionExtension,
>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		ChunksManager: indiv_pallet_chunks_manager,
		Members: indiv_pallet_members,
		PeoplePallet: crate,
	}
);

parameter_types! {
	pub const MockDbWeight: RuntimeDbWeight = RuntimeDbWeight {
		read: 10,
		write: 20,
	};
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = MockDbWeight;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub type TxExtension = (frame_system::AuthorizeCall<Test>, TransactionExtension);
pub type Extrinsic =
	sp_runtime::generic::UncheckedExtrinsic<u64, RuntimeCall, UintAuthorityId, TxExtension>;

impl<LocalCall> CreateTransactionBase<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	type Extrinsic = Extrinsic;
	type RuntimeCall = RuntimeCall;
}

impl<LocalCall> CreateTransaction<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	type Extension = TxExtension;
	fn create_transaction(
		call: <Self as CreateTransactionBase<LocalCall>>::RuntimeCall,
		extension: Self::Extension,
	) -> Self::Extrinsic {
		Extrinsic::new_transaction(call, extension)
	}
}

impl<LocalCall> CreateAuthorizedTransaction<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_extension() -> TxExtension {
		(
			frame_system::AuthorizeCall::new(),
			(AsPerson::new(None), frame_system::CheckNonce::from(0)),
		)
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub struct ChunksBenchHelper;

#[cfg(feature = "runtime-benchmarks")]
impl indiv_pallet_chunks_manager::BenchmarkHelper<<MockCrypto as GenerateVerifiable>::StaticChunk>
	for ChunksBenchHelper
{
	fn chunk_page() -> Vec<<MockCrypto as GenerateVerifiable>::StaticChunk> {
		[(); 1024].to_vec()
	}
}

impl indiv_pallet_chunks_manager::Config for Test {
	type WeightInfo = ();
	type Chunk = <MockCrypto as GenerateVerifiable>::StaticChunk;
	type PageSize = ConstU32<1024>;
	type ManagerOrigin = frame_system::EnsureRoot<Self::AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ChunksBenchHelper;
}

pub const MOCK_CONTEXT: Context = *b"pop:polkadot.network/mock       ";

thread_local! {
	static EXTRA_CONTEXTS: core::cell::RefCell<Vec<Context>> = const { core::cell::RefCell::new(Vec::new()) };
	static MOCK_CONTEXT_ENABLED: core::cell::Cell<bool> = const { core::cell::Cell::new(true) };
}

parameter_types! {
	pub storage StaleAliasCleanupInterval: u64 = 5;
}

pub struct TestAccountContexts;
impl frame_support::traits::Contains<Context> for TestAccountContexts {
	fn contains(c: &Context) -> bool {
		if *c == MOCK_CONTEXT {
			return MOCK_CONTEXT_ENABLED.with(|e| e.get());
		}
		EXTRA_CONTEXTS.with(|ec| ec.borrow().contains(c))
	}
}

/// Disable `MOCK_CONTEXT` from `AccountContexts` for the duration of the closure.
pub fn with_mock_context_disabled<R>(f: impl FnOnce() -> R) -> R {
	MOCK_CONTEXT_ENABLED.with(|e| e.set(false));
	let r = f();
	MOCK_CONTEXT_ENABLED.with(|e| e.set(true));
	r
}

/// Add a dynamic context to `AccountContexts`.
pub fn add_extra_context(c: Context) {
	EXTRA_CONTEXTS.with(|ec| ec.borrow_mut().push(c));
}

/// Remove a dynamic context from `AccountContexts`.
pub fn remove_extra_context(c: &Context) {
	EXTRA_CONTEXTS.with(|ec| ec.borrow_mut().retain(|x| x != c));
}

pub struct MockWeights;
impl crate::WeightInfo for MockWeights {
	fn under_alias() -> sp_runtime::Weight {
		Weight::from_parts(3, 3)
	}

	fn set_alias_account() -> sp_runtime::Weight {
		Weight::from_parts(4, 4)
	}

	fn unset_alias_account() -> sp_runtime::Weight {
		Weight::from_parts(5, 5)
	}

	fn force_recognize_personhood(_n: u32) -> sp_runtime::Weight {
		Weight::from_parts(7, 7)
	}

	fn set_personal_id_account() -> sp_runtime::Weight {
		Weight::from_parts(8, 8)
	}

	fn unset_personal_id_account() -> sp_runtime::Weight {
		Weight::from_parts(9, 9)
	}

	fn as_person_alias_with_account() -> Weight {
		Weight::from_parts(20, 20)
	}

	fn as_person_identity_with_account() -> Weight {
		Weight::from_parts(21, 21)
	}

	fn as_person_alias_with_proof() -> Weight {
		Weight::from_parts(22, 22)
	}

	fn as_person_identity_with_proof() -> Weight {
		Weight::from_parts(23, 23)
	}

	fn as_person_alias_with_account_revised() -> Weight {
		Weight::from_parts(24, 24)
	}

	fn clean_up_stale_alias(n: u32) -> Weight {
		Weight::from_parts(25, 25).saturating_mul(n.into())
	}

	fn authorize_clean_up_stale_alias(n: u32) -> Weight {
		Weight::from_parts(26, 26).saturating_mul(n.into())
	}

	fn create_people_collection() -> Weight {
		Weight::from_parts(27, 27)
	}

	fn authorize_create_people_collection() -> Weight {
		Weight::from_parts(28, 28)
	}
}

pub const INVALID_MEMBER: [u8; 32] = [
	1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
	27, 28, 29, 30, 31, 32,
];

/// Wraps `Mock` but rejects `INVALID_MEMBER`.
pub struct MockCrypto;

impl GenerateVerifiable for MockCrypto {
	type Proof = <Mock as GenerateVerifiable>::Proof;
	type Member = <Mock as GenerateVerifiable>::Member;
	type Secret = <Mock as GenerateVerifiable>::Secret;
	type Members = <Mock as GenerateVerifiable>::Members;
	type Signature = <Mock as GenerateVerifiable>::Signature;
	type Commitment = <Mock as GenerateVerifiable>::Commitment;
	type StaticChunk = <Mock as GenerateVerifiable>::StaticChunk;
	type Intermediate = <Mock as GenerateVerifiable>::Intermediate;
	type Config = <Mock as GenerateVerifiable>::Config;

	fn start_members(config: Self::Config) -> Self::Intermediate {
		Mock::start_members(config)
	}
	fn push_members(
		intermediate: &mut Self::Intermediate,
		members: impl Iterator<Item = Self::Member>,
		lookup: impl Fn(core::ops::Range<usize>) -> Result<Vec<Self::StaticChunk>, ()>,
	) -> Result<(), VerifiableError> {
		Mock::push_members(intermediate, members, lookup)
	}
	fn finish_members(inter: Self::Intermediate) -> Self::Members {
		Mock::finish_members(inter)
	}
	fn new_secret(entropy: verifiable::Entropy) -> Self::Secret {
		Mock::new_secret(entropy)
	}
	fn member_from_secret(secret: &Self::Secret) -> Self::Member {
		Mock::member_from_secret(secret)
	}
	fn open(
		config: Self::Config,
		member: &Self::Member,
		members_iter: impl Iterator<Item = Self::Member>,
	) -> Result<Self::Commitment, VerifiableError> {
		Mock::open(config, member, members_iter)
	}
	fn create_multi_context(
		commitment: Self::Commitment,
		secret: &Self::Secret,
		contexts: &[&[u8]],
		message: &[u8],
	) -> Result<(Self::Proof, AliasVec), VerifiableError> {
		Mock::create_multi_context(commitment, secret, contexts, message)
	}
	fn validate_multi_context(
		config: Self::Config,
		proof: &Self::Proof,
		members: &Self::Members,
		contexts: &[&[u8]],
		message: &[u8],
	) -> Result<AliasVec, VerifiableError> {
		Mock::validate_multi_context(config, proof, members, contexts, message)
	}
	fn alias_in_context(secret: &Self::Secret, context: &[u8]) -> Result<Alias, VerifiableError> {
		Mock::alias_in_context(secret, context)
	}
	fn is_member_valid(member: &Self::Member) -> bool {
		*member != INVALID_MEMBER && Mock::is_member_valid(member)
	}
	fn sign(secret: &Self::Secret, message: &[u8]) -> Result<Self::Signature, VerifiableError> {
		Mock::sign(secret, message)
	}
	fn verify_signature(
		signature: &Self::Signature,
		message: &[u8],
		member: &Self::Member,
	) -> bool {
		Mock::verify_signature(signature, message, member)
	}
}

parameter_types! {
	pub const FlexibleRingExp: RingExponent = RingExponent::R2e9;
	pub const MockCollectionOwner: u32 = 1;
	pub const SelfInclusionDelayValue: u64 = 3600;
}

impl indiv_pallet_members::Config for Test {
	type WeightInfo = ();
	type Crypto = MockCrypto;
	type Location = u32;
	type ChunksManager = ChunksManager;
	type Clock = MockTime;
	type MaxCollections = ConstU32<1>;
	type OnboardingQueuePageSize = ConstU32<40>;
	type MaxFlexibleRingExponent = FlexibleRingExp;
	type RingBuildingMemberLimit = ConstU32<100>;
	type OldRootRetentionDuration = ConstU64<600>;
	type OnRingRootChange = ();
	type OffchainWorkerInterval = ConstU64<1>;
	type ManagerOrigin = frame_system::EnsureRoot<Self::AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

impl crate::Config for Test {
	type WeightInfo = MockWeights;
	type MemberService = Members;
	type RingExponent = FlexibleRingExp;
	type CollectionOwner = MockCollectionOwner;
	type AccountContexts = TestAccountContexts;
	type OnboardingQueuePageSize = ConstU32<40>;
	type StaleAliasCleanupInterval = StaleAliasCleanupInterval;
	type SelfInclusionDelay = SelfInclusionDelayValue;
	type ManagerOrigin = frame_system::EnsureRoot<Self::AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = BenchHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchHelper {}

#[cfg(feature = "runtime-benchmarks")]
impl<Chunk> BenchmarkHelper<Chunk> for BenchHelper
where
	Chunk: From<<MockCrypto as verifiable::GenerateVerifiable>::StaticChunk>,
{
	fn valid_account_context() -> Context {
		MOCK_CONTEXT
	}
	fn initialize_chunks() -> Vec<Chunk> {
		vec![]
	}
}

pub fn advance_to(b: u64) {
	while System::block_number() < b {
		System::set_block_number(System::block_number() + 1);
	}
}

pub struct ConfigRecord;

pub fn new_config() -> ConfigRecord {
	ConfigRecord
}

pub struct TestExt(ConfigRecord);
impl TestExt {
	pub fn new() -> Self {
		Self(new_config())
	}

	pub fn execute_with<R>(self, f: impl Fn() -> R) -> R {
		new_test_ext().execute_with(f)
	}
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let page_hashes = vec![];

	// let r9_chunks: Vec<<MockCrypto as GenerateVerifiable>::StaticChunk> =
	// 	(0..(1 << RingExponent::R2e9.exponent())).iter().map(|_| ()).collect();
	// let hash = chunks.using_encoded(|b| sp_io::hashing::blake2_256(b));
	// page_hashes.push((RingExponent::R2e9, hash));

	// let r10_chunks: Vec<<MockCrypto as GenerateVerifiable>::StaticChunk> =
	// 	(0..(1 << RingExponent::R2e10.exponent())).iter().map(|_| ()).collect();
	// let hash = chunks.using_encoded(|b| sp_io::hashing::blake2_256(b));
	// page_hashes.push((RingExponent::R2e10, hash));

	// let r14_chunks: Vec<<MockCrypto as GenerateVerifiable>::StaticChunk> =
	// 	(0..(1 << RingExponent::R2e14.exponent())).iter().map(|_| ()).collect();
	// let hash = chunks.using_encoded(|b| sp_io::hashing::blake2_256(b));
	// page_hashes.push((RingExponent::R2e14, hash));

	RuntimeGenesisConfig {
		system: Default::default(),
		chunks_manager: indiv_pallet_chunks_manager::GenesisConfig::<Test> {
			encoded_chunk_page_hashes: page_hashes,
			..Default::default()
		},
	}
	.build_storage()
	.unwrap()
	.into()
}

/// We gather both error into a single type in order to do `assert_ok` and `assert_err` safely.
/// Otherwise, we can easily miss the inner error in a `Resut<Resut<_, _>, _>`.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TransactionExecutionError {
	Validity(TransactionValidityError),
	// This ignores the post info.
	Dispatch(DispatchErrorWithPostInfo),
}

impl From<DispatchErrorWithPostInfo> for TransactionExecutionError {
	fn from(e: DispatchErrorWithPostInfo) -> Self {
		TransactionExecutionError::Dispatch(e)
	}
}

impl From<TransactionValidityError> for TransactionExecutionError {
	fn from(e: TransactionValidityError) -> Self {
		TransactionExecutionError::Validity(e)
	}
}

impl From<DispatchError> for TransactionExecutionError {
	fn from(e: DispatchError) -> Self {
		TransactionExecutionError::Dispatch(e.into())
	}
}

impl From<InvalidTransaction> for TransactionExecutionError {
	fn from(e: InvalidTransaction) -> Self {
		TransactionExecutionError::Validity(e.into())
	}
}

/// Execute a transaction with the given origin, call and transaction extension.
pub fn exec_tx(
	who: Option<u64>,
	tx_ext: TransactionExtension,
	call: impl Into<RuntimeCall>,
) -> Result<(), TransactionExecutionError> {
	let tx = match who {
		Some(who) => UncheckedExtrinsic::new_signed(call.into(), who, UintAuthorityId(who), tx_ext),
		None => UncheckedExtrinsic::new_transaction(call.into(), tx_ext),
	};

	let info = tx.get_dispatch_info();
	let len = tx.encoded_size();

	// Check and validate the extrinsic.
	let checked = Checkable::check(tx, &ChainContext::<Test>::default())?;
	with_transaction(|| {
		let valid = checked.validate::<Test>(TransactionSource::External, &info, len);
		sp_runtime::TransactionOutcome::Rollback(Result::<_, DispatchError>::Ok(valid))
	})
	.unwrap()?;
	// Finally, apply the extrinsic.
	checked.apply::<Test>(&info, len)??;

	Ok(())
}

pub fn exec_as_alias_tx(
	who: u64,
	call: impl Into<RuntimeCall>,
) -> Result<(), TransactionExecutionError> {
	let nonce = frame_system::Account::<Test>::get(who).nonce;
	let tx_ext = (
		AsPerson::new(Some(AsPersonInfo::AsPersonalAliasWithAccount(nonce))),
		frame_system::CheckNonce::from(nonce),
	);

	exec_tx(Some(who), tx_ext, call)
}

/// Execute a transaction with the revised contextual alias origin with a revision update.
pub fn exec_as_alias_with_updated_revision_tx(
	who: u64,
	key: &<MockCrypto as GenerateVerifiable>::Member,
	secret: &<MockCrypto as GenerateVerifiable>::Secret,
	call: impl Into<RuntimeCall>,
) -> Result<(), TransactionExecutionError> {
	let nonce = frame_system::Account::<Test>::get(who).nonce;
	let id = crate::Keys::<Test>::get(key).expect("id not found");
	let rev_ca = crate::AccountToAlias::<Test>::get(who).expect("alias account not found");
	let record = crate::People::<Test>::get(id).expect("record not found");
	let ring_position = Members::member_status(PEOPLE_MEMBER_IDENTIFIER, &record.key)
		.expect("member not registered");
	let ring_index = ring_position.ring_index().expect("member not included");
	let commitment = {
		let all_keys =
			indiv_pallet_members::RingKeys::<Test>::get((PEOPLE_MEMBER_IDENTIFIER, ring_index, 0));
		MockCrypto::open((), key, all_keys.into_iter()).unwrap()
	};
	let call: RuntimeCall = call.into();
	let other_tx_ext = (frame_system::CheckNonce::<Test>::from(0),);
	// Here we simply ignore implicit as they are null.
	let inherited_implication = (&EXTENSION_VERSION, &call, &other_tx_ext);
	let msg =
		(inherited_implication, "revise", &who, nonce).using_encoded(sp_io::hashing::blake2_256);
	let (proof, _alias) = MockCrypto::create(commitment, secret, &rev_ca.ca.context, &msg)
		.expect("proof creation failed");
	let tx_ext = (
		AsPerson::new(Some(AsPersonInfo::AsPersonalAliasWithAccountRevised(
			nonce,
			proof,
			ring_index,
			rev_ca.ca.context,
		))),
		frame_system::CheckNonce::from(0),
	);

	exec_tx(Some(who), tx_ext, call)
}

pub fn all_keys_in_ring(ring_index: RingIndex) -> Vec<<MockCrypto as GenerateVerifiable>::Member> {
	let ring_status =
		indiv_pallet_members::RingKeysStatus::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, ring_index);
	if ring_status.total == 0 {
		return vec![]
	}
	let members_page_size: u32 =
		<Test as indiv_pallet_members::Config>::MaxFlexibleRingExponent::get().ring_capacity();
	let page_count = (ring_status.total.saturating_sub(1) / members_page_size) + 1u32;
	let mut keys = vec![];
	for i in 0..page_count {
		let page_keys =
			indiv_pallet_members::RingKeys::<Test>::get((PEOPLE_MEMBER_IDENTIFIER, ring_index, i));
		keys.extend(page_keys);
	}
	assert_eq!(ring_status.total as usize, keys.len());
	keys
}

/// Call `set_alias_account` for the given personal id and account.
pub fn setup_alias_account(
	key: &<MockCrypto as GenerateVerifiable>::Member,
	secret: &<MockCrypto as GenerateVerifiable>::Secret,
	context: Context,
	account: u64,
) {
	let id = crate::Keys::<Test>::get(key).expect("id not found");
	let record = crate::People::<Test>::get(id).expect("record not found");
	let ring_position = Members::member_status(PEOPLE_MEMBER_IDENTIFIER, &record.key)
		.expect("member not registered");
	let ring_index = ring_position.ring_index().expect("member not included");
	let commitment = {
		let all_keys = all_keys_in_ring(ring_index);
		MockCrypto::open((), key, all_keys.into_iter()).unwrap()
	};
	let call = RuntimeCall::PeoplePallet(crate::Call::set_alias_account {
		account,
		call_valid_at: frame_system::Pallet::<Test>::block_number(),
	});
	let other_tx_ext = (frame_system::CheckNonce::<Test>::from(0),);
	// Here we simply ignore implicit as they are null.
	let msg = (&EXTENSION_VERSION, &call, &other_tx_ext).using_encoded(sp_io::hashing::blake2_256);
	let (proof, _alias) =
		MockCrypto::create(commitment, secret, &context, &msg).expect("proof creation failed");
	let tx_ext = (
		AsPerson::<Test>::new(Some(AsPersonInfo::AsPersonalAliasWithProof(
			proof, ring_index, context,
		))),
		other_tx_ext.0,
	);
	assert_ok!(exec_tx(None, tx_ext.clone(), call.clone()));
}
