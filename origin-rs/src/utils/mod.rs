use crate::{
	client::{Client, ConnectionConfig, DEFAULT_RPC_ENDPOINT},
	error::Result,
	flavors::ChainFlavor,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use getrandom::getrandom;
use sp_core::crypto::{Ss58AddressFormat, Ss58Codec};
use sp_core::{sr25519, Pair};
use std::time::Duration;
use tokio::time::sleep;

pub const DEFAULT_NODE_URL: &str = DEFAULT_RPC_ENDPOINT;

/// Connect to the supplied node URL or fall back to the local dev node.
pub async fn connect_or_default(url: Option<&str>, flavor: ChainFlavor) -> Result<Client> {
	let target = url.unwrap_or(DEFAULT_NODE_URL);
	let config = ConnectionConfig::new(target.to_string(), flavor);
	Client::connect_with(config).await
}

/// Convenience wrapper around `tokio::time::sleep` for short demo delays.
pub async fn short_delay(duration: Duration) {
	sleep(duration).await;
}

/// Random SR25519 public key rendered as `0x…` hex for demo attribute rotation.
pub fn random_public_key_hex() -> String {
	let mut seed = [0u8; 32];
	getrandom(&mut seed).expect("random seed");
	let pair = sr25519::Pair::from_seed(&seed);
	format!("0x{}", hex::encode(pair.public()))
}

/// Sanitize a human-friendly label into an entity nym prefix accepted by the runtime.
pub fn sanitize_entity_nym(label: &str) -> String {
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

/// Decode a base64 string into UTF-8, falling back to the original on error.
pub fn decode_attr_value(value: &str) -> String {
	match BASE64.decode(value.as_bytes()) {
		Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| value.to_string()),
		Err(_) => value.to_string(),
	}
}

/// Truncate a label using `…` in the middle when it exceeds `max_len` characters.
pub fn truncate_label(value: &str, max_len: usize) -> String {
	if value.len() <= max_len {
		value.to_string()
	} else {
		let keep = max_len.saturating_sub(1);
		let head = keep / 2;
		let tail = keep - head;
		let tail_start = value.len().saturating_sub(tail);
		format!("{}…{}", &value[..head], &value[tail_start..])
	}
}

/// Shorten long values while preserving enough entropy for CLI displays.
pub fn short_label(value: &str, max_len: usize) -> String {
	if value.len() <= max_len {
		return value.to_string();
	}
	let take = max_len.saturating_sub(1);
	let trimmed: String = value.chars().take(take).collect();
	format!("{trimmed}…")
}

/// Format SS58 accounts for CLI output.
pub fn format_account(
	account: &subxt::utils::AccountId32,
	chain_prefix: Ss58AddressFormat,
) -> String {
	use sp_runtime::AccountId32 as RuntimeAccount;
	let raw: [u8; 32] = *account.as_ref();
	let runtime = RuntimeAccount::from(raw);
	runtime.to_ss58check_with_version(chain_prefix)
}
