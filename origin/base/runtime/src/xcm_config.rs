// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot. If not, see <http://www.gnu.org/licenses/>.

//! XCM configurations for the Origin Staging runtime.

use super::{
	parachains_origin, AccountId, AllPalletsWithSystem, ParaId, Runtime, RuntimeCall, RuntimeEvent,
	RuntimeOrigin, TransactionByteFee, WeightToFee,
};
use runtime_parachains::dmp as parachains_dmp;
type Balances = pallet_balances::Pallet<Runtime>;
type XcmPallet = pallet_xcm::Pallet<Runtime>;
type Dmp = parachains_dmp::Pallet<Runtime>;

use frame_support::{
	parameter_types,
	traits::{Contains, Disabled, Equals, Everything, Nothing},
	weights::Weight,
};
use frame_system::EnsureRoot;
use origin_runtime_constants::{currency::MILLI, system_parachain::*};
use polkadot_runtime_common::{
	xcm_sender::{ChildParachainRouter, ExponentialPrice},
	ToAuthor,
};
use sp_core::ConstU32;
use xcm::latest::prelude::*;
use xcm_builder::{
	AccountId32Aliases, AllowExplicitUnpaidExecutionFrom, AllowKnownQueryResponses,
	AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom, ChildParachainAsNative,
	ChildParachainConvertsVia, DescribeAllTerminal, DescribeFamily, FrameTransactionalProcessor,
	FungibleAdapter, HashedDescription, IsChildSystemParachain, IsConcrete, MintLocation,
	SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit,
	TrailingSetTopicAsId, UsingComponents, WeightInfoBounds, WithComputedOrigin, WithUniqueTopic,
	XcmFeeManagerFromComponents,
};
use xcm_executor::XcmExecutor;

parameter_types! {
	pub const RootLocation: Location = Here.into_location();
	pub const TokenLocation: Location = Here.into_location();
	pub const ThisNetwork: NetworkId = Polkadot;
	pub UniversalLocation: InteriorLocation = ThisNetwork::get().into();
	pub CheckAccount: AccountId = XcmPallet::check_account();
	pub LocalCheckAccount: (AccountId, MintLocation) = (CheckAccount::get(), MintLocation::Local);
}

pub type SovereignAccountOf = (
	// We can convert a child parachain using the standard `AccountId` conversion.
	ChildParachainConvertsVia<ParaId, AccountId>,
	// We can directly alias an `AccountId32` into a local account.
	AccountId32Aliases<ThisNetwork, AccountId>,
	// Foreign locations alias into accounts according to a hash of their standard description.
	HashedDescription<AccountId, DescribeFamily<DescribeAllTerminal>>,
);

/// Our asset transactor. This is what allows us to interest with the runtime facilities from the
/// point of view of XCM-only concepts like `Location` and `Asset`.
///
/// Ours is only aware of the Balances pallet, which is mapped to `TokenLocation`.
pub type LocalAssetTransactor = FungibleAdapter<
	// Use this currency:
	Balances,
	// Use this currency when it is a fungible asset matching the given location or name:
	IsConcrete<TokenLocation>,
	// We can convert the Locations with our converter above:
	SovereignAccountOf,
	// Our chain's account ID type (we can't get away without mentioning it explicitly):
	AccountId,
	// We track our teleports in/out to keep total issuance correct.
	LocalCheckAccount,
>;

/// The means that we convert the XCM message origin location into a local dispatch origin.
type LocalOriginConverter = (
	// A `Signed` origin of the sovereign account that the original location controls.
	SovereignSignedViaLocation<SovereignAccountOf, RuntimeOrigin>,
	// A child parachain, natively expressed, has the `Parachain` origin.
	ChildParachainAsNative<parachains_origin::Origin, RuntimeOrigin>,
	// The AccountId32 location type can be expressed natively as a `Signed` origin.
	SignedAccountId32AsNative<ThisNetwork, RuntimeOrigin>,
);

