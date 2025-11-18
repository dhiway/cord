use crate::{
	demo::util::{fresh_authorization, ViewStyle},
	entity::TimelineRow,
	tx,
	types::entity::HistoryEntry,
	utils, Client,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use origin_primitives::identifier::Ss58Identifier;
use serde::Serialize;
use serde_json::json;
use sp_core::crypto::{Ss58AddressFormat, Ss58Codec};
use sp_runtime::AccountId32 as RuntimeAccount;
use std::collections::BTreeMap;
use subxt::utils::AccountId32;

use crate::entity::EntityChainState;

#[derive(Clone, Serialize)]
pub struct EntitySnapshot {
	pub token: String,
	pub display: String,
	pub web: String,
	pub email: String,
	pub entity_nym: Option<String>,
	pub attributes: BTreeMap<String, String>,
	pub active_accounts: Vec<String>,
}

impl EntitySnapshot {
	pub fn from_profile(profile: &serde_json::Value, token: &str) -> Self {
		let mut attributes = BTreeMap::new();
		if let Some(obj) = profile.get("attributes").and_then(|v| v.as_object()) {
			for (key, value) in obj {
				if let Some(text) = value.as_str() {
					attributes.insert(key.clone(), text.to_string());
				}
			}
		}
		Self {
			token: token.to_string(),
			display: profile.get("display").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
			web: profile.get("web").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
			email: profile.get("email").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
			entity_nym: None,
			attributes,
			active_accounts: Vec::new(),
		}
	}

	pub fn set_email(&mut self, value: String) {
		self.email = value;
	}

	pub fn set_attribute(&mut self, key: &str, value: String) {
		self.attributes.insert(key.to_string(), value);
	}

	pub fn set_active_accounts(
		&mut self,
		accounts: &[AccountId32],
		chain_prefix: Ss58AddressFormat,
	) {
		self.active_accounts =
			accounts.iter().map(|acct| format_account(acct, chain_prefix)).collect();
	}

	pub fn set_entity_nym(&mut self, nym: String) {
		self.entity_nym = Some(nym);
	}

	pub fn from_chain_state(state: &EntityChainState, token: &str) -> Self {
		let mut snapshot = Self {
			token: token.to_string(),
			display: state.get("display").unwrap_or("-").to_string(),
			web: state.get("web").unwrap_or("-").to_string(),
			email: state.get("email").unwrap_or("-").to_string(),
			entity_nym: None,
			attributes: BTreeMap::new(),
			active_accounts: Vec::new(),
		};
		for (key, value) in state.attributes_iter() {
			snapshot.attributes.insert(key.clone(), value.clone());
		}
		snapshot
	}

	pub fn print_cli(&self) {
		println!("\n📇 Entity Details:");
		println!("  • Token   : {}", self.token);
		if let Some(nym) = &self.entity_nym {
			println!("  • Nym     : {}", nym);
		}
		println!("  • Display : {}", self.display);
		println!("  • Web     : {}", self.web);
		println!("  • Email   : {}", self.email);
		println!("  • Attributes:");
		for (key, value) in &self.attributes {
			let rendered = utils::short_label(value, MAX_ATTR_LABEL_LEN);
			println!("      ◦ {}: {}", key, rendered);
		}
	}
}

const MAX_ATTR_LABEL_LEN: usize = "entity-demo-78abb661@cord.dev".len() + 5;

pub fn print_history_cli(entries: &[(HistoryEntry, Option<String>)]) {
	println!("\n📜 Attribute Timeline:");
	if entries.is_empty() {
		println!("    • (no recorded attribute changes)");
		return;
	}
	println!(
		"    Version  Action      Key                Value                        Block   Time"
	);
	for (entry, time) in entries {
		let value = truncate_value(&base64_to_utf8(&entry.old_value_base64), 28);
		let time_display = time.as_deref().unwrap_or("unknown");
		println!(
			"{:>10}  {:<10}  {:<18}  {:<28}  #{:<6}  {}",
			entry.version,
			"Rotated",
			entry.key_utf8.clone().unwrap_or_else(|| entry.key_hex.clone()),
			value,
			entry.block.height,
			time_display
		);
	}
}

pub fn print_accounts_cli(accounts: &[AccountId32], chain_prefix: Ss58AddressFormat) {
	println!("\n➡️ Linked Accounts");
	if accounts.is_empty() {
		println!("  ↳ • (none)");
		return;
	}
	for (idx, account) in accounts.iter().enumerate() {
		let formatted = format_account(account, chain_prefix);
		println!("  ↳ • [{}] {}", idx + 1, formatted);
	}
}

fn format_account(account: &AccountId32, chain_prefix: Ss58AddressFormat) -> String {
	let raw: [u8; 32] = *account.as_ref();
	let runtime = RuntimeAccount::from(raw);
	runtime.to_ss58check_with_version(chain_prefix)
}

fn base64_to_utf8(value: &str) -> String {
	match BASE64_STANDARD.decode(value.as_bytes()) {
		Ok(bytes) => match String::from_utf8(bytes) {
			Ok(text) => text,
			Err(_) => value.to_string(),
		},
		Err(_) => value.to_string(),
	}
}

fn truncate_value(value: &str, max_len: usize) -> String {
	if value.len() <= max_len {
		value.to_string()
	} else {
		let mut truncated = value.chars().take(max_len.saturating_sub(1)).collect::<String>();
		truncated.push('…');
		truncated
	}
}

pub fn print_entity_sections(
	snapshot: &EntitySnapshot,
	attr_history: &[HistoryEntry],
	timeline: &[TimelineRow],
	accounts: &[AccountId32],
	style: ViewStyle,
	chain_prefix: Ss58AddressFormat,
) {
	print_entity_core(snapshot, accounts, chain_prefix);
	print_attribute_history(attr_history, style.is_full());
	print_combined_timeline(timeline, style.is_full());
	println!();
}

pub fn print_entity_summary(
	snapshot: &EntitySnapshot,
	accounts: &[AccountId32],
	chain_prefix: Ss58AddressFormat,
) {
	print_entity_core(snapshot, accounts, chain_prefix);
	println!();
}

pub fn print_combined_timeline(entries: &[TimelineRow], full_view: bool) {
	println!("\n🔀 Activity (latest first)");
	if entries.is_empty() {
		println!("    • (no recorded activity)");
		return;
	}
	println!(
		"  ↳    {:>8}  {:>8}  {:>6}    {:<30} {:<34}",
		"Version", "Block", "Index", "Action", "Digest"
	);
	let total = entries.len();
	let mut shown = 0usize;
	let iter: Box<dyn Iterator<Item = &TimelineRow>> =
		if full_view { Box::new(entries.iter()) } else { Box::new(entries.iter().take(10)) };
	for entry in iter {
		println!(
			"       {:>8}  {:>8}  {:>6}    {:<30} {:<34}",
			entry.version,
			format!("#{}", entry.block),
			entry.extrinsic,
			truncate_label(&entry.action, 30),
			utils::short_label(&entry.digest, MAX_LABEL_LEN)
		);
		shown += 1;
	}
	if !full_view && total > shown {
		println!("    … {} older", total - shown);
	}
}

pub fn print_attribute_history(entries: &[HistoryEntry], full_view: bool) {
	println!("\n🔁 Rotations (latest first)");
	if entries.is_empty() {
		println!("  ↳ • (no attribute history)");
		return;
	}
	println!("  ↳    {:>8}  {:>6}    {:<18} {:<34}", "Block", "Index", "Key", "Rotated Value");
	let mut ordered: Vec<&HistoryEntry> = entries.iter().collect();
	ordered.sort_by(|a, b| (b.block.height, b.block.index).cmp(&(a.block.height, a.block.index)));
	let total = ordered.len();
	let mut shown = 0usize;
	let iter: Box<dyn Iterator<Item = &&HistoryEntry>> =
		if full_view { Box::new(ordered.iter()) } else { Box::new(ordered.iter().take(10)) };
	for entry in iter {
		let key = entry.key_utf8.clone().unwrap_or_else(|| entry.key_hex.clone());
		let value =
			utils::short_label(&utils::decode_attr_value(&entry.old_value_base64), MAX_LABEL_LEN);
		println!(
			"       {:>8}  {:>6}    {:<18} {:<34}",
			format!("#{}", entry.block.height),
			entry.block.index,
			truncate_label(&key, 18),
			value
		);
		shown += 1;
	}
	if !full_view && total > shown {
		println!("    … {} older", total - shown);
	}
}

pub fn print_identifier_block(snapshot: &EntitySnapshot) {
	println!("  ↳ • Token  : {}", snapshot.token);
	if let Some(nym) = &snapshot.entity_nym {
		println!("  ↳ • Nym    : {}", nym);
	}
}

pub fn print_entity_core(
	snapshot: &EntitySnapshot,
	accounts: &[AccountId32],
	chain_prefix: Ss58AddressFormat,
) {
	println!("\n⏺️ Entity Snapshot (latest block)");
	println!("\nℹ️ Identifiers");
	print_identifier_block(snapshot);
	println!("\n🈁 Info");
	print_entity_info(snapshot);
	println!("\n🔢 Attributes");
	print_attribute_list(snapshot);
	print_accounts_cli(accounts, chain_prefix);
}

pub fn print_entity_info(snapshot: &EntitySnapshot) {
	println!("  ↳ • Display : {}", snapshot.display);
	println!("    • Web     : {}", snapshot.web);
	println!("    • Email   : {}", snapshot.email);
}

pub fn print_attribute_list(snapshot: &EntitySnapshot) {
	if snapshot.attributes.is_empty() {
		println!("  ↳ • (no custom attributes)");
		return;
	}
	for (key, value) in &snapshot.attributes {
		println!("  ↳ • {:<10} : {}", key, utils::short_label(value, MAX_LABEL_LEN));
	}
}

pub fn truncate_label(value: &str, max_len: usize) -> String {
	if value.len() <= max_len {
		value.to_string()
	} else {
		let mut truncated = value.chars().take(max_len.saturating_sub(1)).collect::<String>();
		truncated.push('…');
		truncated
	}
}

const MAX_LABEL_LEN: usize = 32;

pub async fn render_entity_snapshot(
	client: &Client,
	signer: &tx::signer::Keypair,
	token_identifier: &Ss58Identifier,
	snapshot: &mut EntitySnapshot,
	style: ViewStyle,
	output_json: bool,
	chain_prefix: Ss58AddressFormat,
	min_expected: u32,
	include_history: bool,
) -> anyhow::Result<()> {
	let mut timeline = Vec::new();
	let mut history = Vec::new();
	let linked_accounts: Vec<AccountId32>;

	if include_history {
		let timeline_reference_block = client.view_auth_reference_block().await?;
		let mut timeline_auth = || fresh_authorization(timeline_reference_block, &signer);
		timeline = crate::entity::fetch_full_token_timeline(
			client,
			token_identifier,
			min_expected,
			&mut timeline_auth,
		)
		.await
		.unwrap_or_else(|err| {
			eprintln!("⚠️ unable to fetch timeline: {err}");
			Vec::new()
		});
		let history_reference_block = client.view_auth_reference_block().await?;
		let mut history_auth = || fresh_authorization(history_reference_block, &signer);
		history =
			crate::entity::collect_attribute_history(client, token_identifier, &mut history_auth)
				.await
				.unwrap_or_else(|err| {
					eprintln!("⚠️ unable to fetch attribute history: {err}");
					Vec::new()
				});
	}

	let links_reference_block = client.view_auth_reference_block().await?;
	let mut links_auth = || fresh_authorization(links_reference_block, &signer);
	linked_accounts =
		crate::entity::fetch_linked_accounts(client, token_identifier, &mut links_auth)
			.await
			.unwrap_or_else(|err| {
				eprintln!("⚠️ unable to fetch linked accounts: {err}");
				Vec::new()
			});
	snapshot.set_active_accounts(&linked_accounts, chain_prefix);

	if output_json {
		let attr_json: Vec<_> = history
			.iter()
			.map(|entry| {
				json!({
					"version": entry.version,
					"key": entry.key_utf8.clone().unwrap_or_else(|| entry.key_hex.clone()),
					"oldValue": utils::decode_attr_value(&entry.old_value_base64),
					"block": entry.block.height,
					"extrinsic": entry.block.index,
				})
			})
			.collect();
		let json = json!({
			"snapshot": snapshot,
			"timeline": crate::entity::build_token_activity(&timeline),
			"attributeHistory": attr_json,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
	} else if include_history {
		print_entity_sections(
			snapshot,
			&history,
			&crate::entity::build_token_activity(&timeline),
			&linked_accounts,
			style,
			chain_prefix,
		);
	} else {
		print_entity_summary(snapshot, &linked_accounts, chain_prefix);
	}
	Ok(())
}
