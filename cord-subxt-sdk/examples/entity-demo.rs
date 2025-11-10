use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use codec::Decode;
use cord_primitives::{identifier::Ss58Identifier, packet::Element as RuntimeElement};
use frame_support::traits::ConstU32;
use origin::{
	params::config::CordConfig,
	query::auth::{AuthorizationBuilder, SignatureScheme, ViewAuthorization},
	tx::{self, TxOptions},
	types::{self, entity::AttributeEntry, entity::ElementJson},
};
use pallet_entity::entity::EntityInfo as RuntimeEntityInfo;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use subxt::{blocks::ExtrinsicEvents, utils::AccountId32};
use tokio::time::{sleep, Duration};

const ENTITY_MAX_RAW: u32 = 1024;
const ENTITY_MAX_ATTRS: u32 = 32;
type EntityInfoPacket = RuntimeEntityInfo<ConstU32<ENTITY_MAX_RAW>, ConstU32<ENTITY_MAX_ATTRS>>;
type EntityElement = RuntimeElement<ConstU32<ENTITY_MAX_RAW>>;

#[tokio::main]
async fn main() -> Result<()> {
	let label = unique_label("entity-demo");
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	let signer = tx::signer::dev_alice();
	let account_id = signer_account_id(&signer);
	let profile_template = entity_profile(&label);
	let mut ctx = ensure_entity_token(&client, &signer, &account_id, &profile_template).await?;
	let entity_token = ctx.token.clone();

	let new_email = format!("{label}@cord.dev");
	if submit_attribute_rotation(&client, &signer, "email", new_email.clone()).await? {
		ctx.snapshot.set_email(new_email);
	}

	let website = format!("https://{label}.cord.dev");
	if submit_attribute_add(&client, &signer, "website", website.clone()).await? {
		ctx.snapshot.set_attribute("website", website);
	}

	let final_snapshot = fetch_entity_snapshot(&client, &signer, &entity_token)
		.await?
		.unwrap_or(ctx.snapshot);

	let history_auth = view_auth(&signer)?;
	let history_json = client
		.query()
		.entity()
		.attribute_history_json(&history_auth, &entity_token)
		.await?;
	let history: HistoryEnvelope = serde_json::from_value(history_json)?;

	println!("Entity token: {entity_token}\n");
	print_snapshot(&entity_token, &final_snapshot);
	print_history(&history.data);
	Ok(())
}

async fn ensure_entity_token(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	account_id: &AccountId32,
	profile_template: &serde_json::Value,
) -> Result<EntityContext> {
	if let Some(token) = fetch_account_token(client, signer, account_id).await? {
		println!("Reusing existing entity token {token}");
		let snapshot = fetch_entity_snapshot(client, signer, &token)
			.await?
			.unwrap_or_else(EntitySnapshot::unknown);
		return Ok(EntityContext { token, snapshot });
	}

	match submit_entity_info(client, signer, profile_template.clone()).await {
		Ok(token) => {
			if let Some(resolved) = fetch_account_token(client, signer, account_id).await? {
				if resolved != token {
					println!(
						"Warning: account_token view returned {resolved} but event yielded {token}; continuing"
					);
				}
			} else if let Some(storage_token) =
				fetch_account_token_storage(client, account_id, "post-set-info").await?
			{
				println!(
					"View auth denied but storage lookup resolved token {storage_token}; continuing"
				);
			}
			let snapshot = fetch_entity_snapshot(client, signer, &token)
				.await?
				.unwrap_or_else(|| EntitySnapshot::from_profile(profile_template));
			Ok(EntityContext { token, snapshot })
		},
		Err(err) if err.to_string().contains("EntitySubAccount") => {
			let account_hex = account_hex(account_id);
			println!(
				"Entity already exists, skipping profile provisioning (account {account_hex})"
			);
			for attempt in 0..12 {
				if let Some(token) = fetch_account_token(client, signer, account_id).await? {
					println!("Resolved entity token {token} on attempt {attempt}");
					let snapshot = fetch_entity_snapshot(client, signer, &token)
						.await?
						.unwrap_or_else(EntitySnapshot::unknown);
					return Ok(EntityContext { token, snapshot });
				}
				println!("Attempt {attempt}: entity token still missing for account {account_hex}",);
				sleep(Duration::from_millis(250)).await;
			}
			if let Some(token) =
				fetch_account_token_storage(client, account_id, "post-duplicate").await?
			{
				println!("Storage fallback resolved entity token {token}; continuing");
				let snapshot = fetch_entity_snapshot(client, signer, &token)
					.await?
					.unwrap_or_else(EntitySnapshot::unknown);
				return Ok(EntityContext { token, snapshot });
			}
			Err(
				anyhow!("entity token still missing after duplicate error (account {account_hex})",),
			)
		},
		Err(err) => Err(err),
	}
}

