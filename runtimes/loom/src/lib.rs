// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
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
// `construct_runtime!` does a lot of recursion and requires us to increase the limits.
#![recursion_limit = "1024"]

use codec::{Decode, Encode};
use scale_info::TypeInfo;

pub use cord_primitives::{AccountId, Signature};
use cord_primitives::{AccountIndex, Balance, BlockNumber, DidIdentifier, Hash, Moment, Nonce};
use cord_runtime_common::{impl_runtime_weights, prod_or_fast, BlockHashCount, BlockLength};
pub use identifier::Ss58Identifier;
// Can't use `FungibleAdapter` here until Treasury pallet migrates to fungibles
// <https://github.com/paritytech/polkadot-sdk/issues/226>
#[allow(deprecated)]
use pallet_transaction_payment::{CurrencyAdapter, FeeDetails, RuntimeDispatchInfo};

use frame_support::{
	derive_impl,
	genesis_builder_helper::{build_state, get_preset},
	instances::{Instance1, Instance2},
	ord_parameter_types, parameter_types,
	traits::{
		fungible::{HoldConsideration, NativeFromLeft, NativeOrWithId, UnionOf},
		tokens::imbalance::ResolveAssetTo,
		AsEnsureOriginWithArg, ConstBool, ConstU128, ConstU32, Contains, EitherOfDiverse,
		KeyOwnerProofSystem, LinearStoragePrice, PrivilegeCmp, VariantCountOf,
	},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, ConstantMultiplier},
	PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned, EnsureSignedBy, EnsureWithSuccess};
use pallet_asset_conversion::{AccountIdConverter, Ascending, Chain, WithFirstAsset};
pub use pallet_balances::Call as BalancesCall;
use pallet_identity::legacy::IdentityInfo;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_session::historical as pallet_session_historical;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_inherents::{CheckInherentsResult, InherentData};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, AccountIdLookup, BlakeTwo256, Block as BlockT,
		Extrinsic as ExtrinsicT, NumberFor, OpaqueKeys, SaturatedConversion, Verify,
	},
	transaction_validity::{TransactionPriority, TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, Perbill, Percent, Permill,
};
use sp_staking::SessionIndex;
use sp_std::{cmp::Ordering, prelude::*};

#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

#[cfg(any(feature = "std", test))]
pub use frame_system::Call as SystemCall;
#[cfg(any(feature = "std", test))]
pub use pallet_sudo::Call as SudoCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

/// Constant values used within the runtime.
use cord_loom_runtime_constants::{currency::*, fee::WeightToFee, time::*};
use cord_runtime_common as runtime_common;
use runtime_common::{EverythingToTheTreasury, SlowAdjustingFeeUpdate};

// Weights used in the runtime.
mod weights;
// CORD Pallets
pub use authority_membership;
pub use pallet_network_membership;
pub mod benchmark;
pub use benchmark::DummySignature;
pub use pallet_assets_runtime_api as assets_api;
use pallet_network_membership::RuntimeDispatchWeightInfo;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Max size for serialized extrinsic params for this testing runtime.
/// This is a quite arbitrary but empirically battle tested value.
#[cfg(test)]
pub const CALL_PARAMS_MAX_SIZE: usize = 244;

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
	spec_name: create_runtime_str!("loom"),
	impl_name: create_runtime_str!("dhiway-cord"),
	authoring_version: 0,
	spec_version: 9300,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 2,
	state_version: 1,
};

/// We allow for 2 seconds of block time for computations, with maximum proof size.
pub const MAXIMUM_BLOCK_WEIGHT: Weight =
	Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2), u64::MAX);
impl_runtime_weights!(cord_loom_runtime_constants, MAXIMUM_BLOCK_WEIGHT);

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

type EnsureRootOrCommitteeApproval = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<AccountId, TechnicalCollective, 3, 5>,
>;

parameter_types! {
   pub const Version: RuntimeVersion = VERSION;
   pub const SS58Prefix: u16 = 4926;
}

#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
	type BaseCallFilter = BaseFilter;
	type BlockWeights = BlockWeights;
	type BlockLength = BlockLength;
	type DbWeight = RocksDbWeight;
	type Nonce = Nonce;
	type Hash = Hash;
	type AccountId = AccountId;
	type Lookup = AccountIdLookup<AccountId, ()>;
	type Block = Block;
	type BlockHashCount = BlockHashCount;
	type Version = Version;
	type AccountData = pallet_balances::AccountData<Balance>;
	type SystemWeightInfo = weights::frame_system::WeightInfo<Runtime>;
	type SS58Prefix = SS58Prefix;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type MultiBlockMigrator = ();
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
	pub const NoPreimagePostponement: Option<u32> = Some(10);
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
	fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
		if left == right {
			return Some(Ordering::Equal);
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
	type ScheduleOrigin = EnsureRootOrCommitteeApproval;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = weights::pallet_scheduler::WeightInfo<Runtime>;
	type OriginPrivilegeCmp = OriginPrivilegeCmp;
	type Preimages = Preimage;
}

parameter_types! {
	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub const PreimageBaseDeposit: Balance = 1 * UNITS;
	pub const PreimageByteDeposit: Balance = 1 * NANO_UNITS;
	pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);

}

impl pallet_preimage::Config for Runtime {
	type WeightInfo = weights::pallet_preimage::WeightInfo<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type Consideration = HoldConsideration<
		AccountId,
		Balances,
		PreimageHoldReason,
		LinearStoragePrice<PreimageBaseDeposit, PreimageByteDeposit, Balance>,
	>;
}

