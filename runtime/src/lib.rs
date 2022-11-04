// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

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

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "512"]
// The `from_over_into` warning originates from `construct_runtime` macro.
#![allow(clippy::from_over_into)]
#![warn(unused_crate_dependencies)]

use codec::{Decode, Encode, MaxEncodedLen};
use cord_primitives::{prod_or_fast, AccountIndex, Balance, BlockNumber, Hash, Index, Moment};
pub use cord_primitives::{AccountId, Signature};

use frame_support::{
	construct_runtime,
	dispatch::DispatchClass,
	parameter_types,
	traits::{
		Contains, EitherOfDiverse, InstanceFilter, KeyOwnerProofSystem, LockIdentifier,
		PrivilegeCmp,
	},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		ConstantMultiplier, Weight,
	},
	PalletId, RuntimeDebug,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot, EnsureSigned,
};
use pallet_grandpa::{
	fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};

use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_session::historical as pallet_session_historical;
pub use pallet_transaction_payment::{
	CurrencyAdapter, Multiplier, OnChargeTransaction, TargetedFeeAdjustment,
};
use pallet_transaction_payment::{FeeDetails, RuntimeDispatchInfo};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_inherents::{CheckInherentsResult, InherentData};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdLookup, BlakeTwo256, Block as BlockT, Extrinsic as ExtrinsicT, NumberFor,
		OpaqueKeys, SaturatedConversion, Verify,
	},
	transaction_validity::{TransactionPriority, TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, FixedPointNumber, Perbill, Permill, Perquintill,
};
use sp_staking::SessionIndex;
use sp_std::{cmp::Ordering, prelude::*};

#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::const_assert;

pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_staking::StakerStatus;
pub use pallet_timestamp::Call as TimestampCall;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

/// Implementations of some helper traits passed into runtime modules as
/// associated types.
pub mod impls;
pub use impls::DealWithCredits;

/// Constant values used within the runtime.
use cord_runtime_constants::{currency::*, fee::WeightToFee, time::*};

// Weights used in the runtime.
mod weights;
// CORD Pallets
mod authority_manager;
pub use pallet_author_registry;
pub use pallet_credit_treasury;
pub use pallet_schema;
pub use pallet_space;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Wasm binary unwrapped. If built with `SKIP_WASM_BUILD`, the function panics.
#[cfg(feature = "std")]
pub fn wasm_binary_unwrap() -> &'static [u8] {
	WASM_BINARY.expect(
		"Development wasm binary is not available. This means the client is \
  		 built with `SKIP_WASM_BUILD` flag and it is only usable for \
		 production chains. Please rebuild with the flag disabled.",
	)
}

/// Runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("cord"),
	impl_name: create_runtime_str!("dhiway-cord"),
	authoring_version: 0,
	// Per convention: if the runtime behavior changes, increment spec_version
	// and set impl_version to equal spec_version. If only runtime
	// implementation changes and behavior does not, then leave spec_version as
	// is and increment impl_version.
	spec_version: 7100,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 1,
};

/// The BABE epoch configuration at genesis.
pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
	sp_consensus_babe::BabeEpochConfiguration {
		c: PRIMARY_PROBABILITY,
		allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryVRFSlots,
	};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

/// We currently allow all calls.
pub struct BaseFilter;
impl Contains<RuntimeCall> for BaseFilter {
	fn contains(_c: &RuntimeCall) -> bool {
		true
	}
}

type MoreThanHalfCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
>;

/// We assume that an on-initialize consumes 1% of the weight on average, hence
/// a single extrinsic will not be allowed to consume more than
/// `AvailableBlockRatio - 1%`.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(1);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be
/// used by  Operational  extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = WEIGHT_PER_SECOND.saturating_mul(2);

const_assert!(NORMAL_DISPATCH_RATIO.deconstruct() >= AVERAGE_ON_INITIALIZE_RATIO.deconstruct());

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	pub const Version: RuntimeVersion = VERSION;
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

impl frame_system::Config for Runtime {
	type BaseCallFilter = BaseFilter;
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = RuntimeBlockLength;
	type DbWeight = RocksDbWeight;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = AccountIdLookup<AccountId, ()>;
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type Version = Version;
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = weights::frame_system::WeightInfo<Runtime>;
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * RuntimeBlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
	pub const NoPreimagePostponement: Option<u32> = Some(10);
}

type ScheduleOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>,
>;

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
	fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
		if left == right {
			return Some(Ordering::Equal)
		}

		match (left, right) {
			// Root is greater than anything.
			(OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
			// Check which one has more yes votes.
			(
				OriginCaller::Council(pallet_collective::RawOrigin::Members(l_yes_votes, l_count)),
				OriginCaller::Council(pallet_collective::RawOrigin::Members(r_yes_votes, r_count)),
			) => Some((l_yes_votes * r_count).cmp(&(r_yes_votes * l_count))),
			// For every other origin we don't care, as they are not used for `ScheduleOrigin`.
			_ => None,
		}
	}
}

