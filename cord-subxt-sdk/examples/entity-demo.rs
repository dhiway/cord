use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use codec::Decode;
use cord_primitives::{
	identifier::Ss58Identifier,
	view::{DevElement, ElementView, InfoAttributeHistoryEntry, InfoTokenHistoryEntry},
	view_api::{
		AttributeKey, EntityAccountTokenRequest, EntityAttributeHistoryForKeyRequest,
		EntityAttributeHistoryRequest, EntityInfoBytesRequest, EntityLinkedAccountsRequest,
		EntityNymRequest, TokenTimelineRequest, ViewRequestAuth,
	},
};
use getrandom::getrandom;
use hex;
use oc::scale::{decode_dev_attribute_map, decode_element_view, MetadataResolver};
use oc::{
	demo,
	demo::entity::{self, EntitySnapshot},
	params::config::CordConfig,
	query::auth::{AuthorizationBuilder, SignatureScheme},
	tx,
	tx::nonce::{NonceMode, NonceTracker},
	types::{
		self,
		entity::{AttributeEntry, ElementJson},
	},
	ChainFlavor, Client, Error as OcError,
};
use scale_value::{Composite, Value, ValueDef};
use serde::Serialize;
use serde_json::json;
use sp_core::{sr25519 as sp_sr25519, Pair as _};
use sp_runtime::AccountId32 as RuntimeAccount;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use subxt::{
	blocks::ExtrinsicEvents,
	tx::TxStatus,
	utils::{AccountId32, H256},
};
use tokio::time::{sleep, Duration};

use bs58;

