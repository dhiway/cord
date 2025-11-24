//! Argument transformers and dynamic call builders for Origin pallets.
//!
//! These helpers keep the public tx surface strongly typed while the SDK stays
//! fully dynamic (no codegen). They shape raw user input into pallet-ready
//! SCALE arguments and return `DynamicPayload` ready for submission.

use crate::types::error::OriginSdkError;
use origin_primitives::Ss58Identifier;
use scale_value::Value;
use subxt::{dynamic, tx::DynamicPayload, Metadata};

/// Simple helper to assert raw bytes length > 0.
fn ensure_non_empty(name: &str, bytes: &[u8]) -> Result<(), OriginSdkError> {
	if bytes.is_empty() {
		return Err(OriginSdkError::InvalidInput(format!("{name} cannot be empty")));
	}
	Ok(())
}

pub mod entity {
	use super::*;
	use serde_json::Value as Json;
	use crate::util::codec::element_value_from_json;

	/// Build `Entity::rotate_attribute` dynamic payload.
	pub fn rotate_attribute_call(
		_metadata: &Metadata,
		token: Ss58Identifier,
		key: &str,
		raw_value: &str,
	) -> Result<DynamicPayload, OriginSdkError> {
		ensure_non_empty("key", key.as_bytes())?;
		ensure_non_empty("value", raw_value.as_bytes())?;
		let args = vec![
			Value::from_bytes(token.as_ref()),
			Value::from_bytes(key.as_bytes()),
			Value::from_bytes(raw_value.as_bytes()),
		];
		Ok(dynamic::tx("Entity", "rotate_attribute", args))
	}

	/// JSON helper: encode value as bytes (placeholder until Element builder is added).
	pub fn rotate_attribute_from_json(
		_metadata: &Metadata,
		token: Ss58Identifier,
		key: &str,
		expected: origin_primitives::element::ElementType,
		value: &Json,
	) -> Result<DynamicPayload, OriginSdkError> {
		let elem = element_value_from_json(expected, value)?;
		let args = vec![Value::from_bytes(token.as_ref()), Value::from_bytes(key.as_bytes()), elem];
		Ok(dynamic::tx("Entity", "rotate_attribute", args))
	}

	/// Bulk helper: map a JSON object into multiple attribute updates.
	pub fn rotate_attributes_from_json(
		_metadata: &Metadata,
		token: Ss58Identifier,
		schema: &[(Vec<u8>, origin_primitives::element::ElementType, bool)],
		obj: &serde_json::Value,
	) -> Result<Vec<DynamicPayload>, OriginSdkError> {
		let map = obj.as_object().ok_or_else(|| {
			OriginSdkError::InvalidInput("attributes payload must be a JSON object".into())
		})?;
		let mut calls = Vec::new();
		for (k, kind, _optional) in schema {
			let key_str = String::from_utf8_lossy(k).to_string();
			if let Some(v) = map.get(&key_str) {
				let elem = element_value_from_json(*kind, v)?;
				let args =
					vec![Value::from_bytes(token.as_ref()), Value::from_bytes(k), elem];
				calls.push(dynamic::tx("Entity", "rotate_attribute", args));
			}
		}
		Ok(calls)
	}
}

pub mod registry {
	use super::*;
	use serde::Serialize;

	/// Build `Register::create_registry` payload from pre-encoded schema/config blobs.
	pub fn create_call(
		_metadata: &Metadata,
		registry_id: &[u8],
		schema_raw: &[u8],
		config_raw: &[u8],
		) -> Result<DynamicPayload, OriginSdkError> {
		ensure_non_empty("registry_id", registry_id)?;
		let args = vec![
			Value::from_bytes(registry_id),
			Value::from_bytes(schema_raw),
			Value::from_bytes(config_raw),
		];
		Ok(dynamic::tx("Register", "create_registry", args))
	}

	/// Serialize arbitrary schema/config structures to JSON then bytes for submission.
	pub fn create_from_structs<TSchema, TConfig>(
		metadata: &Metadata,
		registry_id: &[u8],
		schema: &TSchema,
		config: &TConfig,
	) -> Result<DynamicPayload, OriginSdkError>
	where
		TSchema: Serialize,
		TConfig: Serialize,
	{
		let schema_raw =
			serde_json::to_vec(schema).map_err(|e| OriginSdkError::Schema(format!("schema: {e}")))?;
		let config_raw =
			serde_json::to_vec(config).map_err(|e| OriginSdkError::Schema(format!("config: {e}")))?;
		create_call(metadata, registry_id, &schema_raw, &config_raw)
	}
}

pub mod packet {
	use super::*;
	use origin_primitives::registry::RegistryAttributeView;
	use serde_json::Value as Json;

