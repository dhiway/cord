use origin_primitives::Ss58Identifier;
use origin_sdk::{client::signer::OriginSigner, types::OriginAccount, OriginClient};
use serde_json::Value as JsonValue;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let _label = load_label().unwrap_or_else(|_| "demo".into());

	let account = OriginAccount::from_dev("//Alice")?;
	let signer = OriginSigner::from_account(&account)?;
	let client = OriginClient::connect("ws://localhost:9910").await?;

	let entity_id =
		Ss58Identifier::try_from(String::from("5FLSigC9H8J9tDFkhiBSGAL7iFusJqSQuJtVUXwwc7G7R6nW"))
			.map_err(|e| format!("{e:?}"))?;

	let entity = client.query().using(signer).entity().overview(entity_id).await?;
	match entity {
		Some(v) => println!("Entity overview: {:?}", v),
		None => println!("Entity not found or authorization failed"),
	}
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
