//! Subxt config for Origin relay and Orbis system-chain networks.
//!
//! Keeps the SDK dynamic (no codegen) while ensuring hashes, headers, accounts,
//! and signatures align with Origin primitives.

use core::marker::PhantomData;
use origin_primitives::{AccountId, BlockNumber, Signature};
use scale_info::PortableRegistry;
use sp_runtime::MultiAddress;
use subxt::{
	client::ClientState,
	config::{
		substrate::{DynamicHasher256, SubstrateHeader},
		transaction_extensions::{self, AnyOf, TransactionExtension},
		Config, DefaultExtrinsicParamsBuilder, ExtrinsicParams, ExtrinsicParamsEncoder,
		ExtrinsicParamsError,
	},
};

/// Custom SignedExtensions mirroring Origin runtimes.
pub type OriginExtrinsicParams<T> = AnyOf<
	T,
	(
		custom::AuthorizeCall<T>,
		custom::CheckNonZeroSender<T>,
		transaction_extensions::CheckSpecVersion,
		transaction_extensions::CheckTxVersion,
		transaction_extensions::CheckGenesis<T>,
		transaction_extensions::CheckMortality<T>,
		transaction_extensions::CheckNonce,
		custom::CheckWeight<T>,
		transaction_extensions::ChargeTransactionPayment,
		custom::ValidateStorageCalls<T>,
		transaction_extensions::CheckMetadataHash,
		custom::ReviveSetOrigin<T>,
		custom::WeightReclaim<T>,
	),
>;

/// Legacy Hub alias retained for source compatibility; new clients should use Orbis.
pub type OriginHubExtrinsicParams<T> = OriginExtrinsicParams<T>;

/// Signed extensions exposed by Orbis. The five actor-policy extensions encode an unused
/// `Option::None` selector and must be named here, in runtime order, so Subxt matches live
/// metadata. Quota-aware payment remains metadata-compatible with `ChargeAssetTxPayment`; Bulletin
/// validation and Revive origin mapping are also zero-byte fields.
pub type OrbisExtrinsicParams<T> = AnyOf<
	T,
	(
		custom::AsPerson<T>,
		custom::ScoreAsParticipant<T>,
		custom::PeopleLiteAuth<T>,
		custom::AsResources<T>,
		custom::VoterAuth<T>,
		custom::AuthorizeCall<T>,
		custom::CheckNonZeroSender<T>,
		transaction_extensions::CheckSpecVersion,
		transaction_extensions::CheckTxVersion,
		transaction_extensions::CheckGenesis<T>,
		transaction_extensions::CheckMortality<T>,
		transaction_extensions::CheckNonce,
		custom::CheckWeight<T>,
		transaction_extensions::ChargeAssetTxPayment<T>,
		custom::ValidateStorageCalls<T>,
		transaction_extensions::CheckMetadataHash,
		custom::ReviveSetOrigin<T>,
		custom::WeightReclaim<T>,
	),
>;

/// Chain config bound to Origin primitives.
#[derive(Debug, Clone, Copy, Default)]
pub struct OriginConfig;

impl Config for OriginConfig {
	type AccountId = AccountId;
	type Address = MultiAddress<Self::AccountId, u32>;
	type Signature = Signature;
	type Hasher = DynamicHasher256;
	type Header = SubstrateHeader<BlockNumber, DynamicHasher256>;
	type ExtrinsicParams = OriginExtrinsicParams<Self>;
	type AssetId = u32;
}

/// Hub config (same types; separate for demos targeting hub).
#[derive(Debug, Clone, Copy, Default)]
pub struct OriginHubConfig;

impl Config for OriginHubConfig {
	type AccountId = AccountId;
	type Address = MultiAddress<Self::AccountId, u32>;
	type Signature = Signature;
	type Hasher = DynamicHasher256;
	type Header = SubstrateHeader<BlockNumber, DynamicHasher256>;
	type ExtrinsicParams = OriginHubExtrinsicParams<Self>;
	type AssetId = u32;
}

/// Orbis config including Bulletin and Revive transaction extensions.
#[derive(Debug, Clone, Copy, Default)]
pub struct OrbisConfig;

impl Config for OrbisConfig {
	type AccountId = AccountId;
	type Address = MultiAddress<Self::AccountId, u32>;
	type Signature = Signature;
	type Hasher = DynamicHasher256;
	type Header = SubstrateHeader<BlockNumber, DynamicHasher256>;
	type ExtrinsicParams = OrbisExtrinsicParams<Self>;
	type AssetId = u32;
}

