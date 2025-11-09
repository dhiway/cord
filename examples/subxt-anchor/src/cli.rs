use crate::docs::DocTopic;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about = "CORD Subxt anchor walkthrough", propagate_version = true)]
pub struct Cli {
	#[arg(
		long,
		default_value = "ws://127.0.0.1:9944",
		help = "WebSocket endpoint of the local node"
	)]
	pub url: Url,

	#[arg(long, default_value = "//Alice", help = "Signer seed in suri format or hex secret key")]
	pub signer: String,

	#[arg(
		long,
		value_name = "PATH",
		help = "Path to the sample-data JSON used for payload templates"
	)]
	pub sample_data: Option<PathBuf>,

	#[command(subcommand)]
	pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
	/// Execute the full entity → registry → packet happy-path flow.
	Walkthrough,
	/// Print the Markdown explainer for a specific concept.
	Docs {
		#[arg(value_enum)]
		topic: DocTopic,
	},
}
