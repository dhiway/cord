use anyhow::{Context, Result};
use cord_primitives::view_api::{RegisterInfoRequest, RegisterPacketRequest, TokenTimelineRequest};
use origin::{
	demo,
	params::config::CordConfig,
	query::auth::{AuthorizationBuilder, SignatureScheme},
	tx,
};
use serde_json;

#[tokio::main]
async fn main() -> Result<()> {
	let label = demo::random_label("packet-demo");
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	let signer = tx::signer::dev_alice();
	let account_id =
		<tx::signer::sr25519::Keypair as subxt::tx::Signer<CordConfig>>::account_id(&signer);
	let signed_auth = AuthorizationBuilder::from_signer(&signer, SignatureScheme::Sr25519, None)
		.context("failed to build view authorization")?;
	let view_auth = signed_auth.as_request().context("failed to convert view authorization")?;

	let profile = demo::entity_profile(&label);
	let (entity_token, _) =
		demo::ensure_entity_token(&client, &signer, &view_auth, &account_id, &profile).await?;

	let registry_spec = demo::registry_blueprint(&label);
	let registry_id = demo::create_registry(&client, &signer, registry_spec).await?;
	let registry_ss58 = demo::ss58_string(&registry_id);
	println!("Minted registry {registry_ss58}");

	let info_req = RegisterInfoRequest { auth: view_auth.clone(), registry: registry_id.clone() };
	let schema_view = client.query().register().registry_info(&info_req).await?;
	println!(
		"Registry attributes: {:?}",
		schema_view
			.attributes
			.iter()
			.map(|attr| String::from_utf8_lossy(&attr.key).into_owned())
			.collect::<Vec<_>>()
	);

	let packet_payload = demo::packet_attributes(&label, &entity_token);
	let packet_id =
		demo::create_packet(&client, &signer, &registry_ss58, packet_payload, &schema_view).await?;
	println!("Created packet token {}", demo::ss58_string(&packet_id));

	let packet_req = RegisterPacketRequest {
		auth: view_auth.clone(),
		registry: registry_id.clone(),
		packet: packet_id.clone(),
		version: None,
	};
	let packet_view = client.query().register().packet_snapshot(&packet_req).await?;
	println!("\nPacket snapshot:\n{}", serde_json::to_string_pretty(&packet_view)?);

	let timeline_req = TokenTimelineRequest {
		auth: view_auth.clone(),
		token: packet_id,
		start: None,
		limit: Some(10),
	};
	let timeline = client.query().token().timeline(&timeline_req).await?;
	println!("\nToken timeline:\n{}", serde_json::to_string_pretty(&timeline)?);
	Ok(())
}