/// Build Origin extrinsic params (AnyOf tuple ordering must match above).
pub fn build_origin_params<C: Config<ExtrinsicParams = OriginExtrinsicParams<C>>>(
	builder: DefaultExtrinsicParamsBuilder<C>,
) -> <OriginExtrinsicParams<C> as ExtrinsicParams<C>>::Params {
	let (
		_,
		spec_params,
		tx_params,
		nonce_params,
		genesis_params,
		mortality_params,
		_,
		charge_tx_params,
		metadata_params,
	) = builder.build();
	(
		(),
		(),
		spec_params,
		tx_params,
		genesis_params,
		mortality_params,
		nonce_params,
		(),
		charge_tx_params,
		(),
		metadata_params,
		(),
		(),
	)
}

/// Build Origin Hub extrinsic params (same layout).
pub fn build_origin_hub_params<C: Config<ExtrinsicParams = OriginHubExtrinsicParams<C>>>(
	builder: DefaultExtrinsicParamsBuilder<C>,
) -> <OriginHubExtrinsicParams<C> as ExtrinsicParams<C>>::Params {
	let (
		_,
		spec_params,
		tx_params,
		nonce_params,
		genesis_params,
		mortality_params,
		_,
		charge_tx_params,
		metadata_params,
	) = builder.build();
	(
		(),
		(),
		spec_params,
		tx_params,
		genesis_params,
		mortality_params,
		nonce_params,
		(),
		charge_tx_params,
		(),
		metadata_params,
		(),
		(),
	)
}

/// Build Orbis extrinsic params in runtime tuple order.
pub fn build_orbis_params<C: Config<ExtrinsicParams = OrbisExtrinsicParams<C>>>(
	builder: DefaultExtrinsicParamsBuilder<C>,
) -> <OrbisExtrinsicParams<C> as ExtrinsicParams<C>>::Params {
	let (
		_,
		spec_params,
		tx_params,
		nonce_params,
		genesis_params,
		mortality_params,
		charge_asset_params,
		_,
		metadata_params,
	) = builder.build();
	(
		(),
		(),
		(),
		(),
		(),
		(),
		(),
		spec_params,
		tx_params,
		genesis_params,
		mortality_params,
		nonce_params,
		(),
		charge_asset_params,
		(),
		metadata_params,
		(),
		(),
	)
}

/// Convenience alias for online client bound to `OriginConfig`.
pub type Client = subxt::OnlineClient<OriginConfig>;
/// Explicit low-level Orbis client alias.
pub type OrbisClient = subxt::OnlineClient<OrbisConfig>;

mod custom {
	use super::*;

