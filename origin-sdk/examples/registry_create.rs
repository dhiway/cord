use origin_sdk::OriginClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let client = OriginClient::connect("ws://localhost:9944").await?;
	let _ = client.tx().submit(
		"Registry",
		"create",
		Vec::<subxt::dynamic::Value>::new(),
		&DummySigner {},
	).await?;
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
