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
#![recursion_limit = "512"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

extern crate alloc;

pub mod coretime;
pub mod entity;
// Genesis preset configurations.
pub mod genesis_config_presets;
mod meta_v6;
#[cfg(feature = "runtime-benchmarks")]
#[cfg(test)]
mod meta_v6_weight_evidence;
#[cfg(all(test, not(feature = "runtime-benchmarks")))]
mod transaction_policy_vectors;

#[cfg(test)]
mod enterprise_journey;
#[cfg(test)]
mod tests;
mod weights;
pub mod xcm_config;

#[cfg(feature = "runtime-benchmarks")]
use alloc::boxed::Box;
use alloc::{borrow::Cow, string::String, vec, vec::Vec};
use assets_common::{
	local_and_foreign_assets::{LocalFromLeft, TargetFromLeft},
	AssetIdForTrustBackedAssetsConvert,
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::{cmp::Ordering, convert::TryInto};
use cumulus_pallet_parachain_system::{RelayNumberMonotonicallyIncreases, RelaychainDataProvider};
use cumulus_primitives_core::{
	relay_chain::AccountIndex, AggregateMessageOrigin, ParaId, VerifySchedulingSignature,
};
use frame_support::{
	construct_runtime, derive_impl,
	dispatch::DispatchClass,
	genesis_builder_helper::{build_state, get_preset},
	parameter_types,
	traits::{
		fungible, fungibles, tokens::imbalance::ResolveAssetTo, AsEnsureOriginWithArg, ConstBool,
		ConstU128, ConstU32, ConstU64, Contains, EitherOf, EitherOfDiverse, InstanceFilter,
		PrivilegeCmp, TransformOrigin, VariantCountOf,
	},
	weights::{ConstantMultiplier, Weight},
	PalletId,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot, EnsureRootWithSuccess, EnsureSigned,
};
pub use orbis_identity_personhood_runtime_api as identity_personhood_api;
pub use orbis_storage_runtime_api as storage_api;
pub use origin_commons_runtime_constants::async_backing::SLOT_DURATION;
use origin_commons_runtime_constants::{
	async_backing::{
		AVERAGE_ON_INITIALIZE_RATIO, HOURS, MAXIMUM_BLOCK_WEIGHT, MINUTES, NORMAL_DISPATCH_RATIO,
	},
	origin::currency::*,
};
use origin_primitives::identifier::{DecodedIdentifier, Ss58Identifier};
use origin_runtime_constants::{currency::EXISTENTIAL_DEPOSIT, fee, time::DAYS};
use pallet_asset_conversion_tx_payment::SwapAssetAdapter;
use pallet_assets_precompiles::{ForeignIdConfig, InlineIdConfig, ERC20};
use pallet_nfts::PalletFeatures;
pub use pallet_orbis_attestation_runtime_api as attestation_api;
pub use pallet_orbis_names_runtime_api as names_api;
use pallet_orbis_storage_provider::CheckpointContextProvider as _;
use pallet_origin_token::Token as TokenTrait;
use pallet_revive::evm::runtime::EthExtra;
use pallet_transaction_payment::FungibleAdapter;
use pallet_tx_pause::RuntimeCallNameOf;
use pallet_xcm::{EnsureXcm, IsVoiceOfBody};
use parachains_common::{
	message_queue::*, AccountId, AuraId, Balance, BlockNumber, Hash, Header, Nonce, Signature,
};
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};
use scale_info::TypeInfo;
use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
	generic, impl_opaque_keys,
	traits::{BlakeTwo256, Block as BlockT, Verify},
	transaction_validity::{
		TransactionLongevity, TransactionPriority, TransactionSource, TransactionValidity,
	},
	ApplyExtrinsicResult, FixedU128, MultiSignature, MultiSigner,
};
pub use sp_runtime::{MultiAddress, Perbill, Permill};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
/// Runtime API definition for token.
pub use token_origin_commons_runtime_api as token_api;
use verifiable::{ring::bandersnatch::BandersnatchVrfVerifiable, GenerateVerifiable};

use weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight};
use xcm::{
	latest::prelude::*, Version as XcmVersion, VersionedAsset, VersionedAssetId, VersionedAssets,
	VersionedLocation, VersionedXcm,
};
use xcm_config::{
	GovernanceLocation, PriceForSiblingParachainDelivery, XcmOriginToTransactDispatchOrigin,
};
use xcm_runtime_apis::{
	dry_run::{CallDryRunEffects, Error as XcmDryRunApiError, XcmDryRunEffects},
	fees::Error as XcmPaymentApiError,
};

/// Build one relay parent behind the relay-chain tip, avoiding relay fork races in the
/// slot-based elastic authoring pipeline.
const RELAY_PARENT_OFFSET: u32 = 1;

/// Maximum Orbis blocks produced per six-second Origin relay slot. When three cores are assigned,
/// the slot-based collator can build one collation per core (and bundle multiple blocks per
/// collation where the node determines that is appropriate), targeting an effective two-second
/// block interval.
const BLOCK_PROCESSING_VELOCITY: u32 = 3;

/// Capacity required for a three-core pipeline plus the one-block relay-parent offset.
const UNINCLUDED_SEGMENT_CAPACITY: u32 = (3 + RELAY_PARENT_OFFSET) * BLOCK_PROCESSING_VELOCITY;

/// Origin retains six-second relay-chain slots; Orbis obtains throughput from multiple cores and
/// block bundles rather than changing relay consensus timing.
const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6_000;

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

#[cfg(not(feature = "p1-upgrade-candidate"))]
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: Cow::Borrowed("commons"),
	impl_name: Cow::Borrowed("origin-commons"),
	authoring_version: 1,
	spec_version: 33,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 8,
	system_version: 1,
};

/// Evidence-only fast-runtime upgrade candidate, one version after the normal P1 schema.
#[cfg(feature = "p1-upgrade-candidate")]
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: Cow::Borrowed("commons"),
	impl_name: Cow::Borrowed("origin-commons"),
	authoring_version: 1,
	spec_version: 34,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 8,
	system_version: 1,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

/// Calls that can bypass the safe-mode pallet.
pub struct SafeModeWhitelistedCalls;
impl Contains<RuntimeCall> for SafeModeWhitelistedCalls {
	fn contains(call: &RuntimeCall) -> bool {
		match call {
			RuntimeCall::System(_) | RuntimeCall::SafeMode(_) | RuntimeCall::TxPause(_) => true,
			_ => false,
		}
	}
}

/// Calls that cannot be paused by the tx-pause pallet.
pub struct TxPauseWhitelistedCalls;
/// Whitelist `Balances::transfer_keep_alive`, all others are pauseable.
impl Contains<RuntimeCallNameOf<Runtime>> for TxPauseWhitelistedCalls {
	fn contains(full_name: &RuntimeCallNameOf<Runtime>) -> bool {
		match (full_name.0.as_slice(), full_name.1.as_slice()) {
			(b"Balances", b"transfer_keep_alive") => true,
			_ => false,
		}
	}
}

impl pallet_tx_pause::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PauseOrigin = EnsureRoot<AccountId>;
	type UnpauseOrigin = EnsureRoot<AccountId>;
	type WhitelistedCalls = TxPauseWhitelistedCalls;
	type MaxNameLen = ConstU32<256>;
	type WeightInfo = weights::pallet_tx_pause::WeightInfo<Runtime>;
}

parameter_types! {
	pub const EnterDuration: BlockNumber = 4 * HOURS;
	pub const EnterDepositAmount: Balance = 2_000_000 * UNITS;
	pub const ExtendDuration: BlockNumber = 2 * HOURS;
	pub const ExtendDepositAmount: Balance = 1_000_000 * UNITS;
	pub const ReleaseDelay: u32 = 2 * DAYS;
}

impl pallet_safe_mode::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type WhitelistedCalls = SafeModeWhitelistedCalls;
	type EnterDuration = EnterDuration;
	type EnterDepositAmount = EnterDepositAmount;
	type ExtendDuration = ExtendDuration;
	type ExtendDepositAmount = ExtendDepositAmount;
	type ForceEnterOrigin = EnsureRootWithSuccess<AccountId, ConstU32<9>>;
	type ForceExtendOrigin = EnsureRootWithSuccess<AccountId, ConstU32<11>>;
	type ForceExitOrigin = EnsureRoot<AccountId>;
	type ForceDepositOrigin = EnsureRoot<AccountId>;
	type ReleaseDelay = ReleaseDelay;
	type Notify = ();
	type WeightInfo = weights::pallet_safe_mode::WeightInfo<Runtime>;
}

/// We currently allow all calls.
pub struct BaseFilter;
impl Contains<RuntimeCall> for BaseFilter {
	fn contains(_c: &RuntimeCall) -> bool {
		true
	}
}

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;
	pub RuntimeBlockLength: BlockLength = BlockLength::builder()
		.max_length(5 * 1024 * 1024)
		.modify_max_length_for_class(DispatchClass::Normal, |m| {
			*m = NORMAL_DISPATCH_RATIO * *m
		})
		.build();
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

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type BaseCallFilter = meta_v6::BaseFilter;
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = RuntimeBlockLength;
	type AccountId = AccountId;
	type Nonce = Nonce;
	type Hash = Hash;
	type Block = Block;
	type BlockHashCount = BlockHashCount;
	type DbWeight = RocksDbWeight;
	type Version = Version;
	type AccountData = pallet_balances::AccountData<Balance>;
	type SystemWeightInfo = weights::frame_system::WeightInfo<Runtime>;
	type ExtensionsWeightInfo = weights::frame_system_extensions::WeightInfo<Runtime>;
	type SS58Prefix = SS58Prefix;
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = ConstU32<256>;
	type MultiBlockMigrator = MultiBlockMigrations;
	type PostTransactions = meta_v6::MetaTokenMustBeEmpty;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80)
		* RuntimeBlockWeights::get()
			.get(DispatchClass::Normal)
			.max_total
			.unwrap_or_else(|| RuntimeBlockWeights::get().max_block);
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
			// For every other origin we don't care, as they are not used for `ScheduleOrigin`.
			_ => None,
		}
	}
}

impl pallet_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type MaxScheduledPerBlock = ConstU32<512>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type OriginPrivilegeCmp = OriginPrivilegeCmp;
	type Preimages = ();
	type WeightInfo = weights::pallet_scheduler::WeightInfo<Runtime>;
	type BlockNumberProvider = System;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Aura;
	type MinimumPeriod = ConstU64<0>;
	type WeightInfo = weights::pallet_timestamp::WeightInfo<Runtime>;
}

impl cumulus_pallet_weight_reclaim::Config for Runtime {
	type WeightInfo = weights::cumulus_pallet_weight_reclaim::WeightInfo<Runtime>;
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type EventHandler = (CollatorSelection,);
}

parameter_types! {
	pub const IndexDeposit: Balance = EXISTENTIAL_DEPOSIT;
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
	type AccountStore = frame_system::Pallet<Runtime>;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
	type DoneSlashHandler = ();
	type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
}

parameter_types! {
	pub const AssetDeposit: Balance = system_para_deposit(1, 64);
	pub const AssetAccountDeposit: Balance = system_para_deposit(1, 16);
	pub const AssetApprovalDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const AssetsStringLimit: u32 = 50;
	pub const AssetMetadataDepositBase: Balance = system_para_deposit(1, 68);
	pub const AssetMetadataDepositPerByte: Balance = system_para_deposit(0, 1);
	pub OrbisAssetAdmin: AccountId = AccountId::from([0xA5; 32]);
}

/// Local enterprise assets addressable by a compact `u32` identifier.
pub type AssetsInstance = pallet_assets::Instance1;
pub type AssetsFreezerInstance = pallet_assets_freezer::Instance1;
pub type AssetsHolderInstance = pallet_assets_holder::Instance1;
pub type ForeignAssetsInstance = pallet_assets::Instance2;
pub type ForeignAssetsFreezerInstance = pallet_assets_freezer::Instance2;
pub type PoolAssetsInstance = pallet_assets::Instance3;
pub type PoolAssetsFreezerInstance = pallet_assets_freezer::Instance3;

parameter_types! {
	pub TrustBackedAssetsPalletLocation: Location = Location::new(0, [PalletInstance(80)]);
	pub const TrustBackedAssetsPalletIndex: u8 = 80;
	pub AssetConversionFeeAccount: AccountId = AccountId::from([0xA6; 32]);
	pub const AssetConversionPalletId: PalletId = PalletId(*b"py/ascon");
	pub const PoolSetupFee: Balance = UNITS;
	pub const LiquidityWithdrawalFee: Permill = Permill::zero();
	pub const AssetConversionLpFee: Permill = Permill::from_perthousand(3);
}

/// Union of Orbis trust-backed and foreign assets, keyed by canonical XCM locations.
pub type LocalAndForeignAssets = fungibles::UnionOf<
	Assets,
	ForeignAssets,
	LocalFromLeft<
		AssetIdForTrustBackedAssetsConvert<TrustBackedAssetsPalletLocation, Location>,
		u32,
		Location,
	>,
	Location,
	AccountId,
>;

/// Native Origin balance plus every non-pool asset accepted by Orbis conversion pools.
pub type NativeAndAssets = fungible::UnionOf<
	Balances,
	LocalAndForeignAssets,
	TargetFromLeft<xcm_config::OrgnRelayLocation, Location>,
	Location,
	AccountId,
>;

pub type PoolIdToAccountId =
	pallet_asset_conversion::AccountIdConverter<AssetConversionPalletId, (Location, Location)>;

impl pallet_asset_conversion::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type HigherPrecisionBalance = sp_core::U256;
	type AssetKind = Location;
	type Assets = NativeAndAssets;
	type PoolId = (Location, Location);
	type PoolLocator = pallet_asset_conversion::WithFirstAsset<
		xcm_config::OrgnRelayLocation,
		AccountId,
		Location,
		PoolIdToAccountId,
	>;
	type PoolAssetId = u32;
	type PoolAssets = PoolAssets;
	type PoolSetupFee = PoolSetupFee;
	type PoolSetupFeeAsset = xcm_config::OrgnRelayLocation;
	type PoolSetupFeeTarget = ResolveAssetTo<AssetConversionFeeAccount, NativeAndAssets>;
	type LiquidityWithdrawalFee = LiquidityWithdrawalFee;
	type LPFee = AssetConversionLpFee;
	type PalletId = AssetConversionPalletId;
	type MaxSwapPathLength = ConstU32<4>;
	type MintMinLiquidity = ConstU128<100>;
	type WeightInfo = pallet_asset_conversion::weights::SubstrateWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = assets_common::benchmarks::AssetPairFactory<
		xcm_config::OrgnRelayLocation,
		parachain_info::Pallet<Runtime>,
		TrustBackedAssetsPalletIndex,
		Location,
	>;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct ForeignAssetsBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_assets::BenchmarkHelper<Location, ()> for ForeignAssetsBenchmarkHelper {
	fn create_asset_id_parameter(id: u32) -> Location {
		Location::new(1, [Parachain(id)])
	}

	fn create_reserve_id_parameter(_: u32) {}
}

impl pallet_assets::Config<AssetsInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type AssetId = u32;
	type AssetIdParameter = codec::Compact<u32>;
	type ReserveData = ();
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type AssetDeposit = AssetDeposit;
	type MetadataDepositBase = AssetMetadataDepositBase;
	type MetadataDepositPerByte = AssetMetadataDepositPerByte;
	type ApprovalDeposit = AssetApprovalDeposit;
	type StringLimit = AssetsStringLimit;
	type Holder = AssetsHolder;
	type Freezer = AssetsFreezer;
	type Extra = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
	type CallbackHandle = pallet_assets::AutoIncAssetId<Runtime, AssetsInstance>;
	type AssetAccountDeposit = AssetAccountDeposit;
	type RemoveItemsLimit = ConstU32<1000>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

impl pallet_assets_freezer::Config<AssetsFreezerInstance> for Runtime {
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type RuntimeEvent = RuntimeEvent;
}

impl pallet_assets_holder::Config<AssetsHolderInstance> for Runtime {
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeEvent = RuntimeEvent;
}

impl pallet_assets::Config<ForeignAssetsInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type AssetId = Location;
	type AssetIdParameter = Location;
	type ReserveData = ();
	type Currency = Balances;
	type CreateOrigin = EnsureRootWithSuccess<AccountId, OrbisAssetAdmin>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type AssetDeposit = AssetDeposit;
	type MetadataDepositBase = AssetMetadataDepositBase;
	type MetadataDepositPerByte = AssetMetadataDepositPerByte;
	type ApprovalDeposit = AssetApprovalDeposit;
	type StringLimit = AssetsStringLimit;
	type Holder = ();
	type Freezer = ForeignAssetsFreezer;
	type Extra = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
	type CallbackHandle = ();
	type AssetAccountDeposit = AssetAccountDeposit;
	type RemoveItemsLimit = ConstU32<1000>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ForeignAssetsBenchmarkHelper;
}

impl pallet_assets_freezer::Config<ForeignAssetsFreezerInstance> for Runtime {
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type RuntimeEvent = RuntimeEvent;
}

impl pallet_assets::Config<PoolAssetsInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type AssetId = u32;
	type AssetIdParameter = u32;
	type ReserveData = ();
	type Currency = Balances;
	type CreateOrigin = EnsureRootWithSuccess<AccountId, OrbisAssetAdmin>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type AssetDeposit = ConstU128<0>;
	type MetadataDepositBase = ConstU128<0>;
	type MetadataDepositPerByte = ConstU128<0>;
	type ApprovalDeposit = ConstU128<0>;
	type StringLimit = AssetsStringLimit;
	type Holder = ();
	type Freezer = PoolAssetsFreezer;
	type Extra = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
	type CallbackHandle = ();
	type AssetAccountDeposit = ConstU128<0>;
	type RemoveItemsLimit = ConstU32<1000>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

impl pallet_assets_freezer::Config<PoolAssetsFreezerInstance> for Runtime {
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type RuntimeEvent = RuntimeEvent;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct AssetRateBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_asset_rate::AssetKindFactory<Location> for AssetRateBenchmarkHelper {
	fn create_asset_kind(seed: u32) -> Location {
		Location::new(0, [PalletInstance(80), GeneralIndex(seed.into())])
	}
}

impl pallet_asset_rate::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_asset_rate::weights::SubstrateWeight<Runtime>;
	type CreateOrigin = EnsureRoot<AccountId>;
	type RemoveOrigin = EnsureRoot<AccountId>;
	type UpdateOrigin = EnsureRoot<AccountId>;
	type Currency = Balances;
	type AssetKind = Location;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = AssetRateBenchmarkHelper;
}

impl pallet_assets_precompiles::ForeignAssetsConfig for Runtime {
	type ForeignAssetId = Location;
	#[cfg(feature = "runtime-benchmarks")]
	type AssetsInstance = ForeignAssetsInstance;
}

parameter_types! {
	pub const UniquesCollectionDeposit: Balance = 10 * UNITS;
	pub const UniquesItemDeposit: Balance = UNITS / 100;
	pub const UniquesMetadataDepositBase: Balance = system_para_deposit(1, 129);
	pub const UniquesAttributeDepositBase: Balance = system_para_deposit(1, 0);
	pub const UniquesDepositPerByte: Balance = system_para_deposit(0, 1);
}

impl pallet_uniques::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = u32;
	type ItemId = u32;
	type Currency = Balances;
	type ForceOrigin = EnsureRoot<AccountId>;
	type CollectionDeposit = UniquesCollectionDeposit;
	type ItemDeposit = UniquesItemDeposit;
	type MetadataDepositBase = UniquesMetadataDepositBase;
	type AttributeDepositBase = UniquesAttributeDepositBase;
	type DepositPerByte = UniquesDepositPerByte;
	type StringLimit = ConstU32<128>;
	type KeyLimit = ConstU32<32>;
	type ValueLimit = ConstU32<64>;
	type WeightInfo = pallet_uniques::weights::SubstrateWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type Locker = ();
}

parameter_types! {
	pub NftsFeatures: PalletFeatures = PalletFeatures::all_enabled();
	pub const NftsMaxDeadlineDuration: BlockNumber = 12 * 30 * DAYS;
	pub const NftsCollectionDeposit: Balance = UniquesCollectionDeposit::get();
	pub const NftsItemDeposit: Balance = UniquesItemDeposit::get();
	pub const NftsMetadataDepositBase: Balance = UniquesMetadataDepositBase::get();
	pub const NftsAttributeDepositBase: Balance = UniquesAttributeDepositBase::get();
	pub const NftsDepositPerByte: Balance = UniquesDepositPerByte::get();
}

impl pallet_nfts::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = u32;
	type ItemId = u32;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type Locker = ();
	type CollectionDeposit = NftsCollectionDeposit;
	type ItemDeposit = NftsItemDeposit;
	type MetadataDepositBase = NftsMetadataDepositBase;
	type AttributeDepositBase = NftsAttributeDepositBase;
	type DepositPerByte = NftsDepositPerByte;
	type StringLimit = ConstU32<256>;
	type KeyLimit = ConstU32<64>;
	type ValueLimit = ConstU32<256>;
	type ApprovalsLimit = ConstU32<20>;
	type ItemAttributesApprovalsLimit = ConstU32<30>;
	type MaxTips = ConstU32<10>;
	type MaxDeadlineDuration = NftsMaxDeadlineDuration;
	type MaxAttributesPerCall = ConstU32<10>;
	type Features = NftsFeatures;
	type OffchainSignature = Signature;
	type OffchainPublic = <Signature as Verify>::Signer;
	type WeightInfo = pallet_nfts::weights::SubstrateWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type BlockNumberProvider = RelaychainDataProvider<Runtime>;
}

impl pallet_assets_precompiles::PermitConfig for Runtime {
	type ChainId = <Runtime as pallet_revive::Config>::ChainId;
	type WeightInfo = pallet_assets_precompiles::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const TransactionByteFee: Balance = fee::TRANSACTION_BYTE_FEE;
	pub const OperationalFeeMultiplier: u8 = 5;
}

/// Revive-compatible mapping from Orbis execution weight to native fees.
pub type WeightToFee = pallet_revive::evm::fees::BlockRatioFee<
	MILLI,
	{ 100 * ExtrinsicBaseWeight::get().ref_time() as u128 },
	Runtime,
	Balance,
>;

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = FungibleAdapter<Balances, ()>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type WeightInfo = weights::pallet_transaction_payment::WeightInfo<Runtime>;
}

impl pallet_asset_conversion_tx_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = Location;
	type OnChargeAssetTransaction = SwapAssetAdapter<
		xcm_config::OrgnRelayLocation,
		NativeAndAssets,
		AssetConversion,
		ResolveAssetTo<AssetConversionFeeAccount, NativeAndAssets>,
	>;
	type WeightInfo = pallet_asset_conversion_tx_payment::weights::SubstrateWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = AssetConversionTxHelper;
}

impl pallet_skip_feeless_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
	pub const ReviveDepositPerItem: Balance = system_para_deposit(1, 0);
	pub const ReviveDepositPerChildTrieItem: Balance = system_para_deposit(1, 0) / 100;
	pub const ReviveDepositPerByte: Balance = system_para_deposit(0, 1);
	pub ReviveCodeHashLockupDepositPercent: Perbill = Perbill::from_percent(30);
	pub const MaxEthExtrinsicWeight: FixedU128 = FixedU128::from_rational(9, 10);
}