async fn submit_entity_info(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	info_packet: serde_json::Value,
) -> Result<String> {
	let call = client.tx().entity_set_info_json(info_packet).await?;
	let events = submit_and_confirm(client, signer, call).await?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Entity" && ev.variant_name() == "EntityInfoSet" {
			let mut cursor = ev.field_bytes();
			let _: subxt::utils::AccountId32 = Decode::decode(&mut cursor)?;
			let token: Ss58Identifier = Decode::decode(&mut cursor)?;
			println!("Set entity profile for Alice (token {})", ss58_string(&token));
			return Ok(ss58_string(&token));
		}
	}
	Err(anyhow!("EntityInfoSet event not found in block log"))
}

async fn submit_attribute_rotation(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	key: &str,
	value: String,
) -> Result<bool> {
	let entry = AttributeEntry {
		key_hex: types::to_key_hex_from_utf8(key),
		key_utf8: Some(key.into()),
		value: ElementJson::RawBase64(BASE64.encode(value)),
	};
	let call = client.tx().entity_rotate_attribute(entry).await?;
	submit_and_confirm(client, signer, call).await?;
	println!("Rotated attribute '{key}'");
	Ok(true)
}

async fn submit_attribute_add(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	key: &str,
	value: String,
) -> Result<bool> {
	let entry = AttributeEntry {
		key_hex: types::to_key_hex_from_utf8(key),
		key_utf8: Some(key.into()),
		value: ElementJson::RawBase64(BASE64.encode(value)),
	};
	let call = client.tx().entity_add_attributes(vec![entry]).await?;
	match submit_and_confirm(client, signer, call).await {
		Ok(_) => {
			println!("Added attribute '{key}'");
			return Ok(true);
		},
		Err(err) if err.to_string().contains("Entity::AttributeExists") => {
			println!("Attribute '{key}' already present; skipping add step");
		},
		Err(err) => return Err(err),
	}
	Ok(false)
}

