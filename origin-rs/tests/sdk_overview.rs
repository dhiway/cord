use oc::sdk::{
	types::{EntityOverview, PacketOverview, RegisterOverview},
	OriginClient,
};
use origin_primitives::view_api::AuthorizationRequest;

fn fake_auth() -> AuthorizationRequest {
	use origin_primitives::view_api::AuthorizationPayload;
	use sp_core::sr25519;
	let payload: AuthorizationPayload = AuthorizationPayload::try_from(vec![0u8; 8]).unwrap();
	let sig = sp_runtime::MultiSignature::from(sr25519::Signature::from_raw([0u8; 64]));
	let account = sp_core::sr25519::Public::from_raw([1u8; 32]).into();
	AuthorizationRequest { account, payload, signature: sig }
}

// Smoke test: exercise overview construction paths; skip if no node available.
#[tokio::test]
async fn overview_construction_smoke() {
	let url = std::env::var("ORIGIN_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".into());
	let api = match OriginClient::connect(&url).await {
		Ok(api) => api,
		Err(_) => return, // no node running; skip
	};

	let auth = fake_auth();
	let entity_id = "DummyEntityId".try_into().unwrap_or_default();
	let register_id = "DummyRegistryId".try_into().unwrap_or_default();
	let packet_id = "DummyPacketId".try_into().unwrap_or_default();

	let _ = api.entities().overview(&auth, &entity_id).await;
	let _ = api.registers().overview(&auth, &register_id).await;
	let _ = api.packets().overview(&auth, &register_id, &packet_id, None).await;

	// Ensure DTOs are in scope
	let _eo: Option<EntityOverview> = None;
	let _ro: Option<RegisterOverview> = None;
	let _po: Option<PacketOverview> = None;
}
