use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::Serialize;
use sp_runtime::AccountId32 as RuntimeAccount;
use std::collections::BTreeMap;
use subxt::utils::AccountId32;

use crate::types::entity::HistoryEntry;

#[derive(Clone, Serialize)]
pub struct EntitySnapshot {
	pub token: String,
	pub display: String,
	pub legal: String,
	pub web: String,
	pub email: String,
	pub twitter: String,
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
			legal: profile.get("legal").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
			web: profile.get("web").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
			email: profile.get("email").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
			twitter: profile.get("twitter").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
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

	pub fn set_active_accounts(&mut self, accounts: &[AccountId32]) {
		self.active_accounts = accounts
			.iter()
			.map(|acct| {
				let raw: [u8; 32] = *acct.as_ref();
				RuntimeAccount::from(raw).to_string()
			})
			.collect();
	}

	pub fn set_entity_nym(&mut self, nym: String) {
		self.entity_nym = Some(nym);
	}

	pub fn print_cli(&self) {
		println!("\n📇 Entity Details:");
		println!("  • Token   : {}", self.token);
		if let Some(nym) = &self.entity_nym {
			println!("  • Nym     : {}", nym);
		}
		println!("  • Display : {}", self.display);
		println!("  • Legal   : {}", self.legal);
		println!("  • Web     : {}", self.web);
		println!("  • Email   : {}", self.email);
		println!("  • Twitter : {}", self.twitter);
		println!("  • Attributes:");
		for (key, value) in &self.attributes {
			let rendered = short_label(value);
			println!("      ◦ {}: {}", key, rendered);
		}
	}
}

const MAX_ATTR_LABEL_LEN: usize = "entity-demo-78abb661@cord.dev".len() + 5;

fn short_label(value: &str) -> String {
	if value.len() <= MAX_ATTR_LABEL_LEN {
		return value.to_string();
	}
	if value.starts_with("0x") && value.len() > 2 {
		let head = 12.min(value.len());
		let tail = 6.min(value.len().saturating_sub(head + 1));
		let tail_start = value.len().saturating_sub(tail);
		return format!("{}…{}", &value[..head], &value[tail_start..]);
	}
	let take = MAX_ATTR_LABEL_LEN.saturating_sub(1);
	let trimmed: String = value.chars().take(take).collect();
	format!("{trimmed}…")
}

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

pub fn print_accounts_cli(accounts: &[AccountId32]) {
	println!("\n🔗 Linked Accounts:");
	if accounts.is_empty() {
		println!("    • (none)");
		return;
	}
	for (idx, account) in accounts.iter().enumerate() {
		let raw: [u8; 32] = *account.as_ref();
		let runtime = RuntimeAccount::from(raw);
		println!("    • [{}] {}", idx + 1, runtime);
	}
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
