use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, NaiveDateTime, Utc};
use codec::Decode;
use cord_primitives::{
	identifier::Ss58Identifier,
	view::InfoAttributeHistoryEntry,
	view_api::{EntityAttributeHistoryRequest, EntityLinkedAccountsRequest, EntityNymRequest},
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
use serde_json::json;
use std::collections::BTreeMap;
use subxt::{blocks::ExtrinsicEvents, dynamic::storage, utils::AccountId32};

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
	let mut transactions = Vec::new();

	let (entity_token, created) =
		demo::ensure_entity_token(&client, &signer, &view_auth, &account_id, &profile).await?;
	if created {
		transactions.push("Set entity profile for Alice".to_string());
	} else {
		transactions.push("Synced entity profile for Alice".to_string());
	}
	let mut snapshot = EntitySnapshot::from_profile(&profile, &entity_token);

	let entity_nym_prefix = sanitize_entity_nym(&label);
	if submit_entity_nym(&client, &signer, &entity_nym_prefix).await? {
		transactions.push(format!("Set entity nym (token {entity_token})"));
	}

	submit_attribute_rotation(&client, &signer, "email", format!("{label}@cord.dev")).await?;
	snapshot.set_email(format!("{label}@cord.dev"));
	transactions.push("Rotated attribute 'email'".to_string());

	submit_attribute_add(&client, &signer, "website", format!("https://{label}.cord.dev")).await?;
	snapshot.set_attribute("website", format!("https://{label}.cord.dev"));
	transactions.push("Added attribute 'website'".to_string());

	let token_identifier = identifier_from_str(&entity_token)?;
	let history_req =
		EntityAttributeHistoryRequest { auth: view_auth.clone(), token: token_identifier.clone() };
	let history = client.query().entity().attribute_history_entries(&history_req).await?;
	let history_with_time = hydrate_history_with_time(&client, history).await?;

	let sub_req =
		EntityLinkedAccountsRequest { auth: view_auth.clone(), token: token_identifier.clone() };
	let sub_accounts = client.query().entity().linked_accounts(&sub_req).await?;
	snapshot.set_active_accounts(&sub_accounts);

	let nym_req = EntityNymRequest { auth: view_auth.clone(), token: token_identifier };
	if let Some(nym) = client.query().entity().entity_nym(&nym_req).await? {
		snapshot.set_entity_nym(nym);
	}

	if output_json {
		let history_json: Vec<_> = history_with_time
			.iter()
			.map(|(entry, ts)| json!({ "entry": entry, "timestamp": ts }))
			.collect();
		let json = json!({
			"snapshot": snapshot,
			"history": history_json,
			"transactions": transactions,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
		return Ok(());
	}

	print_summary(&snapshot, &history_with_time, &sub_accounts, &transactions);
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
		value: base64_element(value.as_bytes()),
	};
	let call = client.tx().entity_rotate_attribute(entry).await?;
	submit_and_confirm(client, signer, call, &format!("Rotated attribute '{key}'")).await?;
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
		value: base64_element(value.as_bytes()),
	};
	let call = client.tx().entity_add_attributes(vec![entry]).await?;
	match submit_and_confirm(client, signer, call, &format!("Added attribute '{key}'")).await {
		Ok(_) => Ok(()),
		Err(err) => {
			if err.to_string().contains("Entity::AttributeExists") {
				submit_attribute_rotation(client, signer, key, value).await
			} else {
				Err(err)
			}
		},
	}
}

async fn submit_entity_nym(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	prefix: &str,
) -> Result<bool> {
	let call = client.tx().entity_set_entity_nym(prefix).await?;
	match submit_and_confirm(client, signer, call, &format!("Set entity nym prefix '{prefix}'"))
		.await
	{
		Ok(_) => Ok(true),
		Err(err) => {
			if err.to_string().contains("Entity::EntityNymTaken") {
				Ok(false)
			} else {
				Err(err)
			}
		},
	}
}

fn base64_element(bytes: &[u8]) -> ElementJson {
	ElementJson::RawBase64(BASE64.encode(bytes))
}