impl pallet_revive::Config for Runtime {
	type Time = Timestamp;
	type Balance = Balance;
	type Currency = Balances;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type RuntimeOrigin = RuntimeOrigin;
	type DepositPerItem = ReviveDepositPerItem;
	type DepositPerChildTrieItem = ReviveDepositPerChildTrieItem;
	type DepositPerByte = ReviveDepositPerByte;
	type WeightInfo = pallet_revive::weights::SubstrateWeight<Self>;
	type Precompiles = (
		ERC20<Self, InlineIdConfig<0x120>, AssetsInstance>,
		ERC20<Self, ForeignIdConfig<0x220, Self, ForeignAssetsInstance>, ForeignAssetsInstance>,
		ERC20<Self, InlineIdConfig<0x320>, PoolAssetsInstance>,
	);
	type AddressMapper = pallet_revive::AccountId32Mapper<Self>;
	type RuntimeMemory = ConstU32<{ 128 * 1024 * 1024 }>;
	type PVFMemory = ConstU32<{ 512 * 1024 * 1024 }>;
	type AllowEVMBytecode = ConstBool<true>;
	type UploadOrigin = EnsureSigned<Self::AccountId>;
	type InstantiateOrigin = EnsureSigned<Self::AccountId>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type CodeHashLockupDepositPercent = ReviveCodeHashLockupDepositPercent;
	type ChainId = ConstU64<420_001_006>;
	type NativeToEthRatio = ConstU32<100_000_000>;
	type FindAuthor = <Runtime as pallet_authorship::Config>::FindAuthor;
	type FeeInfo = pallet_revive::evm::fees::Info<Address, Signature, EthExtraImpl>;
	type MaxEthExtrinsicWeight = MaxEthExtrinsicWeight;
	type DebugEnabled = ConstBool<false>;
	type AutoMap = ConstBool<true>;
	type GasScale = ConstU32<100_000>;
	type OnBurn = ();
	type Deposit = ();
}

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = system_para_deposit(1, 88);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = system_para_deposit(0, 32);
	pub const MaxSignatories: u32 = 100;
}

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = weights::pallet_multisig::WeightInfo<Runtime>;
	type BlockNumberProvider = System;
}

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = weights::pallet_utility::WeightInfo<Runtime>;
}

parameter_types! {
	// One storage item; key size 32, value size 8; .
	pub const ProxyDepositBase: Balance = system_para_deposit(1, 40);
	// Additional storage item size of 33 bytes.
	pub const ProxyDepositFactor: Balance = system_para_deposit(0, 33);
	pub const MaxProxies: u16 = 32;
	// One storage item; key size 32, value size 16
	pub const AnnouncementDepositBase: Balance = system_para_deposit(1, 48);
	pub const AnnouncementDepositFactor: Balance = system_para_deposit(0, 66);
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
	DecodeWithMemTracking,
	Debug,
	MaxEncodedLen,
	TypeInfo,
)]
pub enum ProxyType {
	Any,
	NonTransfer,
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
				RuntimeCall::System(..)
					| RuntimeCall::ParachainSystem(..)
					| RuntimeCall::Timestamp(..)
					| RuntimeCall::Indices(pallet_indices::Call::claim { .. })
					| RuntimeCall::Indices(pallet_indices::Call::free { .. })
					| RuntimeCall::Indices(pallet_indices::Call::freeze { .. })
					| RuntimeCall::Entity(..)
					| RuntimeCall::Feeless(..)
					| RuntimeCall::Register(..)
					| RuntimeCall::Session(..)
					| RuntimeCall::Utility(..)
					| RuntimeCall::Proxy(..)
					| RuntimeCall::Multisig(..)
					| RuntimeCall::MessageQueue(..)
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
	type BlockNumberProvider = RelaychainDataProvider<Runtime>;
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const RelayOrigin: AggregateMessageOrigin = AggregateMessageOrigin::Parent;
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnSystemEvent = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type DmpQueue = frame_support::traits::EnqueueWithOrigin<MessageQueue, RelayOrigin>;
	type ReservedDmpWeight = ReservedDmpWeight;
	type OutboundXcmpMessageSource = XcmpQueue;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type CheckAssociatedRelayNumber = RelayNumberMonotonicallyIncreases;
	type ConsensusHook = ConsensusHook;
	type WeightInfo = weights::cumulus_pallet_parachain_system::WeightInfo<Runtime>;
	type RelayParentOffset = ConstU32<RELAY_PARENT_OFFSET>;
	type SchedulingSignatureVerifier = ();
}

type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
	Runtime,
	RELAY_CHAIN_SLOT_DURATION_MILLIS,
	BLOCK_PROCESSING_VELOCITY,
	UNINCLUDED_SEGMENT_CAPACITY,
>;

impl parachain_info::Config for Runtime {}

parameter_types! {
	pub MessageQueueServiceWeight: Weight = Perbill::from_percent(20) * RuntimeBlockWeights::get().max_block;
}

impl pallet_message_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_message_queue::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type MessageProcessor =
		pallet_message_queue::mock_helpers::NoopMessageProcessor<AggregateMessageOrigin>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type MessageProcessor = xcm_builder::ProcessXcmMessage<
		AggregateMessageOrigin,
		xcm_executor::XcmExecutor<xcm_config::XcmConfig>,
		RuntimeCall,
	>;
	type Size = u32;
	// The XCMP queue pallet is only ever able to handle the `Sibling(ParaId)` origin:
	type QueueChangeHandler = NarrowOriginToSibling<XcmpQueue>;
	type QueuePausedQuery = NarrowOriginToSibling<XcmpQueue>;
	type HeapSize = sp_core::ConstU32<{ 64 * 1024 }>;
	type MaxStale = sp_core::ConstU32<8>;
	type ServiceWeight = MessageQueueServiceWeight;
	type IdleMaxServiceWeight = MessageQueueServiceWeight;
}

impl cumulus_pallet_aura_ext::Config for Runtime {}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = PolkadotXcm;
	type XcmpQueue = TransformOrigin<MessageQueue, AggregateMessageOrigin, ParaId, ParaIdToSibling>;
	type MaxActiveOutboundChannels = ConstU32<128>;
	// Most on-chain HRMP channels are configured to use 102400 bytes of max message size, so we
	// need to set the page size larger than that until we reduce the channel size on-chain.
	type MaxPageSize = ConstU32<{ 103 * 1024 }>;
	type MaxInboundSuspended = sp_core::ConstU32<1_000>;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type WeightInfo = weights::cumulus_pallet_xcmp_queue::WeightInfo<Runtime>;
	type PriceForSiblingDelivery = PriceForSiblingParachainDelivery;
}

impl cumulus_pallet_xcmp_queue::migration::v5::V5Config for Runtime {
	// This must be the same as the `ChannelInfo` from the `Config`:
	type ChannelList = ParachainSystem;
}

parameter_types! {
	/// Production remains six hours. The existing `fast-runtime` artifact is an explicit,
	/// hash-bound test profile and rotates quickly enough to prove collator enactment.
	pub const Period: u32 = if cfg!(feature = "fast-runtime") { 2 * MINUTES } else { 6 * HOURS };
	pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
	type Currency = Balances;
	type KeyDeposit = ();
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = CollatorSelection;
	// Essentially just Aura, but let's be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type WeightInfo = weights::pallet_session::WeightInfo<Runtime>;
	type DisablingStrategy = ();
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<100_000>;
	type AllowMultipleBlocksPerSlot = ConstBool<true>;
	type SlotDuration = ConstU64<SLOT_DURATION>;
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
}

impl pallet_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = EnsureRoot<AccountId>;
	type PotId = PotId;
	type MaxCandidates = ConstU32<0>;
	type MinEligibleCollators = ConstU32<1>;
	type MaxInvulnerables = ConstU32<20>;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = weights::pallet_collator_selection::WeightInfo<Runtime>;
}

impl pallet_verify_signature::Config for Runtime {
	type Signature = MultiSignature;
	type AccountIdentifier = MultiSigner;
	type WeightInfo = weights::pallet_verify_signature::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

/// Extensions which may change the logical Orbis actor or authorize a privileged call today.
///
/// Keep this alias nested in every full transaction surface. New policy extensions are added to
/// the frozen slot map in ADR 0008 before they are added here.
pub type OriginPolicyExtensions = (
	indiv_pallet_people::extension::AsPerson<Runtime>,
	pallet_orbis_score::ScoreAsParticipant<Runtime>,
	indiv_pallet_people_lite::extension::PeopleLiteAuth<Runtime>,
	indiv_pallet_resources::extension::AsResources<Runtime>,
	pallet_orbis_honour::extension::VoterAuth<Runtime>,
	frame_system::AuthorizeCall<Runtime>,
);

/// Payment policy that skips only after `AuthorizeCall` authenticates an authorized call.
///
/// No skip request is encoded. Consequently a wire transaction cannot request an exemption: the
/// preceding policy tuple must first produce the exact `System::Authorized` origin.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq)]
pub struct ExplicitPayment<T, S> {
	inner: S,
	#[codec(skip)]
	_marker: core::marker::PhantomData<T>,
}

impl<T, S: TypeInfo + 'static> TypeInfo for ExplicitPayment<T, S> {
	type Identity = S;

	fn type_info() -> scale_info::Type {
		S::type_info()
	}
}

impl<T, S: core::fmt::Debug> core::fmt::Debug for ExplicitPayment<T, S> {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		f.debug_tuple("ExplicitPayment").field(&self.inner).finish()
	}
}

impl<T, S> From<S> for ExplicitPayment<T, S> {
	fn from(inner: S) -> Self {
		Self { inner, _marker: Default::default() }
	}
}

/// Codec-transparent adapter preserving the signed nonce/payment owner after an identity policy
/// transforms the dispatch origin into a Resources, Score, or Honour custom origin.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq)]
pub struct AccountAwareResources<S>(pub(crate) S);

impl<S: TypeInfo + 'static> TypeInfo for AccountAwareResources<S> {
	type Identity = S;
	fn type_info() -> scale_info::Type {
		S::type_info()
	}
}

impl<S: core::fmt::Debug> core::fmt::Debug for AccountAwareResources<S> {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		self.0.fmt(f)
	}
}

impl<S> From<S> for AccountAwareResources<S> {
	fn from(value: S) -> Self {
		Self(value)
	}
}

pub struct AccountAwareVal<V> {
	inner: V,
	account: Option<AccountId>,
}

pub struct AccountAwarePre<P> {
	inner: P,
}

impl<S> sp_runtime::traits::TransactionExtension<RuntimeCall> for AccountAwareResources<S>
where
	S: sp_runtime::traits::TransactionExtension<RuntimeCall>,
{
	const IDENTIFIER: &'static str = S::IDENTIFIER;
	type Implicit = S::Implicit;
	type Val = AccountAwareVal<S::Val>;
	type Pre = AccountAwarePre<S::Pre>;

	fn metadata() -> Vec<sp_runtime::traits::TransactionExtensionMetadata> {
		S::metadata()
	}

	fn implicit(
		&self,
	) -> Result<Self::Implicit, sp_runtime::transaction_validity::TransactionValidityError> {
		self.0.implicit()
	}

	fn weight(&self, call: &RuntimeCall) -> Weight {
		self.0.weight(call)
	}

	fn validate(
		&self,
		origin: RuntimeOrigin,
		call: &RuntimeCall,
		info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
		len: usize,
		implicit: Self::Implicit,
		inherited: &impl sp_runtime::traits::Implication,
		source: TransactionSource,
	) -> sp_runtime::traits::ValidateResult<Self::Val, RuntimeCall> {
		let account = match frame_support::traits::OriginTrait::caller(&origin) {
			OriginCaller::Resources(
				indiv_pallet_resources::Origin::<Runtime>::LongTermStorageClaim { payer, .. },
			) => match call {
				RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
					account_id,
					..
				}) if account_id == payer => Some(payer.clone()),
				_ => {
					return Err(
						sp_runtime::transaction_validity::InvalidTransaction::BadSigner.into()
					)
				},
			},
			OriginCaller::Score(pallet_orbis_score::Origin::<Runtime>::AccountParticipant(
				account,
			)) => {
				if matches!(call, RuntimeCall::Score(_)) {
					Some(account.clone())
				} else {
					return Err(
						sp_runtime::transaction_validity::InvalidTransaction::BadSigner.into()
					);
				}
			},
			OriginCaller::Honour(pallet_orbis_honour::Origin::<Runtime>::Voter {
				account, ..
			}) => {
				if matches!(call, RuntimeCall::Honour(_)) {
					Some(account.clone())
				} else {
					return Err(
						sp_runtime::transaction_validity::InvalidTransaction::BadSigner.into()
					);
				}
			},
			_ => None,
		};
		let delegated_origin = account
			.clone()
			.map(|account| frame_system::RawOrigin::Signed(account).into())
			.unwrap_or_else(|| origin.clone());
		self.0
			.validate(delegated_origin, call, info, len, implicit, inherited, source)
			.map(|(validity, inner, _)| (validity, AccountAwareVal { inner, account }, origin))
	}

	fn prepare(
		self,
		value: Self::Val,
		origin: &RuntimeOrigin,
		call: &RuntimeCall,
		info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
		len: usize,
	) -> Result<Self::Pre, sp_runtime::transaction_validity::TransactionValidityError> {
		let delegated_origin = value
			.account
			.map(|account| frame_system::RawOrigin::Signed(account).into())
			.unwrap_or_else(|| origin.clone());
		self.0
			.prepare(value.inner, &delegated_origin, call, info, len)
			.map(|inner| AccountAwarePre { inner })
	}

	fn post_dispatch_details(
		pre: Self::Pre,
		info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
		post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
		len: usize,
		result: &frame_support::dispatch::DispatchResult,
	) -> Result<Weight, sp_runtime::transaction_validity::TransactionValidityError> {
		S::post_dispatch_details(pre.inner, info, post_info, len, result)
	}
}

/// Value passed between explicit-payment validation and preparation.
pub enum ExplicitPaymentIntermediate<A> {
	Apply(A),
	Skip(Weight),
}

impl<T, S> sp_runtime::traits::TransactionExtension<T::RuntimeCall> for ExplicitPayment<T, S>
where
	T: frame_system::Config + Send + Sync,
	T::RuntimeCall: sp_runtime::traits::Dispatchable,
	T::RuntimeOrigin: sp_runtime::traits::AsTransactionAuthorizedOrigin,
	S: sp_runtime::traits::TransactionExtension<T::RuntimeCall>,
{
	const IDENTIFIER: &'static str = S::IDENTIFIER;
	type Implicit = S::Implicit;
	type Val = ExplicitPaymentIntermediate<S::Val>;
	type Pre = ExplicitPaymentIntermediate<S::Pre>;

	fn metadata() -> Vec<sp_runtime::traits::TransactionExtensionMetadata> {
		S::metadata()
	}

	fn implicit(
		&self,
	) -> Result<Self::Implicit, sp_runtime::transaction_validity::TransactionValidityError> {
		self.inner.implicit()
	}

	fn weight(&self, call: &T::RuntimeCall) -> Weight {
		self.inner.weight(call)
	}

	fn validate(
		&self,
		origin: T::RuntimeOrigin,
		call: &T::RuntimeCall,
		info: &sp_runtime::traits::DispatchInfoOf<T::RuntimeCall>,
		len: usize,
		implicit: Self::Implicit,
		inherited_implication: &impl sp_runtime::traits::Implication,
		source: TransactionSource,
	) -> sp_runtime::traits::ValidateResult<Self::Val, T::RuntimeCall> {
		use frame_support::traits::CallerTrait;

		let is_authorized = matches!(
			frame_support::traits::OriginTrait::caller(&origin).as_system_ref(),
			Some(frame_system::RawOrigin::Authorized)
		);
		if is_authorized {
			Ok((
				Default::default(),
				ExplicitPaymentIntermediate::Skip(self.inner.weight(call)),
				origin,
			))
		} else {
			self.inner
				.validate(origin, call, info, len, implicit, inherited_implication, source)
				.map(|(validity, val, origin)| {
					(validity, ExplicitPaymentIntermediate::Apply(val), origin)
				})
		}
	}

	fn prepare(
		self,
		val: Self::Val,
		origin: &T::RuntimeOrigin,
		call: &T::RuntimeCall,
		info: &sp_runtime::traits::DispatchInfoOf<T::RuntimeCall>,
		len: usize,
	) -> Result<Self::Pre, sp_runtime::transaction_validity::TransactionValidityError> {
		match val {
			ExplicitPaymentIntermediate::Apply(val) => self
				.inner
				.prepare(val, origin, call, info, len)
				.map(ExplicitPaymentIntermediate::Apply),
			ExplicitPaymentIntermediate::Skip(weight) => {
				Ok(ExplicitPaymentIntermediate::Skip(weight))
			},
		}
	}

	fn post_dispatch_details(
		pre: Self::Pre,
		info: &sp_runtime::traits::DispatchInfoOf<T::RuntimeCall>,
		post_info: &sp_runtime::traits::PostDispatchInfoOf<T::RuntimeCall>,
		len: usize,
		result: &frame_support::dispatch::DispatchResult,
	) -> Result<Weight, sp_runtime::transaction_validity::TransactionValidityError> {
		match pre {
			ExplicitPaymentIntermediate::Apply(pre) => {
				S::post_dispatch_details(pre, info, post_info, len, result)
			},
			ExplicitPaymentIntermediate::Skip(weight) => Ok(weight),
		}
	}
}

fn default_origin_policy_extensions() -> OriginPolicyExtensions {
	(
		indiv_pallet_people::extension::AsPerson::<Runtime>::new(None),
		pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(None),
		indiv_pallet_people_lite::extension::PeopleLiteAuth::<Runtime>::new(None),
		indiv_pallet_resources::extension::AsResources::<Runtime>::new(None),
		pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(None),
		frame_system::AuthorizeCall::<Runtime>::new(),
	)
}

pub type MetaIdentityBoundPolicies = (
	pallet_orbis_score::ScoreAsParticipant<Runtime>,
	meta_v6::MetaAccountBoundPoliciesV6,
	pallet_orbis_honour::extension::VoterAuth<Runtime>,
);

pub type MetaTxExtension = (
	pallet_verify_signature::VerifySignature<Runtime>,
	meta_v6::ConsumePaidMetaIngress,
	pallet_meta_tx::MetaTxMarker<Runtime>,
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckMortality<Runtime>,
	frame_system::CheckNonce<Runtime>,
	MetaIdentityBoundPolicies,
	pallet_orbis_transaction_storage::extension::ValidateStorageCalls<
		Runtime,
		OrbisStorageCallInspector,
	>,
	frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
);

pub struct OrbisMetaTxWeightInfo;

impl pallet_meta_tx::WeightInfo for OrbisMetaTxWeightInfo {
	fn bare_dispatch(n: u32) -> Weight {
		<weights::pallet_meta_tx::WeightInfo<Runtime> as pallet_meta_tx::WeightInfo>::bare_dispatch(
			n,
		)
		.saturating_add(RocksDbWeight::get().reads(1))
	}
}

impl pallet_meta_tx::Config for Runtime {
	type WeightInfo = OrbisMetaTxWeightInfo;
	type RuntimeEvent = RuntimeEvent;
	type Extension = MetaTxExtension;
}

parameter_types! {
	pub const TokenMaxAuthorizationLen: u32 = 256;
	pub const TokenMaxTimelineViewResults: u32 = 64;
	pub const TokenDefaultTimelineViewResults: u32 = 32;
	pub const TokenAuthorizationTTL: u32 = 30;
}

impl pallet_origin_token::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberProvider = System;
	type MaxAuthorizationLen = TokenMaxAuthorizationLen;
	type MaxTimelineViewResults = TokenMaxTimelineViewResults;
	type DefaultTimelineViewResults = TokenDefaultTimelineViewResults;
	type MaxAuthorizationTTL = TokenAuthorizationTTL;
}

parameter_types! {
	pub const MaxRegistryRawDataLength: u32 = 4096;
	pub const MaxRegistryAdditionalAttributes: u32 = 64;
	pub const MaxRegistryAuthorizationLen: u32 = 256;
	pub const RegisterAuthorizationTTL: u32 = 30;
	pub const MaxPacketListResults: u32 = 200;
}

impl pallet_origin_register::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Token = Token;
	type EntityLookup = Entity;
	type MaxRawDataLength = MaxRegistryRawDataLength;
	type MaxAdditionalAttributes = MaxRegistryAdditionalAttributes;
	type MaxAuthorizationLen = MaxRegistryAuthorizationLen;
	type MaxAuthorizationTTL = RegisterAuthorizationTTL;
	type MaxPacketListResults = MaxPacketListResults;
	type Feeless = Feeless;
	type WeightInfo = pallet_origin_register::weights::SubstrateWeight<Self>;
}

impl pallet_origin_feeless::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_origin_feeless::weights::SubstrateWeight<Runtime>;
	type MaxFeelessTransactionsPerBlock = ConstU32<16>;
}

parameter_types! {
	pub const PeopleMaxAdditionalFields: u32 = 32;
	pub const PeopleMaxRegistrars: u32 = 20;
}

/// SDK-compatible People/People-Lite slice: self-claimed identity plus Sudo-managed attestations
/// and aliases, adapted onto CORD's maintained identity pallet to keep one FRAME dependency graph.
impl pallet_orbis_people::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxSubAccounts = ConstU32<32>;
	type IdentityInformation =
		pallet_orbis_people::identity_info::IdentityInfo<PeopleMaxAdditionalFields>;
	type MaxRegistrars = PeopleMaxRegistrars;
	type RegistrarOrigin = EnsureRoot<AccountId>;
	type OffchainSignature = MultiSignature;
	type SigningPublicKey = MultiSigner;
	type UsernameAuthorityOrigin = EnsureRoot<AccountId>;
	type PendingUsernameExpiration = ConstU32<{ 7 * DAYS }>;
	type MaxSuffixLength = ConstU32<16>;
	type MaxUsernameLength = ConstU32<64>;
	type WeightInfo = pallet_orbis_people::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const PeopleChunkPageSize: u32 = 256;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct ChunksManagerBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl
	indiv_pallet_chunks_manager::BenchmarkHelper<
		<BandersnatchVrfVerifiable as GenerateVerifiable>::StaticChunk,
	> for ChunksManagerBenchmarkHelper
{
	fn chunk_page() -> Vec<<BandersnatchVrfVerifiable as GenerateVerifiable>::StaticChunk> {
		use indiv_support::genesis::ring_verifier_builder_params;
		use verifiable::ring::RingDomainSize;

		ring_verifier_builder_params(RingDomainSize::Domain11)
			.into_iter()
			.take(PeopleChunkPageSize::get() as usize)
			.collect()
	}
}

impl indiv_pallet_chunks_manager::Config for Runtime {
	type WeightInfo = indiv_pallet_chunks_manager::weights::SubstrateWeight<Runtime>;
	type Chunk = <BandersnatchVrfVerifiable as GenerateVerifiable>::StaticChunk;
	type PageSize = PeopleChunkPageSize;
	type ManagerOrigin = EnsureRoot<AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ChunksManagerBenchmarkHelper;
}

