pub mod orb;
pub mod origin;
pub mod origin_hub;

pub mod config;

use subxt::utils::Era;

pub(crate) use config::{build_cord_params, CordConfig};

/// Options resolved by the transaction layer prior to handing off to the signed-extension encoders.
#[derive(Clone, Copy, Debug)]
pub struct PreparedTxOptions {
	pub era: Era,
	pub nonce: u64,
	pub tip: u128,
}

fn builder_with(
	prepared: PreparedTxOptions,
) -> subxt::config::DefaultExtrinsicParamsBuilder<CordConfig> {
	let builder = subxt::config::DefaultExtrinsicParamsBuilder::<CordConfig>::new()
		.tip(prepared.tip)
		.nonce(prepared.nonce);
	match prepared.era {
		Era::Immortal => builder.immortal(),
		Era::Mortal { period, .. } => builder.mortal(period),
	}
}

pub type ParamsPayload =
	<config::CordExtrinsicParams<CordConfig> as subxt::config::ExtrinsicParams<CordConfig>>::Params;

pub(super) fn build_params_from(prepared: PreparedTxOptions) -> ParamsPayload {
	let builder = builder_with(prepared);
	build_cord_params(builder)
}
