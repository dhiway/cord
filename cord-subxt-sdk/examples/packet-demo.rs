use anyhow::{anyhow, Context, Result};
use codec::Decode;
use cord_primitives::identifier::Ss58Identifier;
use origin::{
	params::config::CordConfig,
	query::auth::{AuthorizationBuilder, SignatureScheme},
	tx::{self, TxOptions},
};
use serde_json::json;
use sp_core::hashing::blake2_256;
use subxt::blocks::ExtrinsicEvents;

#[tokio::main]
async fn main() -> Result<()> {
	let label = unique_label("packet-demo");
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	let signer = tx::signer::dev_alice();
	let account_id =
		<tx::signer::sr25519::Keypair as subxt::tx::Signer<CordConfig>>::account_id(&signer);
	let auth = AuthorizationBuilder::from_signer(&signer, SignatureScheme::Sr25519, None)
		.context("failed to build view authorization")?;

	let entity_token = ensure_entity_token(&client, &signer, &auth, &account_id, &label).await?;
	let registry_spec = registry_blueprint(&label);
	let registry_id = create_registry(&client, &signer, registry_spec).await?;
	println!("Minted registry {registry_id}");

	let schema_view = client.query().register().registry_info(&auth, &registry_id).await?;
	println!(
		"Registry attributes: {:?}",
		schema_view
			.attributes
			.iter()
			.map(|attr| String::from_utf8_lossy(&attr.key).into_owned())
			.collect::<Vec<_>>()
	);

	let packet_payload = packet_attributes(&label, &entity_token);
	let packet_id =
		create_packet(&client, &signer, &registry_id, packet_payload, &schema_view).await?;
	println!("Created packet token {packet_id}");

	let packet_view = client
		.query()
		.register()
		.packet_snapshot(&auth, &registry_id, &packet_id, None)
		.await?;
	println!("\nPacket snapshot:\n{}", serde_json::to_string_pretty(&packet_view)?);

	let timeline = client.query().token().timeline(&auth, &packet_id, None, Some(10)).await?;
	println!("\nToken timeline:\n{}", serde_json::to_string_pretty(&timeline)?);
	Ok(())
}

async fn ensure_entity_token(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	auth: &origin::query::auth::ViewAuthorization,
	account_id: &subxt::utils::AccountId32,
	label: &str,
) -> Result<String> {
	if let Some(token) = client.query().entity().account_token(auth, account_id).await? {
		return Ok(token);
	}

	let info = serde_json::json!({
		"display": format!("CORD maintainer {label}"),
		"legal": "CORD Maintainer Demo",
		"web": format!("https://maintainers.cord/{label}"),
		"email": format!("{label}@cord.dev"),
	});
	let call = client.tx().entity_set_info_json(info).await?;
	let events = submit(client, signer, call).await?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Entity" && ev.variant_name() == "EntityInfoSet" {
			let mut cursor = ev.field_bytes();
			let _: subxt::utils::AccountId32 = Decode::decode(&mut cursor)?;
			let token: Ss58Identifier = Decode::decode(&mut cursor)?;
			return Ok(ss58_string(&token));
		}
	}
	Err(anyhow!("EntityInfoSet event not found in block {:?}", events.block_hash()))
}

async fn create_registry(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	spec: serde_json::Value,
) -> Result<String> {
	let call = client.tx().register_create_registry_json(spec).await?;
	let events = submit(client, signer, call).await?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Register" && ev.variant_name() == "RegistryCreated" {
			let mut cursor = ev.field_bytes();
			let registry: Ss58Identifier = Decode::decode(&mut cursor)?;
			return Ok(ss58_string(&registry));
		}
	}
	Err(anyhow!("registry token not found in block {:?}", events.block_hash()))
}

async fn create_packet(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	registry_id: &str,
	attributes: serde_json::Value,
	schema_view: &origin::types::registry::RegistryInfoView,
) -> Result<String> {
	let call = client.tx().packet_create_json(registry_id, attributes, schema_view).await?;
	let events = submit(client, signer, call).await?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Register" && ev.variant_name() == "PacketCreated" {
			let mut cursor = ev.field_bytes();
			let registry: Ss58Identifier = Decode::decode(&mut cursor)?;
			let packet: Ss58Identifier = Decode::decode(&mut cursor)?;
			println!("Packet anchored under {}", ss58_string(&registry));
			return Ok(ss58_string(&packet));
		}
	}
	Err(anyhow!("packet token not found in block {:?}", events.block_hash()))
}

async fn submit(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	call: subxt::tx::DynamicPayload,
) -> Result<ExtrinsicEvents<origin::params::config::CordConfig>> {
	Ok(client
		.tx()
		.sign_and_submit(call, signer, TxOptions::default())
		.await?
		.wait_for_success()
		.await?)
}

fn registry_blueprint(label: &str) -> serde_json::Value {
	json!({
		"info": format!("Packet registry for {label}"),
		"kind": "raw",
		"attribute_schema": [
			{"key": "record_id", "type": "raw"},
			{"key": "controller", "type": "token"},
			{"key": "payload_hash", "type": "hash"},
			{"key": "payload_salt", "type": "raw"},
			{"key": "expires_at", "type": "u64", "optional": true},
			{"key": "notes", "type": "raw", "optional": true}
		],
		"token_spec": {"type": "combo", "keys": ["record_id", "controller"]},
		"lookup_specs": [
			{"type": "combo", "keys": ["record_id", "controller"]},
			{"type": "single", "key": "payload_hash"},
			{"type": "single", "key": "payload_salt"}
		],
	})
}

fn packet_attributes(label: &str, controller: &str) -> serde_json::Value {
	let hash = blake2_256(format!("payload::{label}").as_bytes());
	json!({
		"record_id": format!("{label}-packet"),
		"controller": controller,
		"payload_hash": format!("0x{}", hex::encode(hash)),
		"payload_salt": format!("salt::{label}"),
		"expires_at": 1_893_456_000u64,
		"notes": null,
	})
}

fn ss58_string(id: &Ss58Identifier) -> String {
	String::from_utf8_lossy(id.as_bytes()).into_owned()
}

fn unique_label(prefix: &str) -> String {
	let mut rnd = [0u8; 4];
	let _ = getrandom::getrandom(&mut rnd);
	format!("{prefix}-{}", hex::encode(rnd))
}
