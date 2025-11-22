use origin_sdk::OriginClient;
use subxt::dynamic::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let client = OriginClient::connect("ws://localhost:9944").await?;
	let calls = vec![
		client.call().call("Entity", "set_info", vec![]),
		client.call().call("Entity", "set_info", vec![]),
	];
	let values: Vec<Value> = calls
		.into_iter()
		.map(|c| Value::unnamed_composite(vec![Value::from(c.pallet), Value::from(c.function), Value::from(c.args)]))
		.collect();
	let _ = client.tx().batch_submit(values, &DummySigner {}).await?;
	Ok(())
}

struct DummySigner;

impl origin_sdk::client::signer::Signer for DummySigner {
	fn account_id(&self) -> subxt::utils::AccountId32 {
		subxt::utils::AccountId32::new([0u8; 32])
	}

	fn sign(&self, _payload: &[u8]) -> subxt::utils::MultiSignature {
		subxt::utils::MultiSignature::from(subxt::utils::sr25519::Signature::from_raw([0u8; 64]))
	}
}