parameter_types! {
	pub const MembersFlexibleRingExponent: indiv_support::traits::RingExponent =
		indiv_support::traits::RingExponent::R2e10;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct MembersBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl
	indiv_pallet_members::BenchmarkHelper<
		<BandersnatchVrfVerifiable as GenerateVerifiable>::StaticChunk,
	> for MembersBenchmarkHelper
{
	fn initialize_chunks(
		ring_size: indiv_support::traits::RingExponent,
	) -> Vec<<BandersnatchVrfVerifiable as GenerateVerifiable>::StaticChunk> {
		use indiv_support::genesis::ring_verifier_builder_params;
		let domain = ring_size.try_into().expect("supported ring exponent has a domain");
		ring_verifier_builder_params(domain)
	}

	fn set_time(now: core::time::Duration) {
		pallet_timestamp::Now::<Runtime>::put(now.as_millis() as u64);
	}

	fn set_valid_time() {
		pallet_timestamp::Now::<Runtime>::put(5_000);
	}
}

impl indiv_pallet_members::Config for Runtime {
	type WeightInfo = indiv_pallet_members::weights::SubstrateWeight<Runtime>;
	type Clock = Timestamp;
	type Crypto = BandersnatchVrfVerifiable;
	type Location = Location;
	type ChunksManager = ChunksManager;
	type MaxCollections = ConstU32<100>;
	type OnboardingQueuePageSize = ConstU32<255>;
	type MaxFlexibleRingExponent = MembersFlexibleRingExponent;
	type RingBuildingMemberLimit = ConstU32<100>;
	type OldRootRetentionDuration = ConstU64<600>;
	type OnRingRootChange = MembersNotifier;
	type OffchainWorkerInterval = ConstU32<1>;
	type ManagerOrigin = EnsureRoot<AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = MembersBenchmarkHelper;
}

/// Accept replay requests only from the sibling parachain whose subscription is being serviced.
pub struct EnsureSiblingParachain;
impl frame_support::traits::EnsureOrigin<RuntimeOrigin> for EnsureSiblingParachain {
	type Success = ParaId;

	fn try_origin(origin: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		match origin.clone().into() {
			Ok(cumulus_pallet_xcm::Origin::SiblingParachain(id)) => Ok(id),
			_ => Err(origin),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		Ok(cumulus_pallet_xcm::Origin::SiblingParachain(2_000u32.into()).into())
	}
}

parameter_types! {
	pub MembersNotifierRemoteWeight: Weight = Weight::from_parts(10_000, 0);
}

#[cfg(feature = "runtime-benchmarks")]
pub struct MembersNotifierBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl indiv_pallet_members_notifier::benchmarking::BenchmarkHelper<Runtime>
	for MembersNotifierBenchmarkHelper
{
	fn init() {
		use cumulus_pallet_parachain_system::RelevantMessagingState;
		use cumulus_primitives_core::relay_chain::AbridgedHrmpChannel;

		pallet_timestamp::Now::<Runtime>::put(120_000u64);
		let max = <<Runtime as indiv_pallet_members_notifier::Config>::MaxSubscribers as frame_support::traits::Get<u32>>::get();
		let channel = AbridgedHrmpChannel {
			max_capacity: 1_000,
			max_total_size: 1_000_000,
			max_message_size: 100_000,
			msg_count: 0,
			total_size: 0,
			mqc_head: None,
		};
		let mut egress_channels: Vec<_> = (0..max)
			.chain(1_000..1_000 + max)
			.map(|id| (ParaId::from(id), channel.clone()))
			.collect();
		egress_channels.sort_by_key(|(id, _)| *id);
		egress_channels.dedup_by_key(|(id, _)| *id);
		RelevantMessagingState::<Runtime>::put(
			cumulus_pallet_parachain_system::relay_state_snapshot::MessagingStateSnapshot {
				dmq_mqc_head: Default::default(),
				relay_dispatch_queue_remaining_capacity: Default::default(),
				ingress_channels: Vec::new(),
				egress_channels,
			},
		);
	}

	fn setup_ring_roots(count: u32) {
		use indiv_support::traits::Identifier;
		use verifiable::ring::RingDomainSize;

		let intermediate = BandersnatchVrfVerifiable::start_members(RingDomainSize::Domain11);
		let root = BandersnatchVrfVerifiable::finish_members(intermediate.clone());
		let max = <<Runtime as indiv_pallet_members_notifier::Config>::MaxCollections as frame_support::traits::Get<u32>>::get();
		for collection in 0..max {
			let mut identifier: Identifier = [0; 32];
			identifier[..4].copy_from_slice(&collection.to_be_bytes());
			for index in 0..count {
				indiv_pallet_members::Root::<Runtime>::insert(
					identifier,
					index,
					indiv_pallet_members::RingRoot::<Runtime> {
						root: root.clone(),
						revision: 0,
						intermediate: intermediate.clone(),
					},
				);
			}
			indiv_pallet_members::CurrentRingIndex::<Runtime>::insert(
				identifier,
				count.saturating_sub(1),
			);
		}
	}

	fn set_max_message_size(size: u32) {
		use cumulus_pallet_parachain_system::RelevantMessagingState;
		let mut state =
			RelevantMessagingState::<Runtime>::get().expect("notifier benchmark init ran");
		for (_, channel) in state.egress_channels.iter_mut() {
			channel.max_message_size = size;
		}
		RelevantMessagingState::<Runtime>::put(state);
	}
}

impl indiv_pallet_members_notifier::Config for Runtime {
	type WeightInfo = indiv_pallet_members_notifier::weights::SubstrateWeight<Runtime>;
	type XcmRouter = xcm_config::XcmRouter;
	type ManageOrigin = EnsureRoot<AccountId>;
	type Crypto = BandersnatchVrfVerifiable;
	type Clock = Timestamp;
	type MaxSubscribers = ConstU32<10>;
	type MaxUpdatesPerBatch = ConstU32<10>;
	type MaxCollectionsPerSubscriber = ConstU32<3>;
	type MaxCollections = ConstU32<100>;
	type RingRootsProvider = Members;
	type EnsureSubscriberOrigin = EnsureSiblingParachain;
	type ChannelInfo = ParachainSystem;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = MembersNotifierBenchmarkHelper;
	type UpdateTriggerBlocks = ConstU32<1>;
	type UpdateTriggerThreshold = ConstU32<1>;
	type RequestReplayRemoteWeight = MembersNotifierRemoteWeight;
	type OffchainWorkerInterval = ConstU32<1>;
	type StuckBatchTimeout = ConstU32<100>;
	type ReplayCooldownSeconds = ConstU64<60>;
}

parameter_types! {
	pub LitePeopleCollectionOwner: Location = Location::new(0, [PalletInstance(94)]);
	pub const LitePeopleRingExponent: indiv_support::traits::RingExponent =
		indiv_support::traits::RingExponent::R2e9;
	pub const LitePeopleOnboardingSize: u32 = 3;
}

impl indiv_pallet_people_lite::Config for Runtime {
	type WeightInfo = indiv_pallet_people_lite::weights::SubstrateWeight<Runtime>;
	type AttestationAllowanceManager = EnsureRoot<AccountId>;
	type MemberService = Members;
	type CollectionOwner = LitePeopleCollectionOwner;
	type LiteRingExponent = LitePeopleRingExponent;
	type LiteOnboardingSize = LitePeopleOnboardingSize;
	type AttestationSignature = MultiSignature;
	type LiteConsumerRegistrar = Resources;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

pub const ORBIS_PERSON_CONTEXT: indiv_support::traits::Context = [0x4f; 32];

pub struct PersonhoodAccountContexts;
impl Contains<indiv_support::traits::Context> for PersonhoodAccountContexts {
	fn contains(context: &indiv_support::traits::Context) -> bool {
		context == &ORBIS_PERSON_CONTEXT || context == &pallet_orbis_score::SCORE_CONTEXT
	}
}

parameter_types! {
	pub PersonhoodCollectionOwner: Location = Location::new(0, [PalletInstance(95)]);
	pub const PersonhoodStaleAliasCleanupInterval: BlockNumber = 10;
	pub const PersonhoodSelfInclusionDelay: u64 = 3_600;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct PersonhoodBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl
	indiv_pallet_people::BenchmarkHelper<
		<BandersnatchVrfVerifiable as GenerateVerifiable>::StaticChunk,
	> for PersonhoodBenchmarkHelper
{
	fn valid_account_context() -> indiv_support::traits::Context {
		ORBIS_PERSON_CONTEXT
	}

	fn initialize_chunks() -> Vec<<BandersnatchVrfVerifiable as GenerateVerifiable>::StaticChunk> {
		use indiv_support::genesis::ring_verifier_builder_params;
		use verifiable::ring::RingDomainSize;
		ring_verifier_builder_params(RingDomainSize::Domain11)
	}
}

impl indiv_pallet_people::Config for Runtime {
	type WeightInfo = indiv_pallet_people::weights::SubstrateWeight<Runtime>;
	type MemberService = Members;
	type RingExponent = MembersFlexibleRingExponent;
	type CollectionOwner = PersonhoodCollectionOwner;
	type AccountContexts = PersonhoodAccountContexts;
	type OnboardingQueuePageSize = ConstU32<30>;
	type StaleAliasCleanupInterval = PersonhoodStaleAliasCleanupInterval;
	type SelfInclusionDelay = PersonhoodSelfInclusionDelay;
	type ManagerOrigin = EnsureRoot<AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = PersonhoodBenchmarkHelper;
}

parameter_types! {
	pub ScorePayoutAccountDefault: AccountId = AccountId::new([0x50; 32]);
	pub ScoreCurrencyLocation: Location = Location::here();
	pub ScoreManagerAccountDefault: Option<AccountId> = Some(AccountId::new([0x53; 32]));
	pub const HonourPointFreezeDuration: pallet_orbis_honour::Seconds = 24 * 60 * 60;
	pub const HonourCallMortality: pallet_orbis_honour::Seconds = 5 * 60;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct ScoreBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_orbis_score::benchmarking::BenchmarkHelper<Runtime> for ScoreBenchmarkHelper {
	fn create_member(seed: u64) -> pallet_orbis_score::MemberOf<Runtime> {
		let mut entropy = [0u8; 32];
		entropy[..8].copy_from_slice(&seed.to_le_bytes());
		let secret = BandersnatchVrfVerifiable::new_secret(entropy);
		BandersnatchVrfVerifiable::member_from_secret(&secret)
	}

	fn setup_currency() {}
}

#[cfg(feature = "runtime-benchmarks")]
pub struct HonourBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_orbis_honour::benchmarking::BenchmarkHelper<Runtime> for HonourBenchmarkHelper {
	fn set_time(now: pallet_orbis_honour::Seconds) {
		pallet_timestamp::Now::<Runtime>::put(now.saturating_mul(1_000));
	}

	fn seed_and_create_proof(
		vote: &pallet_orbis_honour::VoteData,
		message: &[u8],
	) -> pallet_orbis_honour::RingProofOf<Runtime> {
		use alloc::{vec, vec::Vec};
		use indiv_support::traits::{AppendOnlyMembers, RingMode};
		use verifiable::ring::RingDomainSize;

		let ring_exponent = <Runtime as indiv_pallet_people::Config>::RingExponent::get();
		let ring_index = 0;
		Members::create_collection(
			PersonhoodCollectionOwner::get(),
			indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			ring_exponent,
			None,
		)
		.expect("benchmark: people collection must be created");

		let secret =
			BandersnatchVrfVerifiable::new_secret(sp_io::hashing::twox_256(b"honour-bench-voter"));
		let member = BandersnatchVrfVerifiable::member_from_secret(&secret);
		Members::add_members(indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER, vec![member])
			.expect("benchmark: ring member must be added");
		Members::initialize_chunks(ring_exponent);
		Members::onboard_all_and_build_ring(
			indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
			ring_index,
		)
		.expect("benchmark: people ring must be built");

		let ring_members =
			Members::ring_members(indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER, ring_index);
		let domain: RingDomainSize =
			ring_exponent.try_into().expect("people ring exponent maps to a domain size");
		let commitment = BandersnatchVrfVerifiable::open(domain, &member, ring_members.into_iter())
			.expect("benchmark: commitment must open");
		let contexts = vote.get_contexts();
		let contexts: Vec<&[u8]> = contexts.iter().map(|context| &context[..]).collect();
		let (proof, _) = BandersnatchVrfVerifiable::create_multi_context(
			commitment, &secret, &contexts, message,
		)
		.expect("benchmark: proof creation must succeed");
		proof
	}
}

impl pallet_orbis_score::Config for Runtime {
	type WeightInfo = pallet_orbis_score::weights::SubstrateWeight<Runtime>;
	type EnsurePerson = indiv_pallet_people::EnsurePersonalAliasInContext<Runtime>;
	type PayoutAccountDefault = ScorePayoutAccountDefault;
	type Currency = Balances;
	type CurrencyLocationInfo = ScoreCurrencyLocation;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type ManagerAccountDefault = ScoreManagerAccountDefault;
	type MaxPayoutRoundSchedules = ConstU32<10>;
	type OffchainWorkInterval = ConstU32<2>;
	type People = Personhood;
	type Crypto = BandersnatchVrfVerifiable;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ScoreBenchmarkHelper;
}

impl pallet_orbis_honour::Config for Runtime {
	type WeightInfo = pallet_orbis_honour::weights::SubstrateWeight<Runtime>;
	type MemberService = Members;
	type Clock = Timestamp;
	type PointFreezeDuration = HonourPointFreezeDuration;
	type CallMortality = HonourCallMortality;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = HonourBenchmarkHelper;
}

parameter_types! {
	pub const ResourcesPersonAuthDuration: u32 = 2 * 24 * 60 * 60;
	pub const ResourcesMinPersonAuthUpdateInterval: u32 = 24 * 60 * 60;
	pub ResourcesAccountsApiAllowance: sp_statement_store::StatementAllowance =
		sp_statement_store::StatementAllowance { max_size: 500 * 1024, max_count: 2 };
	pub const ResourcesStmtStoreSlotsPerPeriod: u32 = 20;
	pub const ResourcesLiteStmtStoreSlotsPerPeriod: u32 = 10;
	pub const ResourcesStmtStoreCleanupLimit: u32 = 50;
	pub const ResourcesStmtStoreReplacementCooldown: u32 = 60;
	pub const ResourcesStmtStoreGraceWindow: u32 = 2 * 24 * 60 * 60;
	pub ResourcesFriendRequestAllowance: sp_statement_store::StatementAllowance =
		sp_statement_store::StatementAllowance { max_size: 10 * 1024, max_count: 1 };
	pub const ResourcesFriendRequestSlotsPerPeriod: u8 = 16;
	pub const ResourcesLiteFriendRequestSlotsPerPeriod: u8 = 8;
	pub const ResourcesFriendRequestPeriodDuration: u32 = 24 * 60 * 60;
	pub const ResourcesFriendRequestGraceWindow: u32 = 60 * 60;
	pub const ResourcesFriendRequestRetentionDuration: u64 = 7 * 24 * 60 * 60;
	pub ResourcesLitePersonStatementLimit: sp_statement_store::StatementAllowance =
		sp_statement_store::StatementAllowance { max_size: 500 * 1024, max_count: 50 };
	pub ResourcesPersonStatementLimit: sp_statement_store::StatementAllowance =
		sp_statement_store::StatementAllowance { max_size: 1024 * 1024, max_count: 200 };
	pub const ResourcesLongTermStoragePeriodDuration: u32 = 14 * 24 * 60 * 60;
	pub const ResourcesLongTermStorageClaimsPerPeriod: u8 = 100;
	pub const ResourcesLongTermStorageGraceWindow: u32 = 60 * 60;
	pub ResourcesLongTermStorageAllowanceForPeople:
		indiv_pallet_resources::types::LongTermStorageAllocation =
		indiv_pallet_resources::types::LongTermStorageAllocation {
			transactions: 100,
			bytes: 8 * 1024 * 1024,
		};
	pub ResourcesLongTermStorageAllowanceForLitePeople:
		indiv_pallet_resources::types::LongTermStorageAllocation =
		indiv_pallet_resources::types::LongTermStorageAllocation {
			transactions: 10,
			bytes: 4 * 1024 * 1024,
		};
	pub const ResourcesLongTermStorageCleanupLimit: u32 = 50;
	pub const ResourcesMaxReservations: u32 = 256;
	pub const ResourcesStorageReservationDuration: BlockNumber = 14 * DAYS;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct ResourcesBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl indiv_pallet_resources::benchmarking::BenchmarkHelper<Runtime> for ResourcesBenchmarkHelper {
	fn set_time(now: core::time::Duration) {
		pallet_timestamp::Now::<Runtime>::put(now.as_millis() as u64);
	}

	fn sign_message(message: &[u8]) -> (AccountId, MultiSignature) {
		use sp_runtime::traits::IdentifyAccount;
		const KEY_TYPE: sp_core::crypto::KeyTypeId = sp_core::crypto::KeyTypeId(*b"rsrc");
		let public = sp_io::crypto::ed25519_generate(KEY_TYPE, None);
		let signature = sp_io::crypto::ed25519_sign(KEY_TYPE, &public, message)
			.expect("benchmark key was inserted immediately before signing");
		(public.into_account().into(), signature.into())
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl indiv_pallet_resources::benchmarking::MetaPolicyBenchmarkHelper for ResourcesBenchmarkHelper {
	fn run(
		scenario: indiv_pallet_resources::benchmarking::MetaPolicyBenchmarkScenario,
	) -> Result<(), frame_benchmarking::BenchmarkError> {
		crate::meta_v6::benchmark_policy_scenario(scenario)
	}
}

impl indiv_pallet_resources::Config for Runtime {
	type WeightInfo = indiv_pallet_resources::weights::SubstrateWeight<Runtime>;
	type MemberService = Members;
	type PersonAuthDuration = ResourcesPersonAuthDuration;
	type MinPersonAuthUpdateInterval = ResourcesMinPersonAuthUpdateInterval;
	type AccountsApiAllowance = ResourcesAccountsApiAllowance;
	type StmtStoreSlotsPerPeriod = ResourcesStmtStoreSlotsPerPeriod;
	type LiteStmtStoreSlotsPerPeriod = ResourcesLiteStmtStoreSlotsPerPeriod;
	type StmtStoreCleanupLimit = ResourcesStmtStoreCleanupLimit;
	type StmtStoreReplacementCooldown = ResourcesStmtStoreReplacementCooldown;
	type StmtStoreGraceWindow = ResourcesStmtStoreGraceWindow;
	type FriendRequestAllowance = ResourcesFriendRequestAllowance;
	type FriendRequestSlotsPerPeriod = ResourcesFriendRequestSlotsPerPeriod;
	type LiteFriendRequestSlotsPerPeriod = ResourcesLiteFriendRequestSlotsPerPeriod;
	type FriendRequestPeriodDuration = ResourcesFriendRequestPeriodDuration;
	type FriendRequestGraceWindow = ResourcesFriendRequestGraceWindow;
	type FriendRequestRetentionDuration = ResourcesFriendRequestRetentionDuration;
	type OffchainWorkerInterval = ConstU32<1>;
	type EnsurePerson = indiv_pallet_people::EnsurePersonalAliasInContext<Runtime>;
	type EnsureLitePerson = indiv_pallet_people_lite::EnsureLitePerson<Runtime>;
	type Clock = Timestamp;
	type OffchainSignature = MultiSignature;
	type LitePersonStatementLimit = ResourcesLitePersonStatementLimit;
	type PersonStatementLimit = ResourcesPersonStatementLimit;
	type LongTermStoragePeriodDuration = ResourcesLongTermStoragePeriodDuration;
	type LongTermStorageGraceWindow = ResourcesLongTermStorageGraceWindow;
	type LongTermStorageClaimsPerPeriod = ResourcesLongTermStorageClaimsPerPeriod;
	type LongTermStorageAllowanceForPeople = ResourcesLongTermStorageAllowanceForPeople;
	type LongTermStorageAllowanceForLitePeople = ResourcesLongTermStorageAllowanceForLitePeople;
	type LongTermStorageDataStore = TransactionStorage;
	type LongTermStorageCleanupLimit = ResourcesLongTermStorageCleanupLimit;
	type MaxReservations = ResourcesMaxReservations;
	type StorageReservationDuration = ResourcesStorageReservationDuration;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ResourcesBenchmarkHelper;
	#[cfg(feature = "runtime-benchmarks")]
	type MetaPolicyBenchmarkHelper = ResourcesBenchmarkHelper;
}

parameter_types! {
	pub const StorageMaxBlockTransactions: u32 = 128;
	pub const StorageMaxTransactionSize: u32 = 256 * 1024;
	pub const StorageMaxPermanentStorageSize: u64 = 16 * 1024 * 1024 * 1024;
	pub const StorageMaxReservations: u32 = 256;
	pub const StorageMaxReservationExpiryBlocks: u32 = 256;
	pub const StorageMaxReservationsPerExpiryBlock: u32 = 256;
	pub const StorageMaxReservationLinks: u32 = 1024;
	pub const StorageTombstoneRetention: BlockNumber = 100;
	pub const StorageAuthorizationPeriod: BlockNumber = 14 * DAYS;
	pub const StorageStoreRenewPriority: TransactionPriority = TransactionPriority::MAX / 4;
	pub const StorageStoreRenewLongevity: TransactionLongevity = DAYS as TransactionLongevity;
	pub const StorageCleanupPriority: TransactionPriority = TransactionPriority::MAX;
	pub const StorageCleanupLongevity: TransactionLongevity = DAYS as TransactionLongevity;
}

/// Recursively exposes Utility calls to Orbis Storage's authorization extension. Storage mutations
/// are required to be direct extrinsics; wrapped mutations are rejected by the extension.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct OrbisStorageCallInspector;

impl OrbisStorageCallInspector {
	fn is_opaque_dispatch_wrapper(call: &RuntimeCall) -> bool {
		matches!(call, RuntimeCall::Multisig(pallet_multisig::Call::approve_as_multi { .. }))
	}

	fn contains_storage_mutation(call: &RuntimeCall, depth: u32) -> bool {
		if matches!(
			call,
			RuntimeCall::TransactionStorage(
				pallet_orbis_transaction_storage::Call::store { .. }
					| pallet_orbis_transaction_storage::Call::store_with_cid_config { .. }
					| pallet_orbis_transaction_storage::Call::force_renew { .. }
					| pallet_orbis_transaction_storage::Call::store_reserved { .. }
					| pallet_orbis_transaction_storage::Call::renew_reserved { .. }
			)
		) {
			return true;
		}
		if Self::is_opaque_dispatch_wrapper(call)
			|| depth >= pallet_orbis_transaction_storage::MAX_WRAPPER_DEPTH
		{
			return true;
		}
		if let RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch { meta_tx, .. }) = call {
			let encoded = meta_tx.encode();
			let Ok((inner, _, _)) =
				<(RuntimeCall, sp_runtime::generic::ExtensionVersion, MetaTxExtension)>::decode(
					&mut encoded.as_slice(),
				)
			else {
				return true;
			};
			return Self::contains_storage_mutation(&inner, depth + 1);
		}
		<Self as pallet_orbis_transaction_storage::CallInspector<Runtime>>::inspect_wrapper(call)
			.is_some_and(|calls| {
				calls.into_iter().any(|inner| Self::contains_storage_mutation(inner, depth + 1))
			})
	}
}

impl pallet_orbis_transaction_storage::CallInspector<Runtime> for OrbisStorageCallInspector {
	fn inspect_wrapper(call: &RuntimeCall) -> Option<Vec<&RuntimeCall>> {
		match call {
			RuntimeCall::Utility(pallet_utility::Call::batch { calls })
			| RuntimeCall::Utility(pallet_utility::Call::batch_all { calls })
			| RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) => {
				Some(calls.iter().collect())
			},
			RuntimeCall::Utility(pallet_utility::Call::as_derivative { call, .. })
			| RuntimeCall::Utility(pallet_utility::Call::dispatch_as { call, .. })
			| RuntimeCall::Utility(pallet_utility::Call::dispatch_as_fallible { call, .. })
			| RuntimeCall::Utility(pallet_utility::Call::with_weight { call, .. }) => {
				Some(vec![call.as_ref()])
			},
			RuntimeCall::Proxy(pallet_proxy::Call::proxy { call, .. })
			| RuntimeCall::Proxy(pallet_proxy::Call::proxy_announced { call, .. })
			| RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 { call, .. })
			| RuntimeCall::Multisig(pallet_multisig::Call::as_multi { call, .. })
			| RuntimeCall::Scheduler(pallet_scheduler::Call::schedule { call, .. })
			| RuntimeCall::Scheduler(pallet_scheduler::Call::schedule_named { call, .. })
			| RuntimeCall::Scheduler(pallet_scheduler::Call::schedule_after { call, .. })
			| RuntimeCall::Scheduler(pallet_scheduler::Call::schedule_named_after {
				call, ..
			})
			| RuntimeCall::Revive(pallet_revive::Call::eth_substrate_call { call, .. })
			| RuntimeCall::Revive(pallet_revive::Call::dispatch_as_fallback_account {
				call, ..
			}) => Some(vec![call.as_ref()]),
			_ => None,
		}
	}

	fn is_storage_mutating_call(call: &RuntimeCall, depth: u32) -> bool {
		Self::contains_storage_mutation(call, depth)
	}
}

impl Contains<RuntimeCall> for OrbisStorageCallInspector {
	fn contains(call: &RuntimeCall) -> bool {
		<Self as pallet_orbis_transaction_storage::CallInspector<Runtime>>::is_storage_mutating_call(
			call, 0,
		)
	}
}

impl pallet_orbis_transaction_storage::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type FeeDestination = ();
	type WeightInfo = pallet_orbis_transaction_storage::weights::SubstrateWeight<Runtime>;
	type MaxBlockTransactions = StorageMaxBlockTransactions;
	type MaxTransactionSize = StorageMaxTransactionSize;
	type MaxPermanentStorageSize = StorageMaxPermanentStorageSize;
	type MaxReservations = StorageMaxReservations;
	type MaxReservationExpiryBlocks = StorageMaxReservationExpiryBlocks;
	type MaxReservationsPerExpiryBlock = StorageMaxReservationsPerExpiryBlock;
	type MaxReservationLinks = StorageMaxReservationLinks;
	type TombstoneRetention = StorageTombstoneRetention;
	type ReservationPurpose = indiv_pallet_resources::types::ReservationPurpose;
	type ResourceClaimLifecycle = Resources;
	// TransactionStorage is not an authority for the new Commons storage plane. New provider
	// allocations are governed exclusively by StorageProvider bucket agreements.
	type ProviderAllocation = ();
	type AuthorizationPeriod = StorageAuthorizationPeriod;
	type AuthorizerRegistrarOrigin = EnsureRoot<AccountId>;
	type Authorizer = EitherOf<
		pallet_orbis_transaction_storage::AsAuthorizer<
			EnsureRoot<AccountId>,
			AccountId,
			BlockNumber,
		>,
		pallet_orbis_transaction_storage::EnsureAllowedAuthorizers<Runtime>,
	>;
	type StoreRenewPriority = StorageStoreRenewPriority;
	type StoreRenewLongevity = StorageStoreRenewLongevity;
	type RemoveExpiredAuthorizationPriority = StorageCleanupPriority;
	type RemoveExpiredAuthorizationLongevity = StorageCleanupLongevity;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = pallet_orbis_transaction_storage::benchmarking::DefaultCheckProofHelper;
}

parameter_types! {
	/// Maximum clock skew accepted for a HOP submit signature: 48 hours in milliseconds.
	pub const HopSubmitTimestampTolerance: u64 = 48 * 60 * 60 * 1_000;
}

impl pallet_orbis_hop_promotion::Config for Runtime {
	type SubmitTimestampTolerance = HopSubmitTimestampTolerance;
	type WeightInfo = weights::pallet_orbis_hop_promotion::WeightInfo<Runtime>;
}

parameter_types! {
	pub MbmServiceWeight: Weight = Perbill::from_percent(80) * RuntimeBlockWeights::get().max_block;
}

impl pallet_migrations::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type Migrations = MbmMigrations;
	// Benchmarks need mocked migrations to guarantee that they succeed.
	#[cfg(feature = "runtime-benchmarks")]
	type Migrations = pallet_migrations::mock_helpers::MockedMigrations;
	type CursorMaxLen = ConstU32<65_536>;
	type IdentifierMaxLen = ConstU32<256>;
	type MigrationStatusHandler = ();
	type FailedMigrationHandler = frame_support::migrations::FreezeChainOnFailedMigration;
	type MaxServiceWeight = MbmServiceWeight;
	type WeightInfo = weights::pallet_migrations::WeightInfo<Runtime>;
}
impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = weights::pallet_sudo::WeightInfo<Runtime>;
}

parameter_types! {
	pub const AttestationMaxSchemaDefinitionLen: u32 = 16 * 1024;
	pub const AttestationMaxAuthorizedIssuers: u32 = 64;
	pub const AttestationMaxSchemasPerCreator: u32 = 256;
	pub const AttestationMaxAttestationsPerIndex: u32 = 1_024;
	pub const AttestationMaxBatchSize: u32 = 64;
	pub const AttestationMaxBatchEncodedLen: u32 = 256 * 1024;
}

impl pallet_orbis_attestation::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Signer = MultiSigner;
	type Signature = MultiSignature;
	type AdminOrigin = EnsureRoot<AccountId>;
	type MaxSchemaDefinitionLen = AttestationMaxSchemaDefinitionLen;
	type MaxAuthorizedIssuers = AttestationMaxAuthorizedIssuers;
	type MaxSchemasPerCreator = AttestationMaxSchemasPerCreator;
	type MaxAttestationsPerIndex = AttestationMaxAttestationsPerIndex;
	type MaxBatchSize = AttestationMaxBatchSize;
	type MaxBatchEncodedLen = AttestationMaxBatchEncodedLen;
	type WeightInfo = pallet_orbis_attestation::weights::SubstrateWeight<Runtime>;
}

/// Joins the single StorageProvider commitment authority with Drive's root-reference index.
/// Drive, S3 and Names all fail closed against this adapter; no legacy content ledger is read.
pub struct OrbisStorageControl;

#[cfg(feature = "runtime-benchmarks")]
pub struct OrbisStorageBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_orbis_drive::benchmarking::BenchmarkHelper for OrbisStorageBenchmarkHelper {
	fn make_publishable(manifest: [u8; 32], provider: [u8; 32]) {
		pallet_orbis_storage_provider::CanonicalManifests::<Runtime>::insert(
			manifest,
			pallet_orbis_storage_provider::CanonicalManifestRecord {
				bucket_id: Default::default(),
				provider_commitment: Some(provider),
				state: pallet_orbis_storage_control_primitives::CommitmentState::Publishable,
				checkpoint: Some(System::block_number()),
				tombstoned_at: None,
			},
		);
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl pallet_orbis_s3::benchmarking::BenchmarkHelper for OrbisStorageBenchmarkHelper {
	fn make_publishable(manifest: [u8; 32], provider: [u8; 32]) {
		<Self as pallet_orbis_drive::benchmarking::BenchmarkHelper>::make_publishable(
			manifest, provider,
		);
	}

	fn make_deletion_satisfied(manifest: [u8; 32]) {
		if System::block_number() < ProviderEvidenceWindow::get() {
			System::set_block_number(ProviderEvidenceWindow::get());
		}
		let now = System::block_number();
		pallet_orbis_storage_provider::GovernedFinalizedCheckpoint::<Runtime>::put(now);
		pallet_orbis_storage_provider::CanonicalManifests::<Runtime>::insert(
			manifest,
			pallet_orbis_storage_provider::CanonicalManifestRecord {
				bucket_id: Default::default(),
				provider_commitment: Some([2; 32]),
				state: pallet_orbis_storage_control_primitives::CommitmentState::Tombstoned,
				checkpoint: Some(now),
				tombstoned_at: Some(now.saturating_sub(ProviderEvidenceWindow::get())),
			},
		);
		let providers = (1..=ProviderMaxAssignedProviders::get())
			.map(|index| [index as u8; 32].into())
			.collect::<Vec<AccountId>>();
		let required: pallet_orbis_storage_provider::AssignedProvidersOf<Runtime> =
			providers.clone().try_into().expect("maximum assigned providers is bounded");
		pallet_orbis_storage_provider::ManifestDeletionRequirements::<Runtime>::insert(
			manifest, required,
		);
		for provider in providers {
			pallet_orbis_storage_provider::ManifestDeletionAcknowledgements::<Runtime>::insert(
				manifest,
				&provider,
				pallet_orbis_storage_provider::DeletionAcknowledgement {
					provider: provider.clone(),
					bucket_id: Default::default(),
					manifest,
					evidence_hash: Default::default(),
					service_key: sp_core::ed25519::Public::from_raw([1; 32]),
					signature: sp_core::ed25519::Signature::from_raw([1; 64]),
					acknowledged_at: now,
				},
			);
		}
	}
}

impl pallet_orbis_storage_control_primitives::CanonicalStorageControl for OrbisStorageControl {
	fn manifest_state(
		manifest: &pallet_orbis_storage_control_primitives::Commitment,
	) -> pallet_orbis_storage_control_primitives::CommitmentState {
		<StorageProvider as pallet_orbis_storage_control_primitives::CanonicalStorageControl>::manifest_state(
			manifest,
		)
	}

	fn provider_commitment_matches(
		manifest: &pallet_orbis_storage_control_primitives::Commitment,
		provider_commitment: &pallet_orbis_storage_control_primitives::Commitment,
	) -> bool {
		<StorageProvider as pallet_orbis_storage_control_primitives::CanonicalStorageControl>::provider_commitment_matches(
			manifest,
			provider_commitment,
		)
	}

	fn is_drive_referenced(manifest: &pallet_orbis_storage_control_primitives::Commitment) -> bool {
		Drive::active_manifest_references(manifest) > 0
	}

	fn deletion_evidence_satisfied(
		manifest: &pallet_orbis_storage_control_primitives::Commitment,
	) -> bool {
		<StorageProvider as pallet_orbis_storage_control_primitives::CanonicalStorageControl>::deletion_evidence_satisfied(
			manifest,
		)
	}
}

impl pallet_orbis_names::ContentReferenceValidator<[u8; 32]> for OrbisStorageControl {
	fn contains(content_hash: &[u8; 32]) -> bool {
		matches!(
			<OrbisStorageControl as pallet_orbis_storage_control_primitives::CanonicalStorageControl>::manifest_state(content_hash),
			pallet_orbis_storage_control_primitives::CommitmentState::Publishable
		)
	}
}

/// Orbis Names keeps canonical identifiers only; Entity remains the sole subject authority.
pub struct OrbisIdentityRegistry;

impl pallet_orbis_names::SubjectReferenceValidator<Ss58Identifier> for OrbisIdentityRegistry {
	fn contains(subject: &Ss58Identifier) -> bool {
		pallet_origin_entity::EntityInfoOf::<Runtime>::contains_key(subject)
	}
}

/// Attestation remains the sole authority for opaque attestation liveness.
pub struct OrbisAttestationRegistry;

impl pallet_orbis_names::AttestationReferenceValidator<Hash> for OrbisAttestationRegistry {
	fn is_live(attestation: &Hash) -> bool {
		pallet_orbis_attestation::Pallet::<Runtime>::is_live(*attestation)
	}
}

parameter_types! {
	pub const DriveMaxNameBytes: u32 = 256;
	pub const DriveMaxPerOwner: u32 = 256;
	pub const DriveMaxControllers: u32 = 32;
}

impl pallet_orbis_drive::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type StorageControl = OrbisStorageControl;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = OrbisStorageBenchmarkHelper;
	type MaxDriveNameBytes = DriveMaxNameBytes;
	type MaxDrivesPerOwner = DriveMaxPerOwner;
	type MaxControllersPerDrive = DriveMaxControllers;
	type WeightInfo = pallet_orbis_drive::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const NamesMaxLabelLength: u32 = 63;
	pub const NamesMaxSaltLength: u32 = 64;
	pub const NamesMaxAddressLength: u32 = 128;
	pub const NamesMaxTextKeyLength: u32 = 32;
	pub const NamesMaxTextValueLength: u32 = 256;
	pub const NamesMaxTextRecords: u32 = 32;
	pub const NamesMaxControllers: u32 = 32;
	pub const NamesMaxRegistrars: u32 = 16;
	pub const NamesMaxBootstrapReservations: u32 = 64;
	pub const NamesMaxNamesPerOwner: u32 = 256;
	pub const NamesMaxChildrenPerName: u32 = 256;
	pub const NamesMaxRootNames: u32 = 10_000;
	pub const NamesMaxNameDepth: u32 = 16;
	pub const NamesMaxCommitmentsPerAccount: u32 = 32;
	pub const NamesMinCommitmentAge: BlockNumber = 2;
	pub const NamesMaxCommitmentAge: BlockNumber = 600;
	pub const NamesRegistrationPeriod: BlockNumber = 365 * DAYS;
	pub const NamesMaxRenewalPeriod: BlockNumber = 365 * DAYS;
}

impl pallet_orbis_names::Config for Runtime {
	type AdminOrigin = entity::IdentityAdminOrigin;
	type SubjectId = Ss58Identifier;
	type SubjectReferenceValidator = OrbisIdentityRegistry;
	type AttestationId = Hash;
	type AttestationReferenceValidator = OrbisAttestationRegistry;
	type ContentCommitment = [u8; 32];
	type ContentReferenceValidator = OrbisStorageControl;
	type MaxLabelLength = NamesMaxLabelLength;
	type MaxSaltLength = NamesMaxSaltLength;
	type MaxAddressLength = NamesMaxAddressLength;
	type MaxTextKeyLength = NamesMaxTextKeyLength;
	type MaxTextValueLength = NamesMaxTextValueLength;
	type MaxTextRecords = NamesMaxTextRecords;
	type MaxControllers = NamesMaxControllers;
	type MaxRegistrars = NamesMaxRegistrars;
	type MaxBootstrapReservations = NamesMaxBootstrapReservations;
	type MaxNamesPerOwner = NamesMaxNamesPerOwner;
	type MaxChildrenPerName = NamesMaxChildrenPerName;
	type MaxRootNames = NamesMaxRootNames;
	type MaxNameDepth = NamesMaxNameDepth;
	type MaxCommitmentsPerAccount = NamesMaxCommitmentsPerAccount;
	type MinCommitmentAge = NamesMinCommitmentAge;
	type MaxCommitmentAge = NamesMaxCommitmentAge;
	type RegistrationPeriod = NamesRegistrationPeriod;
	type MaxRenewalPeriod = NamesMaxRenewalPeriod;
	type WeightInfo = pallet_orbis_names::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const S3MaxBucketNameLen: u32 = 63;
	pub const S3MaxObjectKeyLen: u32 = 1_024;
	pub const S3MaxControllers: u32 = 32;
	pub const S3MaxBucketsPerOwner: u32 = 256;
	pub const S3MaxObjectsPerBucket: u32 = 10_000;
	pub const S3MaxObjectVersions: u32 = 64;
}

impl pallet_orbis_s3::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type StorageControl = OrbisStorageControl;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = OrbisStorageBenchmarkHelper;
	type MaxBucketNameLen = S3MaxBucketNameLen;
	type MaxObjectKeyLen = S3MaxObjectKeyLen;
	type MaxControllers = S3MaxControllers;
	type MaxBucketsPerOwner = S3MaxBucketsPerOwner;
	type MaxObjectsPerBucket = S3MaxObjectsPerBucket;
	type MaxObjectVersions = S3MaxObjectVersions;
	type WeightInfo = pallet_orbis_s3::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const ProviderMaxEndpointBytes: u32 = 512;
	pub const ProviderMaxEntityIdBytes: u32 = 64;
	pub const ProviderMaxBuckets: u32 = 4_096;
	pub const ProviderMaxBucketGrants: u32 = 256;
	pub const ProviderMaxHostDelegationsPerBucket: u32 = 256;
	pub const ProviderMaxCapabilityProductIdBytes: u32 = 128;
	pub const ProviderMaxCapabilityMethods: u32 = 64;
	pub const ProviderMaxCapabilityCidBytes: u32 = 128;
	pub const ProviderMaxHostDelegationLifetime: BlockNumber = 128;
	pub const ProviderMaxReplicas: u32 = 4;
	pub const ProviderMaxAssignedProviders: u32 = 5;
	pub const ProviderMaxAgreements: u32 = 1_024;
	pub const ProviderMaxBucketAgreements: u32 = storage_api::MAX_BUCKET_AGREEMENTS;
	pub const ProviderMaxDutiesPerBlock: u32 = 256;
	pub const ProviderMaxChallengeBacklog: u32 = 256;
	pub const ProviderMaxCapacityReleasesPerBlock: u32 = 256;
	pub const ProviderMaxChallengesPerBlock: u32 = 128;
	pub const ProviderMaxReconciliationRecords: u32 = 128;
	pub const ProviderMaxProofNodes: u32 = 64;
	pub const ProviderMaxEvidencePerProvider: u32 = 1_024;
	pub const ProviderMaxOrganizationHistory: u32 = 64;
	pub const ProviderCheckpointCadence: BlockNumber = 100;
	pub const ProviderCheckpointGrace: BlockNumber = 20;
	pub const ProviderMaxCheckpointAge: BlockNumber = 128;
	pub const ProviderEvidenceWindow: BlockNumber = 256;
}

/// Provider authority is evaluated only from the Entity and native Attestation authorities.
/// It deliberately does not read Humanity, People, PeopleLite or Personhood state.
pub struct OrbisProviderAuthority;
pub struct OrbisCheckpointContext;

impl pallet_orbis_storage_provider::CheckpointContextProvider<Hash, BlockNumber>
	for OrbisCheckpointContext
{
	fn genesis_hash() -> Hash {
		System::block_hash(0)
	}

	fn spec_version() -> u32 {
		VERSION.spec_version
	}

	fn transaction_version() -> u32 {
		VERSION.transaction_version
	}

	fn metadata_hash() -> Hash {
		let extension = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(true);
		<frame_metadata_hash_extension::CheckMetadataHash<Runtime> as
			sp_runtime::traits::TransactionExtension<RuntimeCall>>::implicit(&extension)
			.ok()
			.flatten()
			.map(Hash::from)
			.unwrap_or_default()
	}

	fn block_hash(block: BlockNumber) -> Hash {
		System::block_hash(block)
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl pallet_orbis_storage_provider::benchmarking::BenchmarkHelper<Runtime>
	for OrbisProviderAuthority
{
	fn set_finalized_block(block: BlockNumber) {
		pallet_orbis_storage_provider::GovernedFinalizedCheckpoint::<Runtime>::put(block);
	}

	fn organization(
		provider: &AccountId,
		service_key: &sp_core::ed25519::Public,
		rotation_predecessor: Option<Hash>,
	) -> pallet_orbis_storage_provider::OrganizationRefOf<Runtime> {
		use codec::Encode;
		use sp_runtime::traits::Hash as HashT;

		let entity =
			Ss58Identifier::to_encoded(sp_io::hashing::blake2_256(&provider.encode()), 1006, 53, 1)
				.expect("benchmark identifier is valid");
		let entity_info: pallet_origin_entity::entity::EntityInfo<
			entity::MaxRawDataLength,
			entity::MaxAdditionalAttributes,
		> = Default::default();
		pallet_origin_entity::EntityInfoOf::<Runtime>::insert(&entity, entity_info);
		let schema_id = <Runtime as frame_system::Config>::Hashing::hash_of(&(
			b"benchmark-schema",
			provider,
			service_key,
		));
		let attestation_id = <Runtime as frame_system::Config>::Hashing::hash_of(&(
			b"benchmark-attestation",
			provider,
			service_key,
		));
		let sla_commitment = <Runtime as frame_system::Config>::Hashing::hash_of(&(
			b"benchmark-sla",
			provider,
			service_key,
		));
		let subject_commitment =
			<Runtime as frame_system::Config>::Hashing::hash_of(&(entity.clone(), service_key));
		pallet_orbis_attestation::Schemas::<Runtime>::insert(
			schema_id,
			pallet_orbis_attestation::SchemaRecord::<Runtime> {
				creator: provider.clone(),
				definition: Default::default(),
				definition_commitment: schema_id,
				status: pallet_orbis_attestation::SchemaStatus::Active,
				revocable: true,
				unique: false,
				index_policy: pallet_orbis_attestation::IndexPolicy::None,
				authorized_issuers: Default::default(),
				created_at: System::block_number(),
			},
		);
		pallet_orbis_attestation::Attestations::<Runtime>::insert(
			attestation_id,
			pallet_orbis_attestation::AttestationRecord::<Runtime> {
				issuer: provider.clone(),
				schema: schema_id,
				subject_commitment,
				payload_commitment: sla_commitment,
				status_commitment: Default::default(),
				parent: None,
				expiry: None,
				uniqueness_commitment: None,
				revocable: true,
				issuance_nonce: 0,
				issued_at: System::block_number(),
				revoked_at: None,
				revoked_by: None,
			},
		);
		pallet_orbis_storage_provider::ProviderOrganizationRefV1 {
			entity_id: entity.as_ref().to_vec().try_into().expect("bounded entity identifier"),
			attestation_id,
			schema_id,
			sla_commitment,
			sla_version: 1,
			valid_from: 0,
			valid_until: BlockNumber::MAX,
			rotation_predecessor,
		}
	}

	fn invalidate_authority(
		_: &AccountId,
		organization: &pallet_orbis_storage_provider::OrganizationRefOf<Runtime>,
	) {
		pallet_orbis_attestation::Attestations::<Runtime>::mutate(
			organization.attestation_id,
			|record| {
				if let Some(record) = record {
					record.revoked_at = Some(BlockNumber::default());
				}
			},
		);
	}
}

impl
	pallet_orbis_storage_provider::ProviderAuthority<
		AccountId,
		pallet_orbis_storage_provider::OrganizationRefOf<Runtime>,
		sp_core::ed25519::Public,
		BlockNumber,
	> for OrbisProviderAuthority
{
	fn validate(
		_provider: &AccountId,
		organization: &pallet_orbis_storage_provider::OrganizationRefOf<Runtime>,
		service_key: &sp_core::ed25519::Public,
		finalized_at: BlockNumber,
	) -> Result<(), pallet_orbis_storage_provider::ProviderAuthorityError> {
		use pallet_orbis_storage_provider::ProviderAuthorityError;
		use sp_runtime::traits::Hash as HashT;

		let entity_id = Ss58Identifier::try_from(organization.entity_id.to_vec())
			.map_err(|_| ProviderAuthorityError::OrganizationUnknown)?;
		if !pallet_origin_entity::EntityInfoOf::<Runtime>::contains_key(&entity_id) {
			return Err(ProviderAuthorityError::OrganizationUnknown);
		}
		if finalized_at < organization.valid_from || finalized_at >= organization.valid_until {
			return Err(ProviderAuthorityError::AttestationExpired);
		}
		let attestation =
			pallet_orbis_attestation::Attestations::<Runtime>::get(organization.attestation_id)
				.ok_or(ProviderAuthorityError::AttestationInvalid)?;
		if attestation.schema != organization.schema_id
			|| attestation.issued_at > finalized_at
			|| attestation.revoked_at.is_some_and(|at| at <= finalized_at)
		{
			return Err(ProviderAuthorityError::AttestationInvalid);
		}
		if attestation.expiry.is_some_and(|at| finalized_at >= at) {
			return Err(ProviderAuthorityError::AttestationExpired);
		}
		let schema = pallet_orbis_attestation::Schemas::<Runtime>::get(organization.schema_id)
			.ok_or(ProviderAuthorityError::SlaInvalid)?;
		if schema.status != pallet_orbis_attestation::SchemaStatus::Active
			|| (attestation.issuer != schema.creator
				&& !schema.authorized_issuers.contains(&attestation.issuer))
			|| organization.sla_version != 1
			|| attestation.payload_commitment != organization.sla_commitment
		{
			return Err(ProviderAuthorityError::SlaInvalid);
		}
		let expected_subject =
			<Runtime as frame_system::Config>::Hashing::hash_of(&(entity_id, service_key));
		if service_key.0 == [0u8; 32] || attestation.subject_commitment != expected_subject {
			return Err(ProviderAuthorityError::ServiceKeyInvalid);
		}
		Ok(())
	}
}

impl pallet_orbis_storage_provider::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AdminOrigin = EnsureRoot<AccountId>;
	type MaxEndpointBytes = ProviderMaxEndpointBytes;
	type MaxEntityIdBytes = ProviderMaxEntityIdBytes;
	type MaxProviders = ConstU32<1_024>;
	type MaxBuckets = ProviderMaxBuckets;
	type MaxBucketGrants = ProviderMaxBucketGrants;
	type MaxHostDelegationsPerBucket = ProviderMaxHostDelegationsPerBucket;
	type MaxCapabilityProductIdBytes = ProviderMaxCapabilityProductIdBytes;
	type MaxCapabilityMethods = ProviderMaxCapabilityMethods;
	type MaxCapabilityCidBytes = ProviderMaxCapabilityCidBytes;
	type MaxHostDelegationLifetime = ProviderMaxHostDelegationLifetime;
	type MaxReplicas = ProviderMaxReplicas;
	type MaxAssignedProviders = ProviderMaxAssignedProviders;
	type MaxProviderAgreements = ProviderMaxAgreements;
	type MaxBucketAgreements = ProviderMaxBucketAgreements;
	type MaxDutiesPerBlock = ProviderMaxDutiesPerBlock;
	type MaxChallengeBacklog = ProviderMaxChallengeBacklog;
	type MaxCapacityReleasesPerBlock = ProviderMaxCapacityReleasesPerBlock;
	type MaxChallengesPerBlock = ProviderMaxChallengesPerBlock;
	type MaxReconciliationRecords = ProviderMaxReconciliationRecords;
	type MaxProofNodes = ProviderMaxProofNodes;
	type MaxEvidencePerProvider = ProviderMaxEvidencePerProvider;
	type MaxOrganizationHistory = ProviderMaxOrganizationHistory;
	type CheckpointCadence = ProviderCheckpointCadence;
	type CheckpointGrace = ProviderCheckpointGrace;
	type MaxCheckpointAge = ProviderMaxCheckpointAge;
	type EvidenceWindow = ProviderEvidenceWindow;
	type ProviderAuthority = OrbisProviderAuthority;
	type CheckpointContext = OrbisCheckpointContext;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = OrbisProviderAuthority;
	type WeightInfo = pallet_orbis_storage_provider::weights::SubstrateWeight<Runtime>;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime
	{
		// System support stuff.
		System: frame_system = 0,
		ParachainSystem: cumulus_pallet_parachain_system = 1,
		Timestamp: pallet_timestamp = 2,
		ParachainInfo: parachain_info = 3,
		WeightReclaim: cumulus_pallet_weight_reclaim = 4,

		// Monetary stuff.
		Balances: pallet_balances = 10,
		TransactionPayment: pallet_transaction_payment = 11,
		Indices: pallet_indices = 12,
		SkipFeelessPayment: pallet_skip_feeless_payment = 13,

		// Collator support. The order of these 5 are important and shall not change.
		Authorship: pallet_authorship = 20,
		CollatorSelection: pallet_collator_selection = 21,
		Session: pallet_session = 22,
		Aura: pallet_aura = 23,
		AuraExt: cumulus_pallet_aura_ext = 24,
		Scheduler: pallet_scheduler = 25,

		// XCM & related
		XcmpQueue: cumulus_pallet_xcmp_queue = 30,
		PolkadotXcm: pallet_xcm = 31,
		CumulusXcm: cumulus_pallet_xcm = 32,
		MessageQueue: pallet_message_queue = 34,

		// Handy utilities.
		Utility: pallet_utility = 40,
		Multisig: pallet_multisig = 41,
		Proxy: pallet_proxy = 42,

		// The main stage.
		// The relay Coretime pallet encodes Broker callbacks with pallet index 50.
		Broker: pallet_broker = 50,
		Token: pallet_origin_token = 51,
		Register: pallet_origin_register = 52,
		Entity: pallet_origin_entity = 53,
		Feeless: pallet_origin_feeless = 54,

		// Unified application assets.
		Assets: pallet_assets::<Instance1> = 80,
		AssetsFreezer: pallet_assets_freezer::<Instance1> = 81,
		AssetsHolder: pallet_assets_holder::<Instance1> = 82,
		ForeignAssets: pallet_assets::<Instance2> = 83,
		PoolAssets: pallet_assets::<Instance3> = 84,
		ForeignAssetsFreezer: pallet_assets_freezer::<Instance2> = 85,
		PoolAssetsFreezer: pallet_assets_freezer::<Instance3> = 86,
		Uniques: pallet_uniques = 87,
		Nfts: pallet_nfts = 88,
		AssetRate: pallet_asset_rate = 89,

		// People identity and lightweight aliases.
		People: pallet_orbis_people = 90,
		ChunksManager: indiv_pallet_chunks_manager = 91,
		Members: indiv_pallet_members = 92,
		MembersNotifier: indiv_pallet_members_notifier = 93,
		PeopleLite: indiv_pallet_people_lite = 94,
		Personhood: indiv_pallet_people = 95,
		Resources: indiv_pallet_resources = 96,
		Score: pallet_orbis_score = 97,
		Honour: pallet_orbis_honour = 99,
		Attestation: pallet_orbis_attestation = 105,

		// Solidity and PolkaVM contracts.
		Revive: pallet_revive = 100,

		// Orbis Storage durable transaction storage and proof accounting.
		TransactionStorage: pallet_orbis_transaction_storage = 110,
		HopPromotion: pallet_orbis_hop_promotion = 111,
		Names: pallet_orbis_names = 116,
		StorageProvider: pallet_orbis_storage_provider = 120,
		Drive: pallet_orbis_drive = 121,
		S3: pallet_orbis_s3 = 122,

		// Application asset and payment extensions.
		AssetConversion: pallet_asset_conversion = 200,
		AssetTxPayment: pallet_asset_conversion_tx_payment = 201,

		// Utilities
		MetaTx: pallet_meta_tx = 215,
		TxPause: pallet_tx_pause = 216,
		SafeMode: pallet_safe_mode = 217,
		VerifySignature: pallet_verify_signature = 219,
		// Remark: pallet_remark = 220,
		CoretimeControl: pallet_coretime_control = 221,

		// Migrations pallet
		MultiBlockMigrations: pallet_migrations = 249,

		// Sudo.
		Sudo: pallet_sudo = 255,
	}
);

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with an [`sp_runtime::Justification`].
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The TransactionExtension to the basic transaction logic.
pub type AssetPayment = pallet_origin_feeless::ChargeOrSkipFeeless<
	Runtime,
	pallet_asset_conversion_tx_payment::ChargeAssetTxPayment<Runtime>,
>;
pub type PaymentPolicy = ExplicitPayment<Runtime, AssetPayment>;
pub type AccountAwareCheckNonZero =
	AccountAwareResources<frame_system::CheckNonZeroSender<Runtime>>;
pub type AccountAwareCheckNonce = AccountAwareResources<frame_system::CheckNonce<Runtime>>;
pub type AccountAwarePayment = AccountAwareResources<PaymentPolicy>;

pub type InnerTxExtensions = (
	OriginPolicyExtensions,
	AccountAwareCheckNonZero,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckMortality<Runtime>,
	AccountAwareCheckNonce,
	frame_system::CheckWeight<Runtime>,
	AccountAwarePayment,
	pallet_orbis_transaction_storage::extension::ValidateStorageCalls<
		Runtime,
		OrbisStorageCallInspector,
	>,
	frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	pallet_revive::evm::tx_extension::SetOrigin<Runtime>,
);

/// Storage-proof and unused-execution-weight reclamation wrapped around every Orbis transaction.
/// This is required by the Asset Hub and Orbis Storage execution model, especially when multiple
/// blocks share a collation bundle.
pub type OuterCoreExtensions =
	cumulus_pallet_weight_reclaim::StorageWeightReclaim<Runtime, InnerTxExtensions>;
pub type TxExtensions = meta_v6::PaidMetaScope<OuterCoreExtensions>;

fn paid_tx_extensions(inner: InnerTxExtensions) -> TxExtensions {
	meta_v6::PaidMetaScope::new(inner.into())
}

fn default_inner_tx_extensions(
	nonce: u32,
	payment: PaymentPolicy,
	revive_origin: pallet_revive::evm::tx_extension::SetOrigin<Runtime>,
) -> InnerTxExtensions {
	(
		default_origin_policy_extensions(),
		frame_system::CheckNonZeroSender::<Runtime>::new().into(),
		frame_system::CheckSpecVersion::<Runtime>::new(),
		frame_system::CheckTxVersion::<Runtime>::new(),
		frame_system::CheckGenesis::<Runtime>::new(),
		frame_system::CheckMortality::<Runtime>::from(generic::Era::Immortal),
		AccountAwareResources::from(frame_system::CheckNonce::<Runtime>::from(nonce)),
		frame_system::CheckWeight::<Runtime>::new(),
		payment.into(),
		pallet_orbis_transaction_storage::extension::ValidateStorageCalls::<
			Runtime,
			OrbisStorageCallInspector,
		>::default(),
		canonical_metadata_extension(),
		revive_origin,
	)
}

fn canonical_metadata_extension() -> frame_metadata_hash_extension::CheckMetadataHash<Runtime> {
	#[cfg(test)]
	return frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(
		option_env!("RUNTIME_METADATA_HASH").is_some(),
	);
	#[cfg(not(test))]
	frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(true)
}

/// Extensions applied when an Ethereum transaction is converted into an Orbis extrinsic.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EthExtraImpl;

impl EthExtra for EthExtraImpl {
	type Config = Runtime;
	type ExtensionV0 = TxExtensions;
	type ExtensionOtherVersions = sp_runtime::traits::InvalidVersion;

	fn get_eth_extension(nonce: u32, tip: Balance) -> Self::ExtensionV0 {
		paid_tx_extensions(default_inner_tx_extensions(
			nonce,
			pallet_origin_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(
					tip, None,
				),
			)
			.into(),
			pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::new_from_eth_transaction(),
		))
	}
}

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	pallet_revive::evm::runtime::UncheckedExtrinsic<Address, Signature, EthExtraImpl>;

impl<C> frame_system::offchain::CreateTransactionBase<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type Extrinsic = UncheckedExtrinsic;
	type RuntimeCall = RuntimeCall;
}