parameter_types! {
	pub EpochDuration: u64 = prod_or_fast!(
		EPOCH_DURATION_IN_SLOTS as u64,
		2 * MINUTES as u64,
		"CORD_EPOCH_DURATION"
	);
	pub const SessionsPerEra: SessionIndex = 6;
	pub const BondingDuration: sp_staking::EraIndex = 28;
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
	type WeightInfo = weights::pallet_babe::WeightInfo<Runtime>;
	type MaxAuthorities = MaxAuthorities;
	type MaxNominators = ConstU32<0>;
	type KeyOwnerProof =
		<Historical as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;
	type EquivocationReportSystem =
		pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

parameter_types! {
	pub const IndexDeposit: Balance =  EXISTENTIAL_DEPOSIT;
}

impl pallet_indices::Config for Runtime {
	type AccountIndex = AccountIndex;
	type Currency = Balances;
	type Deposit = IndexDeposit;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_indices::WeightInfo<Runtime>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
	type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
}

parameter_types! {
	pub const TransactionByteFee: Balance = 10 * MICRO_UNITS;
	/// This value increases the priority of `Operational` transactions by adding
	/// a "virtual tip" that's equal to the `OperationalFeeMultiplier * final_fee`.
	pub const OperationalFeeMultiplier: u8 = 5;
}

// Can't use `FungibleAdapter` here until Treasury pallet migrates to fungibles
// <https://github.com/paritytech/polkadot-sdk/issues/226>
#[allow(deprecated)]
impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = CurrencyAdapter<Balances, EverythingToTheTreasury<Runtime>>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
	type Moment = u64;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = weights::pallet_timestamp::WeightInfo<Runtime>;
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
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

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = AccountId;
	type ValidatorIdOf = sp_runtime::traits::ConvertInto;
	type ShouldEndSession = Babe;
	type NextSessionRotation = Babe;
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, AuthorityMembership>;
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
	pub const AssetDeposit: Balance = 10 * MILLI_UNITS;
	pub const ApprovalDeposit: Balance = 5 * MILLI_UNITS;
	pub const StringLimit: u32 = 50;
	pub const MetadataDepositBase: Balance = 10 * MILLI_UNITS;
	pub const MetadataDepositPerByte: Balance = 1 * MICRO_UNITS;
}

impl pallet_assets::Config<Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = u128;
	type AssetId = u32;
	type AssetIdParameter = codec::Compact<u32>;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type ForceOrigin = MoreThanHalfCouncil;
	type AssetDeposit = AssetDeposit;
	type AssetAccountDeposit = ConstU128<UNITS>;
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type ApprovalDeposit = ApprovalDeposit;
	type StringLimit = StringLimit;
	type Freezer = ();
	type Extra = ();
	type CallbackHandle = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
	type RemoveItemsLimit = ConstU32<1000>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

ord_parameter_types! {
	pub const AssetConversionOrigin: AccountId = AccountIdConversion::<AccountId>::into_account_truncating(&AssetConversionPalletId::get());
}

impl pallet_assets::Config<Instance2> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = u128;
	type AssetId = u32;
	type AssetIdParameter = codec::Compact<u32>;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSignedBy<AssetConversionOrigin, AccountId>>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type AssetDeposit = AssetDeposit;
	type AssetAccountDeposit = ConstU128<UNITS>;
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type ApprovalDeposit = ApprovalDeposit;
	type StringLimit = StringLimit;
	type Freezer = ();
	type Extra = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
	type RemoveItemsLimit = ConstU32<1000>;
	type CallbackHandle = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
	pub const AssetConversionPalletId: PalletId = PalletId(*b"py/ascon");
	pub const PoolSetupFee: Balance = 10 * UNITS; // should be more or equal to the existential deposit
	pub const MintMinLiquidity: Balance = 100;  // 100 is good enough when the main currency has 10-12 decimals.
	pub const LiquidityWithdrawalFee: Permill = Permill::from_percent(0);
	pub const Native: NativeOrWithId<u32> = NativeOrWithId::Native;
}

impl pallet_asset_conversion::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = u128;
	type HigherPrecisionBalance = sp_core::U256;
	type AssetKind = NativeOrWithId<u32>;
	type Assets = UnionOf<Balances, Assets, NativeFromLeft, NativeOrWithId<u32>, AccountId>;
	type PoolId = (Self::AssetKind, Self::AssetKind);
	type PoolLocator = Chain<
		WithFirstAsset<
			Native,
			AccountId,
			NativeOrWithId<u32>,
			AccountIdConverter<AssetConversionPalletId, Self::PoolId>,
		>,
		Ascending<
			AccountId,
			NativeOrWithId<u32>,
			AccountIdConverter<AssetConversionPalletId, Self::PoolId>,
		>,
	>;
	type PoolAssetId = <Self as pallet_assets::Config<Instance2>>::AssetId;
	type PoolAssets = PoolAssets;
	type PoolSetupFee = PoolSetupFee;
	type PoolSetupFeeAsset = Native;
	type PoolSetupFeeTarget = ResolveAssetTo<AssetConversionOrigin, Self::Assets>;
	type PalletId = AssetConversionPalletId;
	type LPFee = ConstU32<3>; // means 0.3%
	type LiquidityWithdrawalFee = LiquidityWithdrawalFee;
	type WeightInfo = pallet_asset_conversion::weights::SubstrateWeight<Runtime>;
	type MaxSwapPathLength = ConstU32<4>;
	type MintMinLiquidity = MintMinLiquidity;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
	pub const MaxAdditionalFields: u32 = 20;
}

