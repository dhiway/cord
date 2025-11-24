//! Token demo: resolve identifier and show timeline.
//! cargo run -p origin-sdk --example demo_token -- --endpoint ws://localhost:9944 --token 5C8F... [--seed //Alice] [--meta]

use origin_sdk::{client::signer::MultiKeySigner, OriginClient};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
	#[clap(long, default_value = "ws://localhost:9944")]
	endpoint: String,
	#[clap(long, help = "token ss58 string to resolve")]
	token: String,
	#[clap(long, default_value = "//Alice")]
	seed: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let args = Args::parse();
	let token = origin_primitives::Ss58Identifier::try_from(args.token.as_str())?;
	let signer = MultiKeySigner::from_seed(&args.seed)?;
	let client = OriginClient::connect(&args.endpoint).await?.with_signer(signer);

	let decoded = client.view()?.token().resolve_identifier(token).await?;
	println!("Decoded identifier: {:?}", decoded);

	let timeline = client.view()?.token().timeline(token, None, Some(10)).await?;
	println!("Token timeline: {:?}", timeline);
	Ok(())
}