impl pallet_scheduler::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeEvent = RuntimeEvent;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = ScheduleOrigin;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = weights::pallet_scheduler::WeightInfo<Runtime>;
	type OriginPrivilegeCmp = OriginPrivilegeCmp;
	type PreimageProvider = Preimage;
	type NoPreimagePostponement = NoPreimagePostponement;
}

parameter_types! {
	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub const PreimageBaseDeposit: Balance = deposit(2, 64);
	pub const PreimageByteDeposit: Balance = deposit(0, 1);
}

impl pallet_preimage::Config for Runtime {
	type WeightInfo = weights::pallet_preimage::WeightInfo<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type MaxSize = PreimageMaxSize;
	type BaseDeposit = PreimageBaseDeposit;
	type ByteDeposit = PreimageByteDeposit;
}

parameter_types! {
	pub EpochDuration: u64 = prod_or_fast!(
		EPOCH_DURATION_IN_SLOTS as u64,
		2 * MINUTES as u64,
		"CORD_EPOCH_DURATION"
	);
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
	pub ReportLongevity: u64 =
		BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
	pub const MaxAuthorities: u32 = 1_000;
}

impl pallet_babe::Config for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;

	// session module is the trigger
	type EpochChangeTrigger = pallet_babe::ExternalTrigger;

	type DisabledValidators = Session;

	type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		pallet_babe::AuthorityId,
	)>>::Proof;

	type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		pallet_babe::AuthorityId,
	)>>::IdentificationTuple;

	type KeyOwnerProofSystem = Historical;

	type HandleEquivocation =
		pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, Offences, ReportLongevity>;

	type WeightInfo = ();

	type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
	pub const IndexDeposit: Balance =  100 * UNITS;
}

impl pallet_indices::Config for Runtime {
	type AccountIndex = AccountIndex;
	type Currency = Balances;
	type Deposit = IndexDeposit;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_indices::WeightInfo<Runtime>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 100 * UNITS ;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
}

parameter_types! {
	pub const TransactionByteFee: Balance = 5 * MILLIUNITS;
	pub const OperationalFeeMultiplier: u8 = 5;
	/// The portion of the `NORMAL_DISPATCH_RATIO` that we adjust the fees with. Blocks filled less
	/// than this will decrease the weight and more will increase.
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	/// The adjustment variable of the runtime. Higher values will cause `TargetBlockFullness` to
	/// change the fees more rapidly.
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(75, 1_000_000);
	/// Minimum amount of the multiplier. This value cannot be too low. A test case should ensure
	/// that combined with `AdjustmentVariable`, we can recover from the minimum.
	/// See `multiplier_can_grow_from_zero`.
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 10u128);
}

/// Parameterized slow adjusting fee updated based on
/// <https://w3f-research.readthedocs.io/en/latest/polkadot/Token%20Economics.html#-2.-slow-adjusting-mechanism>
pub type SlowAdjustingFeeUpdate<R> =
	TargetedFeeAdjustment<R, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = CurrencyAdapter<Balances, DealWithCredits<Runtime>>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
}

parameter_types! {
	// pub const MinimumPeriod: u64 = prod_or_fast!(SLOT_DURATION / 2, 500);
		pub MinimumPeriod: u64 = prod_or_fast!(
		MIN_BLOCK_PERIOD as u64,
		500 as u64,
		"CORD_MIN_BLOCK_DURATION"
	);
}

impl pallet_timestamp::Config for Runtime {
	type Moment = u64;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = weights::pallet_timestamp::WeightInfo<Runtime>;
}

parameter_types! {
	pub const UncleGenerations: u32 = 0;
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = ImOnline;
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub grandpa: Grandpa,
		pub babe: Babe,
		pub im_online: ImOnline,
		pub authority_discovery: AuthorityDiscovery,
	}
}

/// Special `ValidatorIdOf` implementation that is just returning the input as
/// result.
pub struct ValidatorIdOf;
impl sp_runtime::traits::Convert<AccountId, Option<AccountId>> for ValidatorIdOf {
	fn convert(a: AccountId) -> Option<AccountId> {
		Some(a)
	}
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = AccountId;
	type ValidatorIdOf = ValidatorIdOf;
	type ShouldEndSession = Babe;
	type NextSessionRotation = Babe;
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, AuthorityManager>;
	type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type WeightInfo = weights::pallet_session::WeightInfo<Runtime>;
}

