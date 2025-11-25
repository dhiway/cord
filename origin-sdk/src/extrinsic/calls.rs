//! Argument transformers and dynamic call builders for Origin pallets.
//!
//! These helpers keep the public tx surface strongly typed while the SDK stays
//! fully dynamic (no codegen). They shape raw user input into pallet-ready
//! SCALE arguments and return `DynamicPayload` ready for submission.

use crate::types::error::OriginSdkError;
use codec::Encode;
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
	use crate::{types::EntityInfoInput, util::codec::element_value_from_json};
	use scale_value::{Composite, Value};
	use serde_json::Value as Json;

	/// Build `Entity::remove_attribute` dynamic payload.
	pub fn remove_attribute_call(
		_metadata: &Metadata,
		key: &[u8],
	) -> Result<DynamicPayload, OriginSdkError> {
		ensure_non_empty("key", key)?;
		let args = vec![Value::from_bytes(key)];
		Ok(dynamic::tx("Entity", "remove_attribute", args))
	}

	/// Build `Entity::set_linked_account`.
	pub fn set_linked_account_call(
		_metadata: &Metadata,
		account: subxt::utils::AccountId32,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![Value::from_bytes(account.0)];
		Ok(dynamic::tx("Entity", "set_linked_account", args))
	}

	/// Build `Entity::revoke_linked_account` (self).
	pub fn revoke_linked_account_call(
		_metadata: &Metadata,
		account: subxt::utils::AccountId32,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![Value::from_bytes(account.0)];
		Ok(dynamic::tx("Entity", "revoke_linked_account", args))
	}

	/// Build `Entity::revoke_linked_account_for` (force origin).
	pub fn revoke_linked_account_for_call(
		_metadata: &Metadata,
		token: Ss58Identifier,
		account: subxt::utils::AccountId32,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![Value::from_bytes(token.as_ref()), Value::from_bytes(account.0)];
		Ok(dynamic::tx("Entity", "revoke_linked_account_for", args))
	}

	/// Build `Entity::rotate_controller`.
	pub fn rotate_controller_call(
		_metadata: &Metadata,
		controller: subxt::utils::AccountId32,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![Value::from_bytes(controller.0)];
		Ok(dynamic::tx("Entity", "rotate_controller", args))
	}

	/// Build `Entity::rotate_controller_for` (force origin).
	pub fn rotate_controller_for_call(
		_metadata: &Metadata,
		token: Ss58Identifier,
		controller: subxt::utils::AccountId32,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![Value::from_bytes(token.as_ref()), Value::from_bytes(controller.0)];
		Ok(dynamic::tx("Entity", "rotate_controller_for", args))
	}

	/// Build `Entity::clear_everything`.
	pub fn clear_everything_call(_metadata: &Metadata) -> Result<DynamicPayload, OriginSdkError> {
		let args: Vec<Value> = Vec::new();
		Ok(dynamic::tx("Entity", "clear_everything", args))
	}

	/// Build `Entity::clear_everything_for` (force origin).
	pub fn clear_everything_for_call(
		_metadata: &Metadata,
		token: Ss58Identifier,
	) -> Result<DynamicPayload, OriginSdkError> {
		Ok(dynamic::tx("Entity", "clear_everything_for", vec![Value::from_bytes(token.as_ref())]))
	}

	/// Build `Entity::set_entity_nym`.
	pub fn set_entity_nym_call(
		_metadata: &Metadata,
		prefix: &[u8],
	) -> Result<DynamicPayload, OriginSdkError> {
		ensure_non_empty("prefix", prefix)?;
		Ok(dynamic::tx("Entity", "set_entity_nym", vec![Value::from_bytes(prefix)]))
	}

	/// Build `Entity::remove_entity_nym`.
	pub fn remove_entity_nym_call(
		_metadata: &Metadata,
		token: Ss58Identifier,
	) -> Result<DynamicPayload, OriginSdkError> {
		Ok(dynamic::tx("Entity", "remove_entity_nym", vec![Value::from_bytes(token.as_ref())]))
	}

	/// Build `Entity::rotate_attribute` dynamic payload.
	pub fn rotate_attribute_call(
		_metadata: &Metadata,
		key: &str,
		raw_value: &str,
	) -> Result<DynamicPayload, OriginSdkError> {
		ensure_non_empty("key", key.as_bytes())?;
		ensure_non_empty("value", raw_value.as_bytes())?;
		let args = vec![Value::from_bytes(key.as_bytes()), Value::from_bytes(raw_value.as_bytes())];
		Ok(dynamic::tx("Entity", "rotate_attribute", args))
	}

	/// Build `Entity::rotate_attribute` from typed ElementInput.
	pub fn rotate_attribute_from_element(
		_metadata: &Metadata,
		key: &[u8],
		val: &origin_primitives::element::Elum<crate::types::entity_input::MaxRawDataLength>,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![Value::from_bytes(key), element_to_value(val)];
		Ok(dynamic::tx("Entity", "rotate_attribute", args))
	}

	/// Build `Entity::rotate_attributes` from typed pairs.
	pub fn rotate_attributes_from_input(
		_metadata: &Metadata,
		ops: &[(
			Vec<u8>,
			origin_primitives::element::Elum<crate::types::entity_input::MaxRawDataLength>,
		)],
	) -> Result<DynamicPayload, OriginSdkError> {
		let items: Vec<Value> = ops
			.iter()
			.map(|(k, v)| Value::unnamed_composite(vec![Value::from_bytes(k), element_to_value(v)]))
			.collect();
		let args = vec![Value::from(items)];
		Ok(dynamic::tx("Entity", "rotate_attributes", args))
	}

	/// Build `Entity::add_attributes` from typed pairs.
	pub fn add_attributes_from_input(
		_metadata: &Metadata,
		ops: &[(
			Vec<u8>,
			origin_primitives::element::Elum<crate::types::entity_input::MaxRawDataLength>,
		)],
	) -> Result<DynamicPayload, OriginSdkError> {
		let encoded = ops.encode();
		let args = vec![Value::from_bytes(&encoded)];
		Ok(dynamic::tx("Entity", "add_attributes", args))
	}

	/// JSON helper: encode value as bytes (placeholder until Element builder is added).
	pub fn rotate_attribute_from_json(
		_metadata: &Metadata,
		key: &str,
		expected: origin_primitives::element::ElementType,
		value: &Json,
	) -> Result<DynamicPayload, OriginSdkError> {
		let elem = element_value_from_json(expected, value)?;
		let args = vec![Value::from_bytes(key.as_bytes()), elem];
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
				let args = vec![Value::from_bytes(token.as_ref()), Value::from_bytes(k), elem];
				calls.push(dynamic::tx("Entity", "rotate_attribute", args));
			}
		}
		Ok(calls)
	}

	/// Build `Entity::set_info` using the SDK-mirrored struct instead of raw Value.
	pub fn set_info_from_struct(
		_metadata: &Metadata,
		info: &EntityInfoInput,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![entity_info_value(info)];
		Ok(dynamic::tx("Entity", "set_info", args))
	}

	pub fn element_to_value(
		elem: &origin_primitives::element::Elum<crate::types::entity_input::MaxRawDataLength>,
	) -> Value {
		use origin_primitives::element::Elum::*;
		match elem {
			None => Value::variant("None", Composite::unnamed(vec![])),
			Raw(bv) => {
				Value::variant("Raw", Composite::unnamed(vec![Value::from_bytes(bv.to_vec())]))
			},
			Bool(b) => Value::variant("Bool", Composite::unnamed(vec![Value::from_bytes([*b])])),
			U64(bytes) => {
				Value::variant("U64", Composite::unnamed(vec![Value::from_bytes(bytes.to_vec())]))
			},
			U128(bytes) => {
				Value::variant("U128", Composite::unnamed(vec![Value::from_bytes(bytes.to_vec())]))
			},
			Hash(bytes) => {
				Value::variant("Hash", Composite::unnamed(vec![Value::from_bytes(bytes.to_vec())]))
			},
			Token(id) => {
				Value::variant("Token", Composite::unnamed(vec![Value::from_bytes(id.as_ref())]))
			},
			CID(bv) => {
				Value::variant("CID", Composite::unnamed(vec![Value::from_bytes(bv.to_vec())]))
			},
		}
	}

	fn attributes_to_value(attrs: &Option<crate::types::entity_input::AttributesInput>) -> Value {
		match attrs {
			None => Value::variant("None", Composite::unnamed(vec![])),
			Some(list) => {
				let pairs: Vec<Value> = list
					.iter()
					.map(|(k, v)| {
						Value::unnamed_composite(vec![
							Value::from_bytes(k.to_vec()),
							element_to_value(v),
						])
					})
					.collect();
				Value::variant("Some", Composite::unnamed(vec![Value::from(pairs)]))
			},
		}
	}

	/// Build dynamic Value representing EntityInfoInput (struct order: display, web, email,
	/// attributes).
	fn entity_info_value(info: &EntityInfoInput) -> Value {
		Value::unnamed_composite(vec![
			element_to_value(&info.display),
			element_to_value(&info.web),
			element_to_value(&info.email),
			attributes_to_value(&info.attributes),
		])
	}
}

