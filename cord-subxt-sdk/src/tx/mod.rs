pub mod dynamic;
pub mod entity;
pub mod nonce;
pub mod signer;

use crate::{client::Client, error::Result, params::config::CordConfig};
use subxt::tx::{self, DynamicPayload};
use subxt::utils::Era;

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
}