impl<C> frame_system::offchain::CreateBare<C> for Runtime
where
	RuntimeCall: From<C>,
{
	fn create_bare(call: RuntimeCall) -> UncheckedExtrinsic {
		sp_runtime::generic::UncheckedExtrinsic::new_bare(call).into()
	}
}

impl<C> frame_system::offchain::CreateTransaction<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type Extension = TxExtensions;

	fn create_transaction(call: RuntimeCall, extension: TxExtensions) -> UncheckedExtrinsic {
		sp_runtime::generic::UncheckedExtrinsic::new_transaction(call, extension).into()
	}
}

impl<C> frame_system::offchain::CreateAuthorizedTransaction<C> for Runtime
where
	RuntimeCall: From<C>,
{
	fn create_extension() -> Self::Extension {
		paid_tx_extensions(default_inner_tx_extensions(
			0,
			pallet_origin_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
			)
			.into(),
			pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::default(),
		))
	}
}

/// Origin and Orbis launch from a clean genesis; no predecessor state is migrated.
pub type Migrations = ();

/// MBM migrations to apply on runtime upgrade.
pub type MbmMigrations = ();

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
	Migrations,
>;

#[cfg(feature = "runtime-benchmarks")]
pub struct AssetConversionTxHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_asset_conversion_tx_payment::BenchmarkHelperTrait<AccountId, Location, Location>
	for AssetConversionTxHelper
{
	fn create_asset_id_parameter(seed: u32) -> (Location, Location) {
		let asset =
			Location::new(1, [Parachain(3_000), PalletInstance(80), GeneralIndex(seed.into())]);
		(asset.clone(), asset)
	}

	fn setup_balances_and_pool(asset_id: Location, account: AccountId) {
		use frame_support::{assert_ok, traits::fungibles::Mutate};

		assert_ok!(ForeignAssets::force_create(
			RuntimeOrigin::root(),
			asset_id.clone(),
			account.clone().into(),
			true,
			1,
		));
		<Balances as frame_support::traits::fungible::Mutate<AccountId>>::set_balance(
			&account,
			(u64::MAX as u128) * 100,
		);
		assert_ok!(ForeignAssets::mint_into(asset_id.clone(), &account, (u64::MAX as u128) * 100,));

		let native = Box::new(xcm_config::OrgnRelayLocation::get());
		let asset = Box::new(asset_id);
		assert_ok!(AssetConversion::create_pool(
			RuntimeOrigin::signed(account.clone()),
			native.clone(),
			asset.clone(),
		));
		assert_ok!(AssetConversion::add_liquidity(
			RuntimeOrigin::signed(account.clone()),
			native,
			asset,
			u64::MAX.into(),
			u64::MAX.into(),
			1,
			1,
			account,
		));
	}
}

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	use super::*;
	use alloc::boxed::Box;
	use origin_commons_runtime_constants::origin::locations::{
		BenchmarkSiblingLocation, BenchmarkSiblingParaId,
	};
	use xcm::latest::Assets as XcmAssets;

	frame_benchmarking::define_benchmarks!(
		[frame_system, SystemBench::<Runtime>]
		[frame_system_extensions, SystemExtensionsBench::<Runtime>]
		[pallet_assets, Assets]
		[pallet_asset_rate, AssetRate]
		[pallet_asset_conversion, AssetConversion]
		[pallet_asset_conversion_tx_payment, AssetTxPayment]
		[pallet_balances, Balances]
		[pallet_broker, Broker]
		[pallet_orbis_transaction_storage, TransactionStorage]
		[pallet_orbis_hop_promotion, HopPromotion]
		[pallet_orbis_storage_provider, StorageProvider]
		[pallet_orbis_drive, Drive]
		[pallet_orbis_s3, S3]
		[pallet_coretime_control, CoretimeControl]
		[pallet_origin_token, Token]
		[pallet_origin_feeless, Feeless]
		[pallet_orbis_people, People]
		[indiv_pallet_chunks_manager, ChunksManager]
		[indiv_pallet_members, Members]
		[indiv_pallet_members_notifier, MembersNotifier]
		[indiv_pallet_people_lite, PeopleLite]
		[indiv_pallet_people, Personhood]
		[indiv_pallet_resources, Resources]
		[pallet_orbis_score, Score]
		[pallet_orbis_honour, Honour]
		[pallet_origin_entity, Entity]
		[pallet_message_queue, MessageQueue]
		[pallet_migrations, MultiBlockMigrations]
		[pallet_multisig, Multisig]
		[pallet_proxy, Proxy]
		[pallet_origin_register, Register]
		[pallet_safe_mode, SafeMode]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		[pallet_transaction_payment, TransactionPayment]
		[pallet_tx_pause, TxPause]
		[pallet_utility, Utility]
		[pallet_verify_signature, VerifySignature]
		[cumulus_pallet_weight_reclaim, WeightReclaim]

		// Cumulus
		[cumulus_pallet_parachain_system, ParachainSystem]
		[cumulus_pallet_xcmp_queue, XcmpQueue]
		// CollatorSelection has MaxCandidates=0 on this enterprise system chain, so its upstream
		// benchmark component bound (`MaxCandidates - 1`) is undefined and is intentionally omitted.
		// XCM
		[pallet_xcm, PalletXcmExtrinsicsBenchmark::<Runtime>]
		[pallet_xcm_benchmarks::fungible, XcmBalances]
		[pallet_xcm_benchmarks::generic, XcmGeneric]
	);

	impl frame_system_benchmarking::Config for Runtime {
		fn setup_set_code_requirements(code: &Vec<u8>) -> Result<(), BenchmarkError> {
			ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
			Ok(())
		}

		fn verify_set_code() {
			System::assert_last_event(
				cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into(),
			);
		}
	}

	impl cumulus_pallet_session_benchmarking::Config for Runtime {
		fn generate_session_keys_and_proof(owner: Self::AccountId) -> (Self::Keys, Vec<u8>) {
			let keys = SessionKeys::generate(&owner.encode(), None);
			(keys.keys, keys.proof.encode())
		}
	}

	impl pallet_transaction_payment::BenchmarkConfig for Runtime {}

	use xcm_config::OrgnRelayLocation;

	parameter_types! {
		pub DeliveryExistentialDepositAsset: Option<Asset> = Some((
			OrgnRelayLocation::get(),
			ExistentialDeposit::get()
		).into());
		pub const RandomParaId: ParaId = ParaId::new(43211234);
	}

	impl pallet_xcm::benchmarking::Config for Runtime {
		type DeliveryHelper = (
			cumulus_primitives_utility::ToParentDeliveryHelper<
				xcm_config::XcmConfig,
				DeliveryExistentialDepositAsset,
				PriceForParentDelivery,
			>,
			polkadot_runtime_common::xcm_sender::ToParachainDeliveryHelper<
				xcm_config::XcmConfig,
				DeliveryExistentialDepositAsset,
				PriceForSiblingParachainDelivery,
				BenchmarkSiblingParaId,
				ParachainSystem,
			>,
		);
		fn reachable_dest() -> Option<Location> {
			Some(Parent.into())
		}

		fn teleportable_asset_and_dest() -> Option<(Asset, Location)> {
			// Relay/native token can be teleported between People and Relay.
			Some((
				Asset { fun: Fungible(ExistentialDeposit::get()), id: AssetId(Parent.into()) },
				Parent.into(),
			))
		}

		fn reserve_transferable_asset_and_dest() -> Option<(Asset, Location)> {
			None
		}

		fn set_up_complex_asset_transfer() -> Option<(XcmAssets, u32, Location, Box<dyn FnOnce()>)>
		{
			// Exercise a deterministic sibling destination for the XCM benchmark.
			let native_location = Parent.into();
			let dest = BenchmarkSiblingLocation::get();

			// Polkadot SDK >= stable2509: HRMP open helper still required in benchmarks.
			ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(
				BenchmarkSiblingParaId::get(),
			);

			pallet_xcm::benchmarking::helpers::native_teleport_as_asset_transfer::<Runtime>(
				native_location,
				dest,
			)
		}

		fn get_asset() -> Asset {
			Asset { id: AssetId(Location::parent()), fun: Fungible(ExistentialDeposit::get()) }
		}
	}

	use xcm_config::PriceForParentDelivery;

	parameter_types! {
		pub XcmBenchmarkExistentialDepositAsset: Option<Asset> = Some((
			OrgnRelayLocation::get(),
			ExistentialDeposit::get()
		).into());
	}

	impl pallet_xcm_benchmarks::Config for Runtime {
		type XcmConfig = xcm_config::XcmConfig;
		type AccountIdConverter = xcm_config::LocationToAccountId;
		type DeliveryHelper = cumulus_primitives_utility::ToParentDeliveryHelper<
			xcm_config::XcmConfig,
			XcmBenchmarkExistentialDepositAsset,
			PriceForParentDelivery,
		>;
		fn valid_destination() -> Result<Location, BenchmarkError> {
			Ok(OrgnRelayLocation::get())
		}
		fn worst_case_holding(_depositable_count: u32) -> xcm_executor::AssetsInHolding {
			use pallet_xcm_benchmarks::MockCredit;
			let mut holding = xcm_executor::AssetsInHolding::new();
			holding
				.fungible
				.insert(AssetId(OrgnRelayLocation::get()), Box::new(MockCredit(1_000_000 * UNITS)));
			holding
		}
	}

	parameter_types! {
		pub const TrustedTeleporter: Option<(Location, Asset)> = Some((
			OrgnRelayLocation::get(),
			Asset { fun: Fungible(UNITS), id: AssetId(OrgnRelayLocation::get()) },
		));
		pub const CheckedAccount: Option<(AccountId, xcm_builder::MintLocation)> = None;
		pub const TrustedReserve: Option<(Location, Asset)> = None;
	}

	impl pallet_xcm_benchmarks::fungible::Config for Runtime {
		type TransactAsset = Balances;

		type CheckedAccount = CheckedAccount;
		type TrustedTeleporter = TrustedTeleporter;
		type TrustedReserve = TrustedReserve;

		fn get_asset() -> Asset {
			Asset { id: AssetId(OrgnRelayLocation::get()), fun: Fungible(UNITS) }
		}
	}

	impl pallet_xcm_benchmarks::generic::Config for Runtime {
		type RuntimeCall = RuntimeCall;
		type TransactAsset = Balances;

		fn worst_case_response() -> (u64, Response) {
			(0u64, Response::Version(Default::default()))
		}

		fn worst_case_asset_exchange() -> Result<(XcmAssets, XcmAssets), BenchmarkError> {
			Err(BenchmarkError::Skip)
		}

		fn universal_alias() -> Result<(Location, Junction), BenchmarkError> {
			Err(BenchmarkError::Skip)
		}

		fn transact_origin_and_runtime_call() -> Result<(Location, RuntimeCall), BenchmarkError> {
			Ok((
				OrgnRelayLocation::get(),
				frame_system::Call::remark_with_event { remark: vec![] }.into(),
			))
		}

		fn subscribe_origin() -> Result<Location, BenchmarkError> {
			Ok(OrgnRelayLocation::get())
		}

		fn claimable_asset() -> Result<(Location, Location, XcmAssets), BenchmarkError> {
			let origin = OrgnRelayLocation::get();
			let assets: XcmAssets = (AssetId(OrgnRelayLocation::get()), 1_000 * UNITS).into();
			let ticket = Location::new(0, []);
			Ok((origin, ticket, assets))
		}

		fn worst_case_for_trader() -> Result<(Asset, WeightLimit), BenchmarkError> {
			Ok((
				Asset { id: AssetId(OrgnRelayLocation::get()), fun: Fungible(1_000_000 * UNITS) },
				WeightLimit::Limited(Weight::from_parts(5000, 5000)),
			))
		}

		fn unlockable_asset() -> Result<(Location, Location, Asset), BenchmarkError> {
			Err(BenchmarkError::Skip)
		}

		fn export_message_origin_and_destination(
		) -> Result<(Location, NetworkId, InteriorLocation), BenchmarkError> {
			Err(BenchmarkError::Skip)
		}

		fn alias_origin() -> Result<(Location, Location), BenchmarkError> {
			Ok((
				Location::new(1, [Parachain(1000)]),
				Location::new(1, [Parachain(1000), AccountId32 { id: [111u8; 32], network: None }]),
			))
		}
	}

	pub use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
	pub use frame_benchmarking::{BenchmarkBatch, BenchmarkError, BenchmarkList};
	pub use frame_support::traits::StorageInfoTrait;
	pub use frame_system_benchmarking::{
		extensions::Pallet as SystemExtensionsBench, Pallet as SystemBench,
	};
	pub use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsicsBenchmark;
	pub type XcmBalances = pallet_xcm_benchmarks::fungible::Pallet<Runtime>;
	pub type XcmGeneric = pallet_xcm_benchmarks::generic::Pallet<Runtime>;
	pub use frame_support::traits::WhitelistedStorageKeys;
	pub use sp_storage::TrackedStorageKey;
}

