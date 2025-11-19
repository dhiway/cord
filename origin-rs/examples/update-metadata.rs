use oc::{client::Client, flavors::ChainFlavor};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let client = Client::connect("ws://127.0.0.1:9944", ChainFlavor::Auto).await?;
	let raw = client.fetch_metadata_blob().await?;
	let flavor = client.flavor();
	let path = match flavor {
		ChainFlavor::OriginHub => "origin-rs/metadata/origin-hub.scale",
		ChainFlavor::Origin => "origin-rs/metadata/origin.scale",
		ChainFlavor::Auto => "origin-rs/metadata/origin.scale",
	};
	std::fs::write(path, raw)?;
	println!("metadata snapshot updated at {path}");
	Ok(())
}
