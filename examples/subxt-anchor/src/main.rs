mod builders;
mod chain;
mod cli;
mod context;
mod docs;
mod flows;
mod formatting;
mod pair_signer;
mod pallets;
mod sample_data;
mod view_client;
mod view_types;

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
use sample_data::SampleData;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
	color_eyre::install()?;
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.with_target(false)
		.compact()
		.init();

	let Cli { url, signer, sample_data, command } = Cli::parse();
	let sample_path = sample_data.unwrap_or_else(default_sample_data_path);
	match command {
		Command::Walkthrough => {
			let samples = SampleData::load(&sample_path).wrap_err_with(|| {
				format!("failed to load sample data from {}", sample_path.display())
			})?;
			let ctx = ExampleContext::connect(url.clone(), &signer)
				.await
				.wrap_err("failed to establish Subxt client")?;
			let mut bytes = [0u8; 4];
			rand::thread_rng().fill_bytes(&mut bytes);
			let run_id =
				format!("{:02x}{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2], bytes[3]);
			let options = FlowOptions::new("anchor-demo".into(), run_id);
			tracing::info!(target: "anchor", label = options.label(), "Starting walkthrough run");
			flows::run_walkthrough(&ctx, &options, &samples).await?;
		},
		Command::Docs { topic } => docs::print(topic).wrap_err("failed to render topic")?,
	}

	Ok(())
}

fn default_sample_data_path() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sample_data/demo.json")
}
