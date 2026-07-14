#[path = "../chain_spec.rs"]
mod chain_spec;
#[path = "../proof_campaign/mod.rs"]
mod proof_campaign;

fn main() -> color_eyre::eyre::Result<()> {
	color_eyre::install()?;
	Ok(proof_campaign::config::run()?)
}
