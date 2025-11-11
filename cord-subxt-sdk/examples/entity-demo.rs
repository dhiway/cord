use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cord_primitives::{
	identifier::Ss58Identifier,
	view_api::{EntityAttributeHistoryRequest, EntitySubAccountsRequest},
};
use origin::{
	demo,
	demo::entity::{self, EntitySnapshot},
	params::config::CordConfig,
	query::auth::{AuthorizationBuilder, SignatureScheme},
	tx,
	types::{
		self,
		entity::{AttributeEntry, ElementJson},
	},
};
use serde_json;
use subxt::{blocks::ExtrinsicEvents, utils::AccountId32};

#[tokio::main]
async fn main() -> Result<()> {
	let label = demo::random_label("entity-demo");
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	let signer = tx::signer::dev_alice();
	let account_id = signer_account_id(&signer);
	let profile = demo::entity_profile(&label);

	let output_json = std::env::args().any(|arg| arg == "--json");
	let signed_auth = AuthorizationBuilder::from_signer(&signer, SignatureScheme::Sr25519, None)
		.context("failed to build view authorization")?;
	let view_auth = signed_auth.as_request().context("failed to convert view authorization")?;

	let (entity_token, created) =
		demo::ensure_entity_token(&client, &signer, &view_auth, &account_id, &profile).await?;
	if created {
		println!("Set entity profile for Alice (token {entity_token})");
	} else {
		println!("Reusing existing entity token {entity_token}");
	}
	let mut snapshot = EntitySnapshot::from_profile(&profile, &entity_token);
	println!("Entity token: {entity_token}");

	submit_attribute_rotation(&client, &signer, "email", format!("{label}@cord.dev")).await?;
	snapshot.set_email(format!("{label}@cord.dev"));
	submit_attribute_add(&client, &signer, "website", format!("https://{label}.cord.dev")).await?;
	snapshot.set_attribute("website", format!("https://{label}.cord.dev"));

	let history_req = EntityAttributeHistoryRequest {
		auth: view_auth.clone(),
		token: identifier_from_str(&entity_token)?,
	};
	let history = client.query().entity().attribute_history_entries(&history_req).await?;

	let sub_req = EntitySubAccountsRequest {
		auth: view_auth.clone(),
		token: identifier_from_str(&entity_token)?,
	};
	let sub_accounts = client.query().entity().sub_accounts(&sub_req).await?;
	snapshot.set_active_accounts(&sub_accounts);

	if output_json {
		let json = snapshot.to_json(&history);
		println!("{}", serde_json::to_string_pretty(&json)?);
	} else {
		snapshot.print_cli();
		entity::print_history_cli(&history);
		entity::print_accounts_cli(&sub_accounts);
	}

	Ok(())
}

async fn submit_attribute_rotation(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	key: &str,
	value: String,
) -> Result<()> {
	let entry = AttributeEntry {
		key_hex: types::to_key_hex_from_utf8(key),
		key_utf8: Some(key.into()),
		value: ElementJson::RawBase64(BASE64.encode(value)),
	};
	let call = client.tx().entity_rotate_attribute(entry).await?;
	submit_and_confirm(client, signer, call).await?;
	println!("Rotated attribute '{key}'");
	Ok(())
}

async fn submit_attribute_add(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	key: &str,
	value: String,
) -> Result<()> {
	let entry = AttributeEntry {
		key_hex: types::to_key_hex_from_utf8(key),
		key_utf8: Some(key.into()),
		value: ElementJson::RawBase64(BASE64.encode(value)),
	};
	let call = client.tx().entity_add_attributes(vec![entry]).await?;
	match submit_and_confirm(client, signer, call).await {
		Ok(_) => println!("Added attribute '{key}'"),
		Err(err) =>
			if err.to_string().contains("Entity::AttributeExists") {
				println!("Attribute '{key}' already present; skipping add step");
			} else {
				return Err(err);
			},
	}
	Ok(())
}

async fn submit_and_confirm(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	call: subxt::tx::DynamicPayload,
) -> Result<ExtrinsicEvents<CordConfig>> {
	Ok(client
		.tx()
		.sign_and_submit(call, signer, tx::TxOptions::default())
		.await?
		.wait_for_success()
		.await?)
}

fn signer_account_id(signer: &tx::signer::sr25519::Keypair) -> AccountId32 {
	<tx::signer::sr25519::Keypair as subxt::tx::Signer<CordConfig>>::account_id(signer)
}

fn identifier_from_str(ss58: &str) -> Result<Ss58Identifier> {
	Ss58Identifier::try_from(ss58.to_string())
		.map_err(|_| anyhow::anyhow!("invalid ss58 identifier"))
}
