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

use crate::*;
use alloc::sync::Arc;
use codec::{Decode, Encode, MaxEncodedLen};
use core::{cell::RefCell, ops::Range, time::Duration};
use frame_support::{
	derive_impl, parameter_types,
	traits::{OffchainWorker, UnixTime},
	weights::RuntimeDbWeight,
};
use frame_system::{
	offchain::{CreateAuthorizedTransaction, CreateTransaction, CreateTransactionBase},
	AuthorizeCall,
};
use scale_info::TypeInfo;
use sp_core::{ConstU32, ConstU64, H256};
use sp_runtime::{
	offchain::{
		testing::{PoolState, TestOffchainExt, TestTransactionPoolExt},
		OffchainDbExt, OffchainWorkerExt, TransactionPoolExt,
	},
	testing::UintAuthorityId,
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, Weight,
};
pub use verifiable::{mock::Mock, Alias, AliasVec, GenerateVerifiable};

// Thread-local storage for mock time
thread_local! {
	static TIME: RefCell<Duration> = const { RefCell::new(Duration::from_secs(1_000_000)) };
}

pub struct MockTime;
impl UnixTime for MockTime {
	fn now() -> Duration {
		TIME.with(|t| *t.borrow())
	}
}

/// Advance time by the given number of seconds.
pub fn advance_time(seconds: u64) {
	TIME.with(|t| {
		*t.borrow_mut() += Duration::from_secs(seconds);
	});
}

// First ring, used in testing.
#[allow(dead_code)]
pub const RI_ZERO: RingIndex = 0;

pub type TransactionExtension = (AuthorizeCall<Test>,);

pub type Header = sp_runtime::generic::Header<u64, sp_runtime::traits::BlakeTwo256>;
pub type Block = sp_runtime::generic::Block<Header, Extrinsic>;
pub type Extrinsic = sp_runtime::generic::UncheckedExtrinsic<
	u64,
	RuntimeCall,
	UintAuthorityId,
	TransactionExtension,
>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		ChunksManager: indiv_pallet_chunks_manager,
		MembersPallet: crate,
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
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

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
	type Extension = TransactionExtension;
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
	fn create_extension() -> Self::Extension {
		(AuthorizeCall::new(),)
	}
}

impl indiv_pallet_chunks_manager::Config for Test {
	type WeightInfo = ();
	type Chunk = <MockCrypto as GenerateVerifiable>::StaticChunk;
	type PageSize = ConstU32<1024>;
	type ManagerOrigin = frame_system::EnsureRoot<Self::AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ChunksBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct ChunksBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl indiv_pallet_chunks_manager::BenchmarkHelper<()> for ChunksBenchmarkHelper {
	fn chunk_page() -> Vec<()> {
		vec![(); 1024]
	}
}

/// A simple location type for testing.
#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	Debug,
	TypeInfo,
	MaxEncodedLen,
	Default,
	codec::DecodeWithMemTracking,
)]
pub struct MockLocation(pub u32);

pub struct MockWeights;
impl crate::WeightInfo for MockWeights {
	fn set_onboarding_size() -> Weight {
		Weight::from_parts(10, 10)
	}

	fn merge_rings() -> Weight {
		Weight::from_parts(11, 11)
	}

	fn should_build_ring(_n: u32) -> Weight {
		Weight::from_parts(14, 14)
	}

	fn build_ring_r2e9(_n: u32) -> Weight {
		Weight::from_parts(14, 14)
	}

	fn build_ring_r2e10(_n: u32) -> Weight {
		Weight::from_parts(14, 14)
	}

	fn build_ring_r2e14(_n: u32) -> Weight {
		Weight::from_parts(14, 14)
	}

	fn onboard_members() -> Weight {
		Weight::from_parts(15, 15)
	}

	fn pending_suspensions_iteration() -> Weight {
		Weight::from_parts(1, 1)
	}

