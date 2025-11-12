use oc::{client::Client, flavors::ChainFlavor};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let client = Client::connect("ws://127.0.0.1:9944", ChainFlavor::Auto).await?;
	let raw = client.fetch_metadata_blob().await?;
	std::fs::write("cord-subxt-sdk/metadata/cord.scale", raw)?;
	println!("metadata snapshot updated");
	Ok(())
}
