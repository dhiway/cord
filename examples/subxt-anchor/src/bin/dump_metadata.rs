use clap::Parser;
use color_eyre::eyre::Result;
use sp_core::OpaqueMetadata;
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Cli {
	#[arg(
		long,
		default_value = "examples/subxt-anchor/metadata/cord.scale",
		help = "Path to write the SCALE-encoded metadata blob"
	)]
	output: PathBuf,
}

fn main() -> Result<()> {
	color_eyre::install()?;
	let cli = Cli::parse();

	let opaque = OpaqueMetadata::new(cord_orb_runtime::Runtime::metadata().into());
	let bytes: &[u8] = opaque.as_ref();
	std::fs::write(&cli.output, bytes)?;
	println!("Wrote metadata to {}", cli.output.display());
	Ok(())
}