	fn remove_suspended_keys(_n: u32) -> Weight {
		Weight::from_parts(16, 16)
	}

	fn merge_queue_pages() -> Weight {
		Weight::from_parts(18, 18)
	}

	fn build_rings_base(_n: u32) -> Weight {
		Weight::from_parts(20, 20)
	}

	fn ensure_can_enqueue_ring_deletion() -> Weight {
		Weight::from_parts(21, 21)
	}

	fn enqueue_ring_deletion_authorized(pages: u32) -> Weight {
		Weight::from_parts(22 + pages as u64, 22)
	}

	fn ensure_can_delete_onboarding_queue_page() -> Weight {
		Weight::from_parts(22, 22)
	}

	fn delete_onboarding_queue_page_authorized(members: u32) -> Weight {
		Weight::from_parts(23 + members as u64, 23)
	}

	fn ensure_can_finalize_collection_deletion() -> Weight {
		Weight::from_parts(23, 23)
	}

	fn finalize_collection_deletion_authorized() -> Weight {
		Weight::from_parts(24, 24)
	}

	fn delete_ring_page_authorized(members: u32) -> Weight {
		Weight::from_parts(25 + members as u64, 25)
	}

	fn self_include_remove_page() -> Weight {
		Weight::from_parts(50, 50)
	}

	fn self_include_keep_page(_n: u32) -> Weight {
		Weight::from_parts(50, 50)
	}

	fn validate_self_include() -> Weight {
		Weight::from_parts(20, 20)
	}

	fn ensure_can_build_ring() -> Weight {
		Weight::from_parts(20, 20)
	}

	fn ensure_can_onboard_members() -> Weight {
		Weight::from_parts(20, 20)
	}

	fn ensure_can_merge_queue_pages() -> Weight {
		Weight::from_parts(20, 20)
	}

	fn ensure_can_remove_suspended_keys() -> Weight {
		Weight::from_parts(20, 20)
	}

	fn ensure_can_delete_ring_page() -> Weight {
		Weight::from_parts(10, 10)
	}

	fn ensure_can_clean_up_old_roots() -> Weight {
		Weight::from_parts(26, 26)
	}

	fn clean_up_old_roots_authorized(n: u32) -> Weight {
		Weight::from_parts(27 + n as u64 * 2, 27)
	}

	fn ensure_can_mark_ring_stale() -> Weight {
		Weight::from_parts(28, 28)
	}