	/// Validate packet body shape against a registry schema (placeholder) and build `create_packet`.
	pub fn issue_call(
		_metadata: &Metadata,
		registry_id: Ss58Identifier,
		registry_schema: &[RegistryAttributeView],
		packet_body: &Json,
	) -> Result<DynamicPayload, OriginSdkError> {
		let obj = packet_body
			.as_object()
			.ok_or_else(|| OriginSdkError::Schema("packet body must be a JSON object".into()))?;

		// Simple validation: required attributes must be present.
		for attr in registry_schema {
			let key_str = String::from_utf8_lossy(&attr.key).to_string();
			if let Some(value) = obj.get(&key_str) {
				validate_element_type(&key_str, attr.kind, value)?;
			} else if !attr.optional {
				return Err(OriginSdkError::Schema(format!(
					"missing required attribute '{key_str}'"
				)));
			}
		}

		// Check for unexpected keys.
		for key in obj.keys() {
			let key_bytes = key.as_bytes();
			let known = registry_schema.iter().any(|a| a.key.as_slice() == key_bytes);
			if !known {
				return Err(OriginSdkError::Schema(format!(
					"attribute '{key}' not defined in registry schema"
				)));
			}
		}

		let body_bytes = serde_json::to_vec(packet_body)
			.map_err(|e| OriginSdkError::Schema(format!("packet body encode: {e}")))?;

		let args = vec![Value::from_bytes(registry_id.as_ref()), Value::from_bytes(&body_bytes)];
		Ok(dynamic::tx("Register", "create_packet", args))
	}

	/// Minimal type validation for JSON values against ElementType.
	fn validate_element_type(
		key: &str,
		expected: origin_primitives::element::ElementType,
		value: &Json,
	) -> Result<(), OriginSdkError> {
		match expected {
			origin_primitives::element::ElementType::Raw => {
				if !value.is_string() && !value.is_number() && !value.is_boolean() && !value.is_array()
				{
					return Err(OriginSdkError::Schema(format!(
						"{key}: expected raw/json, got {value}"
					)));
				}
			},
			origin_primitives::element::ElementType::Bool => {
				if !value.is_boolean() {
					return Err(OriginSdkError::Schema(format!("{key}: expected bool, got {value}")));
				}
			},
			origin_primitives::element::ElementType::U64 => {
				if !(value.is_u64() || value.is_i64()) {
					return Err(OriginSdkError::Schema(format!("{key}: expected u64, got {value}")));
				}
			},
			origin_primitives::element::ElementType::U128 => {
				if let Some(n) = value.as_u64() {
					let _ = n; // always fits in u128
				} else if let Some(s) = value.as_str() {
					s.parse::<u128>()
						.map_err(|e| OriginSdkError::Schema(format!("{key}: invalid u128: {e}")))?;
				} else {
					return Err(OriginSdkError::Schema(format!("{key}: expected u128 (number/string), got {value}")));
				}
			},
			origin_primitives::element::ElementType::Hash => {
				if let Some(s) = value.as_str() {
					if s.len() % 2 != 0 {
						return Err(OriginSdkError::Schema(format!("{key}: hex length must be even")));
					}
					let _ = hex::decode(s).map_err(|e| OriginSdkError::Schema(format!("{key}: bad hex: {e}")))?;
				} else {
					return Err(OriginSdkError::Schema(format!("{key}: expected hex string, got {value}")));
				}
			},
			origin_primitives::element::ElementType::Token => {
				if let Some(s) = value.as_str() {
					origin_primitives::Ss58Identifier::try_from(s.to_string())
						.map_err(|e| OriginSdkError::Schema(format!("{key}: invalid ss58: {e:?}")))?;
				} else {
					return Err(OriginSdkError::Schema(format!("{key}: expected ss58 string, got {value}")));
				}
			},
			origin_primitives::element::ElementType::Cid => {
				if let Some(s) = value.as_str() {
					if s.len() < 8 || s.len() > 128 {
						return Err(OriginSdkError::Schema(format!("{key}: CID length out of range")));
					}
					if !s.is_ascii() || s.chars().any(char::is_whitespace) {
						return Err(OriginSdkError::Schema(format!("{key}: CID must be ASCII without whitespace")));
					}
				} else {
					return Err(OriginSdkError::Schema(format!("{key}: expected CID string, got {value}")));
				}
			},
			origin_primitives::element::ElementType::None => {},
		}
		Ok(())
	}
}

pub mod token {
	use super::*;
	use crate::util::codec::element_value_from_json;

	/// Build `Token::rotate_attribute` for token-level attributes (placeholder).
	pub fn rotate_attribute_call(
		_metadata: &Metadata,
		token: Ss58Identifier,
		key: &[u8],
		value: &[u8],
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![Value::from_bytes(token.as_ref()), Value::from_bytes(key), Value::from_bytes(value)];
		Ok(dynamic::tx("Token", "rotate_attribute", args))
	}

	/// JSON helper: encode to bytes.
	pub fn rotate_attribute_from_json(
		_metadata: &Metadata,
		token: Ss58Identifier,
		key: &[u8],
		expected: origin_primitives::element::ElementType,
		value: &serde_json::Value,
	) -> Result<DynamicPayload, OriginSdkError> {
		let elem = element_value_from_json(expected, value)?;
		let args = vec![Value::from_bytes(token.as_ref()), Value::from_bytes(key), elem];
		Ok(dynamic::tx("Token", "rotate_attribute", args))
	}
}
