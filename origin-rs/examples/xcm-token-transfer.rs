use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use oc::{demo::spinner, tx::TxOptions, ChainFlavor, Client, ConnectionConfig, RetryPolicy};
use serde_json::json;
use sp_core::crypto::Ss58Codec;
use sp_runtime::AccountId32;
use std::{env, time::Duration};
use subxt::tx::TxStatus;

/// Minimal example that reserves and transfers native balances from one hub parachain
/// to another using XCM v5 (`pallet_xcm::reserve_transfer_assets`).
///
/// Usage (default dev keys & amounts):
///   cargo run -p origin-rs --example xcm-token-transfer -- \
///     --node ws://127.0.0.1:9910 \
///     --dest-para 1001 \
///     --to 5GrwvaEF... # destination SS58 account
///     --amount 1000000000
#[tokio::main]
async fn main() -> Result<()> {
	env_logger::init();
	let opts = Options::from_env()?;
	let connection =
		ConnectionConfig::new(opts.node.clone(), ChainFlavor::OriginHub).with_retry(RetryPolicy {
			initial_backoff: Duration::from_millis(200),
			max_backoff: Duration::from_secs(5),
			max_retries: Some(8),
		});
	let connect_spinner =
		spinner::Spinner::start(format!("Connecting to {} ({:?})", opts.node, ChainFlavor::OriginHub));
	let client = Client::connect_with(connection).await?;
	connect_spinner.finish(Some("Connected")).await;
	let signer = oc::tx::signer::dev_alice();

	let dest = json!({
		"V5": {
			"parents": 1,
			"interior": { "X1": { "Parachain": opts.dest_para } }
		}
	});

	let to_bytes: [u8; 32] = opts.to.into();
	let to_hex = hex::encode(to_bytes);
	let beneficiary = json!({
		"V5": {
			"parents": 0,
			"interior": {
				"X1": {
					"AccountId32": { "network": null, "id": to_hex }
				}
			}
		}
	});

	let assets = json!({
		"V5": [
			{
				"id": { "Concrete": { "parents": 0, "interior": "Here" } },
				"fun": { "Fungible": opts.amount }
			}
		]
	});

	let call = client.tx().build_json(
		"PolkadotXcm",
		"reserve_transfer_assets",
		json!({
			"dest": dest,
			"beneficiary": beneficiary,
			"assets": assets,
			"fee_asset_item": 0,
		}),
	).await?;

	let mut progress = client
		.tx()
		.sign_and_submit_then_watch_with_opts(call, &signer, TxOptions::default())
		.await?;

	let ext_hash = progress.extrinsic_hash();
	let in_block = loop {
		let Some(status) = progress.next().await else {
			return Err(anyhow!("extrinsic stream ended before inclusion"));
		};
		match status? {
			TxStatus::InBestBlock(in_block) | TxStatus::InFinalizedBlock(in_block) => break in_block,
			TxStatus::Error { message } => return Err(anyhow!("node error: {message}")),
			TxStatus::Invalid { message } => return Err(anyhow!("invalid transaction: {message}")),
			TxStatus::Dropped { message } => return Err(anyhow!("dropped transaction: {message}")),
			_ => {},
		}
	};
	in_block.wait_for_success().await?;
	println!("Included extrinsic {:?} in block {:?}", ext_hash, in_block.block_hash());

	Ok(())
}

#[derive(Debug)]
struct Options {
	node: String,
	dest_para: u32,
	to: AccountId32,
	amount: u128,
}

impl Options {
	fn from_env() -> Result<Self> {
		let args: Vec<String> = env::args().collect();
		let mut node = "ws://127.0.0.1:9944".to_string();
		let mut dest_para: Option<u32> = None;
		let mut to: Option<AccountId32> = None;
		let mut amount: u128 = 1_000_000_000; // default 0.1 ORU if 10 decimals
		let mut iter = args.iter().skip(1);
		while let Some(arg) = iter.next() {
			match arg.as_str() {
				"--node" => node = iter.next().context("--node requires value")?.to_string(),
				"--dest-para" => {
					dest_para = Some(iter.next().context("--dest-para requires value")?.parse()?);
				},
				"--to" => {
					let raw = iter.next().context("--to requires SS58 address")?;
					to = Some(AccountId32::from_ss58check(raw)?);
				},
				"--amount" => {
					amount = iter.next().context("--amount requires value")?.parse()?;
				},
				_ => {},
			}
		}
		Ok(Self {
			node,
			dest_para: dest_para.context("--dest-para is required")?,
			to: to.context("--to is required")?,
			amount,
		})
	}
}
