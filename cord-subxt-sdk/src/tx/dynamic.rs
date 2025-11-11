use crate::{
	client::Client,
	error::{Error, Result},
	flavors::ChainFlavor,
	params::{self, PreparedTxOptions},
	tx::{nonce, TxOptions},
};
use scale_value::{Composite, Value, ValueDef};
#[allow(unused_imports)]
use subxt::tx::Signer as _;
use subxt::{
	tx::{self, DynamicPayload},
	utils::Era,
};

fn expect_composite(value: Value) -> Result<Composite<()>> {
	match value.value {
		ValueDef::Composite(c) => Ok(c),
		other => Err(Error::Params(format!("expected composite value, got {other:?}"))),
	}
}

/// Encode a dynamic call using the provided arguments.
pub async fn build_call(
	_client: &Client,
	pallet: &str,
	call: &str,
	args: Value,
) -> Result<DynamicPayload> {
	let composite = expect_composite(args)?;
	Ok(tx::dynamic(pallet, call, composite))
}

/// Encode a dynamic call using a JSON shape that mirrors the runtime metadata.
pub async fn build_call_json(
	client: &Client,
	pallet: &str,
	call: &str,
	json_args: serde_json::Value,
) -> Result<DynamicPayload> {
	let value = scale_value::serde::to_value(json_args)
		.map_err(|e| Error::Params(format!("json→value conversion failed: {e}")))?;
	build_call(client, pallet, call, value).await
}

/// Sign and submit a call using the detected chain flavor's signed-extension tuple.
pub async fn sign_and_submit_with_flavor<
	S: subxt::tx::Signer<crate::params::config::CordConfig>,
>(
	client: &Client,
	call: DynamicPayload,
	signer: &S,
	opts: TxOptions,
) -> Result<
	subxt::tx::TxInBlock<
		crate::params::config::CordConfig,
		subxt::OnlineClient<crate::params::config::CordConfig>,
	>,
> {
	let api = &client.api;
	let flavor = client.flavor;
	let mut tx = api.tx();
	let who = signer.account_id();
	let nonce_mode = opts.nonce.unwrap_or_default();
	let nonce_value = nonce::resolve_nonce(client, &who, nonce_mode).await?;
	let tip = opts.tip.unwrap_or(0);
	let era = opts.era.unwrap_or(Era::Immortal);
	let prepared = PreparedTxOptions { era, nonce: nonce_value, tip };
	let params = match flavor {
		ChainFlavor::Orb => params::orb::params(prepared),
		ChainFlavor::Origin => params::origin::params(prepared),
		ChainFlavor::OriginHub | ChainFlavor::Auto => params::origin_hub::params(prepared),
	};

	tx.sign_and_submit_then_watch(&call, signer, params)
		.await
		.map_err(Error::from)?
		.wait_for_finalized()
		.await
		.map_err(Error::from)
}
