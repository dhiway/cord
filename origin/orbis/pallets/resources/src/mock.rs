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

#![allow(unused)]

use crate::*;
use alloc::sync::Arc;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::{cell::RefCell, time::Duration};
use frame_support::{
	derive_impl,
	dispatch::{DispatchErrorWithPostInfo, GetDispatchInfo},
	parameter_types,
	storage::with_transaction,
	traits::{OffchainWorker, OriginTrait},
};
use frame_system::{
	offchain::{CreateAuthorizedTransaction, CreateBare, CreateTransaction, CreateTransactionBase},
	AuthorizeCall, EnsureRoot,
};
use indiv_support::traits::{
	ContextualAlias, Identifier, RevisedContextualAlias, RingExponent, RingIndex,
};
use scale_info::TypeInfo;
use sp_runtime::{
	offchain::{
		testing::{PoolState, TestOffchainExt, TestTransactionPoolExt},
		OffchainDbExt, OffchainWorkerExt, TransactionPoolExt,
	},
	testing::H256,
	traits::{Applyable, BlakeTwo256, Checkable, ConstU32, ConstU64, ConstUint, IdentityLookup},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidityError},
	AccountId32, BuildStorage, DispatchError, TransactionOutcome,
};
use std::collections::BTreeMap;
pub use verifiable::{mock::Mock as MockCrypto, GenerateVerifiable};

pub type Header = sp_runtime::generic::Header<u64, sp_runtime::traits::BlakeTwo256>;
pub type TransactionExtension = (AuthorizeCall<Test>,);
pub type Block = sp_runtime::generic::Block<Header, Extrinsic>;
pub type Extrinsic = sp_runtime::generic::UncheckedExtrinsic<
	AccountId32,
	RuntimeCall,
	AccountAuthority,
	TransactionExtension,
>;

impl<LocalCall> CreateBare<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_bare(call: RuntimeCall) -> Extrinsic {
		Extrinsic::new_bare(call)
	}
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

/// Convert a `u64` to an `AccountId32`.
pub fn id_to_account(id: u64) -> AccountId32 {
	let mut bytes = [0; 32];
	bytes[..8].copy_from_slice(&id.to_le_bytes());
	AccountId32::new(bytes)
}

/// Convert a `u64` to an `Alias`.
pub fn id_to_alias(id: u64) -> Alias {
	let mut bytes = [0; 32];
	bytes[..8].copy_from_slice(&id.to_le_bytes());
	bytes
}

/// Helper function to create a bounded vec username
pub fn username<T: Config>(s: &[u8]) -> Username {
	s.to_vec().try_into().unwrap()
}

/// Helper to create a communication identifier
pub fn comm_id(s: &[u8]) -> CommunicationIdentifier {
	let mut buf = Vec::new();
	while buf.len() < 65 {
		buf.extend_from_slice(s);
	}
	if buf.len() > 65 {
		buf.truncate(65);
	}
	buf.try_into().unwrap()
}

/// Helper to create a valid signature for a lite identity proof
pub fn mock_lite_proof(lite_account: AccountId32) -> AccountAuthority {
	AccountAuthority(lite_account)
}

/// Helper to mock the LitePerson origin
pub fn lite_person_origin(account: u64) -> RuntimeOrigin {
	RuntimeOrigin::from(OriginCaller::PeopleLite(indiv_pallet_people_lite::Origin::LitePerson(
		id_to_account(account),
	)))
}

/// Helper to mock the Person origin
pub fn person_origin_for(alias_id: u64, ring: RingIndex, revision: u32) -> RuntimeOrigin {
	person_origin_for_context(alias_id, RESOURCES_CONTEXT, ring, revision)
}

/// Helper to mock the Person origin in a specific context.
pub fn person_origin_for_context(
	alias_id: u64,
	context: Context,
	ring: RingIndex,
	revision: u32,
) -> RuntimeOrigin {
	let alias = id_to_alias(alias_id);
	let contextual_alias = ContextualAlias { context, alias };
	let revised_alias = RevisedContextualAlias { ca: contextual_alias, ring, revision };
	RuntimeOrigin::from(OriginCaller::People(indiv_pallet_people::Origin::PersonalAlias(
		revised_alias,
	)))
}

