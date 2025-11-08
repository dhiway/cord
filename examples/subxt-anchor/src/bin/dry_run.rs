use clap::Parser;
use codec::Decode;
use color_eyre::eyre::{Context, Result};
use hex::{encode as hex_encode, FromHex};
use sp_runtime::transaction_validity::TransactionValidityError;
use subxt::backend::{
	legacy::{rpc_methods::DryRunDecodeError, LegacyRpcMethods},
	rpc::RpcClient,
};

#[path = "../chain.rs"]
mod chain;
use chain::CordConfig;

/// Minimal helper that dry-runs a signed extrinsic against a live node to surface
/// the exact `TransactionValidityError`.
#[derive(Parser, Debug)]
struct Cli {
	#[arg(long, default_value = "ws://127.0.0.1:9944", help = "WebSocket endpoint of the node")]
	url: String,
	#[arg(long, help = "SCALE-encoded extrinsic in hex (with or without 0x prefix)")]
	extrinsic: String,
}

#[tokio::main]
async fn main() -> Result<()> {
	color_eyre::install()?;
	let cli = Cli::parse();

	let extrinsic = cli.extrinsic.strip_prefix("0x").unwrap_or(&cli.extrinsic);
	let bytes = Vec::from_hex(extrinsic).wrap_err("invalid hex payload")?;

	let rpc = RpcClient::from_insecure_url(cli.url.as_str())
		.await
		.wrap_err("failed to connect to RPC endpoint")?;
	let methods = LegacyRpcMethods::<CordConfig>::new(rpc);

	let result = methods.dry_run(&bytes, None).await.wrap_err("dry run failed")?;
	match result.into_dry_run_result() {
		Ok(outcome) => match outcome {
			subxt::backend::legacy::rpc_methods::DryRunResult::Success => {
				println!("Dry run succeeded: extrinsic dispatch would pass")
			},
			subxt::backend::legacy::rpc_methods::DryRunResult::DispatchError(err) => {
				println!("Dry run dispatch error bytes: 0x{}", hex_encode(err))
			},
			subxt::backend::legacy::rpc_methods::DryRunResult::TransactionValidityError => {
				if result.0.get(0) == Some(&1) {
					let mut cursor = &result.0[1..];
					match TransactionValidityError::decode(&mut cursor) {
						Ok(err) => println!("Transaction validity error: {err:?}"),
						Err(decode_err) => {
							println!("Failed to decode TransactionValidityError: {decode_err:?}")
						},
					}
				} else {
					println!("Transaction validity error (unable to decode detail)");
				}
			},
		},
		Err(err) => match err {
			DryRunDecodeError::WrongNumberOfBytes => {
				println!("Dry run error detail: wrong number of bytes")
			},
			DryRunDecodeError::InvalidBytes => {
				println!("Dry run error detail: invalid byte layout")
			},
		},
	}

	Ok(())
}