pub mod registry {
	use super::*;
	use crate::types::registry_input::{
		DelegatePermissionsInput, RegistryCreateInput, RegistryInfoInput,
		RemoveDelegatePermissionsInput,
	};
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
		let schema_raw = serde_json::to_vec(schema)
			.map_err(|e| OriginSdkError::Schema(format!("schema: {e}")))?;
		let config_raw = serde_json::to_vec(config)
			.map_err(|e| OriginSdkError::Schema(format!("config: {e}")))?;
		create_call(metadata, registry_id, &schema_raw, &config_raw)
	}

	/// Build `Register::create_registry` from the strongly-typed SDK mirror.
	pub fn create_from_input(
		_metadata: &Metadata,
		registry_id: &[u8],
		input: &RegistryCreateInput,
	) -> Result<DynamicPayload, OriginSdkError> {
		ensure_non_empty("registry_id", registry_id)?;
		let args = vec![
			Value::from_bytes(registry_id),
			Value::from_bytes(&input.info.encode()),
			Value::from_bytes(&input.kind.encode()),
			Value::from_bytes(&input.attributes.encode()),
			Value::from_bytes(&input.token_spec.encode()),
			Value::from_bytes(&input.lookup_specs.encode()),
		];
		Ok(dynamic::tx("Register", "create_registry", args))
	}

	/// Build `Register::update_registry_info` from typed Element input.
	pub fn update_info_from_input(
		_metadata: &Metadata,
		registry: &[u8],
		info: &RegistryInfoInput,
	) -> Result<DynamicPayload, OriginSdkError> {
		ensure_non_empty("registry", registry)?;
		let args = vec![Value::from_bytes(registry), Value::from_bytes(&info.encode())];
		Ok(dynamic::tx("Register", "update_registry_info", args))
	}

	/// Build `Register::set_delegate_permissions` from typed input.
	pub fn set_delegate_permissions_from_input(
		_metadata: &Metadata,
		input: &DelegatePermissionsInput,
	) -> Result<DynamicPayload, OriginSdkError> {
		if input.roles.is_empty() {
			return Err(OriginSdkError::InvalidInput("roles cannot be empty".into()));
		}
		let roles_val = Value::from(
			input.roles.iter().map(|r| Value::u128(r.bits() as u128)).collect::<Vec<_>>(),
		);
		let args = vec![
			Value::from_bytes(input.registry.as_ref()),
			Value::from_bytes(input.delegate.0),
			roles_val,
		];
		Ok(dynamic::tx("Register", "set_delegate_permissions", args))
	}

	/// Build `Register::remove_delegate_permissions` from typed input.
	pub fn remove_delegate_permissions_from_input(
		_metadata: &Metadata,
		input: &RemoveDelegatePermissionsInput,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![
			Value::from_bytes(input.registry.as_ref()),
			Value::from_bytes(input.delegate.as_ref()),
		];
		Ok(dynamic::tx("Register", "remove_delegate_permissions", args))
	}
}

