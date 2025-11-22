use origin_sdk::OriginClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let client = OriginClient::connect("ws://localhost:9944").await?;
	let _entity: Result<origin_sdk::types::EntityStateView, _> =
		client.view().call("Entity", "overview", Vec::<subxt::dynamic::Value>::new()).await;
	println!("connected to {} pallets", client.online().metadata().pallets().len());
	Ok(())
}