async fn submit_and_confirm(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	call: subxt::tx::DynamicPayload,
	description: &str,
) -> Result<ExtrinsicEvents<CordConfig>> {
	println!("⏳ {description} ...");
	let events = client
		.tx()
		.sign_and_submit(call, signer, tx::TxOptions::default())
		.await?
		.wait_for_success()
		.await?;
	let block_hash = events.block_hash();
	let block = client.blocks().at(block_hash).await?;
	let block_number = block.number();
	let timestamp = fetch_block_timestamp(client, block_hash).await?;
	if let Some(ts) = &timestamp {
		println!("✅ {description} in block #{block_number} ({ts})");
	} else {
		println!("✅ {description} in block #{block_number}");
	}
	Ok(events)
}

fn signer_account_id(signer: &tx::signer::sr25519::Keypair) -> AccountId32 {
	<tx::signer::sr25519::Keypair as subxt::tx::Signer<CordConfig>>::account_id(signer)
}

fn identifier_from_str(ss58: &str) -> Result<Ss58Identifier> {
	Ss58Identifier::try_from(ss58.to_string())
		.map_err(|_| anyhow::anyhow!("invalid ss58 identifier"))
}

fn sanitize_entity_nym(label: &str) -> String {
	let mut filtered: String = label
		.to_ascii_lowercase()
		.chars()
		.filter(|c| matches!(c, 'a'..='z' | '0'..='9' | '.'))
		.collect();
	while filtered.starts_with('.') {
		filtered.remove(0);
	}
	while filtered.ends_with('.') {
		filtered.pop();
	}
	if filtered.is_empty() {
		filtered.push_str("entity");
	}
	if filtered.len() > 32 {
		filtered.truncate(32);
	}
	filtered
}

fn print_summary(
	snapshot: &EntitySnapshot,
	history: &[(InfoAttributeHistoryEntry, Option<String>)],
	accounts: &[AccountId32],
	transactions: &[String],
) {
	println!("\n🧾 Transactions");
	for tx in transactions {
		println!("  • {}", tx);
	}
	println!("\n🆔 Entity Token: {}", snapshot.token);
	if let Some(nym) = &snapshot.entity_nym {
		println!("🏷️  Entity Nym : {}", nym);
	}
	snapshot.print_cli();
	entity::print_accounts_cli(accounts);
	entity::print_history_cli(history);
}

async fn fetch_block_timestamp(
	client: &origin::Client,
	block_hash: <CordConfig as subxt::Config>::Hash,
) -> Result<Option<String>> {
	use subxt::dynamic::storage;
	let storage_key = storage("Timestamp", "Now", vec![]);
	let maybe_bytes = client.storage().at(block_hash).fetch(&storage_key).await?;
	if let Some(bytes) = maybe_bytes {
		let mut slice = bytes.as_slice();
		let moment = u64::decode(&mut slice)?;
		return Ok(format_unix_ms(moment));
	}
	Ok(None)
}

async fn hydrate_history_with_time(
	client: &origin::Client,
	entries: Vec<InfoAttributeHistoryEntry>,
) -> Result<Vec<(InfoAttributeHistoryEntry, Option<String>)>> {
	let mut cache: BTreeMap<u32, Option<String>> = BTreeMap::new();
	let mut hydrated = Vec::with_capacity(entries.len());
	for entry in entries {
		let ts = if let Some(cached) = cache.get(&entry.block.height) {
			cached.clone()
		} else {
			let hash = client.rpc().block_hash(Some(entry.block.height.into())).await?;
			let ts = match hash {
				Some(h) => fetch_block_timestamp(client, h).await?,
				None => None,
			};
			cache.insert(entry.block.height, ts.clone());
			ts
		};
		hydrated.push((entry, ts));
	}
	Ok(hydrated)
}

fn format_unix_ms(ms: u64) -> Option<String> {
	let secs = (ms / 1000) as i64;
	let nanos = ((ms % 1000) * 1_000_000) as u32;
	let naive = NaiveDateTime::from_timestamp_opt(secs, nanos)?;
	let datetime: DateTime<Utc> = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
	Some(datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string())
}
