use origin_sdk::{
	client::signer::MultiKeySigner, extrinsic::builder::DynamicCallBuilder, OriginClient,
};
use scale_value::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let signer = MultiKeySigner::from_seed("//Alice")?;
	let client = OriginClient::connect("ws://localhost:9944").await?.with_signer(signer.clone());

	let call = DynamicCallBuilder::new().call(
		"Entity",
		"set_attribute",
		vec![
			Value::from_bytes(b"entity-1"),
			Value::from_bytes(b"email"),
			Value::from_bytes(b"a@b.c"),
		],
	);

	let tx = client.metatx()?.sign_and_submit(call).await?;
	println!("Meta-tx submitted: {:?}", tx.hash);
	Ok(())
}