parameter_types! {
	/// The amount of weight an XCM operation takes. This is a safe overestimate.
	pub const BaseXcmWeight: Weight = Weight::from_parts(1_000_000_000, 64 * 1024);
	/// Maximum number of instructions in a single XCM fragment. A sanity check against weight
	/// calculations getting too crazy.
	pub const MaxInstructions: u32 = 100;
	/// The asset ID for the asset that we use to pay for message delivery fees.
	pub FeeAssetId: AssetId = AssetId(TokenLocation::get());
	/// The base fee for the message delivery fees.
	pub const BaseDeliveryFee: u128 = MILLI.saturating_mul(3);
}

pub type PriceForChildParachainDelivery =
	ExponentialPrice<FeeAssetId, BaseDeliveryFee, TransactionByteFee, Dmp>;

/// The XCM router. When we want to send an XCM message, we use this type. It amalgamates all of our
/// individual routers.
pub type XcmRouter = WithUniqueTopic<(
	// Only one router so far - use DMP to communicate with child parachains.
	ChildParachainRouter<Runtime, XcmPallet, PriceForChildParachainDelivery>,
)>;

parameter_types! {
	pub const Orgn: AssetFilter = Wild(AllOf { fun: WildFungible, id: AssetId(TokenLocation::get()) });
	pub OriginHubInLocation: Location = Parachain(ORIGIN_HUB_IN_ID).into_location();
	pub OrgnForOriginHubIn: (AssetFilter, Location) = (Orgn::get(), OriginHubInLocation::get());
	pub OriginHubNaLocation: Location = Parachain(ORIGIN_HUB_NA_ID).into_location();
	pub OrgnForOriginHubNa: (AssetFilter, Location) = (Orgn::get(), OriginHubNaLocation::get());
	pub OriginHubEuLocation: Location = Parachain(ORIGIN_HUB_EU_ID).into_location();
	pub OrgnForOriginHubEu: (AssetFilter, Location) = (Orgn::get(), OriginHubEuLocation::get());
	pub OriginHubApLocation: Location = Parachain(ORIGIN_HUB_AP_ID).into_location();
	pub OrgnForOriginHubAp: (AssetFilter, Location) = (Orgn::get(), OriginHubApLocation::get());
	pub OriginHubMeLocation: Location = Parachain(ORIGIN_HUB_ME_ID).into_location();
	pub OrgnForOriginHubMe: (AssetFilter, Location) = (Orgn::get(), OriginHubMeLocation::get());
	pub OriginHubAfLocation: Location = Parachain(ORIGIN_HUB_AF_ID).into_location();
	pub OrgnForOriginHubAf: (AssetFilter, Location) = (Orgn::get(), OriginHubAfLocation::get());
	pub const MaxAssetsIntoHolding: u32 = 64;
}

pub type TrustedTeleporters = (
	xcm_builder::Case<OrgnForOriginHubIn>,
	xcm_builder::Case<OrgnForOriginHubNa>,
	xcm_builder::Case<OrgnForOriginHubEu>,
	xcm_builder::Case<OrgnForOriginHubAp>,
	xcm_builder::Case<OrgnForOriginHubMe>,
	xcm_builder::Case<OrgnForOriginHubAf>,
);

pub struct OnlyParachains;
impl Contains<Location> for OnlyParachains {
	fn contains(loc: &Location) -> bool {
		matches!(loc.unpack(), (0, [Parachain(_)]))
	}
}

pub struct LocalPlurality;
impl Contains<Location> for LocalPlurality {
	fn contains(loc: &Location) -> bool {
		matches!(loc.unpack(), (0, [Plurality { .. }]))
	}
}