/// Helper to mock the Resources friend request origin.
pub fn friend_request_origin(alias_id: u64) -> RuntimeOrigin {
	RuntimeOrigin::from(OriginCaller::Resources(crate::Origin::FriendRequestAlias(id_to_alias(
		alias_id,
	))))
}

/// Helper to mock the Resources statement store slot origin.
pub fn stmt_store_slot_origin(alias_id: u64) -> RuntimeOrigin {
	RuntimeOrigin::from(OriginCaller::Resources(crate::Origin::StmtStoreAlias(id_to_alias(
		alias_id,
	))))
}

/// Helper to mock the Resources long-term storage claim origin for the people collection.
pub fn lts_people_origin(alias_id: u64) -> RuntimeOrigin {
	RuntimeOrigin::from(OriginCaller::Resources(crate::Origin::LongTermStorageClaim {
		alias: id_to_alias(alias_id),
		collection: crate::types::MembershipCollection::People,
		payer: id_to_account(99),
	}))
}

/// Helper to mock the Resources long-term storage claim origin for the lite-people collection.
pub fn lts_lite_people_origin(alias_id: u64) -> RuntimeOrigin {
	RuntimeOrigin::from(OriginCaller::Resources(crate::Origin::LongTermStorageClaim {
		alias: id_to_alias(alias_id),
		collection: crate::types::MembershipCollection::LitePeople,
		payer: id_to_account(99),
	}))
}

/// Helper to advance time (seconds)
pub fn advance_time_sec(secs: u64) {
	let current_time = TestClock::now().as_secs();
	TestClock::set_time(Duration::from_secs(current_time + secs));
}

/// Helper to set current time (seconds)
pub fn set_time_sec(secs: u64) {
	TestClock::set_time(Duration::from_secs(secs));
}

/// A signature type that is always successful for a given account
#[derive(
	PartialEq,
	Eq,
	Clone,
	Encode,
	Decode,
	DecodeWithMemTracking,
	Debug,
	Hash,
	PartialOrd,
	Ord,
	MaxEncodedLen,
	TypeInfo,
)]
pub struct AccountAuthority(pub AccountId32);

impl IdentifyAccount for AccountAuthority {
	type AccountId = AccountId32;
	fn into_account(self) -> Self::AccountId {
		self.0
	}
}

impl Verify for AccountAuthority {
	type Signer = Self;