impl pallet_identity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxSubAccounts = MaxSubAccounts;
	type IdentityInformation = IdentityInfo<MaxAdditionalFields>;
	type MaxRegistrars = MaxRegistrars;
	type RegistrarOrigin = MoreThanHalfCouncil;
	type OffchainSignature = Signature;
	type SigningPublicKey = <Signature as Verify>::Signer;
	type UsernameAuthorityOrigin = MoreThanHalfCouncil;
	type PendingUsernameExpiration = ConstU32<{ 7 * DAYS }>;
	type MaxSuffixLength = ConstU32<7>;
	type MaxUsernameLength = ConstU32<32>;
	type WeightInfo = pallet_identity::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const MaxRegistryEntryBlobSize: u32 = 4 * 1024; // 4KB in bytes
}

impl pallet_entries::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxEncodedInputLength = MaxEncodedInputLength;
	type MaxRegistryEntryBlobSize = MaxRegistryEntryBlobSize;
	type WeightInfo = ();
}

parameter_types! {
	pub MotionDuration: BlockNumber = prod_or_fast!(3 * DAYS, 2 * MINUTES, "CORD_MOTION_DURATION");
	pub const MaxProposals: u32 = 100;
	pub const MaxMembers: u32 = 50;
	pub MaxProposalWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = MotionDuration;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective::WeightInfo<Runtime>;
	type SetMembersOrigin = EnsureRoot<Self::AccountId>;
	type MaxProposalWeight = MaxProposalWeight;
}

impl pallet_membership::Config<pallet_membership::Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type IsMember = NetworkMembership;
	type AddOrigin = MoreThanHalfCouncil;
	type RemoveOrigin = MoreThanHalfCouncil;
	type SwapOrigin = MoreThanHalfCouncil;
	type ResetOrigin = MoreThanHalfCouncil;
	type PrimeOrigin = MoreThanHalfCouncil;
	type MembershipInitialized = Council;
	type MembershipChanged = Council;
	type MaxMembers = MaxMembers;
	type WeightInfo = weights::pallet_membership::WeightInfo<Runtime>;
}

type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = MotionDuration;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective::WeightInfo<Runtime>;
	type SetMembersOrigin = EnsureRoot<Self::AccountId>;
	type MaxProposalWeight = MaxProposalWeight;
}

impl pallet_membership::Config<pallet_membership::Instance2> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type IsMember = NetworkMembership;
	type AddOrigin = MoreThanHalfCouncil;
	type RemoveOrigin = MoreThanHalfCouncil;
	type SwapOrigin = MoreThanHalfCouncil;
	type ResetOrigin = MoreThanHalfCouncil;
	type PrimeOrigin = MoreThanHalfCouncil;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
	type MaxMembers = MaxMembers;
	type WeightInfo = weights::pallet_membership::WeightInfo<Runtime>;
}

parameter_types! {
	pub TreasuryAccount: AccountId = Treasury::account_id();
	pub const ProposalBond: Permill = Permill::from_percent(2);
	pub const ProposalBondMinimum: Balance = 5 * UNITS;
	pub const SpendPeriod: BlockNumber = 7 * DAYS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(10);
	pub const TipReportDepositBase: Balance = 1 * UNITS;
	pub const DataDepositPerByte: Balance = 2 * MILLI_UNITS;
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const MaximumReasonLength: u32 = 300;
	pub const MaxApprovals: u32 = 100;
	pub const MaxBalance: Balance = Balance::max_value();
	pub const SpendPayoutPeriod: BlockNumber = 7 * DAYS;
}

impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = Balances;
	type RejectOrigin = MoreThanHalfCouncil;
	type RuntimeEvent = RuntimeEvent;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = Treasury;
	type SpendFunds = ();
	type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
	type MaxApprovals = MaxApprovals;
	type SpendOrigin = EnsureWithSuccess<EnsureRoot<AccountId>, AccountId, MaxBalance>;
	type AssetKind = ();
	type Beneficiary = AccountId;
	type BeneficiaryLookup = Indices;
	type Paymaster = frame_support::traits::tokens::pay::PayFromAccount<Balances, TreasuryAccount>;
	type BalanceConverter = frame_support::traits::tokens::UnityAssetBalanceConversion;
	type PayoutPeriod = SpendPayoutPeriod;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

impl pallet_offences::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = AuthorityMembership;
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
}

parameter_types! {
	pub const MaxSetIdSessionEntries: u32 = BondingDuration::get() * SessionsPerEra::get();
}

impl pallet_grandpa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;

	type WeightInfo = ();
	type MaxAuthorities = MaxAuthorities;
	type MaxNominators = ();
	type MaxSetIdSessionEntries = MaxSetIdSessionEntries;
	type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;
	type EquivocationReportSystem =
		pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
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
		nonce: Nonce,
	) -> Option<(RuntimeCall, <UncheckedExtrinsic as ExtrinsicT>::SignaturePayload)> {
		use sp_runtime::traits::StaticLookup;
		let tip = 0;

		// take the biggest period possible.
		let period =
			BlockHashCount::get().checked_next_power_of_two().map(|c| c / 2).unwrap_or(2) as u64;

		let current_block = System::block_number()
			.saturated_into::<u64>()
			// The `System::block_number` is initialized with `n+1`,
			// so the actual block number is `n`.
			.saturating_sub(1);
		let extra: SignedExtra = (
			pallet_network_membership::CheckNetworkMembership::<Runtime>::new(),
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
	pub const MaxProposalLength: u16 = 5;
}
impl authority_membership::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type IsMember = NetworkMembership;
	type MinAuthorities = ConstU32<3>;
	type AuthorityMembershipOrigin = MoreThanHalfCouncil;
}