fn entity_profile(label: &str) -> serde_json::Value {
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

async fn submit_and_confirm(
	client: &origin::Client,
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

fn unique_label(prefix: &str) -> String {
	let mut rnd = [0u8; 4];
	let _ = getrandom::getrandom(&mut rnd);
	format!("{prefix}-{}", hex::encode(rnd))
}

fn ss58_string(id: &Ss58Identifier) -> String {
	String::from_utf8_lossy(id.as_bytes()).into_owned()
}

fn signer_account_id(signer: &tx::signer::sr25519::Keypair) -> AccountId32 {
	<tx::signer::sr25519::Keypair as subxt::tx::Signer<CordConfig>>::account_id(signer)
}

fn account_hex(account: &AccountId32) -> String {
	let raw: &[u8] = account.as_ref();
	format!("0x{}", hex::encode(raw))
}

async fn fetch_account_token(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	account_id: &AccountId32,
) -> Result<Option<String>> {
	let auth = view_auth(signer)?;
	Ok(client.query().entity().account_token(&auth, account_id).await?)
}

async fn fetch_account_token_storage(
	client: &origin::Client,
	account_id: &AccountId32,
	context: &str,
) -> Result<Option<String>> {
	if let Some(token) = client.state().entity_token_of_account(account_id).await? {
		let ss58 = ss58_string(&token);
		println!("[storage] ctx={context} resolved token {ss58}");
		Ok(Some(ss58))
	} else {
		Ok(None)
	}
}

fn view_auth(signer: &tx::signer::sr25519::Keypair) -> Result<ViewAuthorization> {
	AuthorizationBuilder::from_signer(signer, SignatureScheme::Sr25519, None)
		.context("failed to build view authorization")
}

async fn fetch_entity_snapshot(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	token_ss58: &str,
) -> Result<Option<EntitySnapshot>> {
	let auth = view_auth(signer)?;
	let bytes = client.query().entity().entity_info_bytes(&auth, token_ss58).await?;
	let Some(raw) = bytes else {
		return Ok(None);
	};
	let info = EntityInfoPacket::decode(&mut &raw[..])
		.map_err(|e| anyhow!("failed to decode entity info: {e}"))?;
	Ok(Some(EntitySnapshot::from_entity_info(&info)))
}

#[derive(Deserialize)]
struct HistoryEnvelope {
	data: Vec<types::entity::InfoAttributeHistoryEntry>,
}

struct EntityContext {
	token: String,
	snapshot: EntitySnapshot,
}

#[derive(Clone, Default)]
struct EntitySnapshot {
	display: Option<String>,
	legal: Option<String>,
	web: Option<String>,
	email: Option<String>,
	twitter: Option<String>,
	attributes: BTreeMap<String, String>,
}

impl EntitySnapshot {
	fn from_profile(profile: &serde_json::Value) -> Self {
		let mut snapshot = Self::unknown();
		snapshot.display = profile.get("display").and_then(|v| v.as_str()).map(str::to_owned);
		snapshot.legal = profile.get("legal").and_then(|v| v.as_str()).map(str::to_owned);
		snapshot.web = profile.get("web").and_then(|v| v.as_str()).map(str::to_owned);
		snapshot.email = profile.get("email").and_then(|v| v.as_str()).map(str::to_owned);
		snapshot.twitter = profile.get("twitter").and_then(|v| v.as_str()).map(str::to_owned);
		if let Some(attrs) = profile.get("attributes").and_then(|v| v.as_object()) {
			for (key, value) in attrs {
				if let Some(val) = value.as_str() {
					snapshot.attributes.insert(key.clone(), val.to_string());
				}
			}
		}
		snapshot
	}

	fn unknown() -> Self {
		Self {
			display: None,
			legal: None,
			web: None,
			email: None,
			twitter: None,
			attributes: BTreeMap::new(),
		}
	}

	fn set_email(&mut self, value: String) {
		self.email = Some(value);
	}

	fn set_attribute(&mut self, key: &str, value: String) {
		self.attributes.insert(key.to_string(), value);
	}

	fn from_entity_info(info: &EntityInfoPacket) -> Self {
		let mut snapshot = Self::unknown();
		snapshot.display = element_to_string(&info.display);
		snapshot.legal = element_to_string(&info.legal);
		snapshot.web = element_to_string(&info.web);
		snapshot.email = element_to_string(&info.email);
		snapshot.twitter = element_to_string(&info.twitter);
		if let Some(attrs) = &info.attributes {
			for (key, value) in attrs.iter() {
				let key_str = String::from_utf8_lossy(key.as_slice()).to_string();
				if let Some(val) = element_to_string(value) {
					snapshot.attributes.insert(key_str, val);
				}
			}
		}
		snapshot
	}
}

fn print_snapshot(token: &str, snapshot: &EntitySnapshot) {
	println!("Current entity snapshot:");
	println!("  Token  : {token}");
	println!("  Display: {}", snapshot.display.as_deref().unwrap_or("unknown"));
	println!("  Legal  : {}", snapshot.legal.as_deref().unwrap_or("unknown"));
	println!("  Web    : {}", snapshot.web.as_deref().unwrap_or("unknown"));
	println!("  Email  : {}", snapshot.email.as_deref().unwrap_or("unknown"));
	println!("  Twitter: {}", snapshot.twitter.as_deref().unwrap_or("unknown"));
	println!("  Attributes:");
	if snapshot.attributes.is_empty() {
		println!("    (none)");
	} else {
		for (key, value) in &snapshot.attributes {
			println!("    - {key}: {value}");
		}
	}
	println!();
}

fn print_history(entries: &[types::entity::InfoAttributeHistoryEntry]) {
	println!("Attribute timeline:");
	println!("    Version  Block    Key         Old Value (base64)");
	let mut sorted = entries.to_vec();
	sorted.sort_by_key(|entry| (entry.block.height, entry.block.index, entry.version));
	for entry in sorted {
		let key = entry.key_utf8.as_deref().unwrap_or(&entry.key_hex);
		println!(
			"    {:>7}  #{:<6} {:<11} {}",
			entry.version, entry.block.height, key, entry.old_value_base64
		);
	}
	println!();
}

fn element_to_string(element: &EntityElement) -> Option<String> {
	match element {
		RuntimeElement::None => None,
		RuntimeElement::Raw(bytes) => String::from_utf8(bytes.to_vec()).ok(),
		RuntimeElement::Bool(flag) => Some((if *flag == 0 { "false" } else { "true" }).into()),
		RuntimeElement::U64(raw) => Some(u64::from_le_bytes(*raw).to_string()),
		RuntimeElement::U128(raw) => Some(u128::from_le_bytes(*raw).to_string()),
		RuntimeElement::Hash(bytes) => Some(format!("0x{}", hex::encode(bytes))),
		RuntimeElement::Token(id) => Some(String::from_utf8_lossy(id.as_bytes()).to_string()),
		RuntimeElement::CID(bytes) => Some(format!("0x{}", hex::encode(bytes))),
	}
}
