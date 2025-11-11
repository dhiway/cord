use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use codec::Decode;
use cord_primitives::{
	identifier::Ss58Identifier,
	view::InfoTokenHistoryEntry,
	view_api::{
		EntityAccountTokenRequest, EntityLinkedAccountsRequest, EntityNymRequest,
		TokenTimelineRequest,
	},
};
use origin::{
	demo,
	demo::entity::{self, EntitySnapshot},
	params::config::{build_cord_params, CordConfig},
	query::auth::{AuthorizationBuilder, SignatureScheme},
	tx,
	types::{
		self,
		entity::{AttributeEntry, ElementJson},
	},
};
use serde::Serialize;
use serde_json::json;
use sp_runtime::AccountId32 as RuntimeAccount;
use std::fmt;
use subxt::{
	blocks::ExtrinsicEvents,
	config::DefaultExtrinsicParamsBuilder,
	tx::TxStatus,
	utils::{AccountId32, H256},
};

#[tokio::main]
async fn main() -> Result<()> {
	let label = demo::random_label("entity-demo");
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	let signer = tx::signer::dev_alice();
	let account_id = signer_account_id(&signer);
	let mut nonce_tracker = NonceTracker::new(account_id.clone());
	let profile = demo::entity_profile(&label);

	let output_json = std::env::args().any(|arg| arg == "--json");
	let signed_auth = AuthorizationBuilder::from_signer(&signer, SignatureScheme::Sr25519, None)
		.context("failed to build view authorization")?;
	let view_auth = signed_auth.as_request().context("failed to convert view authorization")?;
	let mut transactions = Vec::new();

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

	let entity_nym_prefix = sanitize_entity_nym(&label);
	if submit_entity_nym(&client, &signer, &entity_nym_prefix, &mut nonce_tracker).await? {
		transactions.push(format!("Set entity nym (token {entity_token})"));
	}

	match ensure_attribute(
		&client,
		&signer,
		"email",
		format!("{label}@cord.dev"),
		AttributeIntent::PreferRotate,
		&mut nonce_tracker,
	)
	.await?
	{
		AttributeResult::Rotated => transactions.push("Rotated attribute 'email'".to_string()),
		AttributeResult::Added => transactions.push("Added attribute 'email'".to_string()),
	}
	snapshot.set_email(format!("{label}@cord.dev"));

	match ensure_attribute(
		&client,
		&signer,
		"website",
		format!("https://{label}.cord.dev"),
		AttributeIntent::PreferAdd,
		&mut nonce_tracker,
	)
	.await?
	{
		AttributeResult::Rotated => transactions.push("Rotated attribute 'website'".to_string()),
		AttributeResult::Added => transactions.push("Added attribute 'website'".to_string()),
	}
	snapshot.set_attribute("website", format!("https://{label}.cord.dev"));

	let token_identifier = identifier_from_str(&entity_token)?;
	let timeline_req = TokenTimelineRequest {
		auth: view_auth.clone(),
		token: token_identifier.clone(),
		start: Some(0),
		limit: Some(32),
	};
	let token_timeline_entries = client.query().token().timeline(&timeline_req).await?;
	let combined_timeline = build_token_activity(&token_timeline_entries);

	let sub_req =
		EntityLinkedAccountsRequest { auth: view_auth.clone(), token: token_identifier.clone() };
	let sub_accounts = client.query().entity().linked_accounts(&sub_req).await?;
	snapshot.set_active_accounts(&sub_accounts);

	let nym_req = EntityNymRequest { auth: view_auth.clone(), token: token_identifier };
	if let Some(nym) = client.query().entity().entity_nym(&nym_req).await? {
		snapshot.set_entity_nym(nym);
	}

	if output_json {
		let json = json!({
			"snapshot": snapshot,
			"timeline": combined_timeline,
			"transactions": transactions,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
		return Ok(());
	}

	println!();
	print_summary(&snapshot, &combined_timeline, &sub_accounts, &transactions);
	Ok(())
}

async fn ensure_entity_token_verbose(
	client: &origin::Client,
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
		println!("\n📍 Entity profile already exists (token {token})");
		return Ok((token, false));
	}

	let call = client.tx().entity_set_info_json(profile.clone()).await?;
	let events =
		submit_and_confirm(client, signer, call, "\n📍Set entity profile for Alice", nonce_tracker)
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
	client: &origin::Client,
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

#[derive(Copy, Clone)]
enum AttributeIntent {
	PreferRotate,
	PreferAdd,
}

#[derive(Copy, Clone)]
enum AttributeResult {
	Rotated,
	Added,
}

async fn ensure_attribute(
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	key: &str,
	value: String,
	intent: AttributeIntent,
	nonce_tracker: &mut NonceTracker,
) -> Result<AttributeResult> {
	match intent {
		AttributeIntent::PreferRotate => {
			match submit_attribute_rotation(client, signer, key, value.clone(), nonce_tracker).await
			{
				Ok(_) => Ok(AttributeResult::Rotated),
				Err(SubmitError::Invalid(msg)) => {
					println!("  ↳ ⚠️ attribute '{key}' rotation rejected ({msg}); adding instead");
					submit_attribute_add(client, signer, key, value, nonce_tracker).await?;
					Ok(AttributeResult::Added)
				},
				Err(err) => Err(anyhow!(err)),
			}
		},
		AttributeIntent::PreferAdd => {
			match submit_attribute_add(client, signer, key, value.clone(), nonce_tracker).await {
				Ok(_) => Ok(AttributeResult::Added),
				Err(err) => {
					if err.message().contains("AttributeExists") {
						println!("  ↳ ⚠️ attribute '{key}' already exists; rotating instead");
						submit_attribute_rotation(client, signer, key, value, nonce_tracker)
							.await?;
						Ok(AttributeResult::Rotated)
					} else {
						Err(anyhow!(err))
					}
				},
			}
		},
	}
}

async fn submit_attribute_rotation(
	client: &origin::Client,
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
	client: &origin::Client,
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
	client: &origin::Client,
	signer: &tx::signer::sr25519::Keypair,
	call: subxt::tx::DynamicPayload,
	description: &str,
	nonce_tracker: &mut NonceTracker,
) -> Result<ExtrinsicEvents<CordConfig>, SubmitError> {
	println!("⏳ {description} ...");
	let nonce = nonce_tracker.next(client).await?;
	let params = build_cord_params(
		DefaultExtrinsicParamsBuilder::<CordConfig>::new()
			.nonce(nonce)
			.tip(0)
			.immortal(),
	);
	let mut tx = client.online().tx();
	let mut progress = match tx.sign_and_submit_then_watch(&call, signer, params).await {
		Ok(progress) => progress,
		Err(err) => {
			nonce_tracker.rewind();
			return Err(SubmitError::Node(err.to_string()));
		},
	};
	while let Some(status) = progress.next().await {
		let status = match status {
			Ok(s) => s,
			Err(err) => {
				nonce_tracker.rewind();
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
				let events =
					in_block.wait_for_success().await.map_err(SubmitError::from_subxt_error)?;
				println!("  ↳ 📦 included in block {block_label}");
				// println!("✅ {description} recorded in block {block_label}");
				return Ok(events);
			},
			TxStatus::InFinalizedBlock(in_block) => {
				let block_hash = in_block.block_hash();
				let block_label = block_label(client, block_hash).await;
				let events =
					in_block.wait_for_success().await.map_err(SubmitError::from_subxt_error)?;
				println!("  ↳ 🛡️ finalized in block {block_label}");
				// println!("✅ {description} finalized in block {block_label}");
				return Ok(events);
			},
			TxStatus::Error { message } => {
				nonce_tracker.rewind();
				return Err(SubmitError::Node(message));
			},
			TxStatus::Invalid { message } => {
				nonce_tracker.rewind();
				return Err(SubmitError::Invalid(message));
			},
			TxStatus::Dropped { message } => {
				nonce_tracker.rewind();
				return Err(SubmitError::Dropped(message));
			},
		}
	}
	nonce_tracker.rewind();
	Err(SubmitError::StreamEnded)
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
}

async fn block_label(client: &origin::Client, block_hash: H256) -> String {
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

#[derive(Clone, Serialize)]
struct TimelineRow {
	pub version: u64,
	pub action: String,
	pub digest: String,
	pub block: u32,
	pub extrinsic: u32,
}

struct NonceTracker {
	next: Option<u64>,
	account: AccountId32,
}

impl NonceTracker {
	fn new(account: AccountId32) -> Self {
		Self { next: None, account }
	}

	fn rewind(&mut self) {
		self.next = None;
	}

	async fn next(&mut self, client: &origin::Client) -> Result<u64, SubmitError> {
		if self.next.is_none() {
			let nonce = client
				.online()
				.tx()
				.account_nonce(&self.account)
				.await
				.map_err(|e| SubmitError::Node(e.to_string()))?;
			self.next = Some(nonce);
		}
		let current = self.next.expect("nonce set above");
		self.next = Some(current + 1);
		Ok(current)
	}
}

fn build_token_activity(entries: &[InfoTokenHistoryEntry]) -> Vec<TimelineRow> {
	let mut rows: Vec<_> = entries
		.iter()
		.map(|entry| (entry.block.height, entry.block.index, entry))
		.collect();
	rows.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
	rows.iter()
		.enumerate()
		.map(|(idx, (_, _, entry))| TimelineRow {
			version: idx as u64,
			action: entry.action_utf8.clone().unwrap_or_else(|| entry.action_hex.clone()),
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
	println!("    (Version 0 represents token genesis)");
	println!("    Version  Action                         Digest               Block   Extrinsic");
	for entry in entries {
		println!(
			"    {:>7}  {:<30} {:<20} #{:<6}  {}",
			entry.version,
			truncate_label(&entry.action, 30),
			entry.digest,
			entry.block,
			entry.extrinsic
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

	fn from_origin_error(err: origin::Error) -> Self {
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