pub mod packet {
	use super::*;
	use crate::types::packet_input::{
		MaxAdditionalAttributes, MaxRawDataLength, PacketAttributesInput, PacketElementInput,
	};
	use frame_support::BoundedVec;
	use origin_primitives::{
		attribute::Attribute, element::ElementType, registry::RegistryAttributeView,
	};
	use serde_json::Value as Json;

	/// Validate packet body shape against a registry schema (placeholder) and build
	/// `create_packet`.
	pub fn issue_call(
		_metadata: &Metadata,
		registry_id: Ss58Identifier,
		registry_schema: &[RegistryAttributeView],
		packet_body: &Json,
	) -> Result<DynamicPayload, OriginSdkError> {
		let obj = packet_body
			.as_object()
			.ok_or_else(|| OriginSdkError::Schema("packet body must be a JSON object".into()))?;

		let entries = build_packet_attributes(registry_schema, obj)?;
		let encoded = entries.encode();
		let args = vec![Value::from_bytes(registry_id.as_ref()), Value::from_bytes(&encoded)];
		Ok(dynamic::tx("Register", "create_packet", args))
	}

	/// Build `Register::create_packet` from already-flattened attributes (key, Element bytes).
	pub fn issue_from_flat(
		_metadata: &Metadata,
		registry_id: Ss58Identifier,
		attributes: &[(Vec<u8>, Vec<u8>)],
	) -> Result<DynamicPayload, OriginSdkError> {
		let mut bounded =
			BoundedVec::<(Attribute, PacketElementInput), MaxAdditionalAttributes>::new();
		for (k, v_bytes) in attributes {
			let key: Attribute = Attribute::try_from(k.clone())
				.map_err(|_| OriginSdkError::InvalidInput("attribute key too long".into()))?;
			let elem: PacketElementInput = codec::Decode::decode(&mut &v_bytes[..])
				.map_err(|e| OriginSdkError::Decode(format!("element decode: {e}")))?;
			bounded
				.try_push((key, elem))
				.map_err(|_| OriginSdkError::InvalidInput("too many attributes".into()))?;
		}
		let args =
			vec![Value::from_bytes(registry_id.as_ref()), Value::from_bytes(&bounded.encode())];
		Ok(dynamic::tx("Register", "create_packet", args))
	}