	fn mark_ring_stale_authorized() -> Weight {
		Weight::from_parts(29, 29)
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
		lookup: impl Fn(Range<usize>) -> Result<Vec<Self::StaticChunk>, ()>,
	) -> Result<(), verifiable::Error> {
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
	) -> Result<Self::Commitment, verifiable::Error> {
		Mock::open(config, member, members_iter)
	}
	fn create_multi_context(
		commitment: Self::Commitment,
		secret: &Self::Secret,
		contexts: &[&[u8]],
		message: &[u8],
	) -> Result<(Self::Proof, AliasVec), verifiable::Error> {
		Mock::create_multi_context(commitment, secret, contexts, message)
	}
	fn validate_multi_context(
		config: Self::Config,
		proof: &Self::Proof,
		members: &Self::Members,
		contexts: &[&[u8]],
		message: &[u8],
	) -> Result<AliasVec, verifiable::Error> {
		Mock::validate_multi_context(config, proof, members, contexts, message)
	}
	fn alias_in_context(secret: &Self::Secret, context: &[u8]) -> Result<Alias, verifiable::Error> {
		Mock::alias_in_context(secret, context)
	}
	fn is_member_valid(member: &Self::Member) -> bool {
		*member != INVALID_MEMBER && Mock::is_member_valid(member)
	}
	fn sign(secret: &Self::Secret, message: &[u8]) -> Result<Self::Signature, verifiable::Error> {
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
	pub const FlexibleRingExp: indiv_support::traits::RingExponent = indiv_support::traits::RingExponent::R2e9;
}

impl crate::Config for Test {
	type WeightInfo = MockWeights;
	type Crypto = MockCrypto;
	type Location = MockLocation;
	type ChunksManager = ChunksManager;
	type Clock = MockTime;
	type MaxCollections = ConstU32<10>;
	// Use a larger page size for benchmarks to support full ring onboarding (R2e9 capacity = 255)
	#[cfg(not(feature = "runtime-benchmarks"))]
	type OnboardingQueuePageSize = ConstU32<40>;
	#[cfg(feature = "runtime-benchmarks")]
	type OnboardingQueuePageSize = ConstU32<255>;
	type MaxFlexibleRingExponent = FlexibleRingExp;
	type RingBuildingMemberLimit = ConstU32<100>;
	type OldRootRetentionDuration = ConstU64<600>; // 10 minutes in seconds for old root retention.
	type OnRingRootChange = ();
	type OffchainWorkerInterval = ConstU64<1>;
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
	fn initialize_chunks(_ring_size: indiv_support::traits::RingExponent) -> Vec<Chunk> {
		vec![]
	}
	fn set_time(now: Duration) {
		TIME.with(|t| *t.borrow_mut() = now);
	}
	fn set_valid_time() {
		TIME.with(|t| *t.borrow_mut() = Duration::from_secs(5));
	}
}

#[allow(dead_code)]
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
#[allow(dead_code)]
impl TestExt {
	pub fn new() -> Self {
		Self(new_config())
	}