#[cfg(feature = "runtime-benchmarks")]
use benches::*;

fn provider_api_info(
	provider: &AccountId,
	record: pallet_orbis_storage_provider::ProviderRecordOf<Runtime>,
) -> storage_api::ProviderInfo<Hash, BlockNumber> {
	storage_api::ProviderInfo {
		endpoint: record.endpoint.to_vec(),
		organization: storage_api::OrganizationInfo {
			entity_id: record.organization.entity_id.to_vec(),
			attestation_id: record.organization.attestation_id,
			schema_id: record.organization.schema_id,
			sla_commitment: record.organization.sla_commitment,
			sla_version: record.organization.sla_version,
			valid_from: record.organization.valid_from,
			valid_until: record.organization.valid_until,
			rotation_predecessor: record.organization.rotation_predecessor,
		},
		service_key: storage_api::ServiceKeyInfo {
			active: record.service_key.active.0,
			active_version: record.service_key.active_version,
			previous: record.service_key.previous.map(|key| key.0),
			pending: record.service_key.pending.map(|key| key.0),
			pending_version: record.service_key.pending_version,
			pending_effective_at: record.service_key.pending_effective_at,
		},
		capacity_bytes: record.capacity_bytes,
		allocated_bytes: record.allocated_bytes,
		pending_bytes: record.pending_bytes,
		status: match record.status {
			pallet_orbis_storage_provider::ProviderStatus::Active => {
				storage_api::ProviderStatus::Active
			},
			pallet_orbis_storage_provider::ProviderStatus::Suspended => {
				storage_api::ProviderStatus::Suspended
			},
		},
		last_heartbeat: record.last_heartbeat,
		overdue_challenges: pallet_orbis_storage_provider::OverdueChallenges::<Runtime>::get(
			provider,
		),
		authority_validated_at: record.authority_validated_at,
	}
}

