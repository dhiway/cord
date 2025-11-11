pub mod entity;
use crate::{
	client::Client,
	error::{Error, Result},
	params::config::CordConfig,
	tx::{self, TxOptions},
};
use codec::Decode;
use cord_primitives::{
	identifier::Ss58Identifier,
	registry::RegistryInfoView,
	view_api::{EntityAccountTokenRequest, EntityInfoBytesRequest, ViewRequestAuth},
};
use getrandom::getrandom;
use hex;
use serde_json::{json, Value as JsonValue};
use sp_runtime::AccountId32 as RuntimeAccount;
use subxt::{blocks::ExtrinsicEvents, utils::AccountId32};

pub fn random_label(prefix: &str) -> String {
	let mut rnd = [0u8; 4];
	let _ = getrandom(&mut rnd);
	format!("{prefix}-{}", hex::encode(rnd))
}

pub fn ss58_string(id: &Ss58Identifier) -> String {
	String::from_utf8_lossy(id.as_bytes()).into_owned()
}

pub async fn ensure_entity_token(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	view_auth: &ViewRequestAuth,
	account_id: &AccountId32,
	profile: &JsonValue,
) -> Result<(String, bool)> {
	let raw: [u8; 32] = *account_id.as_ref();
	let account = RuntimeAccount::from(raw);
	let request = EntityAccountTokenRequest { auth: view_auth.clone(), account };
	if let Some(token) = client.query().entity().account_token(&request).await? {
		return Ok((token, false));
	}

	let call = client.tx().entity_set_info_json(profile.clone()).await?;
	let events = submit_and_wait(client, signer, call).await?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Entity" && ev.variant_name() == "EntityInfoSet" {
			let mut cursor = ev.field_bytes();
			let _: subxt::utils::AccountId32 = decode_value(&mut cursor)?;
			let token: Ss58Identifier = decode_value(&mut cursor)?;
			return Ok((ss58_string(&token), true));
		}
	}
	Err(Error::NotFound("EntityInfoSet event not found".into()))
}

pub async fn create_registry(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	spec: JsonValue,
) -> Result<Ss58Identifier> {
	let call = client.tx().register_create_registry_json(spec).await?;
	let events = submit_and_wait(client, signer, call).await?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Register" && ev.variant_name() == "RegistryCreated" {
			let mut cursor = ev.field_bytes();
			let registry: Ss58Identifier = decode_value(&mut cursor)?;
			return Ok(registry);
		}
	}
	Err(Error::NotFound("RegistryCreated event not found".into()))
}

pub async fn create_packet(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	registry_ss58: &str,
	attributes: JsonValue,
	registry_view: &RegistryInfoView,
) -> Result<Ss58Identifier> {
	let call = client.tx().packet_create_json(registry_ss58, attributes, registry_view).await?;
	let events = submit_and_wait(client, signer, call).await?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Register" && ev.variant_name() == "PacketCreated" {
			let mut cursor = ev.field_bytes();
			let _registry: Ss58Identifier = decode_value(&mut cursor)?;
			let packet: Ss58Identifier = decode_value(&mut cursor)?;
			return Ok(packet);
		}
	}
	Err(Error::NotFound("PacketCreated event not found".into()))
}

pub async fn submit_and_wait(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	call: subxt::tx::DynamicPayload,
) -> Result<ExtrinsicEvents<CordConfig>> {
	Ok(client
		.tx()
		.sign_and_submit(call, signer, TxOptions::default())
		.await?
		.wait_for_success()
		.await?)
}

pub fn registry_blueprint(label: &str) -> JsonValue {
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

pub fn packet_attributes(label: &str, controller: &str) -> JsonValue {
	let hash = sp_core::hashing::blake2_256(format!("payload::{label}").as_bytes());
	json!({
		"record_id": format!("{label}-packet"),
		"controller": controller,
		"payload_hash": format!("0x{}", hex::encode(hash)),
		"payload_salt": format!("salt::{label}"),
		"expires_at": 1_893_456_000u64,
		"notes": null,
	})
}

pub fn entity_profile(label: &str) -> JsonValue {
	json!({
		"display": format!("CORD SDK entity run {label}"),
		"legal": "CORD Demo LLC",
		"web": format!("https://demo.cord/{label}"),
		"email": format!("{label}@cord.dev"),
		"twitter": format!("@{label}"),
		"attributes": {
			"support": "support@cord.dev"
		}
	})
}

pub async fn fetch_entity_info_bytes(
	client: &Client,
	auth: &ViewRequestAuth,
	token: &Ss58Identifier,
) -> Result<Option<Vec<u8>>> {
	let req = EntityInfoBytesRequest { auth: auth.clone(), token: token.clone() };
	client.query().entity().entity_info_bytes(&req).await
}

fn decode_value<T: Decode>(cursor: &mut &[u8]) -> Result<T> {
	T::decode(cursor).map_err(|e| Error::Codec(e.to_string()))
}
