use crate::{
	api::runtime,
	error::{Error, Result},
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bs58;
use hex;
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

fn empty_fields() -> Composite<()> {
	Composite::unnamed(Vec::new())
}

/// Encode a byte slice as a SCALE dynamic vector.
pub fn bytes_value(bytes: &[u8]) -> Value {
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
			let raw = crate::types::hex_to_bytes(hexstr)?;
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

/// Convert a `(key, ElementJson)` pair into the tuple expected by runtime metadata.
pub fn attribute_pair_value(key: &[u8], element: &ElementJson) -> Result<Value> {
	let key_value = bytes_value(key);
	let element_value = element_json_to_dynamic(element)?;
	Ok(Value::unnamed_composite([key_value, element_value]))
}

pub fn element_text_from_runtime(
	element: &runtime::runtime_types::cord_primitives::element::Elum,
) -> Option<String> {
	match element {
		runtime::runtime_types::cord_primitives::element::Elum::None => None,
		runtime::runtime_types::cord_primitives::element::Elum::Raw(bytes) => {
			match String::from_utf8(bytes.0.clone()) {
				Ok(text) => Some(text),
				Err(_) => Some(format!("0x{}", hex::encode(&bytes.0))),
			}
		},
		runtime::runtime_types::cord_primitives::element::Elum::Bool(flag) => {
			Some(((*flag) != 0).to_string())
		},
		runtime::runtime_types::cord_primitives::element::Elum::U64(bytes) => {
			Some(u64::from_le_bytes(*bytes).to_string())
		},
		runtime::runtime_types::cord_primitives::element::Elum::U128(bytes) => {
			Some(u128::from_le_bytes(*bytes).to_string())
		},
		runtime::runtime_types::cord_primitives::element::Elum::Hash(digest) => {
			Some(format!("0x{}", hex::encode(digest)))
		},
		runtime::runtime_types::cord_primitives::element::Elum::Token(identifier) => {
			Some(String::from_utf8_lossy(identifier.as_ref()).into_owned())
		},
		runtime::runtime_types::cord_primitives::element::Elum::CID(bytes) => {
			Some(bs58::encode(bytes.0.clone()).into_string())
		},
	}
}

pub fn element_json_from_runtime(
	element: &runtime::runtime_types::cord_primitives::element::Elum,
) -> ElementJson {
	match element {
		runtime::runtime_types::cord_primitives::element::Elum::None => ElementJson::None,
		runtime::runtime_types::cord_primitives::element::Elum::Raw(bytes) => {
			ElementJson::RawBase64(BASE64.encode(&bytes.0))
		},
		runtime::runtime_types::cord_primitives::element::Elum::Bool(flag) => {
			ElementJson::Bool(*flag != 0)
		},
		runtime::runtime_types::cord_primitives::element::Elum::U64(bytes) => {
			ElementJson::U64(u64::from_le_bytes(*bytes))
		},
		runtime::runtime_types::cord_primitives::element::Elum::U128(bytes) => {
			ElementJson::U128(u128::from_le_bytes(*bytes))
		},
		runtime::runtime_types::cord_primitives::element::Elum::Hash(digest) => {
			ElementJson::HashHex(format!("0x{}", hex::encode(digest)))
		},
		runtime::runtime_types::cord_primitives::element::Elum::Token(identifier) => {
			ElementJson::TokenSs58(String::from_utf8_lossy(identifier.as_ref()).into_owned())
		},
		runtime::runtime_types::cord_primitives::element::Elum::CID(bytes) => {
			ElementJson::CidBase58(bs58::encode(bytes.0.clone()).into_string())
		},
	}
}