pub struct FullIdentificationOf;
impl sp_runtime::traits::Convert<AccountId, Option<()>> for FullIdentificationOf {
	fn convert(_: AccountId) -> Option<()> {
		Some(Default::default())
	}
}

impl pallet_session::historical::Config for Runtime {
	type FullIdentification = ();
	type FullIdentificationOf = FullIdentificationOf;
}

parameter_types! {
	pub const SessionsPerEra: SessionIndex = 6;
	pub const BondingDuration: sp_staking::EraIndex = 28;
}

parameter_types! {
	pub LaunchPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1, "CORD_LAUNCH_PERIOD");
	pub VotingPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 2 * MINUTES, "CORD_VOTING_PERIOD");
	pub FastTrackVotingPeriod: BlockNumber = prod_or_fast!(3 * HOURS, 2 * MINUTES, "CORD_FAST_TRACK_VOTING_PERIOD");
	pub const MinimumDeposit: Balance = 100 * WAY;
	pub EnactmentPeriod: BlockNumber = prod_or_fast!(8 * DAYS, 1, "CORD_ENACTMENT_PERIOD");
	pub CooloffPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1, "CORD_COOLOFF_PERIOD");
	pub const InstantAllowed: bool = true;
	pub const MaxVotes: u32 = 100;
	pub const MaxProposals: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type VoteLockingPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type MinimumDeposit = MinimumDeposit;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin =
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>;
	/// A majority can have the next scheduled referendum be a straight
	/// majority-carries vote.
	type ExternalMajorityOrigin =
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>;
	/// A unanimous council can have the next scheduled referendum be a straight
	/// default-carries (NTB) vote.
	type ExternalDefaultOrigin =
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>;
	/// Two thirds of the technical committee can have an
	/// `ExternalMajority/ExternalDefault` vote be tabled immediately and with a
	/// shorter voting/enactment period.
	type FastTrackOrigin =
		pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>;
	type InstantOrigin =
		pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>;
	type InstantAllowed = InstantAllowed;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to
	// it.
	type CancellationOrigin = EitherOfDiverse<
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
	>;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	// To cancel a proposal before it has been passed, the technical committee must
	// be unanimous or Root must agree.
	type CancelProposalOrigin = EitherOfDiverse<
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
	>;
	// Any single technical committee member may veto a coming council proposal,
	// however they can only do it once and it lasts only for the cooloff period.
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
	type CooloffPeriod = CooloffPeriod;
	type PreimageByteDeposit = PreimageByteDeposit;
	type OperationalPreimageOrigin = pallet_collective::EnsureMember<AccountId, CouncilCollective>;
	type Slash = CreditTreasury;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type MaxVotes = MaxVotes;
	type WeightInfo = weights::pallet_democracy::WeightInfo<Runtime>;
	type MaxProposals = MaxProposals;
}

parameter_types! {
	pub CouncilMotionDuration: BlockNumber = prod_or_fast!(3 * DAYS, 2 * MINUTES, "CORD_MOTION_DURATION");
	pub const CouncilMaxProposals: u32 = 100;
	pub const CouncilMaxMembers: u32 = 100;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = CouncilMotionDuration;
	type MaxProposals = CouncilMaxProposals;
	type MaxMembers = CouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective_council::WeightInfo<Runtime>;
}

parameter_types! {
	pub const CandidacyBond: Balance = 100_000 * WAY;
	// 1 storage item created, key size is 32 bytes, value size is 16+16.
	pub const VotingBondBase: Balance = deposit(1, 64);
	// additional data per vote is 32 bytes (account id).
	pub const VotingBondFactor: Balance = deposit(0, 32);
	/// Daily council elections
	pub TermDuration: BlockNumber = prod_or_fast!(24 * HOURS, 2 * MINUTES, "CORD_TERM_DURATION");
	pub const DesiredMembers: u32 = 15;
	pub const DesiredRunnersUp: u32 = 13;
	pub const MaxVoters: u32 = 10 * 500;
	pub const MaxCandidates: u32 = 100;
	pub const PhragmenElectionPalletId: LockIdentifier = *b"phrelect";
}

// Make sure that there are no more than MaxMembers members elected via
// phragmen.
const_assert!(DesiredMembers::get() <= CouncilMaxMembers::get());

impl pallet_elections_phragmen::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ChangeMembers = Council;
	type InitializeMembers = Council;
	type CurrencyToVote = frame_support::traits::U128CurrencyToVote;
	type CandidacyBond = CandidacyBond;
	type VotingBondBase = VotingBondBase;
	type VotingBondFactor = VotingBondFactor;
	type LoserCandidate = CreditTreasury;
	type KickedMember = CreditTreasury;
	type DesiredMembers = DesiredMembers;
	type DesiredRunnersUp = DesiredRunnersUp;
	type TermDuration = TermDuration;
	type MaxVoters = MaxVoters;
	type MaxCandidates = MaxCandidates;
	type PalletId = PhragmenElectionPalletId;
	type WeightInfo = weights::pallet_elections_phragmen::WeightInfo<Runtime>;
}