#[tokio::main]
async fn main() -> Result<()> {
	let label = demo::random_label("entity-demo");
	let client = Client::connect("ws://127.0.0.1:9944", ChainFlavor::Auto).await?;
	let signer = tx::signer::dev_alice();
	let account_id = signer_account_id(&signer);
	let mut nonce_tracker = NonceTracker::new(account_id.clone());
	let demo_value = format!("Demo attribute for {label}");
	let initial_public_key = random_public_key_hex();
	let mut profile = demo::entity_profile(&label);
	seed_profile_attribute(&mut profile, "demo", &demo_value);
	seed_profile_attribute(&mut profile, "public_key", &initial_public_key);

	let output_json = std::env::args().any(|arg| arg == "--json");
	let signed_auth = AuthorizationBuilder::from_signer(&signer, SignatureScheme::Sr25519, None)
		.context("failed to build view authorization")?;
	let view_auth = signed_auth.as_request().context("failed to convert view authorization")?;
	let mut transactions = Vec::new();
	let mut mutated_keys: BTreeSet<Vec<u8>> = BTreeSet::new();

	let (entity_token, created) = ensure_entity_token_verbose(
		&client,
		&signer,
		&view_auth,
		&account_id,
		&profile,
		&mut nonce_tracker,
	)
	.await?;
	if created {
		transactions.push("Set entity profile for Alice".to_string());
	} else {
		transactions.push("Synced entity profile for Alice".to_string());
	}
	let mut snapshot = EntitySnapshot::from_profile(&profile, &entity_token);
	let token_identifier = identifier_from_str(&entity_token)?;
	let chain_state = if created {
		EntityChainState::from_profile(&profile)
	} else {
		match fetch_entity_chain_state(&client, &view_auth, &token_identifier).await {
			Ok(state) => state,
			Err(err) => {
				println!(
					"⚠️ unable to fetch on-chain entity snapshot ({}); proceeding with optimistic diff",
					err
				);
				EntityChainState::default()
			},
		}
	};
	let entity_nym_prefix = sanitize_entity_nym(&label);
	let nym_req = EntityNymRequest { auth: view_auth.clone(), token: token_identifier.clone() };
	let existing_nym = client.query().entity().entity_nym(&nym_req).await?;
	if let Some(nym) = existing_nym {
		println!("ℹ️ Entity nym already set: {nym}");
		snapshot.set_entity_nym(nym);
	} else if submit_entity_nym(&client, &signer, &entity_nym_prefix, &mut nonce_tracker).await? {
		snapshot.set_entity_nym(format!("{entity_nym_prefix}.myn.social"));
		transactions.push(format!("Set entity nym (token {entity_token})"));
	}

	let email_value = format!("{label}@cord.dev");
	if created {
		println!(
			"ℹ️ entity initialized; demo/public_key attributes were seeded during profile creation"
		);
		snapshot.set_email(email_value.clone());
		snapshot.set_attribute("demo", demo_value.clone());
		snapshot.set_attribute("public_key", initial_public_key.clone());
	} else {
		match plan_attribute_update(chain_state.get("email"), &email_value) {
			AttributePlan::Skip => {
				println!("ℹ️ attribute 'email' already set; skipping extrinsic");
				snapshot.set_email(email_value.clone());
			},
			plan @ (AttributePlan::Add | AttributePlan::Rotate) => {
				apply_attribute_plan(
					plan,
					&client,
					&signer,
					"email",
					email_value.clone(),
					&mut nonce_tracker,
				)
				.await?;
				match plan {
					AttributePlan::Add => transactions.push("Added attribute 'email'".to_string()),
					AttributePlan::Rotate => {
						transactions.push("Rotated attribute 'email'".to_string())
					},
					AttributePlan::Skip => unreachable!(),
				}
				snapshot.set_email(email_value.clone());
				mutated_keys.insert(b"email".to_vec());
			},
		}

		match plan_attribute_update(chain_state.get("demo"), &demo_value) {
			AttributePlan::Skip => {
				println!("ℹ️ attribute 'demo' already set; skipping extrinsic");
				snapshot.set_attribute("demo", demo_value.clone());
			},
			plan @ (AttributePlan::Add | AttributePlan::Rotate) => {
				apply_attribute_plan(
					plan,
					&client,
					&signer,
					"demo",
					demo_value.clone(),
					&mut nonce_tracker,
				)
				.await?;
				match plan {
					AttributePlan::Add => transactions.push("Added attribute 'demo'".to_string()),
					AttributePlan::Rotate => {
						transactions.push("Rotated attribute 'demo'".to_string())
					},
					AttributePlan::Skip => unreachable!(),
				}
				snapshot.set_attribute("demo", demo_value.clone());
				mutated_keys.insert(b"demo".to_vec());
			},
		}

		let rotation_public_key = random_public_key_hex();
		match plan_attribute_update(chain_state.get("public_key"), &rotation_public_key) {
			AttributePlan::Skip => {
				println!("ℹ️ attribute 'public_key' already set; skipping extrinsic");
				snapshot.set_attribute("public_key", rotation_public_key.clone());
			},
			plan @ (AttributePlan::Add | AttributePlan::Rotate) => {
				apply_attribute_plan(
					plan,
					&client,
					&signer,
					"public_key",
					rotation_public_key.clone(),
					&mut nonce_tracker,
				)
				.await?;
				match plan {
					AttributePlan::Add => {
						transactions.push("Added attribute 'public_key'".to_string())
					},
					AttributePlan::Rotate => {
						transactions.push("Rotated attribute 'public_key'".to_string())
					},
					AttributePlan::Skip => unreachable!(),
				}
				snapshot.set_attribute("public_key", rotation_public_key.clone());
				mutated_keys.insert(b"public_key".to_vec());
			},
		}
		short_delay(Duration::from_secs(1)).await;
	}

	short_delay(Duration::from_secs(2)).await;
	let token_timeline_entries =
		fetch_full_token_timeline(&client, &view_auth, &token_identifier).await?;
	let combined_timeline = build_token_activity(&token_timeline_entries);

	let attribute_history =
		collect_attribute_history(&client, &view_auth, &token_identifier, &mutated_keys).await?;

	let sub_req =
		EntityLinkedAccountsRequest { auth: view_auth.clone(), token: token_identifier.clone() };
	let sub_accounts = client.query().entity().linked_accounts(&sub_req).await?;
	snapshot.set_active_accounts(&sub_accounts);

	// snapshot already updated if nym exists or newly set.

	if output_json {
		let attr_json: Vec<_> = attribute_history
			.iter()
			.enumerate()
			.map(|(idx, entry)| {
				json!({
					"version": idx as u64,
					"key": entry.key_utf8.clone().unwrap_or_else(|| entry.key_hex.clone()),
					"oldValue": decode_attr_value(&entry.old_value_base64),
					"block": entry.block.height,
					"extrinsic": entry.block.index,
				})
			})
			.collect();
		let json = json!({
			"snapshot": snapshot,
			"timeline": combined_timeline,
			"attributeHistory": attr_json,
			"transactions": transactions,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
		return Ok(());
	}

	println!();
	print_summary(&snapshot, &attribute_history, &combined_timeline, &sub_accounts, &transactions);
	Ok(())
}

async fn ensure_entity_token_verbose(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	view_auth: &cord_primitives::view_api::ViewRequestAuth,
	account_id: &AccountId32,
	profile: &serde_json::Value,
	nonce_tracker: &mut NonceTracker,
) -> Result<(String, bool)> {
	let raw: [u8; 32] = *account_id.as_ref();
	let runtime_account = RuntimeAccount::from(raw);
	let request = EntityAccountTokenRequest { auth: view_auth.clone(), account: runtime_account };
	if let Some(token) = client.query().entity().account_token(&request).await? {
		println!("ℹ️ Entity profile already exists (token {token})");
		return Ok((token, false));
	}

	let call = client.tx().entity_set_info_json(profile.clone()).await?;
	let events =
		submit_and_confirm(client, signer, call, "Set entity profile for Alice", nonce_tracker)
			.await
			.map_err(|e| anyhow!(e))?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Entity" && ev.variant_name() == "EntityInfoSet" {
			let mut cursor = ev.field_bytes();
			let _: AccountId32 = Decode::decode(&mut cursor)
				.map_err(|e| anyhow!("failed to decode account: {e}"))?;
			let token: Ss58Identifier =
				Decode::decode(&mut cursor).map_err(|e| anyhow!("failed to decode token: {e}"))?;
			return Ok((demo::ss58_string(&token), true));
		}
	}
	Err(anyhow!("EntityInfoSet event not found"))
}