parameter_types! {
	pub const MaxWellKnownNodes: u32 = 1_000;
	pub const MaxPeerIdLength: u32 = 128;
	pub const MaxNodeIdLength: u32 = 53;
}

impl pallet_node_authorization::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxWellKnownNodes = MaxWellKnownNodes;
	type MaxPeerIdLength = MaxPeerIdLength;
	type MaxNodeIdLength = MaxNodeIdLength;
	type NodeAuthorizationOrigin = MoreThanHalfCouncil;
	type WeightInfo = ();
}

parameter_types! {
	pub const MembershipPeriod: BlockNumber = YEAR;
	pub const MaxMembersPerBlock: u32 = 1_000;
	pub const MaxEventsHistory: u32 = u32::MAX;
}

impl pallet_network_membership::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type NetworkMembershipOrigin = MoreThanHalfCouncil;
	type MembershipPeriod = MembershipPeriod;
	type MaxMembersPerBlock = MaxMembersPerBlock;
	type WeightInfo = weights::pallet_network_membership::WeightInfo<Runtime>;
}

impl identifier::Config for Runtime {
	type MaxEventsHistory = MaxEventsHistory;
}

impl pallet_runtime_upgrade::Config for Runtime {
	type SetCodeOrigin = EnsureRootOrCommitteeApproval;
}

parameter_types! {
	#[derive(Debug, Clone, Eq, PartialEq, TypeInfo, Decode, Encode)]
	pub const MaxNewKeyAgreementKeys: u32 = 10;
	#[derive(Clone)]
	pub const MaxPublicKeysPerDid: u32 = 20;
	#[derive(Debug, Clone, Eq, PartialEq)]
	pub const MaxTotalKeyAgreementKeys: u32 = 19;
	pub const MaxBlocksTxValidity: BlockNumber =  2 * HOURS;
	pub const MaxNumberOfServicesPerDid: u32 = 25;
	pub const MaxServiceIdLength: u32 = 50;
	pub const MaxServiceTypeLength: u32 = 50;
	pub const MaxServiceUrlLength: u32 = 200;
	pub const MaxNumberOfTypesPerService: u32 = 1;
	pub const MaxNumberOfUrlsPerService: u32 = 1;
}

impl pallet_did::Config for Runtime {
	type DidIdentifier = DidIdentifier;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type RuntimeOrigin = RuntimeOrigin;

	#[cfg(not(feature = "runtime-benchmarks"))]
	type EnsureOrigin = pallet_did::EnsureDidOrigin<Self::DidIdentifier, AccountId>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type OriginSuccess = pallet_did::DidRawOrigin<AccountId, Self::DidIdentifier>;
	#[cfg(feature = "runtime-benchmarks")]
	type EnsureOrigin = EnsureSigned<Self::DidIdentifier>;
	#[cfg(feature = "runtime-benchmarks")]
	type OriginSuccess = Self::DidIdentifier;

	type MaxNewKeyAgreementKeys = MaxNewKeyAgreementKeys;
	type MaxPublicKeysPerDid = MaxPublicKeysPerDid;
	type MaxTotalKeyAgreementKeys = MaxTotalKeyAgreementKeys;
	type MaxBlocksTxValidity = MaxBlocksTxValidity;
	type MaxNumberOfServicesPerDid = MaxNumberOfServicesPerDid;
	type MaxServiceIdLength = MaxServiceIdLength;
	type MaxServiceTypeLength = MaxServiceTypeLength;
	type MaxServiceUrlLength = MaxServiceUrlLength;
	type MaxNumberOfTypesPerService = MaxNumberOfTypesPerService;
	type MaxNumberOfUrlsPerService = MaxNumberOfUrlsPerService;
	type WeightInfo = weights::pallet_did::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MinNameLength: u32 = 3;
	pub const MaxNameLength: u32 = 64;
	pub const MaxPrefixLength: u32 = 54;
}

impl pallet_did_name::Config for Runtime {
	type BanOrigin = EnsureRoot<AccountId>;
	type EnsureOrigin = pallet_did::EnsureDidOrigin<DidIdentifier, AccountId>;
	type OriginSuccess = pallet_did::DidRawOrigin<AccountId, DidIdentifier>;
	type RuntimeEvent = RuntimeEvent;
	type MaxNameLength = MaxNameLength;
	type MinNameLength = MinNameLength;
	type MaxPrefixLength = MaxPrefixLength;
	type DidName = pallet_did_name::did_name::AsciiDidName<Runtime>;
	type DidNameOwner = DidIdentifier;
	type WeightInfo = weights::pallet_did_name::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MaxEncodedSchemaLength: u32 = 15_360;
}

impl pallet_schema::Config for Runtime {
	type SchemaCreatorId = DidIdentifier;
	type EnsureOrigin = pallet_did::EnsureDidOrigin<DidIdentifier, AccountId>;
	type OriginSuccess = pallet_did::DidRawOrigin<AccountId, DidIdentifier>;
	type RuntimeEvent = RuntimeEvent;
	type MaxEncodedSchemaLength = MaxEncodedSchemaLength;
	type WeightInfo = weights::pallet_schema::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MaxSpaceDelegates: u32 = 10_000;
}

impl pallet_chain_space::Config for Runtime {
	type SpaceCreatorId = DidIdentifier;
	type EnsureOrigin = pallet_did::EnsureDidOrigin<DidIdentifier, AccountId>;
	type OriginSuccess = pallet_did::DidRawOrigin<AccountId, DidIdentifier>;
	type RuntimeEvent = RuntimeEvent;
	type ChainSpaceOrigin = MoreThanHalfCouncil;
	type NetworkPermission = NetworkParameters;
	type MaxSpaceDelegates = MaxSpaceDelegates;
	type WeightInfo = weights::pallet_chain_space::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MaxRegistryBlobSize: u32 = 4 * 1024;
	pub const MaxEncodedInputLength: u32 = 30;
	pub const MaxRegistryDelegates: u32 = 10_000;
}

