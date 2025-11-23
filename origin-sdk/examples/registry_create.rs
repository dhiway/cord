use origin_sdk::client::signer::MultiKeySigner;
use origin_sdk::{extrinsic::builder::DynamicCallBuilder, OriginClient};
use scale_value::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let signer = MultiKeySigner::from_seed("//Alice")?;
	let client = OriginClient::connect("ws://localhost:9944").await?.with_signer(signer.clone());

	// Demo args: registry id + info bytes; adjust to your chain schema.
	let call = DynamicCallBuilder::new().call(
		"Register",
		"create",
		vec![Value::from_bytes(b"demo-registry"), Value::from_bytes(b"demo-info")],
	);

	let tx = client.tx()?.submit(&call.pallet, &call.function, call.args).await?;
	println!("Submitted registry create hash: {:?}", tx.hash);
	Ok(())
}