parameter_types! {
	pub TechnicalMotionDuration: BlockNumber = prod_or_fast!(3 * DAYS, 2 * MINUTES, "CORD_MOTION_DURATION");
	pub const TechnicalMaxProposals: u32 = 100;
	pub const TechnicalMaxMembers: u32 = 50;
}

type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TechnicalMotionDuration;
	type MaxProposals = TechnicalMaxProposals;
	type MaxMembers = TechnicalMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective_technical_committee::WeightInfo<Runtime>;
}

impl pallet_membership::Config<pallet_membership::Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = MoreThanHalfCouncil;
	type RemoveOrigin = MoreThanHalfCouncil;
	type SwapOrigin = MoreThanHalfCouncil;
	type ResetOrigin = MoreThanHalfCouncil;
	type PrimeOrigin = MoreThanHalfCouncil;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
	type MaxMembers = TechnicalMaxMembers;
	type WeightInfo = weights::pallet_membership::WeightInfo<Runtime>;
}

parameter_types! {
	pub PalletCreditMotionDuration: BlockNumber = prod_or_fast!(2 * DAYS, 2 * MINUTES,
"CORD_CREDIT_MOTION_DURATION");
	pub const PalletCreditMaxProposals: u32 = 100;
	pub const PalletCreditMaxMembers: u32 = 10;
}

type CreditTreasuryCollective = pallet_collective::Instance3;
impl pallet_collective::Config<CreditTreasuryCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = PalletCreditMotionDuration;
	type MaxProposals = PalletCreditMaxProposals;
	type MaxMembers = PalletCreditMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective_credit::WeightInfo<Runtime>;
}

type CreditCouncilApproval = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CreditTreasuryCollective, 1, 2>,
>;

impl pallet_membership::Config<pallet_membership::Instance2> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = CreditCouncilApproval;
	type RemoveOrigin = CreditCouncilApproval;
	type SwapOrigin = CreditCouncilApproval;
	type ResetOrigin = CreditCouncilApproval;
	type PrimeOrigin = CreditCouncilApproval;
	type MembershipInitialized = CreditTreasuryCouncil;
	type MembershipChanged = CreditTreasuryCouncil;
	type MaxMembers = PalletCreditMaxMembers;
	type WeightInfo = weights::pallet_membership::WeightInfo<Runtime>;
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 100 * UNITS;
	pub const ProposalBondMaximum: Balance = 500 * UNITS;
	pub const SpendPeriod: BlockNumber = 7 * DAYS;
	pub const Burn: Permill = Permill::from_perthousand(2);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const MaxApprovals: u32 = 100;

}

type ApproveOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
>;
type RejectOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
>;

impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = Balances;
	type ApproveOrigin = ApproveOrigin;
	type RejectOrigin = RejectOrigin;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = CreditTreasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type ProposalBondMaximum = ProposalBondMaximum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = CreditTreasury;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	type WeightInfo = weights::pallet_treasury::WeightInfo<Runtime>;
	type SpendOrigin = frame_support::traits::NeverEnsureOrigin<Balance>;
}

impl pallet_offences::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = ();
}

impl pallet_authority_discovery::Config for Runtime {
	type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
	pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
	pub const MaxPeerInHeartbeats: u32 = 10_000;
	pub const MaxPeerDataEncodingSize: u32 = 1_000;
	pub const MaxKeys: u32 = 100_000;
}

impl pallet_im_online::Config for Runtime {
	type AuthorityId = ImOnlineId;
	type RuntimeEvent = RuntimeEvent;
	type ValidatorSet = Historical;
	type NextSessionRotation = Babe;
	type ReportUnresponsiveness = Offences;
	type UnsignedPriority = ImOnlineUnsignedPriority;
	type WeightInfo = weights::pallet_im_online::WeightInfo<Runtime>;
	type MaxKeys = MaxKeys;
	type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
	type MaxPeerDataEncodingSize = MaxPeerDataEncodingSize;
}

impl pallet_grandpa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;

	type KeyOwnerProof =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

	type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		GrandpaId,
	)>>::IdentificationTuple;

	type KeyOwnerProofSystem = Historical;

	type HandleEquivocation = pallet_grandpa::EquivocationHandler<
		Self::KeyOwnerIdentification,
		Offences,
		ReportLongevity,
	>;

	type WeightInfo = ();
	type MaxAuthorities = MaxAuthorities;
}