	pub fn execute_with<R>(self, f: impl Fn() -> R) -> R {
		new_test_ext().execute_with(f)
	}
}

// Thread-local storage for the offchain transaction pool.
thread_local! {
	static TRANSACTION_POOL: RefCell<Arc<parking_lot::RwLock<PoolState>>> =
		RefCell::new(Arc::new(parking_lot::RwLock::new(PoolState {
			transactions: Vec::new(),
		})));
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	use codec::Encode;
	use indiv_support::traits::RingExponent;

	// Create a page of chunks and compute its hash
	let chunks: Vec<<MockCrypto as GenerateVerifiable>::StaticChunk> = [(); 1024].to_vec();
	let encoded_chunks = chunks.encode();
	let page_hash = sp_io::hashing::blake2_256(&encoded_chunks);

	let storage = RuntimeGenesisConfig {
		system: Default::default(),
		chunks_manager: indiv_pallet_chunks_manager::GenesisConfig::<Test> {
			encoded_chunk_page_hashes: vec![(RingExponent::R2e9.exponent(), vec![page_hash])],
			..Default::default()
		},
	}
	.build_storage()
	.unwrap();

	let mut ext: sp_io::TestExternalities = storage.into();
	let (offchain, _state) = TestOffchainExt::new();
	let (pool, state) = TestTransactionPoolExt::new();
	TRANSACTION_POOL.set(state);
	ext.register_extension(OffchainDbExt::new(offchain.clone()));
	ext.register_extension(OffchainWorkerExt::new(offchain));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext
}

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Test,
	Block,
	frame_system::ChainContext<Test>,
	Test,
	AllPalletsWithSystem,
	(),
>;

/// Advance the chain to `target_block`.
pub fn advance_to_block(target_block: frame_system::pallet_prelude::BlockNumberFor<Test>) {
	loop {
		let current = frame_system::Pallet::<Test>::block_number();
		if current >= target_block {
			break;
		}

		// Execute offchain worker.
		AllPalletsWithSystem::offchain_worker(current);

		// Advance time by 2 seconds (2000 ms).
		TIME.with(|t| {
			*t.borrow_mut() += Duration::from_millis(2_000);
		});

		// Advance block number by 1.
		let next = current.saturating_add(1u64);
		frame_system::Pallet::<Test>::initialize(&next, &Default::default(), &Default::default());

		// Run transactions submitted by the offchain worker.
		let transactions = {
			TRANSACTION_POOL.with_borrow_mut(|pool| std::mem::take(&mut pool.write().transactions))
		};
		for tx in transactions {
			let tx = Decode::decode(&mut &tx[..]).unwrap();
			Executive::apply_extrinsic(tx).expect("tx valid").expect("tx succeeds");
		}
	}
}

/// Inspect the offchain transaction pool, returning a decoded clone of every transaction currently
/// queued in it.
pub fn inspect_pool_transactions() -> Vec<Extrinsic> {
	TRANSACTION_POOL.with_borrow(|pool| {
		pool.read()
			.transactions
			.iter()
			.map(|bytes| Decode::decode(&mut &bytes[..]).expect("offchain tx decodes"))
			.collect()
	})
}

/// Run the offchain worker for the given `block`.
pub fn run_offchain_worker(block: u64) {
	AllPalletsWithSystem::offchain_worker(block);
}

/// Test identifier for a collection.
pub const TEST_IDENTIFIER: Identifier = [1u8; 32];
pub const TEST_IDENTIFIER_2: Identifier = [2u8; 32];
pub const NONEXISTENT_IDENTIFIER: Identifier = [255u8; 32];

/// Create a Flexible test collection with the given identifier.
pub fn create_test_collection(identifier: Identifier, onboarding_size: u32) {
	create_test_collection_with_mode(identifier, onboarding_size, RingMode::Flexible);
}

/// Create an AppendOnly test collection with the given identifier.
#[allow(dead_code)]
pub fn create_append_only_collection(identifier: Identifier, onboarding_size: u32) {
	create_test_collection_with_mode(identifier, onboarding_size, RingMode::AppendOnly);
}

/// Create a Flexible test collection with self-inclusion enabled.
#[allow(dead_code)]
pub fn create_self_inclusion_collection(
	identifier: Identifier,
	onboarding_size: u32,
	delay_secs: u64,
) {
	use indiv_support::traits::RingExponent;
	let owner = MockLocation(1);
	<MembersPallet as AppendOnlyMembers>::create_collection(
		owner,
		&identifier,
		onboarding_size,
		RingMode::Flexible,
		RingExponent::R2e9,
		Some(delay_secs),
	)
	.expect("Failed to create collection");
}

/// Create a test collection with the given identifier and mode.
pub fn create_test_collection_with_mode(
	identifier: Identifier,
	onboarding_size: u32,
	mode: RingMode,
) {
	use indiv_support::traits::RingExponent;
	let owner = MockLocation(1);
	<MembersPallet as AppendOnlyMembers>::create_collection(
		owner,
		&identifier,
		onboarding_size,
		mode,
		RingExponent::R2e9,
		None,
	)
	.expect("Failed to create collection");
}

/// Generate members for a collection.
pub fn generate_members(
	identifier: Identifier,
	start: u8,
	end: u8,
) -> Vec<(MemberOf<Test>, SecretOf<Test>)> {
	let mut members = Vec::new();
	for i in start..=end {
		let secret = MockCrypto::new_secret([i; 32]);
		let public = MockCrypto::member_from_secret(&secret);
		members.push((public, secret));
	}

	let member_keys: Vec<_> = members.iter().map(|(m, _)| *m).collect();
	<MembersPallet as AppendOnlyMembers>::add_members(&identifier, member_keys)
		.expect("Failed to add members");

	members
}

/// Create a unique secret using a thread-local counter.
pub fn create_unique_secret() -> SecretOf<Test> {
	use sp_io::hashing::twox_256;

	thread_local! {
		static UNIQUE_SECRET_COUNTER: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
	}

	let seed = UNIQUE_SECRET_COUNTER.with(|c| {
		let v = c.get();
		c.set(v + 1);
		v
	});
	MockCrypto::new_secret(twox_256(&seed.to_le_bytes()))
}