	macro_rules! empty_extension {
		($name:ident, $identifier:literal) => {
			pub struct $name<T: Config>(PhantomData<T>);

			impl<T: Config> ExtrinsicParams<T> for $name<T> {
				type Params = ();

				fn new(
					_client: &ClientState<T>,
					_params: Self::Params,
				) -> Result<Self, ExtrinsicParamsError> {
					Ok(Self(PhantomData))
				}
			}

			impl<T: Config> ExtrinsicParamsEncoder for $name<T> {
				fn encode_value_to(&self, v: &mut Vec<u8>) {
					// Runtime constructors use `new(None)`. SCALE encodes that unused policy
					// selection as the single-byte Option::None discriminant.
					v.push(0);
				}
			}

			impl<T: Config> TransactionExtension<T> for $name<T> {
				type Decoded = Option<()>;

				fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
					identifier == $identifier
				}
			}
		};
	}

	empty_extension!(AsPerson, "AsPerson");
	empty_extension!(ScoreAsParticipant, "ScoreAsParticipant");
	empty_extension!(PeopleLiteAuth, "PeopleLiteAuth");
	empty_extension!(AsResources, "AsResources");
	empty_extension!(VoterAuth, "HonourAuth");

	pub struct AuthorizeCall<T: Config>(PhantomData<T>);

	impl<T: Config> ExtrinsicParams<T> for AuthorizeCall<T> {
		type Params = ();
		fn new(
			_client: &ClientState<T>,
			_params: Self::Params,
		) -> Result<Self, ExtrinsicParamsError> {
			Ok(Self(PhantomData))
		}
	}

	impl<T: Config> ExtrinsicParamsEncoder for AuthorizeCall<T> {
		fn encode_value_to(&self, _v: &mut Vec<u8>) {}
	}

	impl<T: Config> TransactionExtension<T> for AuthorizeCall<T> {
		type Decoded = ();
		fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
			identifier == "AuthorizeCall"
		}
	}

	pub struct CheckNonZeroSender<T: Config>(PhantomData<T>);

	impl<T: Config> ExtrinsicParams<T> for CheckNonZeroSender<T> {
		type Params = ();
		fn new(
			_client: &ClientState<T>,
			_params: Self::Params,
		) -> Result<Self, ExtrinsicParamsError> {
			Ok(Self(PhantomData))
		}
	}

	impl<T: Config> ExtrinsicParamsEncoder for CheckNonZeroSender<T> {
		fn encode_value_to(&self, _v: &mut Vec<u8>) {}
	}

	impl<T: Config> TransactionExtension<T> for CheckNonZeroSender<T> {
		type Decoded = ();
		fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
			identifier == "CheckNonZeroSender"
		}
	}

	pub struct CheckWeight<T: Config>(PhantomData<T>);

	impl<T: Config> ExtrinsicParams<T> for CheckWeight<T> {
		type Params = ();
		fn new(
			_client: &ClientState<T>,
			_params: Self::Params,
		) -> Result<Self, ExtrinsicParamsError> {
			Ok(Self(PhantomData))
		}
	}

	impl<T: Config> ExtrinsicParamsEncoder for CheckWeight<T> {
		fn encode_value_to(&self, _v: &mut Vec<u8>) {}
	}

	impl<T: Config> TransactionExtension<T> for CheckWeight<T> {
		type Decoded = ();
		fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
			identifier == "CheckWeight"
		}
	}

	pub struct WeightReclaim<T: Config>(PhantomData<T>);

	impl<T: Config> ExtrinsicParams<T> for WeightReclaim<T> {
		type Params = ();
		fn new(
			_client: &ClientState<T>,
			_params: Self::Params,
		) -> Result<Self, ExtrinsicParamsError> {
			Ok(Self(PhantomData))
		}
	}

	impl<T: Config> ExtrinsicParamsEncoder for WeightReclaim<T> {
		fn encode_value_to(&self, _v: &mut Vec<u8>) {}
	}

	impl<T: Config> TransactionExtension<T> for WeightReclaim<T> {
		type Decoded = ();
		fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
			identifier == "WeightReclaim"
		}
	}

	pub struct ValidateStorageCalls<T: Config>(PhantomData<T>);

	impl<T: Config> ExtrinsicParams<T> for ValidateStorageCalls<T> {
		type Params = ();
		fn new(
			_client: &ClientState<T>,
			_params: Self::Params,
		) -> Result<Self, ExtrinsicParamsError> {
			Ok(Self(PhantomData))
		}
	}

	impl<T: Config> ExtrinsicParamsEncoder for ValidateStorageCalls<T> {
		fn encode_value_to(&self, _v: &mut Vec<u8>) {}
	}

	impl<T: Config> TransactionExtension<T> for ValidateStorageCalls<T> {
		type Decoded = ();
		fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
			identifier == "ValidateStorageCalls"
		}
	}

	pub struct ReviveSetOrigin<T: Config>(PhantomData<T>);

	impl<T: Config> ExtrinsicParams<T> for ReviveSetOrigin<T> {
		type Params = ();
		fn new(
			_client: &ClientState<T>,
			_params: Self::Params,
		) -> Result<Self, ExtrinsicParamsError> {
			Ok(Self(PhantomData))
		}
	}

	impl<T: Config> ExtrinsicParamsEncoder for ReviveSetOrigin<T> {
		fn encode_value_to(&self, _v: &mut Vec<u8>) {}
	}

	impl<T: Config> TransactionExtension<T> for ReviveSetOrigin<T> {
		type Decoded = ();
		fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
			identifier == "EthSetOrigin"
		}
	}
}

// mod custom {
// 	use super::*;

// 	macro_rules! empty_ext {
// 		($name:ident, $ident:literal) => {
// 			pub struct $name<T: Config>(PhantomData<T>);
// 			impl<T: Config> ExtrinsicParams<T> for $name<T> {
// 				type Params = ();
// 				fn new(
// 					_client: &ClientState<T>,
// 					_params: Self::Params,
// 				) -> Result<Self, ExtrinsicParamsError> {
// 					Ok(Self(PhantomData))
// 				}
// 			}
// 			impl<T: Config> ExtrinsicParamsEncoder for $name<T> {
// 				fn encode_value_to(&self, _v: &mut Vec<u8>) {}
// 			}
// 			impl<T: Config> TransactionExtension<T> for $name<T> {
// 				type Decoded = ();
// 				fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
// 					identifier == $ident
// 				}
// 			}
// 		};
// 	}

// 	empty_ext!(AuthorizeCall, "AuthorizeCall");
// 	empty_ext!(CheckNonZeroSender, "CheckNonZeroSender");
// 	empty_ext!(CheckWeight, "CheckWeight");
// }
