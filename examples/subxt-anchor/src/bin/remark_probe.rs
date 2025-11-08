use clap::Parser;
use color_eyre::eyre::WrapErr;
use sp_core::{sr25519, Pair as CryptoPair};
use subxt::{config::DefaultExtrinsicParamsBuilder, OnlineClient};

#[path = "../chain.rs"]
mod chain;
#[path = "../pair_signer.rs"]
mod pair_signer;
use chain::{build_cord_params, CordConfig};
use pair_signer::PairSigner;

#[subxt::subxt(runtime_metadata_path = "metadata/cord.scale")]
pub mod cord {}

#[derive(Parser, Debug)]
#[command(about = "Send a simple system.remark using the CordConfig bindings")]
struct Cli {
	#[arg(long, default_value = "ws://127.0.0.1:9944", help = "WebSocket endpoint of the node")]
	url: String,
	#[arg(long, default_value = "//Alice", help = "Signer seed (SURI format)")]
	signer: String,
	#[arg(
		long,
		default_value = "hello from remark_probe",
		help = "Message inserted into the remark"
	)]
	message: String,
	#[arg(long, default_value_t = 0u128, help = "Optional tip attached to the extrinsic")]
	tip: u128,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
	color_eyre::install()?;
	let cli = Cli::parse();

	let client = OnlineClient::<CordConfig>::from_url(cli.url.as_str())
		.await
		.wrap_err_with(|| format!("failed to connect to {}", cli.url))?;

	let keypair = sr25519::Pair::from_string(&cli.signer, None).wrap_err("invalid signer seed")?;
	let signer = PairSigner::new(keypair);

	let remark_tx = cord::tx().system().remark(cli.message.into_bytes());
	let params = build_cord_params(DefaultExtrinsicParamsBuilder::<CordConfig>::new().tip(cli.tip));

	let events = client
		.tx()
		.sign_and_submit_then_watch(&remark_tx, &signer, params)
		.await?
		.wait_for_finalized_success()
		.await?;

	println!("Remark finalized (extrinsic index {:?})", events.extrinsic_index());

	Ok(())
}
