use core::marker::PhantomData;
use scale_info::PortableRegistry;
use subxt::{
	client::ClientState,
	config::{
		substrate::{AccountId32, DynamicHasher256, MultiAddress, MultiSignature, SubstrateHeader},
		transaction_extensions::{self, TransactionExtension},
		Config, DefaultExtrinsicParamsBuilder, ExtrinsicParams, ExtrinsicParamsEncoder,
		ExtrinsicParamsError,
	},
};

/// Runtime configuration that mirrors CORD's signed extension tuple
/// (`CheckNonZeroSender`, `CheckSpecVersion`, `CheckTxVersion`, `CheckGenesis`,
/// `CheckMortality`, `CheckNonce`, `CheckWeight`, `ChargeTransactionPayment`, `WeightReclaim`).
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum CordConfig {}

pub type CordExtrinsicParams<T> = transaction_extensions::AnyOf<
	T,
	(
		custom::CheckNonZeroSender<T>,
		transaction_extensions::CheckSpecVersion,
		transaction_extensions::CheckTxVersion,
		transaction_extensions::CheckGenesis<T>,
		transaction_extensions::CheckMortality<T>,
		transaction_extensions::CheckNonce,
		custom::CheckWeight<T>,
		transaction_extensions::ChargeTransactionPayment,
		custom::WeightReclaim<T>,
	),
>;

impl Config for CordConfig {
	type AccountId = AccountId32;
	type Address = MultiAddress<Self::AccountId, u32>;
	type Signature = MultiSignature;
	type Hasher = DynamicHasher256;
	type Header = SubstrateHeader<u32, Self::Hasher>;
	type ExtrinsicParams = CordExtrinsicParams<Self>;
	type AssetId = u32;
}

/// Helper that adapts Subxt's default parameter builder into the shorter tuple the runtime expects.
pub fn build_cord_params(
	builder: DefaultExtrinsicParamsBuilder<CordConfig>,
) -> <CordExtrinsicParams<CordConfig> as ExtrinsicParams<CordConfig>>::Params {
	let (_, spec, tx, nonce, genesis, mortality, _, charge_tx, _) = builder.build();
	((), spec, tx, genesis, mortality, nonce, (), charge_tx, ())
}

pub fn default_cord_params(
) -> <CordExtrinsicParams<CordConfig> as ExtrinsicParams<CordConfig>>::Params {
	build_cord_params(DefaultExtrinsicParamsBuilder::<CordConfig>::new())
}

mod custom {
	use super::*;

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
