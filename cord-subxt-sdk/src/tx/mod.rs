pub mod dynamic;
pub mod entity;
pub mod nonce;
pub mod signer;

use crate::{client::Client, error::Result, params::config::CordConfig};
use subxt::tx::DynamicPayload;
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
		dynamic::build_call(&self.client.api, pallet, call, args).await
	}

	pub async fn build_json(
		&self,
		pallet: &str,
		call: &str,
		json_args: serde_json::Value,
	) -> Result<DynamicPayload> {
		dynamic::build_call_json(&self.client.api, pallet, call, json_args).await
	}

	pub async fn sign_and_submit<S: subxt::tx::Signer<CordConfig>>(
		&self,
		call: DynamicPayload,
		signer: &S,
		opts: TxOptions,
	) -> Result<subxt::tx::TxInBlock<CordConfig>> {
		dynamic::sign_and_submit_with_flavor(
			&self.client.api,
			self.client.flavor,
			call,
			signer,
			opts,
		)
		.await
	}

	pub async fn submit(&self, signed_xt: Vec<u8>) -> Result<subxt::utils::H256> {
		self.client
			.api
			.tx()
			.submit_bytes(&signed_xt)
			.await
			.map_err(crate::error::Error::from)
	}
}
