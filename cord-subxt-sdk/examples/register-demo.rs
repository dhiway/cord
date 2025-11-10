use anyhow::{anyhow, Context, Result};
use codec::Decode;
use cord_primitives::identifier::Ss58Identifier;
use origin::{
	params::config::CordConfig,
	query::auth::{AuthorizationBuilder, SignatureScheme},
	tx::{self, TxOptions},
	types,
};
use serde_json::json;
use subxt::blocks::ExtrinsicEvents;

#[tokio::main]
async fn main() -> Result<()> {
	let label = unique_label("register-demo");
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	let signer = tx::signer::dev_alice();
	let account_id =
		<tx::signer::sr25519::Keypair as subxt::tx::Signer<CordConfig>>::account_id(&signer);
	let auth = AuthorizationBuilder::from_signer(&signer, SignatureScheme::Sr25519, None)
		.context("failed to build view authorization")?;

	let entity_token = ensure_entity_token(&client, &signer, &auth, &account_id, &label).await?;
	println!("Maintainer entity token {entity_token}");
	let registry_spec = registry_blueprint(&label);
	let registry_id = create_registry(&client, &signer, registry_spec).await?;
	println!("Minted registry token {registry_id}\n");

	let info = client.query().register().info_json(&auth, &registry_id).await?;
	println!("registry info:\n{}", serde_json::to_string_pretty(&info)?);

	let schema = client.query().register().schema(&auth, &registry_id).await?;
	println!("\nattribute schema ({} keys):", schema.attributes.len());
	for attr in schema.attributes {
		println!(
			"- {} ({:?}){}",
			as_utf8(&attr.key),
			attr.kind,
			if attr.optional { " [optional]" } else { "" }
		);
	}

	let lookups = client.query().register().lookup_specs(&auth, &registry_id).await?;
	println!("\nlookup specs: {lookups:?}");
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
	Err(anyhow!("RegistryCreated event not found in block {:?}", events.block_hash()))
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
		"info": format!("Demo registry for {label}"),
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

fn as_utf8(bytes: &[u8]) -> String {
	String::from_utf8_lossy(bytes).into_owned()
}

fn ss58_string(id: &Ss58Identifier) -> String {
	String::from_utf8_lossy(id.as_bytes()).into_owned()
}

fn unique_label(prefix: &str) -> String {
	let mut rnd = [0u8; 4];
	let _ = getrandom::getrandom(&mut rnd);
	format!("{prefix}-{}", hex::encode(rnd))
}
