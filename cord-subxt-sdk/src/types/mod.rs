pub mod element;
pub mod entity;
pub mod registry;

pub use element::{attribute_pair_value, bytes_value, element_json_to_dynamic, ElementJson};
pub use registry::{
	info_element_from_value, PacketPayload, PayloadMode, RegistryBlueprint, RegistrySchema,
};

use crate::error::{Error, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use core::str::FromStr;
use scale_value::Value;
use subxt::utils::AccountId32;

/// Convert a UTF-8 key into the on-chain hex representation.
pub fn to_key_hex_from_utf8(s: &str) -> String {
	format!("0x{}", hex::encode(s.as_bytes()))
}

/// Parse an SS58 string into an [`AccountId32`].
pub fn ss58_to_account32(s: &str) -> Result<AccountId32> {
	AccountId32::from_str(s).map_err(|e| Error::Params(format!("invalid ss58: {e:?}")))
}

/// Decode a hex string (with or without `0x`).
pub fn hex_to_bytes(hexstr: &str) -> Result<Vec<u8>> {
	let trimmed = hexstr.strip_prefix("0x").unwrap_or(hexstr);
	hex::decode(trimmed).map_err(|e| Error::Params(e.to_string()))
}

/// Decode a base64 string.
pub fn base64_to_bytes(s: &str) -> Result<Vec<u8>> {
	BASE64.decode(s).map_err(|e| Error::Params(e.to_string()))
}

/// Encode an SS58 identifier into the SCALE `Value` shape expected by runtime APIs.
pub fn identifier_value(ss58: &str) -> Result<Value> {
	if ss58.is_empty() {
		return Err(Error::Params("ss58 identifier cannot be empty".into()));
	}
	let bytes = ss58.as_bytes();
	if bytes.len() > 64 {
		return Err(Error::Params("ss58 identifier exceeds 64 bytes".into()));
	}
	Ok(bytes_value(bytes))
}
