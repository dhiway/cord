use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	println!("health: {:?}", client.health().await?);
	println!("runtime: {:?}", client.runtime_version().await?);
	println!("metadata hash: 0x{}", hex::encode(client.metadata_hash().await?));
	Ok(())
}
