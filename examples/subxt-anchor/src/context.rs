use color_eyre::eyre::{eyre, Context, Result};
use hex::encode as hex_encode;
use sp_core::{blake2_256, sr25519, Pair as CryptoPair};
use std::sync::Arc;
use subxt::{
	backend::{legacy::rpc_methods::LegacyRpcMethods, rpc::RpcClient},
	blocks::ExtrinsicEvents,
	config::DefaultExtrinsicParamsBuilder,
	tx::TxStatus,
	OnlineClient,
};
use url::Url;

use crate::{
	chain::{build_cord_params, CordConfig},
	pair_signer::PairSigner,
};

const LOCAL_METADATA: &[u8] = include_bytes!("../metadata/cord.scale");

async fn ensure_metadata_alignment(rpc: &RpcClient) -> Result<()> {
	let remote = LegacyRpcMethods::<CordConfig>::new(rpc.clone())
		.state_get_metadata(None)
		.await
		.wrap_err("failed to download runtime metadata from node")?
		.into_raw();
	let remote_hash = blake2_256(&remote);
	let local_hash = blake2_256(LOCAL_METADATA);
	if remote_hash != local_hash {
		return Err(eyre!(
			"runtime metadata mismatch. local hash {LOCAL}, node hash {REMOTE}. \\nRun `cargo run -p cord-subxt-anchor --bin dump_metadata -- --output examples/subxt-anchor/metadata/cord.scale` and rebuild/restart the node to realign.",
			LOCAL = hex_encode(local_hash),
			REMOTE = hex_encode(remote_hash)
		));
	}
	Ok(())
}

pub struct ExampleContext {
	pub client: Arc<OnlineClient<CordConfig>>,
	rpc: Arc<RpcClient>,
	pub signer: PairSigner,
	pub account_id: subxt::utils::AccountId32,
}

impl ExampleContext {
	pub async fn connect(url: Url, suri: &str) -> Result<Self> {
		let rpc = RpcClient::from_insecure_url(url.as_str())
			.await
			.wrap_err_with(|| format!("failed to connect to {}", url))?;
		ensure_metadata_alignment(&rpc).await?;
		let client = OnlineClient::<CordConfig>::from_rpc_client(rpc.clone())
			.await
			.wrap_err("failed to initialize Subxt client")?;
		let pair = sr25519::Pair::from_string(suri, None).wrap_err("invalid signer seed")?;
		let signer = PairSigner::new(pair);
		let account_id = signer.account_id().clone();
		Ok(Self { client: Arc::new(client), rpc: Arc::new(rpc), signer, account_id })
	}

	async fn next_nonce(&self) -> Result<u64> {
		LegacyRpcMethods::<CordConfig>::new((*self.rpc).clone())
			.system_account_next_index(&self.account_id)
			.await
			.wrap_err("failed to fetch account nonce")
	}

	pub async fn submit<T>(&self, label: &str, payload: &T) -> Result<ExtrinsicEvents<CordConfig>>
	where
		T: subxt::tx::Payload + Sync,
	{
		let mut attempts = 0;
		let mut explicit_nonce: Option<u64> = None;
		'outer: loop {
			let mut builder = DefaultExtrinsicParamsBuilder::<CordConfig>::new().tip(0u128);
			if let Some(nonce) = explicit_nonce {
				builder = builder.nonce(nonce);
			}
			let params = build_cord_params(builder);
			let mut tx_client = self.client.tx();
			let signed = tx_client
				.create_signed(payload, &self.signer, params)
				.await
				.wrap_err_with(|| format!("prepare {label} extrinsic"))?;

			if std::env::var_os("ANCHOR_DUMP_EXTR").is_some() {
				let encoded = hex_encode(signed.encoded());
				tracing::info!(target: "anchor", "prepared {label} extrinsic 0x{encoded}");
			}

			let mut progress = signed
				.submit_and_watch()
				.await
				.wrap_err_with(|| format!("submit {label} extrinsic"))?;

			while let Some(status) = progress.next().await {
				let status = status.wrap_err_with(|| format!("{label} extrinsic status"))?;
				match status {
					TxStatus::InBestBlock(in_block) | TxStatus::InFinalizedBlock(in_block) => {
						let events = in_block
							.wait_for_success()
							.await
							.wrap_err_with(|| format!("{label} extrinsic failed"))?;
						return Ok(events);
					},
					TxStatus::Invalid { message }
						if message.contains("InvalidTransaction::Stale") && attempts < 3 =>
					{
						attempts += 1;
						let nonce = self.next_nonce().await?;
						explicit_nonce = Some(nonce);
						tracing::warn!(target = "anchor", "{label} extrinsic had stale nonce; retry {attempts} with nonce {nonce}");
						continue 'outer;
					},
					TxStatus::Error { message }
					| TxStatus::Invalid { message }
					| TxStatus::Dropped { message } => {
						return Err(eyre!("{label} extrinsic error: {message}"));
					},
					_ => continue,
				}
			}
			return Err(eyre!("{label} extrinsic stream ended before inclusion"));
		}
	}
}
