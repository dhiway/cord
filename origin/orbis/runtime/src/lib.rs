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

#[cfg(test)]
mod tests;
mod weights;
pub mod xcm_config;

use alloc::{borrow::Cow, string::String, vec, vec::Vec};
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
		AsEnsureOriginWithArg, ConstBool, ConstU32, ConstU64, Contains, EitherOfDiverse,
		Everything, InstanceFilter, PrivilegeCmp, TransformOrigin, VariantCountOf,
	},
	weights::{ConstantMultiplier, Weight},
	PalletId,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot, EnsureRootWithSuccess, EnsureSigned,
};
pub use origin_hub_system_runtime_constants::async_backing::SLOT_DURATION;
use origin_hub_system_runtime_constants::{
	async_backing::{
		AVERAGE_ON_INITIALIZE_RATIO, HOURS, MAXIMUM_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO,
	},
	origin::{
		consensus::{
			async_backing::UNINCLUDED_SEGMENT_CAPACITY, BLOCK_PROCESSING_VELOCITY,
			RELAY_CHAIN_SLOT_DURATION_MILLIS,
		},
		currency::*,
	},
};
use origin_primitives::identifier::{DecodedIdentifier, Ss58Identifier};
use origin_runtime_constants::{currency::EXISTENTIAL_DEPOSIT, fee, time::DAYS};
use pallet_assets_precompiles::{InlineIdConfig, ERC20};
use pallet_revive::evm::runtime::EthExtra;
use pallet_token::Token as TokenTrait;
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
	traits::{BlakeTwo256, Block as BlockT},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, FixedU128, MultiSignature, MultiSigner,
};
pub use sp_runtime::{MultiAddress, Perbill, Permill};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
/// Runtime API definition for token.
pub use token_origin_hub_runtime_api as token_api;

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

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: Cow::Borrowed("orbis"),
	impl_name: Cow::Borrowed("dhiway-orbis"),
	authoring_version: 1,
	spec_version: 6,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 2,
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
	type BaseCallFilter = Everything;
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
}

/// Local enterprise assets addressable by a compact `u32` identifier.
pub type AssetsInstance = pallet_assets::Instance1;

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
	type Holder = ();
	type Freezer = ();
	type Extra = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
	type CallbackHandle = pallet_assets::AutoIncAssetId<Runtime, AssetsInstance>;
	type AssetAccountDeposit = AssetAccountDeposit;
	type RemoveItemsLimit = ConstU32<1000>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
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
	type Precompiles = (ERC20<Self, InlineIdConfig<0x120>, AssetsInstance>,);
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
				RuntimeCall::System(..) |
					RuntimeCall::ParachainSystem(..) |
					RuntimeCall::Timestamp(..) |
					RuntimeCall::Indices(pallet_indices::Call::claim { .. }) |
					RuntimeCall::Indices(pallet_indices::Call::free { .. }) |
					RuntimeCall::Indices(pallet_indices::Call::freeze { .. }) |
					RuntimeCall::Entity(..) |
					RuntimeCall::Feeless(..) |
					RuntimeCall::Register(..) |
					RuntimeCall::Session(..) |
					RuntimeCall::Utility(..) |
					RuntimeCall::Proxy(..) |
					RuntimeCall::Multisig(..) |
					RuntimeCall::MessageQueue(..)
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
	type RelayParentOffset = ConstU32<0>;
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
	pub const Period: u32 = 6 * HOURS;
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

pub type MetaTxExtension = (
	pallet_verify_signature::VerifySignature<Runtime>,
	pallet_meta_tx::MetaTxMarker<Runtime>,
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckMortality<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
);

impl pallet_meta_tx::Config for Runtime {
	type WeightInfo = weights::pallet_meta_tx::WeightInfo<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type Extension = MetaTxExtension;
	#[cfg(feature = "runtime-benchmarks")]
	type Extension = pallet_meta_tx::WeightlessExtension<Runtime>;
}

parameter_types! {
	pub const TokenMaxAuthorizationLen: u32 = 256;
	pub const TokenMaxTimelineViewResults: u32 = 64;
	pub const TokenDefaultTimelineViewResults: u32 = 32;
	pub const TokenAuthorizationTTL: u32 = 30;
}

impl pallet_token::Config for Runtime {
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

impl pallet_register::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Token = Token;
	type EntityLookup = Entity;
	type MaxRawDataLength = MaxRegistryRawDataLength;
	type MaxAdditionalAttributes = MaxRegistryAdditionalAttributes;
	type MaxAuthorizationLen = MaxRegistryAuthorizationLen;
	type MaxAuthorizationTTL = RegisterAuthorizationTTL;
	type MaxPacketListResults = MaxPacketListResults;
	type Feeless = Feeless;
	type WeightInfo = pallet_register::weights::SubstrateWeight<Self>;
}

impl pallet_feeless::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_feeless::weights::SubstrateWeight<Runtime>;
	type MaxFeelessTransactionsPerBlock = ConstU32<16>;
}

parameter_types! {
	pub const PeopleMaxAdditionalFields: u32 = 32;
	pub const PeopleMaxRegistrars: u32 = 20;
}