/// Submits a transaction with the node's public and signature type. Adheres to
/// the signed extension format of the chain.
impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		public: <Signature as Verify>::Signer,
		account: AccountId,
		nonce: <Runtime as frame_system::Config>::Index,
	) -> Option<(RuntimeCall, <UncheckedExtrinsic as ExtrinsicT>::SignaturePayload)> {
		use sp_runtime::traits::StaticLookup;
		// take the biggest period possible.
		let period =
			BlockHashCount::get().checked_next_power_of_two().map(|c| c / 2).unwrap_or(2) as u64;

		let current_block = System::block_number()
			.saturated_into::<u64>()
			// The `System::block_number` is initialized with `n+1`,
			// so the actual block number is `n`.
			.saturating_sub(1);
		let tip = 0;
		let extra: SignedExtra = (
			frame_system::CheckNonZeroSender::<Runtime>::new(),
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckMortality::<Runtime>::from(generic::Era::mortal(
				period,
				current_block,
			)),
			frame_system::CheckNonce::<Runtime>::from(nonce),
			frame_system::CheckWeight::<Runtime>::new(),
			pallet_author_registry::CheckAuthorRegistry::<Runtime>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
		);
		let raw_payload = SignedPayload::new(call, extra)
			.map_err(|e| {
				log::warn!("Unable to create signed payload: {:?}", e);
			})
			.ok()?;
		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
		let (call, extra, _) = raw_payload.deconstruct();
		let address = <Runtime as frame_system::Config>::Lookup::unlookup(account);
		Some((call, (address, signature, extra)))
	}
}

impl frame_system::offchain::SigningTypes for Runtime {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type Extrinsic = UncheckedExtrinsic;
	type OverarchingCall = RuntimeCall;
}

parameter_types! {
	pub const BasicDeposit: Balance = 200 * UNITS;       // 258 bytes on-chain
	pub const FieldDeposit: Balance = 50 * UNITS;        // 66 bytes on-chain
	pub const SubAccountDeposit: Balance = 50 * UNITS;   // 53 bytes on-chain
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}

impl pallet_identity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BasicDeposit = BasicDeposit;
	type FieldDeposit = FieldDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxRegistrars = MaxRegistrars;
	type Slashed = Treasury;
	type ForceOrigin = MoreThanHalfCouncil;
	type RegistrarOrigin = MoreThanHalfCouncil;
	type WeightInfo = weights::pallet_identity::WeightInfo<Runtime>;
}

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = weights::pallet_utility::WeightInfo<Runtime>;
}

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = deposit(1, 88);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 32);
	pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = weights::pallet_multisig::WeightInfo<Runtime>;
}

parameter_types! {
	pub const ConfigDepositBase: Balance = 200 * UNITS;
	pub const FriendDepositFactor: Balance = 50 * UNITS;
	pub const MaxFriends: u16 = 9;
	pub const RecoveryDeposit: Balance = 500 * UNITS;
}

impl pallet_recovery::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ConfigDepositBase = ConfigDepositBase;
	type FriendDepositFactor = FriendDepositFactor;
	type MaxFriends = MaxFriends;
	type RecoveryDeposit = RecoveryDeposit;
}

parameter_types! {
	// One storage item; key size 32, value size 8; .
	pub const ProxyDepositBase: Balance = deposit(1, 8);
	// Additional storage item size of 33 bytes.
	pub const ProxyDepositFactor: Balance = deposit(0, 33);
	pub const MaxProxies: u16 = 32;
	pub const AnnouncementDepositBase: Balance = deposit(1, 8);
	pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
	pub const MaxPending: u16 = 32;
}

/// The type used to represent the kinds of proxying allowed.
#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Encode,
	Decode,
	RuntimeDebug,
	MaxEncodedLen,
	scale_info::TypeInfo,
)]
pub enum ProxyType {
	Any,
	NonTransfer,
	Governance,
	IdentityJudgement,
	CancelProxy,
}

impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}

impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, c: &RuntimeCall) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::NonTransfer => matches!(
				c,
				RuntimeCall::System(..) |
				RuntimeCall::Babe(..) |
				RuntimeCall::Timestamp(..) |
				RuntimeCall::Indices(pallet_indices::Call::claim {..}) |
				RuntimeCall::Indices(pallet_indices::Call::free {..}) |
				RuntimeCall::Indices(pallet_indices::Call::freeze {..}) |
				// Specifically omitting Indices `transfer`, `force_transfer`
				// Specifically omitting the entire Balances pallet
				RuntimeCall::Authorship(..) |
				RuntimeCall::Session(..) |
				RuntimeCall::Grandpa(..) |
				RuntimeCall::ImOnline(..) |
				RuntimeCall::Democracy(..) |
				RuntimeCall::Council(..) |
				RuntimeCall::TechnicalCommittee(..) |
				RuntimeCall::Elections(..) |
				RuntimeCall::TechnicalMembership(..) |
				RuntimeCall::Treasury(..) |
				RuntimeCall::Utility(..) |
				RuntimeCall::Identity(..) |
				RuntimeCall::Recovery(pallet_recovery::Call::as_recovered {..}) |
				RuntimeCall::Recovery(pallet_recovery::Call::vouch_recovery {..}) |
				RuntimeCall::Recovery(pallet_recovery::Call::claim_recovery {..}) |
				RuntimeCall::Recovery(pallet_recovery::Call::close_recovery {..}) |
				RuntimeCall::Recovery(pallet_recovery::Call::remove_recovery {..}) |
				RuntimeCall::Recovery(pallet_recovery::Call::cancel_recovered {..}) |
				// Specifically omitting Recovery `create_recovery`, `initiate_recovery`
				RuntimeCall::Scheduler(..) |
				RuntimeCall::Proxy(..) |
				RuntimeCall::Multisig(..)
			),
			ProxyType::Governance => matches!(
				c,
				RuntimeCall::Democracy(..) |
					RuntimeCall::Council(..) |
					RuntimeCall::TechnicalCommittee(..) |
					RuntimeCall::Elections(..) |
					RuntimeCall::Treasury(..) |
					RuntimeCall::Utility(..)
			),
			ProxyType::IdentityJudgement => matches!(
				c,
				RuntimeCall::Identity(pallet_identity::Call::provide_judgement { .. }) |
					RuntimeCall::Utility(..)
			),
			ProxyType::CancelProxy => {
				matches!(c, RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. }))
			},
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			(ProxyType::NonTransfer, _) => true,
			_ => false,
		}
	}
}

impl pallet_proxy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = weights::pallet_proxy::WeightInfo<Runtime>;
	type MaxPending = MaxPending;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

impl authority_manager::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityOrigin = MoreThanHalfCouncil;
}

parameter_types! {

	pub AuthorshipDuration: BlockNumber = prod_or_fast!(
		AUTHORSHIP_DURATION_IN_BLOCKS,
		16 * MINUTES,
		"CORD_AUTHORSHIP_DURATION"
	);
	pub DelegationBlockLimit: BlockNumber = prod_or_fast!(
		AUTHORSHIP_DELEGATION_IN_BLOCKS,
		8 * MINUTES,
		"CORD_AUTHORSHIP_DELEGATION_DURATION"
	);
	pub const MaxBlockProposals: u32 = 25;
	pub const MaxRegistryBlockEntries: u32 = 5000;
}

impl pallet_author_registry::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ApproveOrigin = MoreThanHalfCouncil;
	type Currency = Balances;
	type CreditCollector = CreditTreasury;
	type AuthorshipDuration = AuthorshipDuration;
	type DelegationBlockLimit = DelegationBlockLimit;
	type MaxBlockProposals = MaxBlockProposals;
	type MaxRegistryBlockEntries = MaxRegistryBlockEntries;
	type WeightInfo = weights::pallet_author_registry::WeightInfo<Runtime>;
}

type CreditTreasuryApproveOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureMember<AccountId, CreditTreasuryCollective>,
>;

parameter_types! {
	pub const CreditTreasuryPalletId: PalletId = PalletId(*b"py/crdit");
}

impl pallet_credit_treasury::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ApproveOrigin = CreditTreasuryApproveOrigin;
	type PalletId = CreditTreasuryPalletId;
	type WeightInfo = weights::pallet_credit_treasury::WeightInfo<Runtime>;
}

parameter_types! {
	pub const SchemaFee: Balance = 10 * WAY;
	pub const MaxEncodedMetaLength: u32 = 5_000;	// 5 Kb
}

impl pallet_schema::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type SchemaFee = SchemaFee;
	type FeeCollector = CreditTreasury;
	type Signature = Signature;
	type Signer = <Signature as Verify>::Signer;
	type EnsureOrigin = EnsureSigned<Self::AccountId>;
	type MaxEncodedMetaLength = MaxEncodedMetaLength;
	type WeightInfo = weights::pallet_schema::WeightInfo<Runtime>;
}

parameter_types! {
	pub const SpaceFee: Balance = 10 * WAY;
	pub const BaseFee: Balance = 2 * WAY;
	pub const MaxSpaceAuthorities: u32 = 50;
	pub const MaxRegistryEntries: u32 = 1_000;
}

