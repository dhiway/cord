use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	match client
		.views()
		.call_json("Register", "info", serde_json::json!({"token": "demo"}))
		.await
	{
		Ok(value) => println!("register view output: {value:#}",),
		Err(err) => eprintln!("register view not available yet: {err}"),
	}
	Ok(())
}