	fn verify<L: sp_runtime::traits::Lazy<[u8]>>(
		&self,
		_msg: L,
		signer: &<Self::Signer as IdentifyAccount>::AccountId,
	) -> bool {
		self.0 == *signer
	}
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		ChunksManager: indiv_pallet_chunks_manager,
		Members: indiv_pallet_members,
		Resources: crate,
		People: indiv_pallet_people,
		PeopleLite: indiv_pallet_people_lite,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type RuntimeTask = RuntimeTask;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstUint<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstUint<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl indiv_pallet_chunks_manager::Config for Test {
	type WeightInfo = ();
	type Chunk = <MockCrypto as GenerateVerifiable>::StaticChunk;
	type PageSize = ConstU32<1024>;
	type ManagerOrigin = EnsureRoot<Self::AccountId>;
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

parameter_types! {
	pub const FlexibleRingExp: RingExponent = RingExponent::R2e9;
	pub const MockCollectionOwner: u32 = 1;
}

impl indiv_pallet_members::Config for Test {
	type WeightInfo = ();
	type Crypto = MockCrypto;
	type Location = u32;
	type ChunksManager = ChunksManager;
	type Clock = TestClock;
	type MaxCollections = ConstU32<10>;
	type OnboardingQueuePageSize = ConstU32<40>;
	type MaxFlexibleRingExponent = FlexibleRingExp;
	type RingBuildingMemberLimit = ConstU32<100>;
	type OldRootRetentionDuration = ConstU64<600>;
	type OnRingRootChange = ();
	type OffchainWorkerInterval = ConstU64<1>;
	type ManagerOrigin = EnsureRoot<Self::AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

thread_local! {
	pub static MOCK_UNIX_TIME: RefCell<Duration> = RefCell::new(Default::default());
	pub static BULLETIN_STORAGE_SHOULD_FAIL: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
	pub static BULLETIN_RESERVATIONS: RefCell<BTreeMap<ReservationId, (AccountId32, ReservationPurpose, u64)>> = RefCell::new(BTreeMap::new());
}

pub struct MockBulletinStorage;

impl indiv_support::traits::TwoPhaseStorage<AccountId32, ReservationId, ReservationPurpose, u64>
	for MockBulletinStorage
{
	fn reserve(
		id: ReservationId,
		who: &AccountId32,
		purpose: &ReservationPurpose,
		_len: u64,
		_count: u32,
		expires_at: u64,
	) -> sp_runtime::DispatchResult {
		if BULLETIN_STORAGE_SHOULD_FAIL.with(|f| f.get()) {
			Err(sp_runtime::DispatchError::Other("mock bulletin failure"))
		} else {
			BULLETIN_RESERVATIONS.with(|rows| {
				rows.borrow_mut().insert(id, (who.clone(), purpose.clone(), expires_at));
			});
			Ok(())
		}
	}

	fn cancel(who: &AccountId32, id: ReservationId) -> sp_runtime::DispatchResult {
		if BULLETIN_STORAGE_SHOULD_FAIL.with(|f| f.get()) {
			Err(sp_runtime::DispatchError::Other("mock bulletin failure"))
		} else {
			BULLETIN_RESERVATIONS.with(|rows| {
				let mut rows = rows.borrow_mut();
				let Some((owner, _, _)) = rows.get(&id) else {
					return Err(sp_runtime::DispatchError::Other("missing reservation"));
				};
				if owner != who {
					return Err(sp_runtime::DispatchError::Other("not owner"));
				}
				rows.remove(&id);
				Ok(())
			})?;
			Ok(())
		}
	}

	fn expire_due(now: u64, limit: u32) -> Result<Vec<ReservationId>, sp_runtime::DispatchError> {
		if BULLETIN_STORAGE_SHOULD_FAIL.with(|f| f.get()) {
			return Err(sp_runtime::DispatchError::Other("mock bulletin failure"));
		}
		Ok(BULLETIN_RESERVATIONS.with(|rows| {
			let mut rows = rows.borrow_mut();
			let ids = rows
				.iter()
				.filter_map(|(id, (_, _, expires_at))| (*expires_at <= now).then_some(*id))
				.take(limit as usize)
				.collect::<Vec<_>>();
			for id in &ids {
				rows.remove(id);
			}
			ids
		}))
	}
}

pub struct TestClock;

impl UnixTime for TestClock {
	fn now() -> Duration {
		MOCK_UNIX_TIME.with(|mock| *mock.borrow())
	}
}

impl TestClock {
	fn set_time(now: Duration) {
		MOCK_UNIX_TIME.with(|mock| *mock.borrow_mut() = now);
	}
}

pub struct MockPerson;
impl frame_support::traits::EnsureOriginWithArg<RuntimeOrigin, Context> for MockPerson {
	type Success = Alias;

	fn try_origin(
		origin: RuntimeOrigin,
		_context: &Context,
	) -> Result<Self::Success, RuntimeOrigin> {
		match origin.caller() {
			OriginCaller::People(indiv_pallet_people::Origin::PersonalAlias(contextual_alias)) => {
				Ok(contextual_alias.ca.alias)
			},
			_ => Err(origin),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(_context: &Context) -> Result<RuntimeOrigin, ()> {
		unimplemented!()
	}
}

impl indiv_support::traits::CountedMembers for MockPerson {
	fn active_count() -> u32 {
		0
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn set_active_count(_count: u32) {}
}

pub struct MockLitePerson;
impl frame_support::traits::EnsureOrigin<RuntimeOrigin> for MockLitePerson {
	type Success = AccountId32;

	fn try_origin(origin: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		match origin.caller() {
			OriginCaller::PeopleLite(indiv_pallet_people_lite::Origin::LitePerson(account)) => {
				Ok(account.clone())
			},
			_ => Err(origin),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		unimplemented!()
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub struct Helper;
#[cfg(feature = "runtime-benchmarks")]
impl indiv_pallet_people_lite::BenchmarkHelper<AccountId32, AccountAuthority> for Helper {
	fn sign_message(_message: &[u8]) -> (AccountId32, AccountAuthority) {
		([0u8; 32].into(), AccountAuthority([0u8; 32].into()))
	}
}

impl indiv_pallet_people_lite::Config for Test {
	type WeightInfo = ();
	type AttestationAllowanceManager = EnsureRoot<Self::AccountId>;
	type MemberService = Members;
	type CollectionOwner = MockCollectionOwner;
	type LiteRingExponent = FlexibleRingExp;
	type LiteOnboardingSize = ConstU32<10>;
	type AttestationSignature = AccountAuthority;
	type LiteConsumerRegistrar = Resources;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = Helper;
}

impl indiv_pallet_people::Config for Test {
	type WeightInfo = ();
	type AccountContexts = ();
	type OnboardingQueuePageSize = ConstU32<512>;
	type MemberService = Members;
	type CollectionOwner = MockCollectionOwner;
	type RingExponent = FlexibleRingExp;
	type StaleAliasCleanupInterval = ConstU64<5>;
	type SelfInclusionDelay = ConstU64<3600>;
	type ManagerOrigin = EnsureRoot<Self::AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
	pub LitePersonStatementLimit: sp_statement_store::StatementAllowance = sp_statement_store::StatementAllowance {
		max_size: 4 * 1024, // 4 KiB
		max_count: 10,
	};
	pub PersonStatementLimit: sp_statement_store::StatementAllowance = sp_statement_store::StatementAllowance {
		max_size: 16 * 1024, // 16 KiB
		max_count: 50,
	};
	pub AccountsApiAllowance: sp_statement_store::StatementAllowance = sp_statement_store::StatementAllowance {
		max_size: 1024,
		max_count: 2,
	};
	pub FriendRequestAllowance: sp_statement_store::StatementAllowance = sp_statement_store::StatementAllowance {
		max_size: 512,
		max_count: 1,
	};
	pub const FriendRequestSlotsPerPeriod: u8 = 8;
	pub const LiteFriendRequestSlotsPerPeriod: u8 = 4;
	pub const FriendRequestPeriodDuration: u32 = 24 * 60 * 60;
	pub const FriendRequestGraceWindow: u32 = 60 * 60;
	pub const FriendRequestRetentionDuration: u64 = 7 * 24 * 60 * 60;
	pub const StmtStoreSlotsPerPeriod: u32 = 8;
	pub const LiteStmtStoreSlotsPerPeriod: u32 = 4;
	pub const StmtStoreCleanupLimit: u32 = 10;
	pub const StmtStoreReplacementCooldown: u32 = 60 * 60; // 1 hour
	pub const StmtStoreGraceWindow: u32 = 2 * 24 * 60 * 60;
	pub const LongTermStoragePeriodDuration: u32 = 24 * 60 * 60;
	pub const LongTermStorageGraceWindow: u32 = 60 * 60;
	pub const LongTermStorageClaimsPerPeriod: u8 = 10;
	pub const LongTermStorageCleanupLimit: u32 = 50;
	pub LongTermStorageAllowanceForPeople: LongTermStorageAllocation = LongTermStorageAllocation {
		transactions: 32,
		bytes: 64 * 1024 * 1024,
	};
	pub LongTermStorageAllowanceForLitePeople: LongTermStorageAllocation = LongTermStorageAllocation {
		transactions: 8,
		bytes: 2 * 1024 * 1024,
	};
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl benchmarking::BenchmarkHelper<Test> for BenchmarkHelper {
	fn set_time(now: Duration) {
		MOCK_UNIX_TIME.with(|mock| *mock.borrow_mut() = now);
	}

	fn sign_message(message: &[u8]) -> (AccountId32, AccountAuthority) {
		let account = AccountId32::new([42u8; 32]);
		let signature = AccountAuthority(account.clone());
		(account, signature)
	}
}

impl Config for Test {
	type WeightInfo = ();
	type MemberService = Members;
	type MaxUsernameLength = ConstU32<32>;
	type MinUsernameLength = ConstU32<7>;
	type PersonAuthDuration = ConstU32<20>;
	type MinPersonAuthUpdateInterval = ConstU32<10>;
	type MaxReservationQueueLength = ConstU32<10>;
	type AccountsApiAllowance = AccountsApiAllowance;
	type StmtStoreSlotsPerPeriod = StmtStoreSlotsPerPeriod;
	type LiteStmtStoreSlotsPerPeriod = LiteStmtStoreSlotsPerPeriod;
	type StmtStoreCleanupLimit = StmtStoreCleanupLimit;
	type StmtStoreReplacementCooldown = StmtStoreReplacementCooldown;
	type StmtStoreGraceWindow = StmtStoreGraceWindow;
	type FriendRequestAllowance = FriendRequestAllowance;
	type FriendRequestSlotsPerPeriod = FriendRequestSlotsPerPeriod;
	type LiteFriendRequestSlotsPerPeriod = LiteFriendRequestSlotsPerPeriod;
	type FriendRequestPeriodDuration = FriendRequestPeriodDuration;
	type FriendRequestGraceWindow = FriendRequestGraceWindow;
	type FriendRequestRetentionDuration = FriendRequestRetentionDuration;
	type OffchainWorkerInterval = ConstU64<1>;
	type EnsurePerson = indiv_pallet_people::EnsurePersonalAliasInContext<Test>;
	type EnsureLitePerson = indiv_pallet_people_lite::EnsureLitePerson<Test>;
	type Clock = TestClock;
	type OffchainSignature = AccountAuthority;
	type LitePersonStatementLimit = LitePersonStatementLimit;
	type PersonStatementLimit = PersonStatementLimit;
	type ManagerOrigin = EnsureRoot<Self::AccountId>;
	type LongTermStoragePeriodDuration = LongTermStoragePeriodDuration;
	type LongTermStorageGraceWindow = LongTermStorageGraceWindow;
	type LongTermStorageClaimsPerPeriod = LongTermStorageClaimsPerPeriod;
	type LongTermStorageAllowanceForPeople = LongTermStorageAllowanceForPeople;
	type LongTermStorageAllowanceForLitePeople = LongTermStorageAllowanceForLitePeople;
	type LongTermStorageDataStore = MockBulletinStorage;
	type LongTermStorageCleanupLimit = LongTermStorageCleanupLimit;
	type MaxReservations = ConstU32<256>;
	type StorageReservationDuration = ConstU64<100>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = BenchmarkHelper;
	#[cfg(feature = "runtime-benchmarks")]
	type MetaPolicyBenchmarkHelper = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	use codec::Encode;

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

thread_local! {
	static TRANSACTION_POOL: RefCell<Arc<parking_lot::RwLock<PoolState>>> =
		RefCell::new(Arc::new(parking_lot::RwLock::new(PoolState {
			transactions: Vec::new(),
		})));
}

pub type Executive = frame_executive::Executive<
	Test,
	Block,
	frame_system::ChainContext<Test>,
	Test,
	AllPalletsWithSystem,
	(),
>;

pub fn advance_to_block(target_block: frame_system::pallet_prelude::BlockNumberFor<Test>) {
	loop {
		let current = frame_system::Pallet::<Test>::block_number();
		if current >= target_block {
			break;
		}

		AllPalletsWithSystem::offchain_worker(current);

		let next = current.saturating_add(1u64);
		frame_system::Pallet::<Test>::initialize(&next, &Default::default(), &Default::default());

		let transactions = {
			TRANSACTION_POOL.with_borrow_mut(|pool| std::mem::take(&mut pool.write().transactions))
		};
		for tx in transactions {
			let tx = Decode::decode(&mut &tx[..]).unwrap();
			Executive::apply_extrinsic(tx).expect("tx valid").expect("tx succeeds");
		}
	}
}

/// We gather both error into a single type in order to do `assert_ok` and `assert_err` safely.
/// Otherwise, we can easily miss the inner error in a `Result<Result<_, _>, _>`.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TransactionExecutionError {
	Validity(TransactionValidityError),
	// This ignores the post info.
	Dispatch(DispatchErrorWithPostInfo),
}

impl TransactionExecutionError {
	#[allow(unused)]
	pub fn unwrap_dispatch(self) -> DispatchErrorWithPostInfo {
		let Self::Dispatch(error) = self else {
			panic!("validity error unwrapped as dispatch");
		};
		error
	}
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

/// Execute a bare extrinsic with the given call.
pub fn exec_tx(x: Extrinsic) -> Result<(), TransactionExecutionError> {
	let info = x.get_dispatch_info();
	let len = x.encoded_size();

	let checked = Checkable::check(x, &frame_system::ChainContext::<Test>::default())?;

	// validation is always rollbacked in production.
	with_transaction(|| {
		let valid = checked.validate::<Test>(TransactionSource::External, &info, len);

		TransactionOutcome::Rollback(Result::<_, DispatchError>::Ok(valid))
	})
	.unwrap()?;

	checked.apply::<Test>(&info, len)??;

	Ok(())
}
