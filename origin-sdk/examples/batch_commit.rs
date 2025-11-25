use origin_sdk::{
	client::signer::MultiKeySigner, extrinsic::builder::DynamicCallBuilder, OriginClient,
};
use scale_value::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let signer = MultiKeySigner::from_seed("//Alice")?;
	let client = OriginClient::connect("ws://localhost:9944").await?.with_signer(signer.clone());

	let builder = DynamicCallBuilder::new();
	let call1 = builder.call("Entity", "set_entity_nym", vec![Value::from_bytes(b"nym-a")]);
	let call2 = builder.call("Entity", "remove_entity_nym", vec![Value::from_bytes(b"some-id")]);

	let outcome = client.batch()?.call(call1).call(call2).submit_and_wait_finalized().await?;

	println!("batch finalized in block {:?}", outcome.block);
	Ok(())
}