	/// Build `Register::create_packet` from typed packet attributes.
	pub fn issue_from_input(
		_metadata: &Metadata,
		registry_id: Ss58Identifier,
		attributes: &PacketAttributesInput,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args =
			vec![Value::from_bytes(registry_id.as_ref()), Value::from_bytes(&attributes.encode())];
		Ok(dynamic::tx("Register", "create_packet", args))
	}

	/// Build `Register::update_packet` from typed attributes.
	pub fn update_from_input(
		_metadata: &Metadata,
		registry_id: Ss58Identifier,
		packet_id: Ss58Identifier,
		attributes: &PacketAttributesInput,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![
			Value::from_bytes(registry_id.as_ref()),
			Value::from_bytes(packet_id.as_ref()),
			Value::from_bytes(&attributes.encode()),
		];
		Ok(dynamic::tx("Register", "update_packet", args))
	}

	/// Build `Register::revoke_packet`.
	pub fn revoke_call(
		_metadata: &Metadata,
		registry_id: Ss58Identifier,
		packet_id: Ss58Identifier,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args =
			vec![Value::from_bytes(registry_id.as_ref()), Value::from_bytes(packet_id.as_ref())];
		Ok(dynamic::tx("Register", "revoke_packet", args))
	}

	/// Build `Register::restore_packet`.
	pub fn restore_call(
		_metadata: &Metadata,
		registry_id: Ss58Identifier,
		packet_id: Ss58Identifier,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args =
			vec![Value::from_bytes(registry_id.as_ref()), Value::from_bytes(packet_id.as_ref())];
		Ok(dynamic::tx("Register", "restore_packet", args))
	}

	/// Build `Register::delete_packet`.
	pub fn delete_call(
		_metadata: &Metadata,
		registry_id: Ss58Identifier,
		packet_id: Ss58Identifier,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args =
			vec![Value::from_bytes(registry_id.as_ref()), Value::from_bytes(packet_id.as_ref())];
		Ok(dynamic::tx("Register", "delete_packet", args))
	}

	/// Build `Register::set_packet_status`.
	pub fn set_status_call(
		_metadata: &Metadata,
		registry_id: Ss58Identifier,
		packet_id: Ss58Identifier,
		status: origin_primitives::packet::PacketStatus,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![
			Value::from_bytes(registry_id.as_ref()),
			Value::from_bytes(packet_id.as_ref()),
			Value::from_bytes(&status.encode()),
		];
		Ok(dynamic::tx("Register", "set_packet_status", args))
	}

