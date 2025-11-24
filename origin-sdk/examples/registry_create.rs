use origin_sdk::{extrinsic::calls::registry, OriginClient};
use origin_sdk::client::signer::MultiKeySigner;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let signer = MultiKeySigner::from_seed("//Alice")?;
	let client = OriginClient::connect("ws://localhost:9944").await?.with_signer(signer.clone());

	let schema = serde_json::json!({
		"attributes": [
			{ "key": "name", "kind": "Raw", "optional": false },
			{ "key": "email", "kind": "Raw", "optional": true }
		]
	});
	let config = serde_json::json!({ "version": 1 });

	let metadata = client.metadata();
	let call = registry::create_from_structs(&metadata, b"registry-01", &schema, &config)?;

	let outcome = client
		.tx()?
		.submit(&call.pallet, &call.function, call.args.clone())
		.await?
		.wait_finalized()
		.await?;
	println!("registry created in block {:?}", outcome.block);
	Ok(())
}
