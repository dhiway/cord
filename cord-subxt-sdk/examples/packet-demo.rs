use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	println!("Connected to node for packet demo: runtime {:?}", client.runtime_version().await?);
	println!("Packet extrinsic builders will be added in the next iteration.");
	Ok(())
}
