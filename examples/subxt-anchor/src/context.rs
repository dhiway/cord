use color_eyre::eyre::{Context, Result};
use hex::encode as hex_encode;
use sp_core::{sr25519, Pair as CryptoPair};
use std::sync::Arc;
use subxt::{blocks::ExtrinsicEvents, config::DefaultExtrinsicParamsBuilder, OnlineClient};
use url::Url;

use crate::{
	chain::{build_cord_params, CordConfig},
	pair_signer::PairSigner,
};

pub struct ExampleContext {
	pub client: Arc<OnlineClient<CordConfig>>,
	pub signer: PairSigner,
	pub account_id: subxt::utils::AccountId32,
}

impl ExampleContext {
	pub async fn connect(url: Url, suri: &str) -> Result<Self> {
		let client = OnlineClient::<CordConfig>::from_url(url.as_str())
			.await
			.wrap_err_with(|| format!("failed to connect to {}", url))?;
		let pair = sr25519::Pair::from_string(suri, None).wrap_err("invalid signer seed")?;
		let signer = PairSigner::new(pair);
		let account_id = signer.account_id().clone();
		Ok(Self { client: Arc::new(client), signer, account_id })
	}

	pub async fn submit<T>(&self, label: &str, payload: &T) -> Result<ExtrinsicEvents<CordConfig>>
	where
		T: subxt::tx::Payload + Sync,
	{
		let params =
			build_cord_params(DefaultExtrinsicParamsBuilder::<CordConfig>::new().tip(0u128));

		let mut tx_client = self.client.tx();
		let signed = tx_client
			.create_signed(payload, &self.signer, params)
			.await
			.wrap_err_with(|| format!("prepare {label} extrinsic"))?;

		if std::env::var_os("ANCHOR_DUMP_EXTR").is_some() {
			let encoded = hex_encode(signed.encoded());
			tracing::info!(target: "anchor", "prepared {label} extrinsic 0x{encoded}");
		}

		let progress = signed
			.submit_and_watch()
			.await
			.wrap_err_with(|| format!("submit {label} extrinsic"))?;

		progress
			.wait_for_finalized_success()
			.await
			.wrap_err_with(|| format!("{label} extrinsic failed"))
	}
}
