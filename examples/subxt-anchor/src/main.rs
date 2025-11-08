mod builders;
mod chain;
mod cli;
mod context;
mod docs;
mod flows;
mod pair_signer;

/// Generated Subxt bindings for the local CORD runtime.
/// Refresh `metadata/cord.scale` with `subxt metadata --url <node> --output metadata/cord.scale`
/// after each runtime upgrade so the metadata hash matches the node.
#[subxt::subxt(
	runtime_metadata_path = "metadata/cord.scale",
	derive_for_all_types = "Clone, Debug, PartialEq, Eq"
)]
pub mod cord {}

use clap::Parser;
use cli::{Cli, Command};
use color_eyre::eyre::WrapErr;
use context::ExampleContext;
use flows::FlowOptions;
use rand::RngCore;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
	color_eyre::install()?;
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.with_target(false)
		.compact()
		.init();

	let cli = Cli::parse();
	match cli.command {
		Command::Walkthrough => {
			let ctx = ExampleContext::connect(cli.url.clone(), &cli.signer)
				.await
				.wrap_err("failed to establish Subxt client")?;
			let mut bytes = [0u8; 4];
			rand::thread_rng().fill_bytes(&mut bytes);
			let run_id =
				format!("{:02x}{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2], bytes[3]);
			let options = FlowOptions::new("anchor-demo".into(), run_id);
			tracing::info!(target: "anchor", label = options.label(), "Starting walkthrough run");
			flows::run_walkthrough(&ctx, &options).await?;
		},
		Command::Docs { topic } => docs::print(topic).wrap_err("failed to render topic")?,
	}

	Ok(())
}
