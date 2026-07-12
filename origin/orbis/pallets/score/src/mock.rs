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
use codec::Encode;
use core::time::Duration;
use frame_support::{
	derive_impl,
	dispatch::{DispatchErrorWithPostInfo, GetDispatchInfo},
	parameter_types,
	storage::with_transaction,
	traits::{Everything, UnixTime},
};
use frame_system::{
	offchain::{CreateAuthorizedTransaction, CreateBare, CreateTransaction, CreateTransactionBase},
	EnsureRoot,
};
use indiv_support::traits::RingExponent;
use sp_core::{ConstU32, ConstU64, ConstUint, H256};
use sp_runtime::{
	testing::UintAuthorityId,
	traits::{Applyable, BlakeTwo256, Checkable, IdentityLookup},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidityError},
	BuildStorage, DispatchError, TransactionOutcome,
};
pub use verifiable::{mock::Mock, GenerateVerifiable};

pub struct MockTime;
impl UnixTime for MockTime {
	fn now() -> Duration {
		Duration::from_secs(1_000_000)
	}
}

pub type TransactionExtension = (crate::ScoreAsParticipant<Test>,);

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
		Members: indiv_pallet_members,
		PalletScore: crate,
		Balances: pallet_balances,
		People: indiv_pallet_people,
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
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstUint<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstUint<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl indiv_pallet_chunks_manager::Config for Test {
	type WeightInfo = ();
	type Chunk = <Mock as GenerateVerifiable>::StaticChunk;
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
	type Crypto = Mock;
	type Location = u32;
	type ChunksManager = ChunksManager;
	type Clock = MockTime;
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

parameter_types! {
	pub const ExistentialDeposit: u64 = 5;
	pub const MaxReserves: u32 = 50;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = u64;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
}

parameter_types! {
	pub const ScorePotId: PalletId = PalletId(*b"scorepot");
	pub const BalancesLocation: Location = Location::here();
	pub ScoreManagerAccount: Option<u64> = Some(99);
}

impl<LocalCall> CreateBare<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_bare(call: Self::RuntimeCall) -> Self::Extrinsic {
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
		(crate::ScoreAsParticipant::new(None),)
	}
}

impl indiv_pallet_people::Config for Test {
	type WeightInfo = ();
	type MemberService = Members;
	type CollectionOwner = MockCollectionOwner;
	type AccountContexts = Everything;
	type OnboardingQueuePageSize = ConstUint<512>;
	type RingExponent = FlexibleRingExp;
	type StaleAliasCleanupInterval = ConstUint<5>;
	type SelfInclusionDelay = ConstUint<3600>;
	type ManagerOrigin = EnsureRoot<Self::AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

impl crate::Config for Test {
	type WeightInfo = ();
	type EnsurePerson = indiv_pallet_people::EnsurePersonalAliasInContext<Test>;
	type ScorePotId = ScorePotId;
	type Currency = Balances;
	type CurrencyLocationInfo = BalancesLocation;
	type ManagerOrigin = EnsureRoot<Self::AccountId>;
	type ManagerAccountDefault = ScoreManagerAccount;
	type MaxPayoutRoundSchedules = ConstUint<10>;
	type OffchainWorkInterval = ConstUint<2>;
	type People = People;
	type Crypto = Mock;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = Test;
}

#[cfg(feature = "runtime-benchmarks")]
impl benchmarking::BenchmarkHelper<Test> for Test {
	fn create_member(seed: u64) -> MemberOf<Test> {
		let mut entropy = [0u8; 32];
		entropy[..8].copy_from_slice(&seed.to_le_bytes()[..]);
		entropy
	}
	fn setup_currency() {}
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	// Create a page of chunks and compute its hash
	let chunks: Vec<<Mock as GenerateVerifiable>::StaticChunk> = [(); 1024].to_vec();
	let encoded_chunks = chunks.encode();
	let page_hash = sp_io::hashing::blake2_256(&encoded_chunks);

	let storage = RuntimeGenesisConfig {
		system: Default::default(),
		chunks_manager: indiv_pallet_chunks_manager::GenesisConfig::<Test> {
			encoded_chunk_page_hashes: vec![(RingExponent::R2e9.exponent(), vec![page_hash])],
			..Default::default()
		},
		..Default::default()
	}
	.build_storage()
	.unwrap();
	let mut ext = sp_io::TestExternalities::from(storage);
	// 100k active members yields a personhood threshold of 21 and absence grace ratio of (1, 6).
	ext.execute_with(|| {
		indiv_pallet_members::ActiveMembers::<Test>::insert(
			indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
			100_000,
		);
		crate::PersonhoodThreshold::<Test>::put(21);
		crate::AbsenceGraceRatio::<Test>::put((1u8, 6u8));
	});
	ext
}

/// We gather both error into a single type in order to do `assert_ok` and `assert_err` safely.
/// Otherwise, we can easily miss the inner error in a `Resut<Resut<_, _>, _>`.
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

/// Execute a bare extrinsic with the given call.
pub fn exec_bare_tx(call: impl Into<RuntimeCall>) -> Result<(), TransactionExecutionError> {
	let x = Extrinsic::new_bare(call.into());

	exec_tx(x)
}

/// Execute a signed extrinsic with the given call.
#[allow(unused)]
pub fn exec_signed_tx(
	account: u64,
	call: impl Into<RuntimeCall>,
) -> Result<(), TransactionExecutionError> {
	let x = Extrinsic::new_signed(
		call.into(),
		account,
		UintAuthorityId(account),
		(crate::ScoreAsParticipant::<Test>::new(None),),
	);

	exec_tx(x)
}

/// Execute a signed extrinsic with the **participant** transaction extension and the given call.
pub fn exec_participant_score_tx(
	account: u64,
	tx_ext: crate::ScoreAsParticipantData<<Test as frame_system::Config>::Nonce>,
	call: impl Into<RuntimeCall>,
) -> Result<(), TransactionExecutionError> {
	let x = Extrinsic::new_signed(
		call.into(),
		account,
		UintAuthorityId(account),
		(crate::ScoreAsParticipant::<Test>::new(Some(tx_ext)),),
	);

	exec_tx(x)
}

/// Generate a mock key pair for testing
pub fn mock_key(
	id: u64,
) -> (<Mock as GenerateVerifiable>::Member, <Mock as GenerateVerifiable>::Secret) {
	let mut entropy = [0u8; 32];
	entropy[0..8].copy_from_slice(&id.to_le_bytes());
	let secret = Mock::new_secret(entropy);
	let member = Mock::member_from_secret(&secret);
	(member, secret)
}
