use crate::error::{Error, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use scale_value::{Composite, Value};
use serde::{Deserialize, Serialize};

/// JSON-friendly representation of on-chain `Element` variants.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ElementJson {
	None,
	RawBase64(String),
	Bool(bool),
	U64(u64),
	U128(u128),
	HashHex(String),
	TokenSs58(String),
	CidBase58(String),
}

/// Attribute entry used when constructing entity extrinsics.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeEntry {
	pub key_hex: String,
	#[serde(default)]
	pub key_utf8: Option<String>,
	pub value: ElementJson,
}

/// Lightweight block descriptor returned by dev view functions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevEventBlockView {
	pub height: u32,
	pub index: u32,
}

/// Attribute history entry JSON representation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoAttributeHistoryEntry {
	pub key_hex: String,
	#[serde(default)]
	pub key_utf8: Option<String>,
	pub version: u64,
	pub old_value_base64: String,
	pub block: DevEventBlockView,
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
		let key = bytes_value(&self.key_bytes()?);
		let element = element_json_to_dynamic(&self.value)?;
		Ok(Value::unnamed_composite([key, element]))
	}
}

fn empty_fields() -> Composite<()> {
	Composite::unnamed(Vec::new())
}

fn bytes_value(bytes: &[u8]) -> Value {
	Value::unnamed_composite(bytes.iter().copied().map(|b| Value::u128(b as u128)))
}

fn fixed_bytes<const N: usize>(bytes: Vec<u8>, label: &str) -> Result<[u8; N]> {
	if bytes.len() != N {
		return Err(Error::Params(format!("{label} must be {N} bytes")));
	}
	let mut out = [0u8; N];
	out.copy_from_slice(&bytes);
	Ok(out)
}

/// Convert JSON element into a SCALE dynamic value compatible with runtime metadata.
pub fn element_json_to_dynamic(value: &ElementJson) -> Result<Value> {
	match value {
		ElementJson::None => Ok(Value::variant("None", empty_fields())),
		ElementJson::RawBase64(data) => {
			let decoded = BASE64.decode(data).map_err(|e| Error::Params(e.to_string()))?;
			Ok(Value::unnamed_variant("Raw", [bytes_value(&decoded)]))
		},
		ElementJson::Bool(flag) => {
			let raw = if *flag { 1u8 } else { 0u8 };
			Ok(Value::unnamed_variant("Bool", [Value::u128(raw as u128)]))
		},
		ElementJson::U64(num) => {
			let le = num.to_le_bytes();
			Ok(Value::unnamed_variant("U64", [bytes_value(&le)]))
		},
		ElementJson::U128(num) => {
			let le = num.to_le_bytes();
			Ok(Value::unnamed_variant("U128", [bytes_value(&le)]))
		},
		ElementJson::HashHex(hexstr) => {
			let raw = super::hex_to_bytes(hexstr)?;
			let arr = fixed_bytes::<32>(raw, "hash")?;
			Ok(Value::unnamed_variant("Hash", [bytes_value(&arr)]))
		},
		ElementJson::TokenSs58(token) => {
			if token.is_empty() {
				return Err(Error::Params("token ss58 value is empty".into()));
			}
			Ok(Value::unnamed_variant("Token", [bytes_value(token.as_bytes())]))
		},
		ElementJson::CidBase58(cid) => {
			let raw = bs58::decode(cid).into_vec().map_err(|e| Error::Params(e.to_string()))?;
			Ok(Value::unnamed_variant("CID", [bytes_value(&raw)]))
		},
	}
}
