use origin_sdk::OriginClient;
use subxt::dynamic::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let client = OriginClient::connect("ws://localhost:9944").await?;
	let call = client.call().call("Entity", "set_info", vec![]);
	let wrapped = client.metatx().wrap(call);
	println!("meta-tx wrapper: {:?}", wrapped);
	Ok(())
}
