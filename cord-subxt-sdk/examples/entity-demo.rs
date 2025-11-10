use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use codec::Decode;
use cord_primitives::identifier::Ss58Identifier;
use origin::{
	params::config::CordConfig,
	query::auth::{AuthorizationBuilder, SignatureScheme, ViewAuthorization},
	scale::MetadataResolver,
	tx::{self, TxOptions},
	types::{self, entity::AttributeEntry, entity::ElementJson},
};
use scale_value::{Composite, Value, ValueDef};
use serde_json::json;
use std::collections::BTreeMap;
use subxt::{blocks::ExtrinsicEvents, utils::AccountId32};
use tokio::time::{sleep, Duration};

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
	let history = client
		.query()
		.entity()
		.attribute_history_json(&history_auth, &entity_token)
		.await?;

	println!("Entity token: {entity_token}\n");
	print_snapshot(&entity_token, &final_snapshot);
	print_history(&history);
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
	let metadata = client.metadata();
	let resolver = MetadataResolver::new(&metadata);
	let ty_id = entity_info_type_id(&resolver)?;
	let value = resolver
		.decode_value(ty_id, &raw)
		.map_err(|e| anyhow!("failed to decode entity info via metadata: {e}"))?;
	Ok(Some(EntitySnapshot::from_value(&value)))
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

	fn from_value(value: &Value<u32>) -> Self {
		let mut snapshot = Self::unknown();
		if let ValueDef::Composite(Composite::Named(fields)) = &value.value {
			for (name, field_value) in fields {
				match name.as_str() {
					"display" => snapshot.display = element_text(field_value),
					"legal" => snapshot.legal = element_text(field_value),
					"web" => snapshot.web = element_text(field_value),
					"email" => snapshot.email = element_text(field_value),
					"twitter" => snapshot.twitter = element_text(field_value),
					"attributes" => snapshot.attributes = attributes_from_value(field_value),
					_ => {},
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

fn entity_info_type_id(resolver: &MetadataResolver<'_>) -> Result<u32> {
	resolver
		.find_type(|ty| {
			let segments = &ty.ty.path.segments;
			segments.last().map(|s| s.as_str()) == Some("EntityInfo")
				&& segments.iter().any(|s| s == "pallet_entity")
		})
		.ok_or_else(|| anyhow!("EntityInfo type not found in runtime metadata"))
}

fn element_text(value: &Value<u32>) -> Option<String> {
	let variant = match &unwrap_newtype(value).value {
		ValueDef::Variant(v) => v,
		_ => return None,
	};
	let field = first_field(&variant.values);
	match variant.name.as_str() {
		"None" => None,
		"Raw" => field
			.and_then(bytes_from_value)
			.map(|bytes| String::from_utf8(bytes.clone()).unwrap_or_else(|_| BASE64.encode(bytes))),
		"Bool" => field
			.and_then(|f| f.as_u128())
			.map(|num| if num == 0 { "false" } else { "true" }.to_string()),
		"U64" => field
			.and_then(|f| bytes_from_value(f))
			.and_then(|bytes| bytes.try_into().ok().map(u64::from_le_bytes).map(|n| n.to_string())),
		"U128" => field.and_then(|f| bytes_from_value(f)).and_then(|bytes| {
			bytes.try_into().ok().map(u128::from_le_bytes).map(|n| n.to_string())
		}),
		"Hash" => field
			.and_then(bytes_from_value)
			.map(|bytes| format!("0x{}", hex::encode(bytes))),
		"Token" => field.and_then(bytes_from_value).map(|bytes| {
			String::from_utf8(bytes.clone()).unwrap_or_else(|_| format!("0x{}", hex::encode(bytes)))
		}),
		"CID" => field.and_then(bytes_from_value).map(|bytes| bs58::encode(bytes).into_string()),
		_ => None,
	}
}

fn attributes_from_value(value: &Value<u32>) -> BTreeMap<String, String> {
	let mut map = BTreeMap::new();
	let Some(inner) = option_inner(value) else {
		return map;
	};
	if let Some(entries) = sequence_items(inner) {
		for entry in entries {
			if let Some((key_value, element_value)) = tuple_fields(entry) {
				if let Some(key_bytes) = bytes_from_value(key_value) {
					let key = String::from_utf8(key_bytes.clone())
						.unwrap_or_else(|_| format!("0x{}", hex::encode(key_bytes)));
					if let Some(val) = element_text(element_value) {
						map.insert(key, val);
					}
				}
			}
		}
	}
	map
}

fn option_inner<'a>(value: &'a Value<u32>) -> Option<&'a Value<u32>> {
	match &value.value {
		ValueDef::Variant(var) => match var.name.as_str() {
			"None" => None,
			"Some" => first_field(&var.values),
			_ => None,
		},
		_ => Some(value),
	}
}

fn first_field<'a>(composite: &'a Composite<u32>) -> Option<&'a Value<u32>> {
	match composite {
		Composite::Named(fields) => fields.first().map(|(_, v)| v),
		Composite::Unnamed(items) => items.first(),
	}
}

fn bytes_from_value(value: &Value<u32>) -> Option<Vec<u8>> {
	let inner = unwrap_newtype(value);
	match &inner.value {
		ValueDef::Composite(Composite::Unnamed(items)) => {
			if items.iter().all(|item| item.as_u128().is_some()) {
				Some(items.iter().map(|item| item.as_u128().unwrap() as u8).collect())
			} else {
				None
			}
		},
		ValueDef::Composite(Composite::Named(_)) => None,
		ValueDef::Variant(var) => first_field(&var.values).and_then(bytes_from_value),
		ValueDef::Primitive(_) => inner.as_u128().map(|n| vec![n as u8]),
		ValueDef::BitSequence(bits) => {
			let mut out = Vec::new();
			let mut accum = 0u8;
			let mut count = 0;
			for bit in bits.iter() {
				if bit {
					accum |= 1 << count;
				}
				count += 1;
				if count == 8 {
					out.push(accum);
					accum = 0;
					count = 0;
				}
			}
			if count > 0 {
				out.push(accum);
			}
			Some(out)
		},
	}
}

fn unwrap_newtype<'a>(value: &'a Value<u32>) -> &'a Value<u32> {
	match &value.value {
		ValueDef::Composite(Composite::Named(fields)) if fields.len() == 1 => {
			unwrap_newtype(&fields[0].1)
		},
		ValueDef::Composite(Composite::Unnamed(items)) if items.len() == 1 => {
			unwrap_newtype(&items[0])
		},
		_ => value,
	}
}

fn sequence_items<'a>(value: &'a Value<u32>) -> Option<Vec<&'a Value<u32>>> {
	match &unwrap_newtype(value).value {
		ValueDef::Composite(Composite::Unnamed(items)) => Some(items.iter().collect()),
		_ => None,
	}
}

fn tuple_fields<'a>(value: &'a Value<u32>) -> Option<(&'a Value<u32>, &'a Value<u32>)> {
	let items = sequence_items(value)?;
	if items.len() == 2 {
		Some((items[0], items[1]))
	} else {
		None
	}
}
