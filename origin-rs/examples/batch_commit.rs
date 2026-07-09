use oc::{client::signer::OriginSigner, types::OriginAccount, OriginClient};
use scale_value::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let acct = OriginAccount::from_dev("//Alice")?;
	let signer = OriginSigner::from_account(&acct)?;
	let client = OriginClient::connect("ws://localhost:9944").await?;
	let account = client.tx().using(signer.clone());

	let builder = oc::extrinsic::builder::DynamicCallBuilder::new();
	let call1 = builder.call("Entity", "set_entity_nym", vec![Value::from_bytes(b"nym-a")]);
	let call2 = builder.call("Entity", "remove_entity_nym", vec![Value::from_bytes(b"some-id")]);

	let outcome = account.batch().call(call1).call(call2).submit_and_wait_finalized().await?;

	println!("batch finalized in block {:?}", outcome.block);
	Ok(())
}
