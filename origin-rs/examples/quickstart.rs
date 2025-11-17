use anyhow::Result;
use oc::OriginClient;
use scale_value::value;

#[tokio::main]
async fn main() -> Result<()> {
	let url = std::env::var("ORIGIN_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".into());
	let client = OriginClient::connect(&url).await?;

	// Basic health/runtime info (via dynamic client)
	println!("connected to {url}");

	// Dynamic storage: System.Number
	if let Some(number) = client.storage_value("System", "Number", vec![]).await? {
		println!("best block: {:?}", number);
	}

	// Dynamic view: Entity.details (requires auth + token args; provide dummy example)
	let args = value!({
		"auth": {
			"signature": value! { "0x00" },
			"expiry": 0u32,
		},
		"token": "dummy-token-id",
	});
	let _ = client.call_view("Entity", "details", args).await.map_err(|e| {
		println!("view call failed (expected if dummy auth): {e}");
		e
	});
	Ok(())
}
