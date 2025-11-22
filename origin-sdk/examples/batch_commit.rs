use origin_sdk::client::signer::MultiKeySigner;
use origin_sdk::{extrinsic::builder::DynamicCallBuilder, OriginClient};
use scale_value::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let signer = MultiKeySigner::from_seed("//Alice", "")?;
	let client = OriginClient::connect("ws://localhost:9944", signer.clone()).await?;

	let call1 = DynamicCallBuilder::new().call(
		"Entity",
		"set_attribute",
		vec![
			Value::from_bytes(b"entity-1"),
			Value::from_bytes(b"email"),
			Value::from_bytes(b"a@b.c"),
		],
	);
	let call2 = DynamicCallBuilder::new().call(
		"Entity",
		"set_attribute",
		vec![
			Value::from_bytes(b"entity-1"),
			Value::from_bytes(b"web"),
			Value::from_bytes(b"https://example.com"),
		],
	);

	let outcome = client.tx().batch().call(call1).call(call2).submit_and_wait_finalized().await?;
	println!("Batch submitted in block {:?} hash {:?}", outcome.block, outcome.hash);
	Ok(())
}