impl pallet_space::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type SpaceFee = SpaceFee;
	type BaseFee = BaseFee;
	type FeeCollector = CreditTreasury;
	type Signature = Signature;
	type Signer = <Signature as Verify>::Signer;
	type EnsureOrigin = EnsureSigned<Self::AccountId>;
	type MaxSpaceAuthorities = MaxSpaceAuthorities;
	type MaxRegistryEntries = MaxRegistryEntries;
	type MaxEncodedMetaLength = MaxEncodedMetaLength;
	type WeightInfo = weights::pallet_space::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MaxParentChecks: u32 = 5;
	pub const MaxRevocations: u32 = 5;
	pub const MaxRemovals: u32 = 5;
	#[derive(Clone)]
	pub const MaxChildren: u32 = 1000;
}

impl pallet_trust_hierarchy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type HierarchyFee = SpaceFee;
	type NodeFee = BaseFee;
	type FeeCollector = CreditTreasury;
	type Signature = Signature;
	type Signer = <Signature as Verify>::Signer;
	type EnsureOrigin = EnsureSigned<Self::AccountId>;
	type MaxParentChecks = MaxParentChecks;
	type MaxRevocations = MaxRevocations;
	type MaxRemovals = MaxRemovals;
	type MaxChildren = MaxChildren;
	type WeightInfo = weights::pallet_trust_hierarchy::WeightInfo<Runtime>;
}

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
}

construct_runtime! {
	pub enum Runtime where
		Block = Block,
		NodeBlock = cord_primitives::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// Basic stuff; balances is uncallable initially.
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>} = 0,
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>} = 1,
		Preimage: pallet_preimage::{Pallet, Call, Storage, Event<T>} = 10,

		// Babe must be before session.
		Babe: pallet_babe::{Pallet, Call, Storage, Config, ValidateUnsigned} = 2,

		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 3,
		Indices: pallet_indices::{Pallet, Call, Storage, Config<T>, Event<T>} = 4,
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 5,
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>} = 32,

		// Consensus support.
		Authorship: pallet_authorship::{Pallet, Call, Storage} = 6,
		Offences: pallet_offences::{Pallet, Storage, Event} = 7,
		Historical: pallet_session_historical::{Pallet} = 33,
		Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>} = 8,
		Grandpa: pallet_grandpa::{Pallet, Call, Storage, Config, Event, ValidateUnsigned} = 11,
		ImOnline: pallet_im_online::{Pallet, Call, Storage, Event<T>, ValidateUnsigned, Config<T>} = 12,
		AuthorityDiscovery: pallet_authority_discovery::{Pallet, Config} = 13,

		// Governance stuff; uncallable initially.
		Democracy: pallet_democracy::{Pallet, Call, Storage, Config<T>, Event<T>} = 15,
		Council: pallet_collective::<Instance1>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 16,
		TechnicalCommittee: pallet_collective::<Instance2>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 17,
		CreditTreasuryCouncil: pallet_collective::<Instance3>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 18,
		Elections: pallet_elections_phragmen::{Pallet, Call, Storage, Event<T>, Config<T>} = 19,
		TechnicalMembership: pallet_membership::<Instance1>::{Pallet, Call, Storage, Event<T>, Config<T>} = 20,
		CreditTreasuryMembership: pallet_membership::<Instance2>::{Pallet, Call, Storage, Event<T>, Config<T>} = 21,
		Treasury: pallet_treasury::{Pallet, Call, Storage, Config, Event<T>} = 22,
		CreditTreasury: pallet_credit_treasury::{Pallet, Call, Storage, Config, Event<T>} = 23,

		// Utility module.
		Utility: pallet_utility::{Pallet, Call, Event} = 31,

		// Less simple identity module.
		Identity: pallet_identity::{Pallet, Call, Storage, Event<T>} = 35,

		// Social recovery module.
		Recovery: pallet_recovery::{Pallet, Call, Storage, Event<T>} = 36,

		// Proxy module. Late addition.
		Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>} = 37,

		// Multisig module. Late addition.
		Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 38,

		// Authority Manager pallet.
		AuthorityManager: authority_manager::{Pallet, Call, Storage, Event<T>} = 101,

		// Author Registry pallet.
		AuthorRegistry: pallet_author_registry::{Pallet, Call, Storage, Event<T>, Config<T>} =102,

		// Trust Hierarchy pallet.
		TrustHierarchy: pallet_trust_hierarchy::{Pallet, Call, Storage, Event<T>} = 103,

		// Schema Manager pallet.
		Schema: pallet_schema::{Pallet, Call, Storage, Event<T>} = 104,

		// Schema Manager pallet.
		Space: pallet_space::{Pallet, Call, Storage, Event<T>} = 105,

		// Sudo.
		Sudo: pallet_sudo::{Pallet, Call, Storage, Event<T>, Config<T>} = 255,
	}
}

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// `BlockId` type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The `SignedExtension` to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckMortality<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_author_registry::CheckAuthorRegistry<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	define_benchmarks!(
		[frame_benchmarking, BaselineBench::<Runtime>]
		[pallet_babe, Babe]
		[pallet_balances, Balances]
		[pallet_collective, Council]
		[pallet_democracy, Democracy]
		[pallet_grandpa, Grandpa]
		[pallet_identity, Identity]
		[pallet_im_online, ImOnline]
		[pallet_indices, Indices]
		[pallet_membership, TechnicalMembership]
		[pallet_multisig, Multisig]
		[pallet_offences, OffencesBench::<Runtime>]
		[pallet_preimage, Preimage]
		[pallet_proxy, Proxy]
		[pallet_recovery, Recovery]
		[pallet_scheduler, Scheduler]
		[pallet_session, SessionBench::<Runtime>]
		[frame_system, SystemBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		[pallet_treasury, Treasury]
		[pallet_utility, Utility]
	);
}

