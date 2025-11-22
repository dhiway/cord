use origin_sdk::OriginClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let client = OriginClient::connect("ws://localhost:9944").await?;
	let _entity: origin_sdk::types::EntityOverview = client
		.view()
		.entity()
		.overview(
			dummy_auth(),
			origin_primitives::Ss58Identifier::try_from("5FLSigC9H8J9tDFkhiBSGAL7iFusJqSQuJtVUXwwc7G7R6nW").unwrap(),
		)
		.await?;
	println!("entity overview fetched");
	Ok(())
}

fn dummy_auth() -> origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
> {
	use codec::Encode;
	let payload = b"auth".to_vec();
	let signature = origin_primitives::Signature::from(origin_primitives::Signature::from(
		sp_core::sr25519::Signature::from_raw([0u8; 64]),
	));
	origin_primitives::Authorization {
		account: origin_primitives::AccountId::from(sp_core::sr25519::Public::from_raw([0u8; 32])),
		payload,
		signature,
	}
}