/// SDK-compatible People/People-Lite slice: self-claimed identity plus Sudo-managed attestations
/// and aliases, adapted onto CORD's maintained identity pallet to keep one FRAME dependency graph.
impl pallet_cord_identity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxSubAccounts = ConstU32<32>;
	type IdentityInformation =
		pallet_cord_identity::legacy::IdentityInfo<PeopleMaxAdditionalFields>;
	type MaxRegistrars = PeopleMaxRegistrars;
	type RegistrarOrigin = EnsureRoot<AccountId>;
	type OffchainSignature = MultiSignature;
	type SigningPublicKey = MultiSigner;
	type UsernameAuthorityOrigin = EnsureRoot<AccountId>;
	type PendingUsernameExpiration = ConstU32<{ 7 * DAYS }>;
	type MaxSuffixLength = ConstU32<16>;
	type MaxUsernameLength = ConstU32<64>;
	type WeightInfo = pallet_cord_identity::weights::SubstrateWeight<Runtime>;
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

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime
	{
		// System support stuff.
		System: frame_system = 0,
		ParachainSystem: cumulus_pallet_parachain_system = 1,
		Timestamp: pallet_timestamp = 2,
		ParachainInfo: parachain_info = 3,

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
		Token: pallet_token = 51,
		Register: pallet_register = 52,
		Entity: pallet_entity = 53,
		Feeless: pallet_feeless = 54,

		// Unified application assets.
		Assets: pallet_assets::<Instance1> = 80,

		// People identity and lightweight aliases.
		People: pallet_cord_identity = 90,

		// Solidity and PolkaVM contracts.
		Revive: pallet_revive = 100,

		// Utilities
		MetaTx: pallet_meta_tx = 215,
		TxPause: pallet_tx_pause = 216,
		SafeMode: pallet_safe_mode = 217,
		VerifySignature: pallet_verify_signature = 219,
		// Remark: pallet_remark = 220,

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
pub type TxExtensions = (
	frame_system::AuthorizeCall<Runtime>,
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckMortality<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_feeless::ChargeOrSkipFeeless<
		Runtime,
		pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
	>,
	frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	pallet_revive::evm::tx_extension::SetOrigin<Runtime>,
);

/// Extensions applied when an Ethereum transaction is converted into an Orbis extrinsic.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EthExtraImpl;

impl EthExtra for EthExtraImpl {
	type Config = Runtime;
	type ExtensionV0 = TxExtensions;
	type ExtensionOtherVersions = sp_runtime::traits::InvalidVersion;

	fn get_eth_extension(nonce: u32, tip: Balance) -> Self::ExtensionV0 {
		(
			frame_system::AuthorizeCall::<Runtime>::new(),
			frame_system::CheckNonZeroSender::<Runtime>::new(),
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckMortality::<Runtime>::from(generic::Era::Immortal),
			frame_system::CheckNonce::<Runtime>::from(nonce),
			frame_system::CheckWeight::<Runtime>::new(),
			pallet_feeless::ChargeOrSkipFeeless::from(
				pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
			),
			frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
			pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::new_from_eth_transaction(),
		)
			.into()
	}
}

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	pallet_revive::evm::runtime::UncheckedExtrinsic<Address, Signature, EthExtraImpl>;

pub type Migrations = migrations::Unreleased;
/// Migrations to apply on runtime upgrade.
#[allow(deprecated, missing_docs)]
pub mod migrations {
	/// Unreleased migrations. Add new ones here:
	pub type Unreleased = ();
}

/// MBM migrations to apply on runtime upgrade.
pub type MbmMigrations = ();

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	use super::*;
	use alloc::boxed::Box;
	use origin_hub_system_runtime_constants::origin::locations::{
		OriginHubLocation, OriginHubParaId,
	};
	use xcm::latest::Assets as XcmAssets;

	frame_benchmarking::define_benchmarks!(
		[frame_system, SystemBench::<Runtime>]
		[frame_system_extensions, SystemExtensionsBench::<Runtime>]
		[pallet_assets, Assets]
		[pallet_balances, Balances]
		[pallet_broker, Broker]
		[pallet_entity, Entity]
		[pallet_message_queue, MessageQueue]
		[pallet_meta_tx, MetaTx]
		[pallet_migrations, MultiBlockMigrations]
		[pallet_multisig, Multisig]
		[pallet_proxy, Proxy]
		[pallet_register, Register]
		[pallet_safe_mode, SafeMode]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		[pallet_transaction_payment, TransactionPayment]
		[pallet_tx_pause, TxPause]
		[pallet_utility, Utility]
		[pallet_verify_signature, VerifySignature]

		// Cumulus
		[cumulus_pallet_parachain_system, ParachainSystem]
		[cumulus_pallet_xcmp_queue, XcmpQueue]
		[pallet_collator_selection, CollatorSelection]
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
				OriginHubParaId,
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
			// Only supports native token teleports to default Origin Hub parachain
			let native_location = Parent.into();
			let dest = OriginHubLocation::get();

			// Polkadot SDK >= stable2509: HRMP open helper still required in benchmarks.
			ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(
				OriginHubParaId::get(),
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
			0
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

	impl token_api::TokenOriginHubRuntimeApi<Block> for Runtime {
		fn decode_token(token: Vec<u8>) -> Option<token_api::DecodedTokenApi> {
			let ss58_id = Ss58Identifier::try_from(token).ok()?;
			let decoded: DecodedIdentifier = <pallet_token::Pallet<Runtime> as TokenTrait<Runtime>>::resolve_token(&ss58_id).ok()?;
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
			if !pallet_token::StateVersion::<Runtime>::contains_key(&ss58_id) {
				return token_api::TokenStatusApi::TokenNotFound;
			}
			let version = pallet_token::StateVersion::<Runtime>::get(&ss58_id);
			let last_state = version.checked_sub(1);
			token_api::TokenStatusApi::Found { last_state }
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