impl pallet_registries::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxRegistryDelegates = MaxRegistryDelegates;
	type MaxRegistryBlobSize = MaxRegistryBlobSize;
	type MaxEncodedInputLength = MaxEncodedInputLength;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxDigestsPerBatch: u16 = 1_000;
	pub const MaxRemoveEntries: u16 = 1_000;
}

impl pallet_statement::Config for Runtime {
	type EnsureOrigin = pallet_did::EnsureDidOrigin<DidIdentifier, AccountId>;
	type OriginSuccess = pallet_did::DidRawOrigin<AccountId, DidIdentifier>;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_statement::WeightInfo<Runtime>;
	type MaxDigestsPerBatch = MaxDigestsPerBatch;
	type MaxRemoveEntries = MaxRemoveEntries;
}

impl pallet_remark::Config for Runtime {
	type WeightInfo = weights::pallet_remark::WeightInfo<Runtime>;
	type RuntimeEvent = RuntimeEvent;
}

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = weights::pallet_sudo::WeightInfo<Runtime>;
}

impl pallet_network_score::Config for Runtime {
	type RatingProviderIdOf = DidIdentifier;
	type EnsureOrigin = pallet_did::EnsureDidOrigin<DidIdentifier, AccountId>;
	type OriginSuccess = pallet_did::DidRawOrigin<AccountId, DidIdentifier>;
	type RuntimeEvent = RuntimeEvent;
	type MaxEncodedValueLength = ConstU32<128>;
	type MaxRatingValue = ConstU32<50>;
	type WeightInfo = weights::pallet_network_score::WeightInfo<Runtime>;
}

impl pallet_config::Config for Runtime {}
impl pallet_insecure_randomness_collective_flip::Config for Runtime {}
impl pallet_root_testing::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

pub enum AllowBalancesCall {}

impl frame_support::traits::Contains<RuntimeCall> for AllowBalancesCall {
	fn contains(call: &RuntimeCall) -> bool {
		matches!(call, RuntimeCall::Balances(BalancesCall::transfer_allow_death { .. }))
	}
}

fn schedule<T: pallet_contracts::Config>() -> pallet_contracts::Schedule<T> {
	pallet_contracts::Schedule {
		limits: pallet_contracts::Limits {
			runtime_memory: 1024 * 1024 * 1024,
			..Default::default()
		},
		..Default::default()
	}
}

parameter_types! {
	pub const DepositPerItem: Balance = deposit(1, 0);
	pub const DepositPerByte: Balance = deposit(0, 1);
	pub Schedule: pallet_contracts::Schedule<Runtime> = schedule::<Runtime>();
	pub const DefaultDepositLimit: Balance = deposit(1024, 1024 * 1024);
	pub const CodeHashLockupDepositPercent: Perbill = Perbill::from_percent(20);
	pub const MaxDelegateDependencies: u32 = 32;
}

impl pallet_contracts::Config for Runtime {
	type Time = Timestamp;
	type Randomness = RandomnessCollectiveFlip;
	type Currency = Balances;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type CallFilter = AllowBalancesCall;
	type DepositPerItem = DepositPerItem;
	type DepositPerByte = DepositPerByte;
	type CallStack = [pallet_contracts::Frame<Self>; 23];
	type WeightPrice = pallet_transaction_payment::Pallet<Self>;
	type WeightInfo = pallet_contracts::weights::SubstrateWeight<Self>;
	type ChainExtension = ();
	type Schedule = Schedule;
	type AddressGenerator = pallet_contracts::DefaultAddressGenerator;
	type MaxCodeLen = ConstU32<{ 128 * 1024 }>;
	type DefaultDepositLimit = DefaultDepositLimit;
	type MaxStorageKeyLen = ConstU32<128>;
	type MaxDebugBufferLen = ConstU32<{ 2 * 1024 * 1024 }>;
	type MaxTransientStorageSize = ConstU32<{ 1 * 1024 * 1024 }>;
	type UnsafeUnstableInterface = ConstBool<true>;
	type CodeHashLockupDepositPercent = CodeHashLockupDepositPercent;
	type MaxDelegateDependencies = MaxDelegateDependencies;
	type RuntimeHoldReason = RuntimeHoldReason;
	type Environment = ();
	type Debug = ();
	type ApiVersion = ();
	type Migrations = ();
	type Xcm = ();
	type UploadOrigin = EnsureSigned<Self::AccountId>;
	type InstantiateOrigin = EnsureSigned<Self::AccountId>;
}