sp_api::impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block);
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(block: Block, data: InherentData) -> CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}

		fn current_set_id() -> fg_primitives::SetId {
			Grandpa::current_set_id()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			equivocation_proof: fg_primitives::EquivocationProof<
				<Block as BlockT>::Hash,
				NumberFor<Block>,
			>,
			key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Grandpa::submit_unsigned_equivocation_report(
				equivocation_proof,
				key_owner_proof,
			)
		}

		fn generate_key_ownership_proof(
			_set_id: fg_primitives::SetId,
			authority_id: GrandpaId,
		) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
			use codec::Encode;

			Historical::prove((fg_primitives::KEY_TYPE, authority_id))
				.map(|p| p.encode())
				.map(fg_primitives::OpaqueKeyOwnershipProof::new)
		}
	}

	impl sp_consensus_babe::BabeApi<Block> for Runtime {
		fn configuration() -> sp_consensus_babe::BabeConfiguration {
			let epoch_config = Babe::epoch_config().unwrap_or(BABE_GENESIS_EPOCH_CONFIG);
			sp_consensus_babe::BabeConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get().into(),
				c: epoch_config.c,
				authorities: Babe::authorities().to_vec(),
				randomness: Babe::randomness(),
				allowed_slots: epoch_config.allowed_slots,
			}
		}

		fn current_epoch_start() -> sp_consensus_babe::Slot {
			Babe::current_epoch_start()
		}

		fn current_epoch() -> sp_consensus_babe::Epoch {
			Babe::current_epoch()
		}

		fn next_epoch() -> sp_consensus_babe::Epoch {
			Babe::next_epoch()
		}

		fn generate_key_ownership_proof(
			_slot: sp_consensus_babe::Slot,
			authority_id: sp_consensus_babe::AuthorityId,
		) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
			use codec::Encode;

			Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
				.map(|p| p.encode())
				.map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
			key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Babe::submit_unsigned_equivocation_report(
				equivocation_proof,
				key_owner_proof,
			)
		}
	}

	impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityDiscoveryId> {
			AuthorityDiscovery::authorities()
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
		Block,
		Balance,
	> for Runtime {
		fn query_info(uxt: <Block as BlockT>::Extrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(call: RuntimeCall, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(call: RuntimeCall, len: u32) -> FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
	}


	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade() -> (Weight, Weight) {
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here. If any of the pre/post migration checks fail, we shall stop
			// right here and right now.
			let weight = Executive::try_runtime_upgrade().unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
			state_root_check: bool,
			select: frame_try_runtime::TryStateSelect
		) -> Weight {
			log::info!(
				target: "node-runtime",
				"try-runtime: executing block {:?} / root checks: {:?} / try-state-select: {:?}",
				block.header.hash(),
				state_root_check,
				select,
			);
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here.
			Executive::try_execute_block(block, state_root_check, select).unwrap()
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;

			use frame_system_benchmarking::Pallet as SystemBench;
			use frame_benchmarking::baseline::Pallet as Baseline;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig,
		) -> Result<
			Vec<frame_benchmarking::BenchmarkBatch>,
			sp_runtime::RuntimeString,
		> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, TrackedStorageKey};
			use frame_system_benchmarking::Pallet as SystemBench;
			use frame_benchmarking::baseline::Pallet as Baseline;

			impl frame_system_benchmarking::Config for Runtime {}
			impl frame_benchmarking::baseline::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			Ok(batches)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_system::offchain::CreateSignedTransaction;

	#[test]
	fn validate_transaction_submitter_bounds() {
		fn is_submit_signed_transaction<T>()
		where
			T: CreateSignedTransaction<Call>,
		{
		}

		is_submit_signed_transaction::<Runtime>();
	}
}
