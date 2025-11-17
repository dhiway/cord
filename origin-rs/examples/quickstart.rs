use anyhow::Result;
use oc::{DynamicApis, OriginClient};
use scale_value::value;

#[tokio::main]
async fn main() -> Result<()> {
	let url = std::env::var("ORIGIN_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".into());
	let client = OriginClient::connect(&url).await?;
	let dyn_apis = DynamicApis::new(client.clone());

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
	let _ = dyn_apis.entity().details(args).await.map_err(|e| {
		println!("view call failed (expected if dummy auth): {e}");
		e
	});

	// Dynamic events (finalized)
	let mut events = dyn_apis.subscribe_finalized_events(64).await?;
	if let Some(ev) = events.next().await {
		println!("first event: {}::{} at #{}", ev.pallet, ev.variant, ev.block_number);
	}
	Ok(())
}