	/// Minimal type validation for JSON values against ElementType.
	fn validate_element_type(
		key: &str,
		expected: origin_primitives::element::ElementType,
		value: &Json,
	) -> Result<(), OriginSdkError> {
		match expected {
			origin_primitives::element::ElementType::Raw => {
				if !value.is_string()
					&& !value.is_number()
					&& !value.is_boolean()
					&& !value.is_array()
				{
					return Err(OriginSdkError::Schema(format!(
						"{key}: expected raw/json, got {value}"
					)));
				}
			},
			origin_primitives::element::ElementType::Bool => {
				if !value.is_boolean() {
					return Err(OriginSdkError::Schema(format!(
						"{key}: expected bool, got {value}"
					)));
				}
			},
			origin_primitives::element::ElementType::U64 => {
				if !(value.is_u64() || value.is_i64()) {
					return Err(OriginSdkError::Schema(format!(
						"{key}: expected u64, got {value}"
					)));
				}
			},
			origin_primitives::element::ElementType::U128 => {
				if let Some(n) = value.as_u64() {
					let _ = n; // always fits in u128
				} else if let Some(s) = value.as_str() {
					s.parse::<u128>()
						.map_err(|e| OriginSdkError::Schema(format!("{key}: invalid u128: {e}")))?;
				} else {
					return Err(OriginSdkError::Schema(format!(
						"{key}: expected u128 (number/string), got {value}"
					)));
				}
			},
			origin_primitives::element::ElementType::Hash => {
				if let Some(s) = value.as_str() {
					if s.len() % 2 != 0 {
						return Err(OriginSdkError::Schema(format!(
							"{key}: hex length must be even"
						)));
					}
					let _ = hex::decode(s)
						.map_err(|e| OriginSdkError::Schema(format!("{key}: bad hex: {e}")))?;
				} else {
					return Err(OriginSdkError::Schema(format!(
						"{key}: expected hex string, got {value}"
					)));
				}
			},
			origin_primitives::element::ElementType::Token => {
				if let Some(s) = value.as_str() {
					origin_primitives::Ss58Identifier::try_from(s.to_string()).map_err(|e| {
						OriginSdkError::Schema(format!("{key}: invalid ss58: {e:?}"))
					})?;
				} else {
					return Err(OriginSdkError::Schema(format!(
						"{key}: expected ss58 string, got {value}"
					)));
				}
			},
			origin_primitives::element::ElementType::Cid => {
				if let Some(s) = value.as_str() {
					if s.len() < 8 || s.len() > 128 {
						return Err(OriginSdkError::Schema(format!(
							"{key}: CID length out of range"
						)));
					}
					if !s.is_ascii() || s.chars().any(char::is_whitespace) {
						return Err(OriginSdkError::Schema(format!(
							"{key}: CID must be ASCII without whitespace"
						)));
					}
				} else {
					return Err(OriginSdkError::Schema(format!(
						"{key}: expected CID string, got {value}"
					)));
				}
			},
			origin_primitives::element::ElementType::None => {},
		}
		Ok(())
	}

	fn build_packet_attributes(
		registry_schema: &[RegistryAttributeView],
		obj: &serde_json::Map<String, Json>,
	) -> Result<BoundedVec<(Attribute, PacketElementInput), MaxAdditionalAttributes>, OriginSdkError>
	{
		let mut out = BoundedVec::<(Attribute, PacketElementInput), MaxAdditionalAttributes>::new();

		for attr in registry_schema {
			let key_str = String::from_utf8_lossy(&attr.key).to_string();
			let key: Attribute = Attribute::try_from(attr.key.clone())
				.map_err(|_| OriginSdkError::InvalidInput("attribute key too long".into()))?;

			if let Some(value) = obj.get(&key_str) {
				validate_element_type(&key_str, attr.kind, value)?;
				let elem = element_from_json(attr.kind, value)?;
				out.try_push((key, elem))
					.map_err(|_| OriginSdkError::InvalidInput("too many attributes".into()))?;
			} else if !attr.optional {
				return Err(OriginSdkError::Schema(format!(
					"missing required attribute '{key_str}'"
				)));
			}
		}

		// Reject unknown keys.
		for key in obj.keys() {
			let key_bytes = key.as_bytes();
			let known = registry_schema.iter().any(|a| a.key.as_slice() == key_bytes);
			if !known {
				return Err(OriginSdkError::Schema(format!(
					"attribute '{key}' not defined in registry schema"
				)));
			}
		}

		Ok(out)
	}

