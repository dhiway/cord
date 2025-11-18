use anyhow::Result;
use oc::sdk::OriginClient;
use origin_primitives::view_api::AuthorizationRequest;

/// Dummy auth: in real use, sign properly and fill payload.
fn fake_auth() -> AuthorizationRequest {
	use origin_primitives::view_api::AuthorizationPayload;
	use sp_core::sr25519;
	let payload: AuthorizationPayload = AuthorizationPayload::try_from(vec![0u8; 8]).unwrap();
	let sig = sp_runtime::MultiSignature::from(sr25519::Signature::from_raw([0u8; 64]));
	let account = sp_core::sr25519::Public::from_raw([1u8; 32]).into();
	AuthorizationRequest { account, payload, signature: sig }
}

#[tokio::main]
async fn main() -> Result<()> {
	let url = std::env::var("ORIGIN_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".into());
	let api = OriginClient::connect(&url).await?;

	let auth = fake_auth();
	let entity_id = "DummyEntityId".try_into().unwrap_or_default();
	let register_id = "DummyRegistryId".try_into().unwrap_or_default();
	let packet_id = "DummyPacketId".try_into().unwrap_or_default();

	// Entity overview: info + history + timeline (each capped at 20)
	let entity_overview = api.entities().overview(&auth, &entity_id).await;
	println!("entity overview: {:?}", entity_overview);

	// Register overview: info + lookup specs (lightweight)
	let register_overview = api.registers().overview(&auth, &register_id).await;
	println!("register overview: {:?}", register_overview);

	// Packet overview: metadata + state + timeline (metadata via dedicated view)
	let packet_overview = api.packets().overview(&auth, &register_id, &packet_id, None).await;
	println!("packet overview: {:?}", packet_overview);

	Ok(())
}