fn control_bucket_api_info(
	bucket_id: Hash,
	record: pallet_orbis_storage_provider::BucketRecordOf<Runtime>,
) -> storage_api::ControlBucketInfo<AccountId, Hash, BlockNumber> {
	storage_api::ControlBucketInfo {
		bucket_id,
		owner: record.owner,
		version: record.version,
		policy: record.policy,
		primary: record.primary,
		replicas: record.replicas.to_vec(),
		grants: record
			.grants
			.into_iter()
			.map(|grant| storage_api::BucketGrantInfo {
				account: grant.account,
				role: match grant.role {
					pallet_orbis_storage_provider::BucketRole::Reader => {
						storage_api::BucketRole::Reader
					},
					pallet_orbis_storage_provider::BucketRole::Writer => {
						storage_api::BucketRole::Writer
					},
					pallet_orbis_storage_provider::BucketRole::Admin => {
						storage_api::BucketRole::Admin
					},
				},
			})
			.collect(),
		created_at: record.created_at,
	}
}

fn host_delegation_api_info(
	grant_id: Hash,
	record: pallet_orbis_storage_provider::HostDelegationRecordOf<Runtime>,
) -> storage_api::HostDelegationInfo<AccountId, Hash, BlockNumber> {
	storage_api::HostDelegationInfo {
		grant_id,
		bucket_id: record.bucket_id,
		owner: record.owner,
		issuance_nonce: record.issuance_nonce,
		issuer_key_id: record.issuer_key_id,
		issuer_public_key: record.issuer_public_key.0,
		key_version: record.key_version,
		state_version: record.state_version,
		key_activated_at: record.key_activated_at,
		product_id: record.product_id.to_vec(),
		methods: record.methods.to_vec(),
		cid: record.cid.map(|cid| cid.to_vec()),
		max_bytes: record.max_bytes,
		issued_at: record.issued_at,
		expires_at: record.expires_at,
		revoked_at: record.revoked_at,
	}
}

fn capability_authority_info(
	grant_id: Hash,
) -> storage_api::Versioned<storage_api::HostDelegationInfo<AccountId, Hash, BlockNumber>> {
	storage_api::Versioned::new(
		pallet_orbis_storage_provider::HostDelegations::<Runtime>::get(grant_id)
			.map(|record| host_delegation_api_info(grant_id, record)),
	)
}

fn agreement_api_info(
	agreement_id: Hash,
	record: pallet_orbis_storage_provider::AgreementRecordOf<Runtime>,
) -> storage_api::AgreementInfo<AccountId, Hash, BlockNumber> {
	storage_api::AgreementInfo {
		agreement_id,
		owner: record.owner,
		bucket_id: record.bucket_id,
		primary: record.primary,
		replicas: record.replicas.to_vec(),
		bytes: record.bytes,
		created_at: record.created_at,
		expires_at: record.expires_at,
		release_at: record.release_at,
		state_version: record.version,
		status: match record.status {
			pallet_orbis_storage_provider::AgreementStatus::Proposed => {
				storage_api::AgreementStatus::Proposed
			},
			pallet_orbis_storage_provider::AgreementStatus::Active => {
				storage_api::AgreementStatus::Active
			},
			pallet_orbis_storage_provider::AgreementStatus::Suspended => {
				storage_api::AgreementStatus::Suspended
			},
			pallet_orbis_storage_provider::AgreementStatus::Cancelled => {
				storage_api::AgreementStatus::Cancelled
			},
			pallet_orbis_storage_provider::AgreementStatus::Expired => {
				storage_api::AgreementStatus::Expired
			},
		},
	}
}

fn challenge_api_info(
	challenge_id: Hash,
	record: pallet_orbis_storage_provider::ChallengeRecordOf<Runtime>,
) -> storage_api::ChallengeInfo<AccountId, Hash, BlockNumber> {
	storage_api::ChallengeInfo {
		challenge_id,
		bucket_id: record.bucket_id,
		provider: record.provider,
		expected_commitment: storage_api::CommitmentInfo {
			mmr_root: record.expected_commitment.mmr_root,
			start_seq: record.expected_commitment.start_seq,
			leaf_count: record.expected_commitment.leaf_count,
		},
		location: storage_api::ChunkLocationInfo {
			leaf_index: record.location.leaf_index,
			chunk_index: record.location.chunk_index,
		},
		due_at: record.due_at,
		status: match record.status {
			pallet_orbis_storage_provider::ChallengeStatus::Open => {
				storage_api::ChallengeStatus::Open
			},
			pallet_orbis_storage_provider::ChallengeStatus::Proved => {
				storage_api::ChallengeStatus::Proved
			},
			pallet_orbis_storage_provider::ChallengeStatus::TimedOut => {
				storage_api::ChallengeStatus::TimedOut
			},
		},
	}
}

fn drive_api_info(
	drive_id: Hash,
	record: pallet_orbis_drive::DriveRecordOf<Runtime>,
) -> storage_api::DriveInfo<AccountId, Hash, BlockNumber> {
	storage_api::DriveInfo {
		drive_id,
		owner: record.owner,
		name: record.name.to_vec(),
		root_manifest: record.root_manifest,
		root_provider_commitment: record.root_provider_commitment,
		version: record.version,
		status: match record.status {
			pallet_orbis_drive::DriveStatus::Active => storage_api::ContainerStatus::Active,
			pallet_orbis_drive::DriveStatus::Archived => storage_api::ContainerStatus::Archived,
		},
		created_at: record.created_at,
		updated_at: record.updated_at,
		controllers: pallet_orbis_drive::DriveGrants::<Runtime>::get(drive_id)
			.into_iter()
			.map(|grant| grant.subject)
			.collect(),
	}
}

fn bucket_api_info(
	bucket_id: Hash,
	record: pallet_orbis_s3::BucketRecord<Runtime>,
) -> storage_api::BucketInfo<AccountId, Hash, BlockNumber> {
	storage_api::BucketInfo {
		bucket_id,
		name: record.name.to_vec(),
		owner: record.owner,
		controllers: record.controllers.to_vec(),
		status: match record.status {
			pallet_orbis_s3::BucketStatus::Active => storage_api::ContainerStatus::Active,
			pallet_orbis_s3::BucketStatus::Archived => storage_api::ContainerStatus::Archived,
			pallet_orbis_s3::BucketStatus::Deleted => storage_api::ContainerStatus::Deleted,
		},
		versioning_enabled: record.versioning_enabled,
		version: record.version,
		live_objects: record.live_objects,
		created_at: record.created_at,
		updated_at: record.updated_at,
	}
}

fn object_api_info(
	bucket_id: Hash,
	key: Vec<u8>,
	record: pallet_orbis_s3::ObjectRecord<Runtime>,
) -> storage_api::ObjectInfo<AccountId, Hash, BlockNumber> {
	storage_api::ObjectInfo {
		object_id: record.object_id,
		bucket_id,
		key,
		content_hash: record.content_hash,
		provider_commitment: record.provider_commitment,
		version: record.version,
		deleted: record.deleted,
		updated_by: record.updated_by,
		updated_at: record.updated_at,
	}
}

fn api_page_bounds(len: usize, cursor: Option<u32>, limit: u32) -> (usize, usize, Option<u32>) {
	let start = (cursor.unwrap_or(0) as usize).min(len);
	let end = start
		.saturating_add(limit.clamp(1, storage_api::MAX_PAGE_SIZE) as usize)
		.min(len);
	(start, end, (end < len).then_some(end as u32))
}

fn checkpoint_duty_page(
	provider: AccountId,
	cursor: Option<storage_api::CheckpointDutyCursor<BlockNumber>>,
	limit: u32,
) -> Result<
	storage_api::CheckpointDutyPage<
		storage_api::CheckpointDutyInfo<AccountId, Hash, BlockNumber>,
		BlockNumber,
	>,
	storage_api::CheckpointDutyPageError,
> {
	if limit == 0 || limit > storage_api::MAX_CHECKPOINT_DUTY_PAGE_SIZE {
		return Err(storage_api::CheckpointDutyPageError::PageLimitInvalid);
	}
	let snapshot_checkpoint =
		pallet_orbis_storage_provider::GovernedFinalizedCheckpoint::<Runtime>::get()
			.ok_or(storage_api::CheckpointDutyPageError::FinalizedCheckpointUnavailable)?;
	let mut duties = pallet_orbis_storage_provider::BucketIds::<Runtime>::get()
		.into_iter()
		.filter_map(|bucket_id| {
			let duty = pallet_orbis_storage_provider::Pallet::<Runtime>::checkpoint_duty_at(
				bucket_id,
				snapshot_checkpoint,
			)?;
			if duty.primary != provider && !duty.replicas.contains(&provider) {
				return None;
			}
			Some(duty)
		})
		.collect::<Vec<_>>();
	duties.sort_by(|left, right| left.bucket_id.encode().cmp(&right.bucket_id.encode()));
	let start = match cursor {
		None => 0,
		Some(cursor) => {
			if cursor.snapshot_checkpoint != snapshot_checkpoint {
				return Err(storage_api::CheckpointDutyPageError::CursorSnapshotStale);
			}
			duties
				.iter()
				.position(|duty| duty.bucket_id.encode() == cursor.last_key)
				.map(|index| index.saturating_add(1))
				.ok_or(storage_api::CheckpointDutyPageError::CursorKeyInvalid)?
		},
	};
	let end = core::cmp::min(start.saturating_add(limit as usize), duties.len());
	let items = duties[start..end]
		.iter()
		.filter_map(|duty| {
			let has_previous = duty.previous_commitment.is_some();
			let authority = |member: &AccountId, role: storage_api::ProviderDutyRole, order: u8| {
				let confirmed_checkpoint = pallet_orbis_storage_provider::ReplicaCheckpoint::<
					Runtime,
				>::get(duty.bucket_id, member);
				let Some(record) = pallet_orbis_storage_provider::Providers::<Runtime>::get(member)
				else {
					return Some(storage_api::ProviderDutyAuthority {
						provider: member.clone(),
						role,
						order,
						active_service_key_version: 0,
						active_service_key: [0; 32],
						endpoint_hash: Hash::zero(),
						organization_sla_eligible: false,
						overdue_challenge: false,
						eligible: false,
						may_sign: false,
						may_initiate: false,
						exclusion: Some(storage_api::ProviderDutyExclusion::MissingProvider),
						initiation_exclusion: (role == storage_api::ProviderDutyRole::Replica)
							.then_some(
								storage_api::ProviderDutyExclusion::ReplicaCheckpointMissingOrStale,
							),
						confirmed_checkpoint,
					})
				};
				let pending_is_active = record
					.service_key
					.pending_effective_at
					.is_some_and(|at| at <= snapshot_checkpoint) &&
					record.service_key.pending.is_some();
				let active_key = if pending_is_active {
					record.service_key.pending.unwrap()
				} else {
					record.service_key.active
				};
				let active_version = if pending_is_active {
					record
						.service_key
						.pending_version
						.unwrap_or_else(|| record.service_key.active_version.saturating_add(1))
				} else {
					record.service_key.active_version
				};
				let authority_error =
					<OrbisProviderAuthority as pallet_orbis_storage_provider::ProviderAuthority<
						AccountId,
						pallet_orbis_storage_provider::OrganizationRefOf<Runtime>,
						sp_core::ed25519::Public,
						BlockNumber,
					>>::validate(
						member, &record.organization, &active_key, snapshot_checkpoint
					)
					.err();
				let organization_sla_eligible = authority_error.is_none();
				let overdue_challenge =
					pallet_orbis_storage_provider::OverdueChallenges::<Runtime>::get(member) > 0;
				let exclusion =
					if record.status != pallet_orbis_storage_provider::ProviderStatus::Active {
						Some(storage_api::ProviderDutyExclusion::Inactive)
					} else if let Some(error) = authority_error {
						Some(match error {
						pallet_orbis_storage_provider::ProviderAuthorityError::OrganizationUnknown =>
							storage_api::ProviderDutyExclusion::OrganizationUnknown,
						pallet_orbis_storage_provider::ProviderAuthorityError::AttestationInvalid =>
							storage_api::ProviderDutyExclusion::AttestationInvalid,
						pallet_orbis_storage_provider::ProviderAuthorityError::AttestationExpired =>
							storage_api::ProviderDutyExclusion::AttestationExpired,
						pallet_orbis_storage_provider::ProviderAuthorityError::SlaInvalid =>
							storage_api::ProviderDutyExclusion::SlaInvalid,
						pallet_orbis_storage_provider::ProviderAuthorityError::ServiceKeyInvalid =>
							storage_api::ProviderDutyExclusion::ServiceKeyInvalid,
					})
					} else if overdue_challenge {
						Some(storage_api::ProviderDutyExclusion::OverdueChallenge)
					} else {
						None
					};
				let initiation_exclusion = if role == storage_api::ProviderDutyRole::Replica &&
					has_previous && confirmed_checkpoint != Some(duty.previous_checkpoint)
				{
					Some(storage_api::ProviderDutyExclusion::ReplicaCheckpointMissingOrStale)
				} else {
					None
				};
				Some(storage_api::ProviderDutyAuthority {
					provider: member.clone(),
					role,
					order,
					active_service_key_version: active_version,
					active_service_key: active_key.0,
					endpoint_hash: Hash::from(sp_io::hashing::blake2_256(
						record.endpoint.as_slice(),
					)),
					organization_sla_eligible,
					overdue_challenge,
					eligible: exclusion.is_none(),
					may_sign: exclusion.is_none(),
					may_initiate: false,
					exclusion,
					initiation_exclusion,
					confirmed_checkpoint,
				})
			};
			let mut authorities = Vec::with_capacity(duty.replicas.len().saturating_add(1));
			authorities.push(authority(&duty.primary, storage_api::ProviderDutyRole::Primary, 0)?);
			for (index, replica) in duty.replicas.iter().enumerate() {
				authorities.push(authority(
					replica,
					storage_api::ProviderDutyRole::Replica,
					index.saturating_add(1) as u8,
				)?);
			}
			let (phase, initiator) = if snapshot_checkpoint < duty.due_at {
				(storage_api::CheckpointDutyPhase::NotDue, None)
			} else if duty.mode ==
				pallet_orbis_storage_provider::CheckpointDutyMode::PromotionPending
			{
				let primary = authorities
					.first()
					.filter(|candidate| candidate.eligible)
					.map(|candidate| candidate.provider.clone());
				if primary.is_none() {
					(storage_api::CheckpointDutyPhase::Unavailable, None)
				} else if authorities
					.iter()
					.skip(1)
					.filter(|candidate| candidate.eligible)
					.count() < 2
				{
					(storage_api::CheckpointDutyPhase::BlockedInsufficientFallbackQuorum, None)
				} else {
					(storage_api::CheckpointDutyPhase::Primary, primary)
				}
			} else if snapshot_checkpoint < duty.grace_until {
				let primary = authorities
					.first()
					.filter(|candidate| candidate.eligible)
					.map(|candidate| candidate.provider.clone());
				let phase = if primary.is_some() {
					storage_api::CheckpointDutyPhase::Primary
				} else {
					storage_api::CheckpointDutyPhase::Unavailable
				};
				(phase, primary)
			} else {
				let mut candidates = authorities
					.iter()
					.filter(|candidate| {
						candidate.role == storage_api::ProviderDutyRole::Replica &&
							candidate.eligible && candidate.initiation_exclusion.is_none()
					})
					.collect::<Vec<_>>();
				candidates.sort_by(|left, right| {
					right
						.confirmed_checkpoint
						.cmp(&left.confirmed_checkpoint)
						.then_with(|| left.provider.encode().cmp(&right.provider.encode()))
				});
				let fallback = candidates.first().map(|candidate| candidate.provider.clone());
				let phase = fallback.as_ref().map_or(
					storage_api::CheckpointDutyPhase::Unavailable,
					|fallback| {
						if authorities
							.iter()
							.filter(|candidate| {
								candidate.eligible && &candidate.provider != fallback
							})
							.count() >= 2
						{
							storage_api::CheckpointDutyPhase::ReplicaFallback
						} else {
							storage_api::CheckpointDutyPhase::ReplicaFallbackPromotion
						}
					},
				);
				(phase, fallback)
			};
			if matches!(
				phase,
				storage_api::CheckpointDutyPhase::Primary |
					storage_api::CheckpointDutyPhase::ReplicaFallback |
					storage_api::CheckpointDutyPhase::ReplicaFallbackPromotion
			) {
				if let Some(initiator) = initiator.as_ref() {
					if let Some(view) =
						authorities.iter_mut().find(|candidate| &candidate.provider == initiator)
					{
						view.may_initiate = true;
					}
				}
			}
			let previous_commitment =
				duty.previous_commitment.map(|commitment| storage_api::CommitmentInfo {
					mmr_root: commitment.mmr_root,
					start_seq: commitment.start_seq,
					leaf_count: commitment.leaf_count,
				});
			let expected_next_start_seq = duty.expected_next_start_seq;
			let genesis_hash = <Runtime as pallet_orbis_storage_provider::Config>::CheckpointContext::genesis_hash();
			let snapshot_hash = <Runtime as pallet_orbis_storage_provider::Config>::CheckpointContext::block_hash(snapshot_checkpoint);
			let metadata_hash = <Runtime as pallet_orbis_storage_provider::Config>::CheckpointContext::metadata_hash();
			let duty_id = pallet_orbis_storage_provider::Pallet::<Runtime>::checkpoint_duty_id(
				duty,
				snapshot_checkpoint,
			);
			Some(storage_api::CheckpointDutyInfo {
				response_version: storage_api::RESPONSE_VERSION,
				commons_genesis_hash: genesis_hash,
				commons_spec_version: VERSION.spec_version,
				commons_transaction_version: VERSION.transaction_version,
				commons_metadata_hash: metadata_hash,
				duty_id,
				bucket_id: duty.bucket_id,
				primary: duty.primary.clone(),
				replicas: duty.replicas.to_vec(),
				authorities,
				initiator,
				phase,
				mode: match duty.mode {
					pallet_orbis_storage_provider::CheckpointDutyMode::Standard =>
						storage_api::CheckpointDutyMode::Standard,
					pallet_orbis_storage_provider::CheckpointDutyMode::PromotionPending =>
						storage_api::CheckpointDutyMode::PromotionPending,
				},
				snapshot_checkpoint,
				snapshot_hash,
				due_at: duty.due_at,
				grace_until: duty.grace_until,
				expected_nonce: snapshot_checkpoint,
				scheduled_at: duty.scheduled_at,
				previous_checkpoint: duty.previous_commitment.map(|_| duty.previous_checkpoint),
				previous_commitment,
				expected_next_start_seq,
				required_primary_confirmations: 1,
				required_replica_confirmations: 2,
			})
		})
		.collect::<Vec<_>>();
	let next_cursor = if end < duties.len() {
		let last = &duties[end - 1];
		Some(storage_api::CheckpointDutyCursor {
			snapshot_checkpoint,
			last_key: last.bucket_id.encode(),
		})
	} else {
		None
	};
	Ok(storage_api::CheckpointDutyPage {
		version: storage_api::RESPONSE_VERSION,
		items,
		next_cursor,
		snapshot_checkpoint,
	})
}