/// The barriers one of which must be passed for an XCM message to be executed.
pub type Barrier = TrailingSetTopicAsId<(
	// Weight that is paid for may be consumed.
	TakeWeightCredit,
	// Expected responses are OK.
	AllowKnownQueryResponses<XcmPallet>,
	WithComputedOrigin<
		(
			// If the message is one that immediately attempts to pay for execution, then allow it.
			AllowTopLevelPaidExecutionFrom<Everything>,
			// Messages coming from system parachains need not pay for execution.
			AllowExplicitUnpaidExecutionFrom<IsChildSystemParachain<ParaId>>,
			// Subscriptions for version tracking are OK.
			AllowSubscriptionsFrom<OnlyParachains>,
		),
		UniversalLocation,
		ConstU32<8>,
	>,
)>;

/// Locations that will not be charged fees in the executor, neither for execution nor delivery.
/// We only waive fees for system functions, which these locations represent.
pub type WaivedLocations = (SystemParachains, Equals<RootLocation>, LocalPlurality);

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type RuntimeCall = RuntimeCall;
	type XcmSender = XcmRouter;
	type XcmEventEmitter = XcmPallet;
	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = LocalOriginConverter;
	type IsReserve = ();
	type IsTeleporter = TrustedTeleporters;
	type UniversalLocation = UniversalLocation;
	type Barrier = Barrier;
	type Weigher = WeightInfoBounds<
		crate::weights::xcm::PolkadotXcmWeight<RuntimeCall>,
		RuntimeCall,
		MaxInstructions,
	>;
	// The weight trader piggybacks on the existing transaction-fee conversion logic.
	type Trader =
		UsingComponents<WeightToFee, TokenLocation, AccountId, Balances, ToAuthor<Runtime>>;
	type ResponseHandler = XcmPallet;
	type AssetTrap = XcmPallet;
	type AssetLocker = ();
	type AssetExchanger = ();
	type SubscriptionService = XcmPallet;
	type PalletInstancesInfo = AllPalletsWithSystem;
	type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
	type FeeManager = XcmFeeManagerFromComponents<WaivedLocations, ()>;
	type MessageExporter = ();
	type UniversalAliases = Nothing;
	type CallDispatcher = RuntimeCall;
	type SafeCallFilter = Everything;
	type Aliasers = Nothing;
	type TransactionalProcessor = FrameTransactionalProcessor;
	type HrmpNewChannelOpenRequestHandler = ();
	type HrmpChannelAcceptedHandler = ();
	type HrmpChannelClosingHandler = ();
	type XcmRecorder = XcmPallet;
}

/// Type to convert an `Origin` type value into a `Location` value which represents an interior
/// location of this chain.
pub type LocalOriginToLocation = (
	// And a usual Signed origin to be used in XCM as a corresponding `AccountId32`.
	SignedToAccountId32<RuntimeOrigin, AccountId, ThisNetwork>,
);

impl pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	// This is safe to enable for everyone (save the possibility of someone spamming a parachain
	// if they're willing to pay the ORU to send from the Relay-chain).
	type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	// Anyone can execute XCM messages locally.
	type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmExecuteFilter = Everything;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type XcmTeleportFilter = Everything;
	// Anyone is able to use reserve transfers regardless of who they are and what they want to
	// transfer.
	type XcmReserveTransferFilter = Everything;
	type Weigher = WeightInfoBounds<
		crate::weights::xcm::PolkadotXcmWeight<RuntimeCall>,
		RuntimeCall,
		MaxInstructions,
	>;
	type UniversalLocation = UniversalLocation;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type Currency = Balances;
	type CurrencyMatcher = IsConcrete<TokenLocation>;
	type TrustedLockers = ();
	type SovereignAccountOf = SovereignAccountOf;
	type MaxLockers = ConstU32<8>;
	type MaxRemoteLockConsumers = ConstU32<0>;
	type RemoteLockConsumerIdentifier = ();
	type WeightInfo = crate::weights::pallet_xcm::WeightInfo<Runtime>;
	type AdminOrigin = EnsureRoot<AccountId>;
	// Aliasing is disabled: xcm_executor::Config::Aliasers allows `Nothing`.
	type AuthorizedAliasConsideration = Disabled;
}
