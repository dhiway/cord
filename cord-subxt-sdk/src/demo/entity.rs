use cord_primitives::view::InfoAttributeHistoryEntry;
use serde::Serialize;
use serde_json::{json, Value};
use sp_runtime::AccountId32 as RuntimeAccount;
use std::collections::BTreeMap;
use subxt::utils::AccountId32;

#[derive(Clone, Serialize)]
pub struct EntitySnapshot {
	pub token: String,
	pub display: String,
	pub legal: String,
	pub web: String,
	pub email: String,
	pub twitter: String,
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

	pub fn to_json(&self, history: &[InfoAttributeHistoryEntry]) -> Value {
		json!({
			"snapshot": self,
			"history": history,
			"activeAccounts": self.active_accounts,
		})
	}

	pub fn print_cli(&self) {
		println!("\nCurrent entity snapshot:");
		println!("  Token  : {}", self.token);
		println!("  Display: {}", self.display);
		println!("  Legal  : {}", self.legal);
		println!("  Web    : {}", self.web);
		println!("  Email  : {}", self.email);
		println!("  Twitter: {}", self.twitter);
		println!("  Attributes:");
		for (key, value) in &self.attributes {
			println!("    - {}: {}", key, value);
		}
	}
}

pub fn print_history_cli(entries: &[InfoAttributeHistoryEntry]) {
	println!("\nAttribute timeline:");
	println!("    Version  Block    Key         Old Value (base64)");
	for entry in entries {
		println!(
			"{:>10}  #{:<4}  {:<10}  {}",
			entry.version, entry.block.height, entry.key_hex, entry.old_value_base64
		);
	}
}

pub fn print_accounts_cli(accounts: &[AccountId32]) {
	println!("\nActive accounts:");
	if accounts.is_empty() {
		println!("    (none)");
		return;
	}
	for (idx, account) in accounts.iter().enumerate() {
		let raw: [u8; 32] = *account.as_ref();
		let runtime = RuntimeAccount::from(raw);
		println!("    - [{}] {}", idx + 1, runtime);
	}
}