fn deletion_duty_page(
	provider: AccountId,
	cursor: Option<storage_api::DeletionDutyCursor<BlockNumber>>,
	limit: u32,
) -> Result<
	storage_api::DeletionDutyPage<
		storage_api::DeletionDutyInfo<AccountId, Hash, BlockNumber>,
		BlockNumber,
	>,
	storage_api::DeletionDutyPageError,
> {
	if limit == 0 || limit > storage_api::MAX_DELETION_DUTY_PAGE_SIZE {
		return Err(storage_api::DeletionDutyPageError::PageLimitInvalid);
	}
	let snapshot_checkpoint =
		pallet_orbis_storage_provider::GovernedFinalizedCheckpoint::<Runtime>::get()
			.ok_or(storage_api::DeletionDutyPageError::FinalizedCheckpointUnavailable)?;
	let manifests = match cursor {
		None => pallet_orbis_storage_provider::ManifestDeletionDuties::<Runtime>::iter_key_prefix(
			&provider,
		)
		.take(limit.saturating_add(1) as usize)
		.collect::<Vec<_>>(),
		Some(cursor) => {
			if cursor.snapshot_checkpoint != snapshot_checkpoint {
				return Err(storage_api::DeletionDutyPageError::CursorSnapshotStale);
			}
			if !pallet_orbis_storage_provider::ManifestDeletionDuties::<Runtime>::contains_key(
				&provider,
				cursor.last_manifest,
			) {
				return Err(storage_api::DeletionDutyPageError::CursorKeyInvalid);
			}
			let raw_key =
				pallet_orbis_storage_provider::ManifestDeletionDuties::<Runtime>::hashed_key_for(
					&provider,
					cursor.last_manifest,
				);
			pallet_orbis_storage_provider::ManifestDeletionDuties::<Runtime>::iter_key_prefix_from(
				&provider, raw_key,
			)
			.take(limit.saturating_add(1) as usize)
			.collect::<Vec<_>>()
		},
	};
	let has_more = manifests.len() > limit as usize;
	let items = manifests
		.into_iter()
		.take(limit as usize)
		.map(|manifest| {
			let record =
				pallet_orbis_storage_provider::CanonicalManifests::<Runtime>::get(manifest)
					.ok_or(storage_api::DeletionDutyPageError::CursorKeyInvalid)?;
			let provider_commitment = record
				.provider_commitment
				.ok_or(storage_api::DeletionDutyPageError::CursorKeyInvalid)?;
			let tombstoned_at = record
				.tombstoned_at
				.ok_or(storage_api::DeletionDutyPageError::CursorKeyInvalid)?;
			Ok(storage_api::DeletionDutyInfo {
				provider: provider.clone(),
				manifest,
				bucket_id: record.bucket_id,
				provider_commitment,
				tombstoned_at,
			})
		})
		.collect::<Result<Vec<_>, storage_api::DeletionDutyPageError>>()?;
	let next_cursor = if has_more {
		Some(storage_api::DeletionDutyCursor {
			snapshot_checkpoint,
			last_manifest: items
				.last()
				.ok_or(storage_api::DeletionDutyPageError::CursorKeyInvalid)?
				.manifest,
		})
	} else {
		None
	};
	Ok(storage_api::DeletionDutyPage {
		version: storage_api::RESPONSE_VERSION,
		items,
		next_cursor,
		snapshot_checkpoint,
	})
}

fn attestation_id_page(
	ids: &[Hash],
	cursor: Option<u32>,
	limit: u32,
) -> attestation_api::IdPage<Hash> {
	if limit == 0 {
		return attestation_api::IdPage::new(Default::default(), None);
	}
	let start = (cursor.unwrap_or(0) as usize).min(ids.len());
	let end = start
		.saturating_add(limit.min(attestation_api::MAX_PAGE_SIZE) as usize)
		.min(ids.len());
	let items = ids[start..end].to_vec().try_into().unwrap_or_default();
	let next = (end < ids.len() && end > start).then_some(end as u32);
	attestation_api::IdPage::new(items, next)
}

