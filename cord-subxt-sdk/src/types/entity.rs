use crate::error::{Error, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use scale_value::Value;
use serde::{Deserialize, Serialize};

use super::element::attribute_pair_value;
pub use super::element::ElementJson;

/// Attribute entry used when constructing entity extrinsics.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeEntry {
	pub key_hex: String,
	#[serde(default)]
	pub key_utf8: Option<String>,
	pub value: ElementJson,
}

impl AttributeEntry {
	/// Render the key as raw bytes, preferring `key_hex` and falling back to UTF-8.
	pub fn key_bytes(&self) -> Result<Vec<u8>> {
		if let Some(hex) =
			self.key_hex.strip_prefix("0x").or_else(|| self.key_hex.strip_prefix("0X"))
		{
			if !hex.is_empty() {
				return hex::decode(hex).map_err(|e| Error::Params(e.to_string()));
			}
		}
		if !self.key_hex.is_empty() {
			return hex::decode(&self.key_hex).map_err(|e| Error::Params(e.to_string()));
		}
		if let Some(utf8) = &self.key_utf8 {
			return Ok(utf8.as_bytes().to_vec());
		}
		Err(Error::Params("attribute key missing (hex or utf8)".into()))
	}

	/// Convert the entry into the `(Vec<u8>, Element)` tuple expected by the runtime metadata.
	pub fn to_dynamic_pair(&self) -> Result<Value> {
		attribute_pair_value(&self.key_bytes()?, &self.value)
	}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRef {
	pub height: u32,
	pub index: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
	pub key_hex: String,
	pub key_utf8: Option<String>,
	pub version: u64,
	pub old_value_base64: String,
	pub block: BlockRef,
}

impl HistoryEntry {
	pub fn from_raw(key: &[u8], version: u64, old_value: &[u8], block: BlockRef) -> Self {
		Self {
			key_hex: hex_string(key),
			key_utf8: maybe_utf8(key),
			version,
			old_value_base64: base64_string(old_value),
			block,
		}
	}
}

fn hex_string(bytes: &[u8]) -> String {
	let mut s = String::from("0x");
	s.push_str(&hex::encode(bytes));
	s
}

fn base64_string(bytes: &[u8]) -> String {
	BASE64.encode(bytes)
}

fn maybe_utf8(bytes: &[u8]) -> Option<String> {
	core::str::from_utf8(bytes).ok().map(|s| s.to_string())
}