#[frame_support::runtime]
mod runtime {
	#[runtime::runtime]
	#[runtime::derive(
		RuntimeCall,
		RuntimeEvent,
		RuntimeError,
		RuntimeOrigin,
		RuntimeFreezeReason,
		RuntimeHoldReason,
		RuntimeSlashReason,
		RuntimeLockId,
		RuntimeTask
	)]
	pub struct Runtime;

	#[runtime::pallet_index(0)]
	pub type System = frame_system;

	#[runtime::pallet_index(1)]
	pub type Utility = pallet_utility;

	#[runtime::pallet_index(2)]
	pub type Babe = pallet_babe;

	#[runtime::pallet_index(3)]
	pub type Timestamp = pallet_timestamp;

	#[runtime::pallet_index(4)]
	pub type AuthorityMembership = authority_membership;

	#[runtime::pallet_index(5)]
	pub type Authorship = pallet_authorship;

	#[runtime::pallet_index(6)]
	pub type Indices = pallet_indices;

	#[runtime::pallet_index(7)]
	pub type Balances = pallet_balances;

	#[runtime::pallet_index(8)]
	pub type Session = pallet_session;

	#[runtime::pallet_index(9)]
	pub type TransactionPayment = pallet_transaction_payment;

	#[runtime::pallet_index(10)]
	pub type Treasury = pallet_treasury;

	#[runtime::pallet_index(12)]
	pub type RandomnessCollectiveFlip = pallet_insecure_randomness_collective_flip;

	#[runtime::pallet_index(20)]
	pub type Council = pallet_collective<Instance1>;

	#[runtime::pallet_index(21)]
	pub type CouncilMembership = pallet_membership<Instance1>;

	#[runtime::pallet_index(22)]
	pub type TechnicalCommittee = pallet_collective<Instance2>;

	#[runtime::pallet_index(23)]
	pub type TechnicalMembership = pallet_membership<Instance2>;

	#[runtime::pallet_index(25)]
	pub type Grandpa = pallet_grandpa;

	#[runtime::pallet_index(26)]
	pub type ImOnline = pallet_im_online;

	#[runtime::pallet_index(27)]
	pub type AuthorityDiscovery = pallet_authority_discovery;

	#[runtime::pallet_index(28)]
	pub type Offences = pallet_offences;

	#[runtime::pallet_index(29)]
	pub type Historical = pallet_session_historical;

	#[runtime::pallet_index(30)]
	pub type Identity = pallet_identity;

	#[runtime::pallet_index(31)]
	pub type Scheduler = pallet_scheduler;

	#[runtime::pallet_index(32)]
	pub type Preimage = pallet_preimage;

	#[runtime::pallet_index(33)]
	pub type Multisig = pallet_multisig;

	#[runtime::pallet_index(34)]
	pub type NodeAuthorization = pallet_node_authorization;

	#[runtime::pallet_index(35)]
	pub type RuntimeUpgrade = pallet_runtime_upgrade;

	#[runtime::pallet_index(36)]
	pub type Assets = pallet_assets<Instance1>;

	#[runtime::pallet_index(37)]
	pub type PoolAssets = pallet_assets<Instance2>;

	#[runtime::pallet_index(38)]
	pub type Contracts = pallet_contracts;

	#[runtime::pallet_index(50)]
	pub type Identifier = identifier;

	#[runtime::pallet_index(51)]
	pub type NetworkMembership = pallet_network_membership;

	#[runtime::pallet_index(52)]
	pub type Did = pallet_did;

	#[runtime::pallet_index(53)]
	pub type Schema = pallet_schema;

	#[runtime::pallet_index(54)]
	pub type ChainSpace = pallet_chain_space;

	#[runtime::pallet_index(55)]
	pub type Statement = pallet_statement;

	#[runtime::pallet_index(56)]
	pub type DidName = pallet_did_name;

	#[runtime::pallet_index(57)]
	pub type NetworkScore = pallet_network_score;

	#[runtime::pallet_index(58)]
	pub type AssetConversion = pallet_asset_conversion;

	#[runtime::pallet_index(59)]
	pub type Remark = pallet_remark;

	#[runtime::pallet_index(60)]
	pub type NetworkParameters = pallet_config;

	#[runtime::pallet_index(61)]
	pub type Registries = pallet_registries;

	#[runtime::pallet_index(62)]
	pub type Entries = pallet_entries;

	#[runtime::pallet_index(254)]
	pub type RootTesting = pallet_root_testing;

	#[runtime::pallet_index(255)]
	pub type Sudo = pallet_sudo;
}

