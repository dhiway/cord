use core::marker::PhantomData;
use scale_info::PortableRegistry;
use subxt::{
	client::ClientState,
	config::{
		substrate::{AccountId32, MultiAddress, MultiSignature, SubstrateHeader},
		transaction_extensions::{self, TransactionExtension},
		Config, DefaultExtrinsicParamsBuilder, ExtrinsicParams, ExtrinsicParamsEncoder,
		ExtrinsicParamsError,
	},
};

pub type OriginExtrinsicParams<T> = transaction_extensions::AnyOf<
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
		transaction_extensions::CheckMetadataHash,
		custom::WeightReclaim<T>,
	),
>;

/// Config for the origin relay runtime.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum OriginConfig {}

/// Config for the origin hub parachain runtime.
///
/// The signed-extension tuple matches [`OriginConfig`], but we expose a distinct
/// type so examples can clearly target origin-hub when desired.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum OriginHubConfig {}

impl Config for OriginConfig {
	type AccountId = AccountId32;
	type Address = MultiAddress<Self::AccountId, u32>;
	type Signature = MultiSignature;
	type Hasher = subxt::config::substrate::DynamicHasher256;
	type Header = SubstrateHeader<u32, Self::Hasher>;
	type ExtrinsicParams = OriginExtrinsicParams<Self>;
	type AssetId = u32;
}

impl Config for OriginHubConfig {
	type AccountId = AccountId32;
	type Address = MultiAddress<Self::AccountId, u32>;
	type Signature = MultiSignature;
	type Hasher = subxt::config::substrate::DynamicHasher256;
	type Header = SubstrateHeader<u32, Self::Hasher>;
	type ExtrinsicParams = OriginExtrinsicParams<Self>;
	type AssetId = u32;
}

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
		metadata_params,
		(),
	)
}

mod custom {
	use super::*;

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
}