pallet_revive::impl_runtime_apis_plus_revive_traits!(
	Runtime,
	Revive,
	Executive,
	EthExtraImpl,

	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(SLOT_DURATION)
		}

		fn authorities() -> Vec<AuraId> {
			pallet_aura::Authorities::<Runtime>::get().into_inner()
		}
	}

	impl cumulus_primitives_core::RelayParentOffsetApi<Block> for Runtime {
		fn relay_parent_offset() -> u32 {
			RELAY_PARENT_OFFSET
		}

		fn max_claim_queue_offset() -> u8 {
			cumulus_pallet_parachain_system::Pallet::<Runtime>::max_claim_queue_offset()
		}
	}

	impl cumulus_primitives_core::SchedulingV3EnabledApi<Block> for Runtime {
		fn scheduling_v3_enabled() -> bool {
			<Runtime as cumulus_pallet_parachain_system::Config>::SchedulingSignatureVerifier::V3_SCHEDULING_ENABLED
		}
	}

	impl cumulus_primitives_core::TargetBlockRate<Block> for Runtime {
		fn target_block_rate() -> u32 {
			BLOCK_PROCESSING_VELOCITY
		}
	}

	impl cumulus_primitives_aura::AuraUnincludedSegmentApi<Block> for Runtime {
		fn can_build_upon(
			included_hash: <Block as BlockT>::Hash,
			slot: cumulus_primitives_aura::Slot,
		) -> bool {
			ConsensusHook::can_build_upon(included_hash, slot)
		}
	}

	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: <Block as BlockT>::LazyBlock) {
			Executive::execute_block(block)
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

		fn metadata_versions() -> Vec<u32> {
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

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: <Block as BlockT>::LazyBlock,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
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

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(owner: Vec<u8>, seed: Option<Vec<u8>>) -> sp_session::OpaqueGeneratedSessionKeys {
			SessionKeys::generate(&owner, seed).into()
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_support::view_functions::runtime_api::RuntimeViewFunction<Block> for Runtime {
		fn execute_view_function(
			id: frame_support::view_functions::ViewFunctionId,
			input: Vec<u8>
		) -> Result<Vec<u8>, frame_support::view_functions::ViewFunctionDispatchError> {
			Runtime::execute_view_function(id, input)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl token_api::TokenOriginCommonsRuntimeApi<Block> for Runtime {
		fn decode_token(token: Vec<u8>) -> Option<token_api::DecodedTokenApi> {
			let ss58_id = Ss58Identifier::try_from(token).ok()?;
			let decoded: DecodedIdentifier = <pallet_origin_token::Pallet<Runtime> as TokenTrait<Runtime>>::resolve_token(&ss58_id).ok()?;
			Some(token_api::DecodedTokenApi {
				origin: decoded.origin,
				network: decoded.network,
				pallet: decoded.pallet,
				genesis: decoded.genesis,
			})
		}

		fn resolve_pallet(index: u16) -> Option<String> {
			Token::resolve_pallet_plain(index).ok()
		}

		fn token_status(token: Vec<u8>) -> token_api::TokenStatusApi {
			let ss58_id = match Ss58Identifier::try_from(token) {
				Ok(id) => id,
				Err(_) => return token_api::TokenStatusApi::InvalidToken,
			};
			let decoded = match Token::resolve_identifier_plain(&ss58_id) {
				Ok(info) => info,
				Err(_) => return token_api::TokenStatusApi::InvalidToken,
			};
			if !decoded.origin || decoded.network != Token::get_network_id() {
				return token_api::TokenStatusApi::WrongChain;
			}
			if Token::resolve_pallet_plain(decoded.pallet).is_err() {
				return token_api::TokenStatusApi::PalletNotFound;
			}
			if !pallet_origin_token::StateVersion::<Runtime>::contains_key(&ss58_id) {
				return token_api::TokenStatusApi::TokenNotFound;
			}
			let version = pallet_origin_token::StateVersion::<Runtime>::get(&ss58_id);
			let last_state = version.checked_sub(1);
			token_api::TokenStatusApi::Found { last_state }
		}
	}

	impl identity_personhood_api::IdentityPersonhoodApi<Block, AccountId> for Runtime {
		fn identity_status(
			account: AccountId,
		) -> identity_personhood_api::Versioned<identity_personhood_api::IdentityStatus> {
			let counts = People::identity_judgement_counts(&account);
			let (judgement_count, requested, reasonable, known_good, out_of_date, low_quality, erroneous) =
				counts.unwrap_or_default();
			identity_personhood_api::Versioned::new(identity_personhood_api::IdentityStatus {
				registered: counts.is_some(),
				judgement_count,
				requested,
				reasonable,
				known_good,
				out_of_date,
				low_quality,
				erroneous,
			})
		}

		fn personhood_status(
			account: AccountId,
		) -> identity_personhood_api::Versioned<identity_personhood_api::PersonhoodStatus> {
			let full_personal_id = indiv_pallet_people::AccountToPersonalId::<Runtime>::get(&account);
			let full_recognized = full_personal_id
				.is_some_and(indiv_pallet_people::People::<Runtime>::contains_key);
			let lite_recognized = indiv_pallet_people_lite::LitePeople::<Runtime>::contains_key(account);
			identity_personhood_api::Versioned::new(
				identity_personhood_api::PersonhoodStatus {
					full_personal_id,
					full_recognized,
					lite_recognized,
				},
			)
		}

		fn attestation_allowance(
			account: AccountId,
		) -> identity_personhood_api::Versioned<identity_personhood_api::AttestationAllowance> {
			identity_personhood_api::Versioned::new(
				identity_personhood_api::AttestationAllowance {
					remaining: indiv_pallet_people_lite::AttestationAllowance::<Runtime>::get(account),
				},
			)
		}
	}


	impl attestation_api::AttestationApi<Block, AccountId, BlockNumber, Hash> for Runtime {
		fn schema_by_id(
			schema: Hash,
		) -> attestation_api::Versioned<attestation_api::SchemaView<AccountId, BlockNumber, Hash>> {
			let value = pallet_orbis_attestation::Schemas::<Runtime>::get(schema).and_then(|record| {
				Some(attestation_api::SchemaView {
					schema,
					creator: record.creator,
					definition: record.definition.to_vec().try_into().ok()?,
					definition_commitment: record.definition_commitment,
					status: match record.status {
						pallet_orbis_attestation::SchemaStatus::Active =>
							attestation_api::SchemaStatus::Active,
						pallet_orbis_attestation::SchemaStatus::Paused =>
							attestation_api::SchemaStatus::Paused,
						pallet_orbis_attestation::SchemaStatus::Retired =>
							attestation_api::SchemaStatus::Retired,
					},
					revocable: record.revocable,
					unique: record.unique,
					index_policy: match record.index_policy {
						pallet_orbis_attestation::IndexPolicy::None =>
							attestation_api::IndexPolicy::None,
						pallet_orbis_attestation::IndexPolicy::Issuer =>
							attestation_api::IndexPolicy::Issuer,
						pallet_orbis_attestation::IndexPolicy::SubjectAndSchema =>
							attestation_api::IndexPolicy::SubjectAndSchema,
						pallet_orbis_attestation::IndexPolicy::IssuerAndSubjectSchema =>
							attestation_api::IndexPolicy::IssuerAndSubjectSchema,
					},
					authorized_issuers: record.authorized_issuers.to_vec().try_into().ok()?,
					created_at: record.created_at,
				})
			});
			attestation_api::Versioned::new(value)
		}

		fn attestation_by_id(
			attestation: Hash,
		) -> attestation_api::Versioned<attestation_api::AttestationView<AccountId, BlockNumber, Hash>> {
			attestation_api::Versioned::new(
				pallet_orbis_attestation::Attestations::<Runtime>::get(attestation).map(|record| {
					attestation_api::AttestationView {
						attestation,
						issuer: record.issuer,
						schema: record.schema,
						subject_commitment: record.subject_commitment,
						payload_commitment: record.payload_commitment,
						status_commitment: record.status_commitment,
						parent: record.parent,
						expiry: record.expiry,
						uniqueness_commitment: record.uniqueness_commitment,
						revocable: record.revocable,
						issuance_nonce: record.issuance_nonce,
						issued_at: record.issued_at,
						revoked_at: record.revoked_at,
						revoked_by: record.revoked_by,
					}
				}),
			)
		}

		fn attestation_live_status(
			attestation: Hash,
		) -> attestation_api::AttestationLiveStatus<BlockNumber> {
			let now = System::block_number();
			match pallet_orbis_attestation::Attestations::<Runtime>::get(attestation) {
				Some(record) => {
					let live = pallet_orbis_attestation::Pallet::<Runtime>::is_live(attestation);
					attestation_api::AttestationLiveStatus::new(
						true,
						live,
						now,
						record.expiry,
						record.revoked_at,
					)
				},
				None => attestation_api::AttestationLiveStatus::new(false, false, now, None, None),
			}
		}

		fn creator_schemas(
			creator: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> attestation_api::IdPage<Hash> {
			let ids = pallet_orbis_attestation::CreatorSchemas::<Runtime>::get(creator);
			attestation_id_page(ids.as_slice(), cursor, limit)
		}

		fn issuer_attestations(
			issuer: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> attestation_api::IdPage<Hash> {
			let ids = pallet_orbis_attestation::IssuerAttestations::<Runtime>::get(issuer);
			attestation_id_page(ids.as_slice(), cursor, limit)
		}

		fn subject_schema_attestations(
			subject_commitment: Hash,
			schema: Hash,
			cursor: Option<u32>,
			limit: u32,
		) -> attestation_api::IdPage<Hash> {
			let ids = pallet_orbis_attestation::SubjectSchemaAttestations::<Runtime>::get(
				schema,
				subject_commitment,
			);
			attestation_id_page(ids.as_slice(), cursor, limit)
		}

		fn next_delegated_nonce(issuer: AccountId) -> attestation_api::DelegatedNonce {
			attestation_api::DelegatedNonce::new(
				pallet_orbis_attestation::NextDelegatedNonce::<Runtime>::get(issuer),
			)
		}

		fn schema_count() -> attestation_api::RegistryCount {
			attestation_api::RegistryCount::new(
				pallet_orbis_attestation::SchemaCount::<Runtime>::get(),
			)
		}

		fn attestation_count() -> attestation_api::RegistryCount {
			attestation_api::RegistryCount::new(
				pallet_orbis_attestation::AttestationCount::<Runtime>::get(),
			)
		}

		fn next_issuance_nonce(issuer: AccountId) -> attestation_api::IssuanceNonce {
			attestation_api::IssuanceNonce::new(
				pallet_orbis_attestation::NextIssuerAttestationNonce::<Runtime>::get(issuer),
			)
		}

		fn external_status(
			issuer: AccountId,
			status_commitment: Hash,
		) -> attestation_api::Versioned<
			attestation_api::ExternalStatusView<AccountId, BlockNumber, Hash>,
		> {
			let key = pallet_orbis_attestation::Pallet::<Runtime>::external_status_key(
				&issuer,
				status_commitment,
			);
			attestation_api::Versioned::new(
				pallet_orbis_attestation::ExternalStatuses::<Runtime>::get(key).map(|record| {
					attestation_api::ExternalStatusView {
						key,
						issuer: record.issuer,
						status_commitment: record.status_commitment,
						revoked_at: record.revoked_at,
					}
				}),
			)
		}
	}

	impl names_api::NamesApi<Block, AccountId, BlockNumber, Hash, Ss58Identifier, Hash, [u8; 32]>
		for Runtime
	{
		fn label_policy_version() -> u16 {
			Names::label_policy_version()
		}

		fn name_by_id(name: Hash) -> names_api::Versioned<names_api::NameView<AccountId, BlockNumber, Hash>> {
			let value = pallet_orbis_names::Names::<Runtime>::get(name).and_then(|record| {
				let label = record.label.to_vec().try_into().ok()?;
				Some(names_api::NameView {
					name,
					parent: record.parent,
					label,
					owner: record.owner,
					expires_at: record.expires_at,
					depth: record.depth,
				})
			});
			names_api::Versioned::new(value)
		}

		fn root_name_by_normalized_label(label: names_api::NormalizedLabel) -> names_api::Versioned<Hash> {
			let value = Names::validate_label(label.to_vec()).ok().and_then(|label| {
				let name = Names::derive_name_id(None, &label);
				pallet_orbis_names::Names::<Runtime>::contains_key(name).then_some(name)
			});
			names_api::Versioned::new(value)
		}

		fn owner_names(
			owner: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> names_api::OwnerNamesPage<Hash> {
			let all = pallet_orbis_names::OwnerNames::<Runtime>::get(owner);
			if limit == 0 {
				return names_api::OwnerNamesPage::new(Default::default(), None);
			}
			let start = cursor.unwrap_or(0) as usize;
			let take = limit.min(names_api::MAX_OWNER_NAMES_PAGE_SIZE) as usize;
			let end = start.saturating_add(take).min(all.len());
			let values = if start < all.len() { all[start..end].to_vec() } else { Vec::new() };
			let names = values.try_into().unwrap_or_default();
			let next_cursor = (end < all.len()).then_some(end as u32);
			names_api::OwnerNamesPage::new(names, next_cursor)
		}

		fn controllers(name: Hash) -> names_api::Versioned<names_api::ControllerItems<AccountId>> {
			let value = pallet_orbis_names::Names::<Runtime>::contains_key(name).then(|| {
				pallet_orbis_names::Controllers::<Runtime>::get(name)
					.to_vec()
					.try_into()
					.expect("runtime controller bound is within API controller bound")
			});
			names_api::Versioned::new(value)
		}

		fn resolve_address(name: Hash) -> names_api::Versioned<names_api::Address> {
			let value = Names::is_name_active(name)
				.then(|| pallet_orbis_names::Names::<Runtime>::get(name))
				.flatten()
				.and_then(|record| record.address)
				.and_then(|address| address.to_vec().try_into().ok());
			names_api::Versioned::new(value)
		}

		fn resolve_subject(name: Hash) -> names_api::Versioned<Ss58Identifier> {
			let value = Names::is_name_active(name)
				.then(|| pallet_orbis_names::Names::<Runtime>::get(name))
				.flatten()
				.and_then(|record| record.subject);
			names_api::Versioned::new(value)
		}

		fn resolve_attestation(name: Hash) -> names_api::Versioned<Hash> {
			let value = Names::is_name_active(name)
				.then(|| pallet_orbis_names::Names::<Runtime>::get(name))
				.flatten()
				.and_then(|record| record.attestation)
				.filter(|attestation| Attestation::is_live(*attestation));
			names_api::Versioned::new(value)
		}

		fn resolve_content(name: Hash) -> names_api::Versioned<[u8; 32]> {
			let value = Names::is_name_active(name)
				.then(|| pallet_orbis_names::Names::<Runtime>::get(name))
				.flatten()
				.and_then(|record| record.content);
			names_api::Versioned::new(value)
		}

		fn resolve_text(name: Hash, key: names_api::TextKey) -> names_api::Versioned<names_api::TextValue> {
			let value = if Names::is_name_active(name) {
				pallet_orbis_names::TextKeyOf::<Runtime>::try_from(key.to_vec())
					.ok()
					.and_then(|key| pallet_orbis_names::TextRecords::<Runtime>::get(name, key))
					.and_then(|value| value.to_vec().try_into().ok())
			} else {
				None
			};
			names_api::Versioned::new(value)
		}

		fn primary_name(owner: AccountId) -> names_api::Versioned<Hash> {
			names_api::Versioned::new(Names::primary_name(&owner))
		}

		fn name_status(name: Hash) -> names_api::NameStatus<BlockNumber> {
			match pallet_orbis_names::Names::<Runtime>::get(name) {
				Some(record) => names_api::NameStatus {
					version: names_api::RESPONSE_VERSION,
					exists: true,
					active: Names::is_name_active(name),
					expires_at: Some(record.expires_at),
				},
				None => names_api::NameStatus {
					version: names_api::RESPONSE_VERSION,
					exists: false,
					active: false,
					expires_at: None,
				},
			}
		}
	}

	impl storage_api::StorageProviderApi<Block, AccountId, Hash, BlockNumber> for Runtime {
		fn provider(
			provider: AccountId,
		) -> storage_api::Versioned<storage_api::ProviderInfo<Hash, BlockNumber>> {
			storage_api::Versioned::new(
				pallet_orbis_storage_provider::Providers::<Runtime>::get(&provider)
					.map(|record| provider_api_info(&provider, record)),
			)
		}

		fn providers(
			cursor: Option<u32>,
			limit: u32,
		) -> storage_api::Page<(AccountId, storage_api::ProviderInfo<Hash, BlockNumber>)> {
			let ids = pallet_orbis_storage_provider::ProviderIds::<Runtime>::get();
			let (start, end, next) = api_page_bounds(ids.len(), cursor, limit);
			let items = ids[start..end]
				.iter()
				.filter_map(|id| {
					pallet_orbis_storage_provider::Providers::<Runtime>::get(id)
						.map(|record| (id.clone(), provider_api_info(id, record)))
				})
				.collect();
			storage_api::Page::new(items, next)
		}

		fn control_bucket(
			bucket_id: Hash,
		) -> storage_api::Versioned<storage_api::ControlBucketInfo<AccountId, Hash, BlockNumber>> {
			storage_api::Versioned::new(
				pallet_orbis_storage_provider::Buckets::<Runtime>::get(bucket_id)
					.map(|record| control_bucket_api_info(bucket_id, record)),
			)
		}

		fn control_buckets(
			cursor: Option<u32>,
			limit: u32,
		) -> storage_api::Page<storage_api::ControlBucketInfo<AccountId, Hash, BlockNumber>> {
			let ids = pallet_orbis_storage_provider::BucketIds::<Runtime>::get();
			let (start, end, next) = api_page_bounds(ids.len(), cursor, limit);
			let items = ids[start..end]
				.iter()
				.filter_map(|id| {
					pallet_orbis_storage_provider::Buckets::<Runtime>::get(id)
						.map(|record| control_bucket_api_info(*id, record))
				})
				.collect();
			storage_api::Page::new(items, next)
		}

		fn capability_authority(
			grant_id: Hash,
		) -> storage_api::Versioned<
			storage_api::HostDelegationInfo<AccountId, Hash, BlockNumber>,
		> {
			capability_authority_info(grant_id)
		}

		fn agreement(
			agreement_id: Hash,
		) -> storage_api::Versioned<storage_api::AgreementInfo<AccountId, Hash, BlockNumber>> {
			storage_api::Versioned::new(
				pallet_orbis_storage_provider::Agreements::<Runtime>::get(agreement_id)
					.map(|record| agreement_api_info(agreement_id, record)),
			)
		}

		fn provider_agreements(
			provider: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> storage_api::Page<storage_api::AgreementInfo<AccountId, Hash, BlockNumber>> {
			let ids = pallet_orbis_storage_provider::ProviderAgreements::<Runtime>::get(provider);
			let (start, end, next) = api_page_bounds(ids.len(), cursor, limit);
			let items = ids[start..end]
				.iter()
				.filter_map(|id| {
					pallet_orbis_storage_provider::Agreements::<Runtime>::get(id)
						.map(|record| agreement_api_info(*id, record))
				})
				.collect();
			storage_api::Page::new(items, next)
		}

		fn agreement_nonce(owner: AccountId) -> u64 {
			pallet_orbis_storage_provider::AgreementNonce::<Runtime>::get(owner)
		}

		fn challenge(
			challenge_id: Hash,
		) -> storage_api::Versioned<storage_api::ChallengeInfo<AccountId, Hash, BlockNumber>> {
			storage_api::Versioned::new(
				pallet_orbis_storage_provider::Challenges::<Runtime>::get(challenge_id)
					.map(|record| challenge_api_info(challenge_id, record)),
			)
		}

		fn challenges_at(
			block: BlockNumber,
			cursor: Option<u32>,
			limit: u32,
		) -> storage_api::Page<storage_api::ChallengeInfo<AccountId, Hash, BlockNumber>> {
			let ids = pallet_orbis_storage_provider::ChallengeBacklog::<Runtime>::get();
			let (start, end, next) = api_page_bounds(ids.len(), cursor, limit);
			let items = ids[start..end]
				.iter()
				.filter_map(|id| {
					pallet_orbis_storage_provider::Challenges::<Runtime>::get(id)
						.filter(|record| record.due_at == block)
						.map(|record| challenge_api_info(*id, record))
				})
				.collect();
			storage_api::Page::new(items, next)
		}

		fn checkpoint(
			bucket_id: Hash,
		) -> storage_api::Versioned<storage_api::CheckpointInfo<AccountId, Hash, BlockNumber>> {
			storage_api::Versioned::new(
				pallet_orbis_storage_provider::BucketSnapshots::<Runtime>::get(bucket_id).map(
					|snapshot| storage_api::CheckpointInfo {
						bucket_id,
						commitment: storage_api::CommitmentInfo {
							mmr_root: snapshot.commitment.mmr_root,
							start_seq: snapshot.commitment.start_seq,
							leaf_count: snapshot.commitment.leaf_count,
						},
						checkpoint_block: snapshot.checkpoint_block,
						primary_signers: snapshot.primary_signers,
						commitment_nonce: snapshot.commitment_nonce,
						replica_confirmations: snapshot.replica_confirmations.to_vec(),
					},
				),
			)
		}

		fn checkpoint_duties(
			provider: AccountId,
			cursor: Option<storage_api::CheckpointDutyCursor<BlockNumber>>,
			limit: u32,
		) -> Result<
			storage_api::CheckpointDutyPage<
				storage_api::CheckpointDutyInfo<AccountId, Hash, BlockNumber>,
				BlockNumber,
			>,
			storage_api::CheckpointDutyPageError,
		> {
			checkpoint_duty_page(provider, cursor, limit)
		}

		fn deletion_duties(
			provider: AccountId,
			cursor: Option<storage_api::DeletionDutyCursor<BlockNumber>>,
			limit: u32,
		) -> Result<
			storage_api::DeletionDutyPage<
				storage_api::DeletionDutyInfo<AccountId, Hash, BlockNumber>,
				BlockNumber,
			>,
			storage_api::DeletionDutyPageError,
		> {
			deletion_duty_page(provider, cursor, limit)
		}

		fn replica_checkpoint(bucket_id: Hash, provider: AccountId) -> Option<BlockNumber> {
			pallet_orbis_storage_provider::ReplicaCheckpoint::<Runtime>::get(bucket_id, provider)
		}

		fn canonical_manifest(
			manifest: [u8; 32],
		) -> storage_api::Versioned<storage_api::ManifestInfo<AccountId, Hash, BlockNumber>> {
			storage_api::Versioned::new(
				pallet_orbis_storage_provider::CanonicalManifests::<Runtime>::get(manifest).map(
					|record| {
						let mut deletion_acknowledged =
							pallet_orbis_storage_provider::ManifestDeletionAcknowledgements::<Runtime>::iter_prefix(
								manifest,
							)
							.map(|(provider, _)| provider)
							.collect::<Vec<_>>();
						deletion_acknowledged
							.sort_by(|left, right| left.encode().cmp(&right.encode()));
						storage_api::ManifestInfo {
						manifest,
						bucket_id: record.bucket_id,
						provider_commitment: record.provider_commitment,
						state: match record.state {
							pallet_orbis_storage_control_primitives::CommitmentState::Publishable =>
								storage_api::ManifestState::Publishable,
							pallet_orbis_storage_control_primitives::CommitmentState::Pending =>
								storage_api::ManifestState::Pending,
							pallet_orbis_storage_control_primitives::CommitmentState::Tombstoned =>
								storage_api::ManifestState::Tombstoned,
							pallet_orbis_storage_control_primitives::CommitmentState::Missing =>
								storage_api::ManifestState::Missing,
						},
						checkpoint: record.checkpoint,
						tombstoned_at: record.tombstoned_at,
						deletion_required:
							pallet_orbis_storage_provider::ManifestDeletionRequirements::<Runtime>::get(
								manifest,
							)
							.to_vec(),
						deletion_acknowledged,
						deletion_evidence_satisfied:
							<StorageProvider as pallet_orbis_storage_control_primitives::CanonicalStorageControl>::deletion_evidence_satisfied(
								&manifest,
							),
						}
					},
				),
			)
		}

		fn provider_evidence(
			provider: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> storage_api::Page<storage_api::EvidenceInfo<AccountId, Hash, BlockNumber>> {
			let evidence = pallet_orbis_storage_provider::ProviderEvidence::<Runtime>::get(provider);
			let (start, end, next) = api_page_bounds(evidence.len(), cursor, limit);
			let items = evidence[start..end]
				.iter()
				.map(|record| storage_api::EvidenceInfo {
					provider: record.provider.clone(),
					bucket_id: record.bucket_id,
					evidence_hash: record.evidence_hash,
					recorded_at: record.recorded_at,
				})
				.collect();
			storage_api::Page::new(items, next)
		}

		fn can_accept_capacity(provider: AccountId, additional_bytes: u64) -> bool {
			pallet_orbis_storage_provider::Providers::<Runtime>::get(provider).is_some_and(|record| {
				record.status == pallet_orbis_storage_provider::ProviderStatus::Active &&
					record.capacity_bytes.saturating_sub(
						record.allocated_bytes.saturating_add(record.pending_bytes),
					) >= additional_bytes
			})
		}

		fn provider_is_eligible(provider: AccountId) -> bool {
			let Some(finalized) =
				pallet_orbis_storage_provider::Pallet::<Runtime>::governed_finalized_checkpoint()
			else {
				return false
			};
			pallet_orbis_storage_provider::Providers::<Runtime>::get(&provider).is_some_and(
				|record| {
					record.status == pallet_orbis_storage_provider::ProviderStatus::Active &&
						record.authority_validated_at == Some(finalized) &&
						pallet_orbis_storage_provider::OverdueChallenges::<Runtime>::get(
							&provider,
						) == 0 &&
						finalized >= record.organization.valid_from &&
						finalized < record.organization.valid_until
				},
			)
		}

		fn governed_finalized_checkpoint() -> Option<BlockNumber> {
			pallet_orbis_storage_provider::Pallet::<Runtime>::governed_finalized_checkpoint()
		}
	}

	impl storage_api::DriveRegistryApi<Block, AccountId, Hash, BlockNumber> for Runtime {
		fn drive(
			drive_id: Hash,
		) -> storage_api::Versioned<storage_api::DriveInfo<AccountId, Hash, BlockNumber>> {
			storage_api::Versioned::new(
				pallet_orbis_drive::Drives::<Runtime>::get(drive_id)
					.map(|record| drive_api_info(drive_id, record)),
			)
		}

		fn drives(
			owner: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> storage_api::Page<storage_api::DriveInfo<AccountId, Hash, BlockNumber>> {
			let ids = pallet_orbis_drive::OwnerDrives::<Runtime>::get(owner);
			let (start, end, next) = api_page_bounds(ids.len(), cursor, limit);
			let items = ids[start..end]
				.iter()
				.filter_map(|id| {
					pallet_orbis_drive::Drives::<Runtime>::get(id)
						.map(|record| drive_api_info(*id, record))
				})
				.collect();
			storage_api::Page::new(items, next)
		}

		fn controllers(
			drive_id: Hash,
			cursor: Option<u32>,
			limit: u32,
		) -> storage_api::Page<AccountId> {
			let grants = pallet_orbis_drive::DriveGrants::<Runtime>::get(drive_id);
			let (start, end, next) = api_page_bounds(grants.len(), cursor, limit);
			storage_api::Page::new(
				grants[start..end].iter().map(|grant| grant.subject.clone()).collect(),
				next,
			)
		}

		fn next_drive_nonce(owner: AccountId) -> u64 {
			pallet_orbis_drive::DriveNonce::<Runtime>::get(owner)
		}

		fn is_drive_owner(owner: AccountId, drive_id: Hash) -> bool {
			pallet_orbis_drive::Drives::<Runtime>::get(drive_id)
				.is_some_and(|record| record.owner == owner)
		}
	}

	impl storage_api::S3RegistryApi<Block, AccountId, Hash, BlockNumber> for Runtime {
		fn bucket(
			bucket_id: Hash,
		) -> storage_api::Versioned<storage_api::BucketInfo<AccountId, Hash, BlockNumber>> {
			storage_api::Versioned::new(
				pallet_orbis_s3::Buckets::<Runtime>::get(bucket_id)
					.map(|record| bucket_api_info(bucket_id, record)),
			)
		}

		fn bucket_by_name(
			name: Vec<u8>,
		) -> storage_api::Versioned<storage_api::BucketInfo<AccountId, Hash, BlockNumber>> {
			let value = pallet_orbis_s3::BucketNameOf::<Runtime>::try_from(name)
				.ok()
				.and_then(pallet_orbis_s3::BucketByName::<Runtime>::get)
				.and_then(|bucket_id| {
					pallet_orbis_s3::Buckets::<Runtime>::get(bucket_id)
						.map(|record| bucket_api_info(bucket_id, record))
				});
			storage_api::Versioned::new(value)
		}

		fn buckets(
			owner: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> storage_api::Page<storage_api::BucketInfo<AccountId, Hash, BlockNumber>> {
			let ids = pallet_orbis_s3::OwnerBuckets::<Runtime>::get(owner);
			let (start, end, next) = api_page_bounds(ids.len(), cursor, limit);
			let items = ids[start..end]
				.iter()
				.filter_map(|id| {
					pallet_orbis_s3::Buckets::<Runtime>::get(id)
						.map(|record| bucket_api_info(*id, record))
				})
				.collect();
			storage_api::Page::new(items, next)
		}

		fn is_bucket_owner(owner: AccountId, bucket_id: Hash) -> bool {
			pallet_orbis_s3::Buckets::<Runtime>::get(bucket_id)
				.is_some_and(|record| record.owner == owner)
		}

		fn object(
			bucket_id: Hash,
			key: Vec<u8>,
		) -> storage_api::Versioned<storage_api::ObjectInfo<AccountId, Hash, BlockNumber>> {
			let value = pallet_orbis_s3::ObjectKeyOf::<Runtime>::try_from(key)
				.ok()
				.and_then(|key| {
					pallet_orbis_s3::Objects::<Runtime>::get(bucket_id, &key)
						.filter(|record| !record.deleted)
						.map(|record| object_api_info(bucket_id, key.to_vec(), record))
				});
			storage_api::Versioned::new(value)
		}

		fn object_keys(
			bucket_id: Hash,
			prefix: Option<Vec<u8>>,
			cursor: Option<storage_api::SnapshotCursor>,
			limit: u32,
		) -> Result<storage_api::SnapshotPage<Vec<u8>>, storage_api::S3ListError> {
			let bucket = pallet_orbis_s3::Buckets::<Runtime>::get(bucket_id)
				.ok_or(storage_api::S3ListError::BucketNotFound)?;
			if bucket.status == pallet_orbis_s3::BucketStatus::Deleted {
				return Err(storage_api::S3ListError::BucketDeleted)
			}
			if limit == 0 || limit > storage_api::MAX_PAGE_SIZE {
				return Err(storage_api::S3ListError::PageLimitInvalid)
			}
			let pallet_cursor = cursor
				.map(|cursor| {
					let last_key = pallet_orbis_s3::ObjectKeyCursor::try_from(cursor.last_key)
						.map_err(|_| storage_api::S3ListError::CursorKeyInvalid)?;
					Ok(pallet_orbis_s3::ListCursor {
						snapshot_version: cursor.snapshot_version,
						last_key,
					})
				})
				.transpose()?;
			if pallet_cursor
				.as_ref()
				.is_some_and(|cursor| cursor.snapshot_version != bucket.version)
			{
				return Err(storage_api::S3ListError::CursorStale)
			}
			let page = S3::list_objects(
				bucket_id,
				prefix.as_deref(),
				pallet_cursor,
				limit,
			)
			.map_err(|_| storage_api::S3ListError::BucketDeleted)?;
			Ok(storage_api::SnapshotPage {
				version: storage_api::RESPONSE_VERSION,
				items: page.objects.into_iter().map(|key| key.to_vec()).collect(),
				next_cursor: page.next_cursor.map(|cursor| storage_api::SnapshotCursor {
					snapshot_version: cursor.snapshot_version,
					last_key: cursor.last_key.to_vec(),
				}),
				snapshot_version: page.snapshot_version,
			})
		}

		fn object_history(
			bucket_id: Hash,
			key: Vec<u8>,
			cursor: Option<u32>,
			limit: u32,
		) -> storage_api::Page<storage_api::ObjectVersionInfo<AccountId, BlockNumber>> {
			let Some(key) = pallet_orbis_s3::ObjectKeyOf::<Runtime>::try_from(key).ok() else {
				return storage_api::Page::new(Vec::new(), None)
			};
			let history = pallet_orbis_s3::ObjectHistory::<Runtime>::get(bucket_id, key);
			let (start, end, next) = api_page_bounds(history.len(), cursor, limit);
			let items = history[start..end]
				.iter()
				.map(|version| storage_api::ObjectVersionInfo {
					content_hash: version.content_hash,
					provider_commitment: version.provider_commitment,
					version: version.version,
					deleted: version.deleted,
					updated_by: version.updated_by.clone(),
					updated_at: version.updated_at,
				})
				.collect();
			storage_api::Page::new(items, next)
		}

		fn object_id(bucket_id: Hash, key: Vec<u8>) -> storage_api::Versioned<Hash> {
			let value = pallet_orbis_s3::ObjectKeyOf::<Runtime>::try_from(key)
				.ok()
				.map(|key| S3::object_id(bucket_id, &key));
			storage_api::Versioned::new(value)
		}
	}

	impl xcm_runtime_apis::fees::XcmPaymentApi<Block> for Runtime {
		fn query_acceptable_payment_assets(xcm_version: xcm::Version) -> Result<Vec<VersionedAssetId>, XcmPaymentApiError> {
			let acceptable_assets = vec![AssetId(xcm_config::OrgnRelayLocation::get())];
			PolkadotXcm::query_acceptable_payment_assets(xcm_version, acceptable_assets)
		}

		fn query_weight_to_asset_fee(weight: Weight, asset: VersionedAssetId) -> Result<u128, XcmPaymentApiError> {
			use crate::xcm_config::XcmConfig;
			type Trader = <XcmConfig as xcm_executor::Config>::Trader;
			PolkadotXcm::query_weight_to_asset_fee::<Trader>(weight, asset)
		}

		fn query_xcm_weight(message: VersionedXcm<()>) -> Result<Weight, XcmPaymentApiError> {
			PolkadotXcm::query_xcm_weight(message)
		}

		fn query_delivery_fees(destination: VersionedLocation, message: VersionedXcm<()>, asset_id: VersionedAssetId) -> Result<VersionedAssets, XcmPaymentApiError> {
			type AssetExchanger = <xcm_config::XcmConfig as xcm_executor::Config>::AssetExchanger;
			PolkadotXcm::query_delivery_fees::<AssetExchanger>(destination, message, asset_id)
		}
	}

	impl xcm_runtime_apis::dry_run::DryRunApi<Block, RuntimeCall, RuntimeEvent, OriginCaller> for Runtime {
		fn dry_run_call(origin: OriginCaller, call: RuntimeCall, result_xcms_version: XcmVersion) -> Result<CallDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
			PolkadotXcm::dry_run_call::<Runtime, xcm_config::XcmRouter, OriginCaller, RuntimeCall>(origin, call, result_xcms_version)
		}

		fn dry_run_xcm(origin_location: VersionedLocation, xcm: VersionedXcm<RuntimeCall>) -> Result<XcmDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
			PolkadotXcm::dry_run_xcm::<xcm_config::XcmRouter>(origin_location, xcm)
		}
	}

	impl xcm_runtime_apis::conversions::LocationToAccountApi<Block, AccountId> for Runtime {
		fn convert_location(location: VersionedLocation) -> Result<
			AccountId,
			xcm_runtime_apis::conversions::Error
		> {
			xcm_runtime_apis::conversions::LocationToAccountHelper::<
				AccountId,
				xcm_config::LocationToAccountId,
			>::convert_location(location)
		}
	}

	impl xcm_runtime_apis::trusted_query::TrustedQueryApi<Block> for Runtime {
		fn is_trusted_reserve(asset: VersionedAsset, location: VersionedLocation) -> xcm_runtime_apis::trusted_query::XcmTrustedQueryResult {
			PolkadotXcm::is_trusted_reserve(asset, location)
		}
		fn is_trusted_teleporter(asset: VersionedAsset, location: VersionedLocation) -> xcm_runtime_apis::trusted_query::XcmTrustedQueryResult {
			PolkadotXcm::is_trusted_teleporter(asset, location)
		}
	}

	impl xcm_runtime_apis::authorized_aliases::AuthorizedAliasersApi<Block> for Runtime {
		fn authorized_aliasers(target: VersionedLocation) -> Result<
			Vec<xcm_runtime_apis::authorized_aliases::OriginAliaser>,
			xcm_runtime_apis::authorized_aliases::Error
		> {
			PolkadotXcm::authorized_aliasers(target)
		}
		fn is_authorized_alias(origin: VersionedLocation, target: VersionedLocation) -> Result<
			bool,
			xcm_runtime_apis::authorized_aliases::Error
		> {
			PolkadotXcm::is_authorized_alias(origin, target)
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	impl sp_transaction_storage_proof::runtime_api::TransactionStorageApi<Block> for Runtime {
		fn retention_period() -> BlockNumber {
			TransactionStorage::retention_period()
		}

		fn indexed_transactions(
			block: BlockNumber,
		) -> Vec<sp_transaction_storage_proof::IndexedTransactionInfo> {
			TransactionStorage::transactions_at(block)
				.map(|transactions| {
					transactions
						.into_iter()
						.map(|transaction| sp_transaction_storage_proof::IndexedTransactionInfo {
							content_hash: transaction.content_hash,
							size: transaction.size,
							hashing: transaction.hashing.into(),
							cid_codec: transaction.cid_codec,
							extrinsic_index: transaction.extrinsic_index,
						})
						.collect()
				})
				.unwrap_or_default()
		}
	}

	impl sp_hop::HopRuntimeApi<Block, AccountId> for Runtime {
		fn can_account_promote(who: AccountId, data_len: u32) -> bool {
			HopPromotion::can_account_promote(&who, data_len)
		}

		fn create_promotion_extrinsic(
			data: Vec<u8>,
			signer: MultiSigner,
			signature: MultiSignature,
			submit_timestamp: u64,
		) -> <Block as BlockT>::Extrinsic {
			use frame_system::offchain::CreateAuthorizedTransaction;
			<Runtime as CreateAuthorizedTransaction<
				pallet_orbis_hop_promotion::Call<Runtime>,
			>>::create_authorized_transaction(
				pallet_orbis_hop_promotion::Call::<Runtime>::promote {
					data,
					signer,
					signature,
					submit_timestamp,
				}
				.into(),
			)
		}

		fn max_promotion_size() -> u32 {
			<Runtime as pallet_orbis_transaction_storage::Config>::MaxTransactionSize::get()
		}

		fn is_promoted_on_chain(hash: [u8; 32]) -> bool {
			HopPromotion::is_promoted_on_chain(hash)
		}
	}

	impl pallet_orbis_transaction_storage_runtime_api::OrbisTransactionStorageApi<Block, AccountId, BlockNumber> for Runtime {
		fn account_authorization(
			account: AccountId,
		) -> Option<pallet_orbis_transaction_storage_runtime_api::AccountAuthorization<BlockNumber>> {
			TransactionStorage::account_authorization(account)
		}

		fn can_store(account: AccountId, data_len: u32) -> bool {
			TransactionStorage::can_store(&account, data_len)
		}

		fn can_renew(
			account: AccountId,
			entry: pallet_orbis_transaction_storage::TransactionRef<BlockNumber>,
		) -> bool {
			TransactionStorage::can_renew(&account, &entry)
		}

		fn stored_content_provenance(
			reference: orbis_transaction_storage_primitives::StorageRef<BlockNumber>,
		) -> Option<orbis_transaction_storage_primitives::StorageActor<AccountId>> {
			TransactionStorage::stored_content_provenance(reference)
		}

		fn resource_reservation(
			reservation_id: orbis_transaction_storage_primitives::ReservationId,
		) -> Option<
			orbis_transaction_storage_primitives::ResourceReservationView<
				AccountId,
				BlockNumber,
			>,
		> {
			TransactionStorage::resource_reservation(reservation_id)
		}

		fn resource_reservation_link(
			reservation_id: orbis_transaction_storage_primitives::ReservationId,
			content_hash: orbis_transaction_storage_primitives::ContentHash,
		) -> Option<
			orbis_transaction_storage_primitives::ResourceReservationLink<
				AccountId,
				BlockNumber,
			>,
		> {
			TransactionStorage::resource_reservation_link(reservation_id, content_hash)
		}

		fn resource_provider_ref(
			reservation_id: orbis_transaction_storage_primitives::ReservationId,
		) -> Option<orbis_transaction_storage_primitives::ProviderAllocationId> {
			TransactionStorage::resource_provider_ref(reservation_id)
		}
	}

	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
		fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
			build_state::<RuntimeGenesisConfig>(config)
		}

		fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
			get_preset::<RuntimeGenesisConfig>(id, &genesis_config_presets::get_preset)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			genesis_config_presets::preset_names()
		}
	}

	impl cumulus_primitives_core::GetParachainInfo<Block> for Runtime {
		fn parachain_id() -> ParaId {
			ParachainInfo::parachain_id()
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(
			block: <Block as BlockT>::LazyBlock,
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
			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			(list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
			let whitelist: Vec<TrackedStorageKey> = AllPalletsWithSystem::whitelisted_storage_keys();
			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			Ok(batches)
		}
	}
);

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}

#[test]
fn orbis_uses_origin_existential_deposit() {
	let relay_ed = origin_runtime_constants::currency::EXISTENTIAL_DEPOSIT;
	let orbis_ed = ExistentialDeposit::get();
	assert_eq!(relay_ed, orbis_ed);
}