#[rustfmt::skip]
impl pallet_did::DeriveDidCallAuthorizationVerificationKeyRelationship for RuntimeCall {
	fn derive_verification_key_relationship(
		&self,
	) -> pallet_did::DeriveDidCallKeyRelationshipResult {
		fn single_key_relationship(
			calls: &[RuntimeCall],
		) -> pallet_did::DeriveDidCallKeyRelationshipResult {
			let init = calls
				.get(0)
				.ok_or(pallet_did::RelationshipDeriveError::InvalidCallParameter)?
				.derive_verification_key_relationship()?;
			calls
				.iter()
				.skip(1)
				.map(RuntimeCall::derive_verification_key_relationship)
				.try_fold(init, |acc, next| {
					if Ok(acc) == next {
						Ok(acc)
					} else {
						Err(pallet_did::RelationshipDeriveError::InvalidCallParameter)
					}
				})
		}
		match self {
			// DID creation is not allowed through the DID proxy.
			RuntimeCall::Did(pallet_did::Call::create { .. }) => {
				Err(pallet_did::RelationshipDeriveError::NotCallableByDid)
			},
			RuntimeCall::Did { .. } => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::DidName { .. } => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::Schema { .. } => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::Statement { .. } => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::NetworkScore { .. } => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::add_delegate { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::CapabilityDelegation)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::add_admin_delegate { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::CapabilityDelegation)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::add_delegator { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::CapabilityDelegation)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::remove_delegate { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::CapabilityDelegation)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::create { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::archive { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::restore { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::subspace_create { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::update_transaction_capacity { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::ChainSpace(pallet_chain_space::Call::update_transaction_capacity_sub { .. }) => {
				Ok(pallet_did::DidVerificationKeyRelationship::Authentication)
			},
			RuntimeCall::Utility(pallet_utility::Call::batch { calls }) => {
				single_key_relationship(&calls[..])
			},
			RuntimeCall::Utility(pallet_utility::Call::batch_all { calls }) => {
				single_key_relationship(&calls[..])
			},
			RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) => {
				single_key_relationship(&calls[..])
			},
			#[cfg(not(feature = "runtime-benchmarks"))]
			_ => Err(pallet_did::RelationshipDeriveError::NotCallableByDid),
			// By default, returns the authentication key
			#[cfg(feature = "runtime-benchmarks")]
			_ => Ok(pallet_did::DidVerificationKeyRelationship::Authentication),
		}
	}

	// Always return a System::remark() extrinsic call
	#[cfg(feature = "runtime-benchmarks")]
	fn get_call_for_did_call_benchmark() -> Self {
		RuntimeCall::System(frame_system::Call::remark { remark: vec![] })
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
	pallet_network_membership::CheckNetworkMembership<Runtime>,
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckMortality<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;

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

type EventRecord = frame_system::EventRecord<
	<Runtime as frame_system::Config>::RuntimeEvent,
	<Runtime as frame_system::Config>::Hash,
>;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	frame_benchmarking::define_benchmarks!(
		[frame_benchmarking, BaselineBench::<Runtime>]
		[pallet_babe, Babe]
		[pallet_balances, Balances]
		[pallet_collective, Council]
		[pallet_contracts, Contracts]
		[pallet_grandpa, Grandpa]
		[pallet_identity, Identity]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_im_online, ImOnline]
		[pallet_indices, Indices]
		[pallet_membership, TechnicalMembership]
		[pallet_multisig, Multisig]
		[pallet_preimage, Preimage]
		[pallet_remark, Remark]
		[pallet_scheduler, Scheduler]
		[frame_system, SystemBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		// [pallet_treasury, Treasury]
		[pallet_utility, Utility]
		[pallet_schema, Schema]
		[pallet_statement, Statement]
		[pallet_chain_space, ChainSpace]
		[pallet_did, Did]
		[pallet_did_name, DidName]
		[pallet_network_membership, NetworkMembership]
		[pallet_network_score, NetworkScore]
		[pallet_sudo, Sudo]
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

		fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> sp_std::vec::Vec<u32> {
			Runtime::metadata_versions()
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

	impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> sp_consensus_grandpa::AuthorityList {
			Grandpa::grandpa_authorities()
		}

		fn current_set_id() -> sp_consensus_grandpa::SetId {
			Grandpa::current_set_id()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			equivocation_proof: sp_consensus_grandpa::EquivocationProof<
				<Block as BlockT>::Hash,
				NumberFor<Block>,
			>,
			key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Grandpa::submit_unsigned_equivocation_report(
				equivocation_proof,
				key_owner_proof,
			)
		}

		fn generate_key_ownership_proof(
			_set_id: sp_consensus_grandpa::SetId,
			authority_id: GrandpaId,
		) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
			use codec::Encode;

			Historical::prove((sp_consensus_grandpa::KEY_TYPE, authority_id))
				.map(|p| p.encode())
				.map(sp_consensus_grandpa::OpaqueKeyOwnershipProof::new)
		}
	}

	impl sp_consensus_babe::BabeApi<Block> for Runtime {
		fn configuration() -> sp_consensus_babe::BabeConfiguration {
			let epoch_config = Babe::epoch_config().unwrap_or(BABE_GENESIS_EPOCH_CONFIG);
			sp_consensus_babe::BabeConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get(),
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

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_assets_runtime_api::AssetsApi<
		Block,
		AccountId,
		Balance,
		u32,
	> for Runtime
	{
		fn account_balances(account: AccountId) -> Vec<(u32, Balance)> {
			Assets::account_balances(account)
		}
	}

	impl pallet_contracts::ContractsApi<Block, AccountId, Balance, BlockNumber, Hash, EventRecord> for Runtime
	{
		fn call(
			origin: AccountId,
			dest: AccountId,
			value: Balance,
			gas_limit: Option<Weight>,
			storage_deposit_limit: Option<Balance>,
			input_data: Vec<u8>,
		) -> pallet_contracts::ContractExecResult<Balance, EventRecord> {
			let gas_limit = gas_limit.unwrap_or(BlockWeights::get().max_block);
			Contracts::bare_call(
				origin,
				dest,
				value,
				gas_limit,
				storage_deposit_limit,
				input_data,
				pallet_contracts::DebugInfo::UnsafeDebug,
				pallet_contracts::CollectEvents::UnsafeCollect,
				pallet_contracts::Determinism::Enforced,
			)
		}

		fn instantiate(
			origin: AccountId,
			value: Balance,
			gas_limit: Option<Weight>,
			storage_deposit_limit: Option<Balance>,
			code: pallet_contracts::Code<Hash>,
			data: Vec<u8>,
			salt: Vec<u8>,
		) -> pallet_contracts::ContractInstantiateResult<AccountId, Balance, EventRecord>
		{
			let gas_limit = gas_limit.unwrap_or(BlockWeights::get().max_block);
			Contracts::bare_instantiate(
				origin,
				value,
				gas_limit,
				storage_deposit_limit,
				code,
				data,
				salt,
				pallet_contracts::DebugInfo::UnsafeDebug,
				pallet_contracts::CollectEvents::UnsafeCollect,
			)
		}

		fn upload_code(
			origin: AccountId,
			code: Vec<u8>,
			storage_deposit_limit: Option<Balance>,
			determinism: pallet_contracts::Determinism,
		) -> pallet_contracts::CodeUploadResult<Hash, Balance>
		{
			Contracts::bare_upload_code(
				origin,
				code,
				storage_deposit_limit,
				determinism,
			)
		}

		fn get_storage(
			address: AccountId,
			key: Vec<u8>,
		) -> pallet_contracts::GetStorageResult {
			Contracts::get_storage(
				address,
				key
			)
		}
	}

	impl pallet_did_runtime_api::DidApi<
		Block,
		DidIdentifier,
		AccountId,
		Hash,
		BlockNumber
	> for Runtime {
		fn query(did: DidIdentifier) -> Option<
			pallet_did_runtime_api::RawDidLinkedInfo<
				DidIdentifier,
				AccountId,
				Hash,
				BlockNumber
			>
		> {
			let details = pallet_did::Did::<Runtime>::get(&did)?;
			let name = pallet_did_name::Names::<Runtime>::get(&did).map(Into::into);
			let service_endpoints = pallet_did::ServiceEndpoints::<Runtime>::iter_prefix(&did).map(|e| From::from(e.1)).collect();

			Some(pallet_did_runtime_api::RawDidLinkedInfo {
				identifier: did.clone(),
				account: did,
				name,
				service_endpoints,
				details: details.into(),
			})
		}
		fn query_by_name(name: Vec<u8>) -> Option<pallet_did_runtime_api::RawDidLinkedInfo<
				DidIdentifier,
				AccountId,
				Hash,
				BlockNumber
			>
		> {
			let dname: pallet_did_name::did_name::AsciiDidName<Runtime> = name.try_into().ok()?;
			pallet_did_name::Owner::<Runtime>::get(&dname)
				.and_then(|owner_info| {
					pallet_did::Did::<Runtime>::get(&owner_info.owner).map(|details| (owner_info, details))
				})
				.map(|(owner_info, details)| {
					let service_endpoints = pallet_did::ServiceEndpoints::<Runtime>::iter_prefix(&owner_info.owner).map(|e| From::from(e.1)).collect();

					pallet_did_runtime_api::RawDidLinkedInfo{
						identifier: owner_info.owner.clone(),
						account: owner_info.owner,
						name: Some(dname.into()),
						service_endpoints,
						details: details.into(),
					}
			})
		}
	}

	impl pallet_transaction_weight_runtime_api::TransactionWeightApi<Block> for Runtime {
		fn query_weight_info(uxt: <Block as BlockT>::Extrinsic) -> RuntimeDispatchWeightInfo {
			NetworkMembership::query_weight_info(uxt)
		}
	}


	impl pallet_asset_conversion::AssetConversionApi<
		Block,
		Balance,
		NativeOrWithId<u32>
	> for Runtime
	{
		fn quote_price_exact_tokens_for_tokens(asset1: NativeOrWithId<u32>, asset2: NativeOrWithId<u32>, amount: Balance, include_fee: bool) -> Option<Balance> {
			AssetConversion::quote_price_exact_tokens_for_tokens(asset1, asset2, amount, include_fee)
		}

		fn quote_price_tokens_for_exact_tokens(asset1: NativeOrWithId<u32>, asset2: NativeOrWithId<u32>, amount: Balance, include_fee: bool) -> Option<Balance> {
			AssetConversion::quote_price_tokens_for_exact_tokens(asset1, asset2, amount, include_fee)
		}

		fn get_reserves(asset1: NativeOrWithId<u32>, asset2: NativeOrWithId<u32>) -> Option<(Balance, Balance)> {
			AssetConversion::get_reserves(asset1, asset2).ok()
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
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
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
	fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			log::info!("try-runtime::on_runtime_upgrade cord.");
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
			state_root_check: bool,
			signature_check: bool,
			select: frame_try_runtime::TryStateSelect,
		) -> Weight {
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here.
			Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{baseline, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;

			use pallet_session_benchmarking::Pallet as SessionBench;
			use frame_system_benchmarking::Pallet as SystemBench;
			use baseline::Pallet as BaselineBench;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			(list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig,
		) -> Result<
			Vec<frame_benchmarking::BenchmarkBatch>,
			sp_runtime::RuntimeString,
		> {
			use frame_support::traits::WhitelistedStorageKeys;
			use frame_benchmarking::{baseline, Benchmarking, BenchmarkBatch };
			use sp_storage::TrackedStorageKey;

			use pallet_session_benchmarking::Pallet as SessionBench;
			use frame_system_benchmarking::Pallet as SystemBench;
			use baseline::Pallet as BaselineBench;

			impl pallet_session_benchmarking::Config for Runtime {}
			impl frame_system_benchmarking::Config for Runtime {}
			impl baseline::Config for Runtime {}

			let mut whitelist: Vec<TrackedStorageKey> = AllPalletsWithSystem::whitelisted_storage_keys();

			// Treasury Account
			// TODO: this is manual for now, someday we might be able to use a
			// macro for this particular key
			let treasury_key = frame_system::Account::<Runtime>::hashed_key_for(Treasury::account_id());
			whitelist.push(treasury_key.to_vec().into());

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);
			Ok(batches)
		}
	}

	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
		fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
			build_state::<RuntimeGenesisConfig>(config)
		}

		fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
			get_preset::<RuntimeGenesisConfig>(id, |_| None)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			vec![]
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
			T: CreateSignedTransaction<RuntimeCall>,
		{
		}

		is_submit_signed_transaction::<Runtime>();
	}
	#[test]
	fn call_size() {
		let size = core::mem::size_of::<RuntimeCall>();
		assert!(
			size <= CALL_PARAMS_MAX_SIZE,
			"size of RuntimeCall {} is more than {CALL_PARAMS_MAX_SIZE} bytes.
			 Some calls have too big arguments, use Box to reduce the size of RuntimeCall.
			 If the limit is too strong, maybe consider increase the limit.",
			size,
		);
	}
}