	fn element_from_json(
		kind: ElementType,
		value: &Json,
	) -> Result<PacketElementInput, OriginSdkError> {
		use frame_support::BoundedVec;
		match kind {
			ElementType::None => Ok(PacketElementInput::None),
			ElementType::Raw => {
				let bytes = serde_json::to_vec(value)
					.map_err(|e| OriginSdkError::InvalidInput(format!("raw encode: {e}")))?;
				let bounded = BoundedVec::<u8, MaxRawDataLength>::try_from(bytes)
					.map_err(|_| OriginSdkError::InvalidInput("raw too large".into()))?;
				Ok(PacketElementInput::Raw(bounded))
			},
			ElementType::Bool => value
				.as_bool()
				.map(|b| PacketElementInput::Bool(b as u8))
				.ok_or_else(|| OriginSdkError::InvalidInput("expected bool".into())),
			ElementType::U64 => value
				.as_u64()
				.map(|n| PacketElementInput::U64(n.to_le_bytes()))
				.ok_or_else(|| OriginSdkError::InvalidInput("expected u64".into())),
			ElementType::U128 => {
				let n = if let Some(u) = value.as_u64() {
					u as u128
				} else if let Some(s) = value.as_str() {
					s.parse::<u128>()
						.map_err(|e| OriginSdkError::InvalidInput(format!("u128 parse: {e}")))?
				} else {
					return Err(OriginSdkError::InvalidInput("expected u128".into()));
				};
				Ok(PacketElementInput::U128(n.to_le_bytes()))
			},
			ElementType::Hash => {
				let s = value
					.as_str()
					.ok_or_else(|| OriginSdkError::InvalidInput("expected hash hex".into()))?;
				let bytes = hex::decode(s)
					.map_err(|e| OriginSdkError::InvalidInput(format!("hash hex: {e}")))?;
				if bytes.len() != 32 {
					return Err(OriginSdkError::InvalidInput("hash must be 32 bytes".into()));
				}
				let mut arr = [0u8; 32];
				arr.copy_from_slice(&bytes);
				Ok(PacketElementInput::Hash(arr))
			},
			ElementType::Token => {
				let s = value
					.as_str()
					.ok_or_else(|| OriginSdkError::InvalidInput("expected ss58 string".into()))?;
				let id = origin_primitives::Ss58Identifier::try_from(s.to_string())
					.map_err(|e| OriginSdkError::InvalidInput(format!("ss58: {e:?}")))?;
				Ok(PacketElementInput::Token(id))
			},
			ElementType::Cid => {
				let s = value
					.as_str()
					.ok_or_else(|| OriginSdkError::InvalidInput("expected cid string".into()))?;
				let bounded = BoundedVec::<u8, MaxRawDataLength>::try_from(s.as_bytes().to_vec())
					.map_err(|_| OriginSdkError::InvalidInput("cid too large".into()))?;
				Ok(PacketElementInput::CID(bounded))
			},
		}
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
		let args = vec![
			Value::from_bytes(token.as_ref()),
			Value::from_bytes(key),
			Value::from_bytes(value),
		];
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

	/// Build `Token::rotate_attribute` from typed Element view.
	pub fn rotate_attribute_from_element(
		_metadata: &Metadata,
		token: Ss58Identifier,
		key: &[u8],
		elem: &origin_primitives::element::Elum<crate::types::entity_input::MaxRawDataLength>,
	) -> Result<DynamicPayload, OriginSdkError> {
		let args = vec![
			Value::from_bytes(token.as_ref()),
			Value::from_bytes(key),
			super::entity::element_to_value(elem),
		];
		Ok(dynamic::tx("Token", "rotate_attribute", args))
	}
}
