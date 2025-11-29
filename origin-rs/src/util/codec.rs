//! Helpers to map user-friendly values to Origin Element encodings and back.

use crate::types::error::OriginSdkError;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use codec::Decode;
use origin_primitives::{element::ElementType, ElementView};
use scale_value::{Composite, Value, ValueDef};

/// Convert a serde_json value into a dynamic `Value` representing an Origin `Element` of the
/// expected type. This keeps extrinsics strongly typed without pallet deps.
pub fn element_value_from_json(
	expected: ElementType,
	json: &serde_json::Value,
) -> Result<Value, OriginSdkError> {
	let variant = match expected {
		ElementType::None => variant("None", Composite::unnamed(vec![])),
		ElementType::Raw => {
			let bytes = serde_json::to_vec(json)
				.map_err(|e| OriginSdkError::InvalidInput(format!("raw encode: {e}")))?;
			variant("Raw", Composite::unnamed(vec![Value::from_bytes(bytes)]))
		},
		ElementType::Bool => {
			let b = json.as_bool().ok_or_else(|| {
				OriginSdkError::InvalidInput(format!("expected bool, got {json}"))
			})?;
			variant("Bool", Composite::unnamed(vec![Value::u128(b as u128)]))
		},
		ElementType::U64 => {
			let n = json
				.as_u64()
				.ok_or_else(|| OriginSdkError::InvalidInput(format!("expected u64, got {json}")))?;
			variant("U64", Composite::unnamed(vec![Value::u128(n as u128)]))
		},
		ElementType::U128 => {
			let n = if let Some(u) = json.as_u64() {
				u as u128
			} else if let Some(s) = json.as_str() {
				s.parse::<u128>()
					.map_err(|e| OriginSdkError::InvalidInput(format!("u128 parse: {e}")))?
			} else {
				return Err(OriginSdkError::InvalidInput(format!("expected u128, got {json}")));
			};
			variant("U128", Composite::unnamed(vec![Value::u128(n)]))
		},
		ElementType::Hash => {
			let s = json.as_str().ok_or_else(|| {
				OriginSdkError::InvalidInput(format!("expected hex string, got {json}"))
			})?;
			let bytes = hex::decode(s)
				.map_err(|e| OriginSdkError::InvalidInput(format!("hash hex: {e}")))?;
			if bytes.len() != 32 {
				return Err(OriginSdkError::InvalidInput("hash must be 32 bytes".into()));
			}
			variant("Hash", Composite::unnamed(vec![Value::from_bytes(bytes)]))
		},
		ElementType::Token => {
			let s = json.as_str().ok_or_else(|| {
				OriginSdkError::InvalidInput(format!("expected ss58 string, got {json}"))
			})?;
			let id = origin_primitives::Ss58Identifier::try_from(s.to_string())
				.map_err(|e| OriginSdkError::InvalidInput(format!("ss58: {e:?}")))?;
			variant("Token", Composite::unnamed(vec![Value::from_bytes(id.as_ref())]))
		},
		ElementType::Cid => {
			let s = json.as_str().ok_or_else(|| {
				OriginSdkError::InvalidInput(format!("expected CID string, got {json}"))
			})?;
			if s.is_empty() {
				return Err(OriginSdkError::InvalidInput("cid cannot be empty".into()));
			}
			variant("CID", Composite::unnamed(vec![Value::from_bytes(s.as_bytes())]))
		},
	};
	Ok(Value { value: ValueDef::Variant(variant), context: () })
}

/// Convert an `ElementView` into serde_json for DX when reading.
pub fn element_view_to_json(ev: &ElementView) -> serde_json::Value {
	use origin_primitives::element::ElementType::*;
	match ev.element_type() {
		None => serde_json::Value::Null,
		Raw => serde_json::Value::String(B64.encode(ev.as_raw().unwrap_or(&[]))),
		Bool => serde_json::Value::Bool(ev.as_bool().unwrap_or(false)),
		U64 => serde_json::Value::Number(ev.as_u64().unwrap_or(0).into()),
		U128 => serde_json::Value::String(format!("{}", ev.as_u128().unwrap_or(0))),
		Hash => serde_json::Value::String(hex::encode(ev.as_hash().unwrap_or(&[0u8; 32]))),
		Token => serde_json::Value::String(
			ev.as_token().map(|t| t.to_string_lossy()).unwrap_or_else(|| "".into()),
		),
		Cid => serde_json::Value::String(
			String::from_utf8_lossy(ev.as_cid().unwrap_or(&[])).to_string(),
		),
	}
}

fn variant(name: &str, vals: Composite<()>) -> scale_value::Variant<()> {
	scale_value::Variant { name: name.to_string(), values: vals }
}

/// Decode SCALE-encoded view bytes into a concrete type.
pub fn decode_view<T: Decode>(bytes: &[u8]) -> Result<T, codec::Error> {
	T::decode(&mut &*bytes)
}
