pub mod dynamic;
pub mod entity;
pub mod nonce;
pub mod packet;
pub mod register;
pub mod signer;
pub mod submitter;

pub use submitter::{SubmitError, SubmitStage, TxSubmitter};

use crate::{
	client::Client,
	error::{Error, Result},
	params,
	params::config::CordConfig,
};
use subxt::{
	dynamic::Value,
	tx::{self, DynamicPayload, TxProgress},
	utils::Era,
};

/// Options that customize how an extrinsic is signed/submitted.
#[derive(Clone, Copy)]
pub struct TxOptions {
	pub nonce: Option<nonce::NonceMode>,
	pub tip: Option<u128>,
	pub era: Option<Era>,
}

impl Default for TxOptions {
	fn default() -> Self {
		Self { nonce: None, tip: None, era: None }
	}
}

/// Facade that exposes extrinsic composition helpers.
pub struct Transactions<'a> {
	pub(crate) client: &'a Client,
}

impl<'a> Transactions<'a> {
	pub async fn build(
		&self,
		pallet: &str,
		call: &str,
		args: subxt::dynamic::Value,
	) -> Result<DynamicPayload> {
		dynamic::build_call(self.client, pallet, call, args).await
	}

	pub async fn build_json(
		&self,
		pallet: &str,
		call: &str,
		json_args: serde_json::Value,
	) -> Result<DynamicPayload> {
		dynamic::build_call_json(self.client, pallet, call, json_args).await
	}

	pub async fn sign_and_submit<S: subxt::tx::Signer<CordConfig>>(
		&self,
		call: DynamicPayload,
		signer: &S,
		opts: TxOptions,
	) -> Result<tx::TxInBlock<CordConfig, subxt::OnlineClient<CordConfig>>> {
		dynamic::sign_and_submit_with_flavor(self.client, call, signer, opts).await
	}

	pub async fn sign_and_submit_then_watch_with_opts<S: subxt::tx::Signer<CordConfig>>(
		&self,
		call: DynamicPayload,
		signer: &S,
		opts: TxOptions,
	) -> Result<TxProgress<CordConfig, subxt::OnlineClient<CordConfig>>> {
		let api = &self.client.api;
		let mut tx = api.tx();
		let who = signer.account_id();
		let nonce_mode = opts.nonce.unwrap_or_default();
		let nonce_value = nonce::resolve_nonce(self.client, &who, nonce_mode).await?;
		let tip = opts.tip.unwrap_or(0);
		let era = opts.era.unwrap_or(Era::Immortal);
		let prepared = params::PreparedTxOptions { era, nonce: nonce_value, tip };
		let params = params::build_params_from(prepared);
		tx.sign_and_submit_then_watch(&call, signer, params).await.map_err(Error::from)
	}

	pub async fn utility_batch_all(&self, calls: Vec<DynamicPayload>) -> Result<DynamicPayload> {
		let values = calls.into_iter().map(|call| call.into_value()).collect::<Vec<_>>();
		let args = Value::named_composite([("calls", Value::unnamed_composite(values))]);
		self.build("Utility", "batch_all", args).await
	}
}