async fn submit_entity_nym(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	prefix: &str,
	nonce_tracker: &mut NonceTracker,
) -> Result<bool> {
	let call = client.tx().entity_set_entity_nym(prefix).await?;
	match submit_and_confirm(
		client,
		signer,
		call,
		&format!("Set entity nym prefix '{prefix}'"),
		nonce_tracker,
	)
	.await
	{
		Ok(_) => Ok(true),
		Err(err) => {
			if err.message().contains("Entity::EntityNymTaken") {
				Ok(false)
			} else {
				Err(anyhow!(err))
			}
		},
	}
}

fn base64_element(bytes: &[u8]) -> ElementJson {
	ElementJson::RawBase64(BASE64.encode(bytes))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum AttributePlan {
	Skip,
	Add,
	Rotate,
}

fn plan_attribute_update(current: Option<&str>, desired: &str) -> AttributePlan {
	match current {
		Some(existing) if existing == desired => AttributePlan::Skip,
		Some(_) => AttributePlan::Rotate,
		None => AttributePlan::Add,
	}
}

async fn apply_attribute_plan(
	plan: AttributePlan,
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	key: &str,
	value: String,
	nonce_tracker: &mut NonceTracker,
) -> Result<()> {
	match plan {
		AttributePlan::Skip => Ok(()),
		AttributePlan::Add => {
			println!("➕ attribute '{key}' missing on-chain; submitting add");
			submit_attribute_add(client, signer, key, value, nonce_tracker)
				.await
				.map_err(|e| anyhow!(e))
		},
		AttributePlan::Rotate => {
			println!("🔁 attribute '{key}' exists with different value; rotating");
			submit_attribute_rotation(client, signer, key, value, nonce_tracker)
				.await
				.map_err(|e| anyhow!(e))
		},
	}
}

async fn submit_attribute_rotation(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	key: &str,
	value: String,
	nonce_tracker: &mut NonceTracker,
) -> Result<(), SubmitError> {
	let entry = AttributeEntry {
		key_hex: types::to_key_hex_from_utf8(key),
		key_utf8: Some(key.into()),
		value: base64_element(value.as_bytes()),
	};
	let call = client
		.tx()
		.entity_rotate_attribute(entry)
		.await
		.map_err(SubmitError::from_origin_error)?;
	submit_and_confirm(client, signer, call, &format!("Rotated attribute '{key}'"), nonce_tracker)
		.await?;
	Ok(())
}

async fn submit_attribute_add(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	key: &str,
	value: String,
	nonce_tracker: &mut NonceTracker,
) -> Result<(), SubmitError> {
	let entry = AttributeEntry {
		key_hex: types::to_key_hex_from_utf8(key),
		key_utf8: Some(key.into()),
		value: base64_element(value.as_bytes()),
	};
	let call = client
		.tx()
		.entity_add_attributes(vec![entry])
		.await
		.map_err(SubmitError::from_origin_error)?;
	submit_and_confirm(client, signer, call, &format!("Added attribute '{key}'"), nonce_tracker)
		.await?;
	Ok(())
}

async fn submit_and_confirm(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	call: subxt::tx::DynamicPayload,
	description: &str,
	nonce_tracker: &mut NonceTracker,
) -> Result<ExtrinsicEvents<CordConfig>, SubmitError> {
	println!("⏳ {description} ...");
	let nonce = nonce_tracker
		.reserve(client)
		.await
		.map_err(|e| SubmitError::Node(e.to_string()))?;
	let mut progress = match client
		.tx()
		.sign_and_submit_then_watch_with_opts(
			call,
			signer,
			tx::TxOptions { nonce: Some(NonceMode::Manual(nonce)), tip: None, era: None },
		)
		.await
	{
		Ok(progress) => progress,
		Err(err) => {
			nonce_tracker.rollback();
			return Err(SubmitError::Node(err.to_string()));
		},
	};
	while let Some(status) = progress.next().await {
		let status = match status {
			Ok(s) => s,
			Err(err) => {
				nonce_tracker.rollback();
				return Err(SubmitError::Node(err.to_string()));
			},
		};
		match status {
			TxStatus::Validated => println!("  ↳ 🟡 validated and queued"),
			TxStatus::Broadcasted => println!("  ↳ 📡 broadcast to peers"),
			TxStatus::NoLongerInBestBlock => {
				println!("  ↳ ⚠️ retracted from best block, waiting for re-inclusion")
			},
			TxStatus::InBestBlock(in_block) => {
				let block_hash = in_block.block_hash();
				let block_label = block_label(client, block_hash).await;
				println!("  ↳ 📦 included in block {block_label}");
				let events =
					in_block.wait_for_success().await.map_err(SubmitError::from_subxt_error)?;
				println!("✅ {description} recorded in block {block_label}");
				nonce_tracker.confirm();
				short_delay(Duration::from_secs(1)).await;
				return Ok(events);
			},
			TxStatus::InFinalizedBlock(in_block) => {
				let block_hash = in_block.block_hash();
				let block_label = block_label(client, block_hash).await;
				println!("  ↳ 🛡️ finalized in block {block_label}");
				let events =
					in_block.wait_for_success().await.map_err(SubmitError::from_subxt_error)?;
				println!("✅ {description} finalized in block {block_label}");
				nonce_tracker.confirm();
				short_delay(Duration::from_secs(1)).await;
				return Ok(events);
			},
			TxStatus::Error { message } => {
				nonce_tracker.rollback();
				return Err(SubmitError::Node(message));
			},
			TxStatus::Invalid { message } => {
				nonce_tracker.rollback();
				return Err(SubmitError::Invalid(message));
			},
			TxStatus::Dropped { message } => {
				nonce_tracker.rollback();
				return Err(SubmitError::Dropped(message));
			},
		}
	}
	nonce_tracker.rollback();
	Err(SubmitError::StreamEnded)
}

fn signer_account_id(signer: &tx::signer::sr25519::Keypair) -> AccountId32 {
	<tx::signer::sr25519::Keypair as subxt::tx::Signer<CordConfig>>::account_id(signer)
}

fn identifier_from_str(ss58: &str) -> Result<Ss58Identifier> {
	Ss58Identifier::try_from(ss58.to_string())
		.map_err(|_| anyhow::anyhow!("invalid ss58 identifier"))
}

#[derive(Default)]
struct EntityChainState {
	reserved: BTreeMap<String, String>,
	attributes: BTreeMap<String, String>,
}

impl EntityChainState {
	fn from_profile(profile: &serde_json::Value) -> Self {
		let mut state = EntityChainState::default();
		let reserved_fields = ["display", "legal", "web", "email", "twitter"];
		for key in reserved_fields {
			let value = profile.get(key).and_then(|v| v.as_str()).map(|s| s.to_string());
			state.insert_reserved(key, value);
		}
		if let Some(attrs) = profile.get("attributes").and_then(|v| v.as_object()) {
			for (key, value) in attrs {
				let val = value.as_str().map(|s| s.to_string());
				state.insert_attribute(key.clone(), val);
			}
		}
		state
	}

	fn insert_reserved(&mut self, key: &str, value: Option<String>) {
		if let Some(val) = value {
			self.reserved.insert(key.to_string(), val);
		}
	}

	fn insert_attribute(&mut self, key: String, value: Option<String>) {
		if let Some(val) = value {
			self.attributes.insert(key, val);
		}
	}

	fn get(&self, key: &str) -> Option<&str> {
		self.reserved
			.get(key)
			.map(|s| s.as_str())
			.or_else(|| self.attributes.get(key).map(|s| s.as_str()))
	}
}

async fn fetch_entity_chain_state(
	client: &Client,
	view_auth: &ViewRequestAuth,
	token: &Ss58Identifier,
) -> Result<EntityChainState> {
	let req = EntityInfoBytesRequest { auth: view_auth.clone(), token: token.clone() };
	let Some(bytes) = client.query().entity().entity_info_bytes(&req).await? else {
		return Ok(EntityChainState::default());
	};
	let metadata = client.metadata();
	let resolver = MetadataResolver::new(&metadata);
	let type_id = find_entity_info_type_id(&resolver)
		.ok_or_else(|| anyhow!("unable to locate EntityInfo type in metadata"))?;
	let value = resolver.decode_value(type_id, &bytes).map_err(|e: OcError| anyhow!(e))?;
	parse_entity_info_value(&value)
}

fn find_entity_info_type_id(resolver: &MetadataResolver) -> Option<u32> {
	resolver.find_type(|ty| match &ty.ty.type_def {
		scale_info::TypeDef::Composite(comp) => {
			let mut has_email = false;
			let mut has_attrs = false;
			for field in &comp.fields {
				if let Some(name) = field.name.as_ref().map(|seg| seg.as_str()) {
					match name {
						"email" => has_email = true,
						"attributes" => has_attrs = true,
						_ => {},
					}
				}
			}
			let ends_with_entity_info =
				ty.ty.path.segments.last().map(|seg| seg.as_str()) == Some("EntityInfo");
			has_email && has_attrs && ends_with_entity_info
		},
		_ => false,
	})
}

fn parse_entity_info_value(value: &Value<u32>) -> Result<EntityChainState> {
	let mut state = EntityChainState::default();
	let ValueDef::Composite(Composite::Named(fields)) = &value.value else {
		return Err(anyhow!("unexpected entity info layout"));
	};
	for (name, field_value) in fields {
		match name.as_str() {
			"display" | "legal" | "web" | "email" | "twitter" => {
				state.insert_reserved(name, element_value_to_string(field_value));
			},
			"attributes" => {
				if let Ok(map) = decode_dev_attribute_map(field_value) {
					for (key, attr) in map {
						state.insert_attribute(key, dev_element_to_string(&attr.value));
					}
				}
			},
			_ => {},
		}
	}
	Ok(state)
}

fn element_value_to_string(value: &Value<u32>) -> Option<String> {
	let view = decode_element_view(value).ok()?;
	element_view_to_string(view)
}

fn element_view_to_string(view: ElementView) -> Option<String> {
	match view {
		ElementView::None => None,
		ElementView::Bool(value) => Some(value.to_string()),
		ElementView::U64(value) => Some(value.to_string()),
		ElementView::U128(value) => Some(value.to_string()),
		ElementView::Hash(bytes) => Some(format!("0x{}", hex::encode(bytes))),
		ElementView::Token(token) => Some(String::from_utf8_lossy(token.as_ref()).into_owned()),
		ElementView::Cid(bytes) => Some(bs58::encode(bytes).into_string()),
		ElementView::Raw(bytes) => match String::from_utf8(bytes.clone()) {
			Ok(text) => Some(text),
			Err(_) => Some(format!("0x{}", hex::encode(bytes))),
		},
	}
}

fn dev_element_to_string(element: &DevElement) -> Option<String> {
	match element {
		DevElement::None => None,
		DevElement::Bool(value) => Some(value.to_string()),
		DevElement::U64(value) => Some(value.to_string()),
		DevElement::U128(value) => Some(value.to_string()),
		DevElement::HashHex(hexstr) => Some(hexstr.clone()),
		DevElement::TokenSs58(token) => Some(token.clone()),
		DevElement::CidBase58(cid) => Some(cid.clone()),
		DevElement::RawBase64(encoded) => match BASE64.decode(encoded.as_bytes()) {
			Ok(bytes) => match String::from_utf8(bytes) {
				Ok(text) => Some(text),
				Err(_) => Some(encoded.clone()),
			},
			Err(_) => Some(encoded.clone()),
		},
	}
}

fn seed_profile_attribute(profile: &mut serde_json::Value, key: &str, value: &str) {
	let Some(attrs) = profile.get_mut("attributes").and_then(|val| val.as_object_mut()) else {
		return;
	};
	attrs.insert(key.to_string(), serde_json::Value::String(value.to_string()));
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
	attr_history: &[InfoAttributeHistoryEntry],
	timeline: &[TimelineRow],
	accounts: &[AccountId32],
	transactions: &[String],
) {
	println!("🧾 Transactions");
	for tx in transactions {
		println!("  • {}", tx);
	}
	println!("\n🆔 Entity Token: {}", snapshot.token);
	if let Some(nym) = &snapshot.entity_nym {
		println!("🏷️ Entity Nym : {}", nym);
	}
	snapshot.print_cli();
	entity::print_accounts_cli(accounts);
	print_combined_timeline(timeline);
	print_attribute_history(attr_history);
}

async fn block_label(client: &Client, block_hash: H256) -> String {
	match client.online().blocks().at(block_hash).await {
		Ok(block) => format!("#{} ({block_hash:?})", block.number()),
		Err(_) => format!("{block_hash:?}"),
	}
}

fn truncate_label(value: &str, max_len: usize) -> String {
	if value.len() <= max_len {
		value.to_string()
	} else {
		let mut truncated = value.chars().take(max_len.saturating_sub(1)).collect::<String>();
		truncated.push('…');
		truncated
	}
}

fn random_public_key_hex() -> String {
	let mut seed = [0u8; 32];
	getrandom(&mut seed).expect("random seed");
	let pair = sp_sr25519::Pair::from_seed(&seed);
	format!("0x{}", hex::encode(pair.public()))
}

const TIMELINE_PAGE_SIZE: u32 = 32;

const ATTRIBUTE_HISTORY_RETRIES: usize = 3;

async fn collect_attribute_history(
	client: &Client,
	view_auth: &ViewRequestAuth,
	token_identifier: &Ss58Identifier,
	mutated_keys: &BTreeSet<Vec<u8>>,
) -> Result<Vec<InfoAttributeHistoryEntry>> {
	let target_hex: Vec<String> =
		mutated_keys.iter().map(|key| format!("0x{}", hex::encode(key))).collect();
	let mut attempt = 0;
	loop {
		let entries =
			fetch_attribute_history_snapshot(client, view_auth, token_identifier, mutated_keys)
				.await?;
		let complete = target_hex.is_empty()
			|| target_hex.iter().all(|hex_key| {
				entries.iter().any(|entry| entry.key_hex.eq_ignore_ascii_case(hex_key))
			});
		if complete || attempt >= ATTRIBUTE_HISTORY_RETRIES {
			return Ok(entries);
		}
		attempt += 1;
		short_delay(Duration::from_secs(1)).await;
	}
}

async fn fetch_attribute_history_snapshot(
	client: &Client,
	view_auth: &ViewRequestAuth,
	token_identifier: &Ss58Identifier,
	mutated_keys: &BTreeSet<Vec<u8>>,
) -> Result<Vec<InfoAttributeHistoryEntry>> {
	let history_req =
		EntityAttributeHistoryRequest { auth: view_auth.clone(), token: token_identifier.clone() };
	let mut entries = match client.query().entity().attribute_history_entries(&history_req).await {
		Ok(entries) => entries,
		Err(OcError::NotFound(_)) => Vec::new(),
		Err(err) => return Err(anyhow!(err)),
	};

	let mut keys_to_fetch: BTreeSet<Vec<u8>> = entries
		.iter()
		.filter_map(|entry| types::hex_to_bytes(&entry.key_hex).ok())
		.filter(|bytes| !bytes.is_empty())
		.collect();
	keys_to_fetch.extend(mutated_keys.iter().cloned());

	for key in keys_to_fetch {
		let Ok(bounded_key) = AttributeKey::try_from(key.clone()) else {
			continue;
		};
		let key_req = EntityAttributeHistoryForKeyRequest {
			auth: view_auth.clone(),
			token: token_identifier.clone(),
			key: bounded_key,
		};
		match client.query().entity().attribute_history_for_key_entries(&key_req).await {
			Ok(mut extra) => entries.append(&mut extra),
			Err(OcError::NotFound(_)) => {},
			Err(err) => return Err(anyhow!(err)),
		}
	}

	let mut unique: BTreeMap<(String, u64), InfoAttributeHistoryEntry> = BTreeMap::new();
	for entry in entries {
		unique.entry((entry.key_hex.clone(), entry.version)).or_insert(entry);
	}
	let mut combined: Vec<_> = unique.into_values().collect();
	combined.sort_by(|a, b| (a.block.height, a.block.index).cmp(&(b.block.height, b.block.index)));
	Ok(combined)
}

async fn fetch_full_token_timeline(
	client: &Client,
	view_auth: &ViewRequestAuth,
	token: &Ss58Identifier,
) -> Result<Vec<(u32, InfoTokenHistoryEntry)>> {
	let mut start = Some(0u32);
	let mut version_cursor = start.unwrap_or(0);
	let mut rows = Vec::new();
	loop {
		let req = TokenTimelineRequest {
			auth: view_auth.clone(),
			token: token.clone(),
			start,
			limit: Some(TIMELINE_PAGE_SIZE),
		};
		let batch = client.query().token().timeline(&req).await?;
		if batch.is_empty() {
			break;
		}
		let fetched = batch.len() as u32;
		for entry in batch {
			rows.push((version_cursor, entry));
			version_cursor = version_cursor.saturating_add(1);
		}
		if fetched < TIMELINE_PAGE_SIZE {
			break;
		}
		start = Some(version_cursor);
	}
	Ok(rows)
}

#[derive(Clone, Serialize)]
struct TimelineRow {
	pub version: u64,
	pub action: String,
	pub action_hex: String,
	pub digest: String,
	pub block: u32,
	pub extrinsic: u32,
}

fn build_token_activity(entries: &[(u32, InfoTokenHistoryEntry)]) -> Vec<TimelineRow> {
	let mut rows: Vec<_> = entries
		.iter()
		.map(|(version, entry)| (*version, entry.block.height, entry.block.index, entry))
		.collect();
	rows.sort_by(|a, b| (a.1, a.2).cmp(&(b.1, b.2)));
	rows.into_iter()
		.map(|(version, _, _, entry)| TimelineRow {
			version: version as u64,
			action: entry.action_utf8.clone().unwrap_or_else(|| entry.action_hex.clone()),
			action_hex: entry.action_hex.clone(),
			digest: truncate_digest(&entry.digest_hex, 16),
			block: entry.block.height,
			extrinsic: entry.block.index,
		})
		.collect()
}

fn print_combined_timeline(entries: &[TimelineRow]) {
	println!("\n🕛 Entity Activity:");
	if entries.is_empty() {
		println!("    • (no recorded activity)");
		return;
	}
	println!(
		"    {:>10}  {:>7}  {:<28} {:<34} {:<20}",
		"Block", "Index", "Action", "Action (Hex)", "Digest"
	);
	for entry in entries {
		println!(
			"    #{:<8}  {:>7}  {:<28} {:<34} {:<20}",
			entry.block,
			entry.extrinsic,
			truncate_label(&entry.action, 28),
			truncate_label(&entry.action_hex, 34),
			entry.digest
		);
	}
}

fn print_attribute_history(entries: &[InfoAttributeHistoryEntry]) {
	println!("\n📜 Attribute Rotations:");
	if entries.is_empty() {
		println!("    • (no attribute history)");
		return;
	}
	println!("    {:>10}  {:>7}  {:<18} {:<30}", "Block", "Index", "Key", "Rotated Value");
	for entry in entries {
		let key = entry.key_utf8.clone().unwrap_or_else(|| entry.key_hex.clone());
		let value = truncate_label(&decode_attr_value(&entry.old_value_base64), 30);
		println!(
			"    #{:<8}  {:>7}  {:<18} {:<30}",
			entry.block.height,
			entry.block.index,
			truncate_label(&key, 18),
			value
		);
	}
}

fn truncate_digest(hex: &str, bytes: usize) -> String {
	let prefix = if hex.starts_with("0x") { 2 } else { 0 };
	let chars = prefix + bytes * 2;
	if hex.len() <= chars {
		hex.to_string()
	} else {
		hex.chars().take(chars).collect()
	}
}

fn decode_attr_value(value: &str) -> String {
	match BASE64.decode(value.as_bytes()) {
		Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| value.to_string()),
		Err(_) => value.to_string(),
	}
}

async fn short_delay(duration: Duration) {
	sleep(duration).await;
}

#[derive(Debug)]
enum SubmitError {
	Invalid(String),
	Dropped(String),
	Node(String),
	Runtime(String),
	StreamEnded,
}

impl SubmitError {
	fn from_subxt_error(err: subxt::Error) -> Self {
		SubmitError::Runtime(err.to_string())
	}

	fn from_origin_error(err: oc::Error) -> Self {
		SubmitError::Node(err.to_string())
	}

	fn message(&self) -> &str {
		match self {
			Self::Invalid(msg) | Self::Dropped(msg) | Self::Node(msg) | Self::Runtime(msg) => msg,
			Self::StreamEnded => "extrinsic stream ended before inclusion",
		}
	}
}

impl fmt::Display for SubmitError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Invalid(msg) => write!(f, "invalid transaction: {msg}"),
			Self::Dropped(msg) => write!(f, "dropped transaction: {msg}"),
			Self::Node(msg) => write!(f, "node error: {msg}"),
			Self::Runtime(msg) => write!(f, "runtime error: {msg}"),
			Self::StreamEnded => write!(f, "extrinsic stream ended before inclusion"),
		}
	}
}

impl std::error::Error for SubmitError {}
