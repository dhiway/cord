use origin_primitives::Ss58Identifier;
use origin_sdk::client::signer::MultiKeySigner;
use origin_sdk::OriginClient;
use serde_json::Value as JsonValue;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let _label = load_label().unwrap_or_else(|_| "demo".into());

	let signer = MultiKeySigner::from_seed("//Alice", "")?;
	let client = OriginClient::connect("ws://localhost:9910", signer.clone()).await?;

	let entity_id =
		Ss58Identifier::try_from(String::from("5FLSigC9H8J9tDFkhiBSGAL7iFusJqSQuJtVUXwwc7G7R6nW"))
			.map_err(|e| format!("{e:?}"))?;

	let entity = client.view().entity().overview(entity_id).await?;
	println!("Entity overview: {:?}", entity);
	Ok(())
}

fn load_label() -> Result<String, Box<dyn std::error::Error>> {
	let data = std::fs::read_to_string("origin-sdk/examples/sample_data/demo.json")?;
	let v: JsonValue = serde_json::from_str(&data)?;
	let label = v
		.get("entity")
		.and_then(|e| e.get("display"))
		.and_then(|d| d.as_str())
		.unwrap_or("demo")
		.replace("{label}", "demo");
	Ok(label)
}
