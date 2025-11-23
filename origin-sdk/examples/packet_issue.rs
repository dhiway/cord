use origin_sdk::client::signer::MultiKeySigner;
use origin_sdk::{extrinsic::builder::DynamicCallBuilder, OriginClient};
use scale_value::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let signer = MultiKeySigner::from_seed("//Alice")?;
	let client = OriginClient::connect("ws://localhost:9944").await?.with_signer(signer.clone());

	let call = DynamicCallBuilder::new().call(
		"Packet",
		"issue",
		vec![Value::from_bytes(b"demo-registry"), Value::from_bytes(b"demo-packet-body")],
	);
	let outcome = client.tx()?.submit(&call.pallet, &call.function, call.args).await?;
	println!("Packet issue submitted: {:?}", outcome.hash);
	Ok(())
}
