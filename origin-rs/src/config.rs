//! Subxt config for Origin networks (Origin base & hub).

use core::marker::PhantomData;
use scale_info::PortableRegistry;
use sp_core::{ecdsa, ed25519, sr25519};
use sp_runtime::MultiSignature as SpMultiSignature;
use subxt::{
	config::{
		substrate::{AccountId32, DynamicHasher256, MultiAddress, MultiSignature, SubstrateHeader},
		transaction_extensions, ClientState, Config, DefaultExtrinsicParamsBuilder,
		TransactionExtension, TransactionExtensions,
	},
	error::TransactionExtensionError,
};

/// Convert Origin runtime account type into Subxt account type.
pub fn account_id_to_subxt(account: &origin_primitives::AccountId) -> AccountId32 {
	let mut bytes = [0u8; 32];
	bytes.copy_from_slice(account.as_ref());
	AccountId32(bytes)
}

/// Convert Subxt account type into Origin runtime account type.
pub fn account_id_from_subxt(account: &AccountId32) -> origin_primitives::AccountId {
	origin_primitives::AccountId::from(account.0)
}

/// Convert Origin runtime signature into Subxt signature type.
pub fn signature_to_subxt(signature: &SpMultiSignature) -> MultiSignature {
	match signature {
		SpMultiSignature::Ed25519(sig) => MultiSignature::Ed25519(sig.0),
		SpMultiSignature::Sr25519(sig) => MultiSignature::Sr25519(sig.0),
		SpMultiSignature::Ecdsa(sig) => MultiSignature::Ecdsa(sig.0),
		SpMultiSignature::Eth(sig) => MultiSignature::Ecdsa(sig.0),
	}
}

/// Convert Subxt signature type into Origin runtime signature.
pub fn signature_from_subxt(signature: &MultiSignature) -> SpMultiSignature {
	match signature {
		MultiSignature::Ed25519(sig) =>
			SpMultiSignature::Ed25519(ed25519::Signature::from_raw(*sig)),
		MultiSignature::Sr25519(sig) =>
			SpMultiSignature::Sr25519(sr25519::Signature::from_raw(*sig)),
		MultiSignature::Ecdsa(sig) => SpMultiSignature::Ecdsa(ecdsa::Signature::from_raw(*sig)),
	}
}

/// Custom transaction extensions used by Origin runtimes.
pub type OriginTransactionExtensions<T> = (
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
);

/// Alias used by hub runtime (same extensions today, distinct type for clarity).
pub type OriginHubTransactionExtensions<T> = OriginTransactionExtensions<T>;

/// Chain config bound to Origin runtimes.
#[derive(Debug, Clone, Copy, Default)]
pub struct OriginConfig;

impl Config for OriginConfig {
	type AccountId = AccountId32;
	type Address = MultiAddress<Self::AccountId, u32>;
	type Signature = MultiSignature;
	type Hasher = DynamicHasher256;
	type Header = SubstrateHeader<subxt::utils::H256>;
	type TransactionExtensions = OriginTransactionExtensions<Self>;
	type AssetId = u32;
}

/// Hub config (same types; separate for demos targeting hub).
#[derive(Debug, Clone, Copy, Default)]
pub struct OriginHubConfig;

impl Config for OriginHubConfig {
	type AccountId = AccountId32;
	type Address = MultiAddress<Self::AccountId, u32>;
	type Signature = MultiSignature;
	type Hasher = DynamicHasher256;
	type Header = SubstrateHeader<subxt::utils::H256>;
	type TransactionExtensions = OriginHubTransactionExtensions<Self>;
	type AssetId = u32;
}

/// Build Origin transaction extension params.
pub fn build_origin_params<C: Config<TransactionExtensions = OriginTransactionExtensions<C>>>(
	builder: DefaultExtrinsicParamsBuilder<C>,
) -> <OriginTransactionExtensions<C> as TransactionExtensions<C>>::Params {
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

/// Build Origin Hub transaction extension params (same layout).
pub fn build_origin_hub_params<
	C: Config<TransactionExtensions = OriginHubTransactionExtensions<C>>,
>(
	builder: DefaultExtrinsicParamsBuilder<C>,
) -> <OriginHubTransactionExtensions<C> as TransactionExtensions<C>>::Params {
	build_origin_params(builder)
}

/// Convenience alias for online client bound to `OriginConfig`.
pub type Client = subxt::OnlineClient<OriginConfig>;

mod custom {
	use super::*;

	macro_rules! empty_ext {
		($name:ident, $ident:literal) => {
			pub struct $name<T: Config>(PhantomData<T>);

			impl<T: Config> TransactionExtension<T> for $name<T> {
				type Decoded = ();
				type Params = ();

				fn new(
					_client: &ClientState<T>,
					_params: Self::Params,
				) -> Result<Self, TransactionExtensionError> {
					Ok(Self(PhantomData))
				}
			}

			impl<T: Config>
				subxt::ext::frame_decode::extrinsics::TransactionExtension<PortableRegistry>
				for $name<T>
			{
				const NAME: &str = $ident;

				fn encode_value_to(
					&self,
					_type_id: u32,
					_type_resolver: &PortableRegistry,
					_v: &mut Vec<u8>,
				) -> Result<(), subxt::ext::frame_decode::extrinsics::TransactionExtensionError> {
					Ok(())
				}

				fn encode_implicit_to(
					&self,
					_type_id: u32,
					_type_resolver: &PortableRegistry,
					_v: &mut Vec<u8>,
				) -> Result<(), subxt::ext::frame_decode::extrinsics::TransactionExtensionError> {
					Ok(())
				}
			}
		};
	}

	empty_ext!(AuthorizeCall, "AuthorizeCall");
	empty_ext!(CheckNonZeroSender, "CheckNonZeroSender");
	empty_ext!(CheckWeight, "CheckWeight");
	empty_ext!(WeightReclaim, "WeightReclaim");
}
