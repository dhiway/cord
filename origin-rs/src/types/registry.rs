use crate::{
	error::{Error, Result},
	runtime,
	runtime_helpers::{attribute_optional, bounded_bytes_vec, bounded_iter, element_type_to_sdk},
	types::{attribute_pair_value, base64_to_bytes, element_json_to_dynamic, ElementJson},
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use origin_primitives::{
	packet::ElementType,
	registry::{RegistryAttributeView, RegistryInfoView},
};
use scale_value::{Composite, Value};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};

const ATTRIBUTE_KEY_MAX: usize = 64;
const ATTRIBUTE_FLAG_OPTIONAL: u8 = 1 << 0;

type RuntimeRegistryInfo = runtime::runtime_types::pallet_register::register::RegistryInfo;
type RuntimeAttributeSpec = runtime::runtime_types::pallet_register::register::AttributeSpec;

/// High-level registry definition parsed from developer JSON.
pub struct RegistryBlueprint {
	pub info: ElementJson,
	pub kind: RegistryKind,
	pub attributes: Vec<SchemaAttribute>,
	pub token_spec: LookupSpecDef,
	pub lookup_specs: Vec<LookupSpecDef>,
}

impl RegistryBlueprint {
	/// Parse a developer-friendly JSON object describing a registry.
	pub fn from_json(value: JsonValue) -> Result<Self> {
		let spec: RegistryBlueprintInput = serde_json::from_value(value)
			.map_err(|e| Error::Params(format!("invalid registry json: {e}")))?;
		Self::from_input(spec)
	}

	fn from_input(input: RegistryBlueprintInput) -> Result<Self> {
		let info = input.info.into_element()?;
		let kind = input
			.kind
			.map(|k| RegistryKind::from_label(&k))
			.transpose()?
			.unwrap_or(RegistryKind::Raw);
		if input.attributes.is_empty() {
			return Err(Error::Params("registry must declare at least one attribute".into()));
		}

		let mut attributes = Vec::new();
		let mut attr_index: BTreeMap<Vec<u8>, SchemaAttribute> = BTreeMap::new();
		for entry in input.attributes.into_iter() {
			let key_bytes = parse_attribute_key(&entry.key)?;
			if attr_index.contains_key(&key_bytes) {
				return Err(Error::Params(format!(
					"duplicate attribute key: {}",
					display_key(&key_bytes)
				)));
			}
			let kind = ElementTypeLabel::from_label(&entry.ty)?;
			let attribute = SchemaAttribute::new(key_bytes.clone(), kind.into(), entry.optional);
			attr_index.insert(key_bytes.clone(), attribute.clone());
			attributes.push(attribute);
		}

		let token_spec = LookupSpecDef::from_input(input.token_spec, &attr_index, false)?;
		if input.lookup_specs.is_empty() {
			return Err(Error::Params("registry must declare at least one lookup spec".into()));
		}
		let mut seen_fingerprints = BTreeSet::new();
		let mut lookup_specs = Vec::new();
		for spec in input.lookup_specs.into_iter() {
			let lookup = LookupSpecDef::from_input(spec, &attr_index, true)?;
			let fingerprint = lookup.fingerprint();
			if !seen_fingerprints.insert(fingerprint) {
				return Err(Error::Params("duplicate lookup spec".into()));
			}
			lookup_specs.push(lookup);
		}

		Ok(Self { info, kind, attributes, token_spec, lookup_specs })
	}

	/// Render the blueprint into dynamic SCALE arguments for `create_registry`.
	pub fn to_call_args(&self) -> Result<Value> {
		let info = element_json_to_dynamic(&self.info)?;
		let kind = self.kind.as_value();
		let attributes = Value::unnamed_composite(
			self.attributes.iter().map(SchemaAttribute::as_value).collect::<Vec<_>>(),
		);
		let token_spec = self.token_spec.as_value();
		let lookup_specs = Value::unnamed_composite(
			self.lookup_specs.iter().map(LookupSpecDef::as_value).collect::<Vec<_>>(),
		);
		Ok(Value::named_composite([
			("info", info),
			("kind", kind),
			("attributes", attributes),
			("token_spec", token_spec),
			("lookup_specs", lookup_specs),
		]))
	}
}

/// Schema view derived from the runtime-provided [`RuntimeRegistryInfo`].
#[derive(Clone)]
pub struct RegistrySchema {
	attributes: BTreeMap<Vec<u8>, SchemaAttribute>,
}

impl RegistrySchema {
	pub fn from_runtime(info: &RuntimeRegistryInfo) -> Self {
		let mut attributes = BTreeMap::new();
		for spec in bounded_iter(&info.attributes) {
			let key = bounded_bytes_vec(&spec.key);
			attributes.insert(key.clone(), SchemaAttribute::from_runtime(spec));
		}
		Self { attributes }
	}

	pub fn from_view(view: &RegistryInfoView) -> Self {
		let mut attributes = BTreeMap::new();
		for spec in &view.attributes {
			attributes.insert(spec.key.clone(), SchemaAttribute::from_view(spec));
		}
		Self { attributes }
	}

	pub fn attribute(&self, key: &[u8]) -> Option<&SchemaAttribute> {
		self.attributes.get(key)
	}

	pub fn build_payload(&self, payload: &JsonValue, mode: PayloadMode) -> Result<PacketPayload> {
		let map = payload
			.as_object()
			.ok_or_else(|| Error::Params("packet payload must be a JSON object".into()))?;
		let mut collected: Vec<(Vec<u8>, ElementJson)> = Vec::new();
		let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
		for (key, value) in map {
			walk_layered(key, value, &mut |path, leaf| {
				let key_bytes = parse_attribute_key(&path)?;
				let schema = self
					.attribute(&key_bytes)
					.ok_or_else(|| Error::Params(format!("unknown attribute key '{path}'")))?;
				if !seen.insert(key_bytes.clone()) {
					return Err(Error::Params(format!("duplicate attribute key '{path}'")));
				}
				let element = convert_element(schema, leaf)?;
				collected.push((key_bytes, element));
				Ok(())
			})?;
		}
		if collected.is_empty() {
			return Err(Error::Params("attributes payload must include at least one field".into()));
		}
		if mode.require_all() {
			let mut missing = Vec::new();
			for schema in self.attributes.values() {
				if !schema.optional && !seen.contains(&schema.key) {
					missing.push(schema.label.clone());
				}
			}
			if !missing.is_empty() {
				return Err(Error::Params(format!(
					"missing required attributes: {}",
					missing.join(", ")
				)));
			}
		}
		collected.sort_by(|a, b| a.0.cmp(&b.0));
		Ok(PacketPayload { entries: collected })
	}
}

/// Mode used when validating packet payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadMode {
	Full,
	Partial,
}

impl PayloadMode {
	fn require_all(self) -> bool {
		matches!(self, PayloadMode::Full)
	}
}

/// Packet attribute payload ready for SCALE encoding.
#[derive(Clone, Debug)]
pub struct PacketPayload {
	entries: Vec<(Vec<u8>, ElementJson)>,
}

impl PacketPayload {
	pub fn as_value(&self) -> Result<Value> {
		let mut pairs = Vec::with_capacity(self.entries.len());
		for (key, element) in &self.entries {
			pairs.push(attribute_pair_value(key, element)?);
		}
		Ok(Value::unnamed_composite(pairs))
	}

	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}
}

#[derive(Clone)]
pub struct SchemaAttribute {
	pub key: Vec<u8>,
	pub label: String,
	pub kind: ElementType,
	pub optional: bool,
}

impl SchemaAttribute {
	fn new(key: Vec<u8>, kind: ElementType, optional: bool) -> Self {
		let label = display_key(&key);
		Self { key, label, kind, optional }
	}

	fn from_runtime(spec: &RuntimeAttributeSpec) -> Self {
		let optional = attribute_optional(&spec.flags);
		let key = bounded_bytes_vec(&spec.key);
		let kind = element_type_to_sdk(&spec.kind);
		Self::new(key, kind, optional)
	}

	fn from_view(view: &RegistryAttributeView) -> Self {
		Self::new(view.key.clone(), view.kind, view.optional)
	}

	fn as_value(&self) -> Value {
		let key_value = crate::types::bytes_value(&self.key);
		let kind_value = element_type_variant(self.kind);
		let flags = if self.optional { ATTRIBUTE_FLAG_OPTIONAL } else { 0u8 };
		Value::named_composite([
			("key", key_value),
			("kind", kind_value),
			("flags", Value::u128(flags as u128)),
		])
	}
}

#[derive(Clone)]
pub enum LookupSpecDef {
	Single(Vec<u8>),
	Combo(Vec<Vec<u8>>),
}

impl LookupSpecDef {
	fn from_input(
		mut input: LookupSpecInput,
		attributes: &BTreeMap<Vec<u8>, SchemaAttribute>,
		disallow_optional: bool,
	) -> Result<Self> {
		let keys = input.collect_keys()?;
		if keys.is_empty() {
			return Err(Error::Params("lookup spec must reference at least one attribute".into()));
		}
		let mut collected = Vec::new();
		for key_label in keys {
			let key_bytes = parse_attribute_key(&key_label)?;
			let attr = attributes
				.get(&key_bytes)
				.ok_or_else(|| Error::Params(format!("unknown lookup key '{key_label}'")))?;
			if disallow_optional && attr.optional {
				return Err(Error::Params(format!(
					"lookup key '{}' cannot reference optional attribute {}",
					key_label, attr.label
				)));
			}
			collected.push(key_bytes);
		}
		Ok(match input.kind.as_str() {
			"single" => Self::Single(collected.remove(0)),
			_ => Self::Combo(collected),
		})
	}

	fn as_value(&self) -> Value {
		match self {
			Self::Single(key) => Value::unnamed_variant("Single", [crate::types::bytes_value(key)]),
			Self::Combo(keys) => {
				let list = Value::unnamed_composite(
					keys.iter().map(|key| crate::types::bytes_value(key)).collect::<Vec<_>>(),
				);
				Value::unnamed_variant("Combo", [list])
			},
		}
	}

	fn fingerprint(&self) -> Vec<u8> {
		let mut material: Vec<Vec<u8>> = match self {
			Self::Single(key) => vec![key.clone()],
			Self::Combo(keys) => keys.clone(),
		};
		material.sort();
		let mut digest = Vec::new();
		for key in material {
			let len = key.len() as u32;
			digest.extend_from_slice(&len.to_be_bytes());
			digest.extend_from_slice(&key);
		}
		digest
	}
}

#[derive(Clone, Copy, Debug)]
pub enum RegistryKind {
	Raw,
	Token,
	Hash,
}

impl RegistryKind {
	fn from_label(label: &str) -> Result<Self> {
		match label.to_ascii_lowercase().as_str() {
			"raw" => Ok(Self::Raw),
			"token" => Ok(Self::Token),
			"hash" => Ok(Self::Hash),
			other => Err(Error::Params(format!("unknown registry kind '{other}'"))),
		}
	}

	fn as_value(&self) -> Value {
		match self {
			RegistryKind::Raw => unit_variant("Raw"),
			RegistryKind::Token => unit_variant("Token"),
			RegistryKind::Hash => unit_variant("Hash"),
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryBlueprintInput {
	#[serde(default)]
	info: InfoField,
	#[serde(default)]
	kind: Option<String>,
	#[serde(alias = "attribute_schema", default)]
	attributes: Vec<AttributeSpecInput>,
	#[serde(rename = "token_spec", alias = "tokenSpec")]
	token_spec: LookupSpecInput,
	#[serde(rename = "lookup_specs", alias = "lookupSpecs", default)]
	lookup_specs: Vec<LookupSpecInput>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum InfoField {
	Element(ElementJson),
	Text(String),
}

impl Default for InfoField {
	fn default() -> Self {
		Self::Element(ElementJson::None)
	}
}

impl InfoField {
	fn into_element(self) -> Result<ElementJson> {
		Ok(match self {
			Self::Element(e) => e,
			Self::Text(text) => {
				let encoded = BASE64.encode(text.as_bytes());
				ElementJson::RawBase64(encoded)
			},
		})
	}
}

/// Convert arbitrary JSON into an [`ElementJson`] suitable for registry info blobs.
pub fn info_element_from_value(value: JsonValue) -> Result<ElementJson> {
	match value {
		JsonValue::String(text) => Ok(ElementJson::RawBase64(BASE64.encode(text.as_bytes()))),
		other => serde_json::from_value(other)
			.map_err(|e| Error::Params(format!("invalid element json: {e}"))),
	}
}

#[derive(Deserialize)]
struct AttributeSpecInput {
	key: String,
	#[serde(rename = "type")]
	ty: String,
	#[serde(default)]
	optional: bool,
}

#[derive(Deserialize)]
struct LookupSpecInput {
	#[serde(rename = "type")]
	kind: String,
	#[serde(default)]
	keys: Vec<String>,
	#[serde(default)]
	key: Option<String>,
}

impl LookupSpecInput {
	fn collect_keys(&mut self) -> Result<Vec<String>> {
		self.kind = self.kind.to_ascii_lowercase();
		match self.kind.as_str() {
			"single" => {
				let key = self
					.key
					.take()
					.ok_or_else(|| Error::Params("single lookup spec requires 'key'".into()))?;
				Ok(vec![key])
			},
			"combo" => {
				if self.keys.is_empty() {
					return Err(Error::Params("combo lookup spec requires 'keys'".into()));
				}
				Ok(self.keys.clone())
			},
			other => Err(Error::Params(format!("unknown lookup spec type '{other}'"))),
		}
	}
}

#[derive(Clone, Copy)]
struct ElementTypeLabel(ElementType);

impl ElementTypeLabel {
	fn from_label(label: &str) -> Result<Self> {
		let kind = match label.to_ascii_lowercase().as_str() {
			"none" => ElementType::None,
			"raw" => ElementType::Raw,
			"bool" => ElementType::Bool,
			"u64" => ElementType::U64,
			"u128" => ElementType::U128,
			"hash" => ElementType::Hash,
			"token" => ElementType::Token,
			"cid" => ElementType::Cid,
			other => {
				return Err(Error::Params(format!("unknown attribute type '{other}'")));
			},
		};
		Ok(Self(kind))
	}
}

impl From<ElementTypeLabel> for ElementType {
	fn from(value: ElementTypeLabel) -> Self {
		value.0
	}
}

fn element_type_variant(kind: ElementType) -> Value {
	match kind {
		ElementType::None => unit_variant("None"),
		ElementType::Raw => unit_variant("Raw"),
		ElementType::Bool => unit_variant("Bool"),
		ElementType::U64 => unit_variant("U64"),
		ElementType::U128 => unit_variant("U128"),
		ElementType::Hash => unit_variant("Hash"),
		ElementType::Token => unit_variant("Token"),
		ElementType::Cid => unit_variant("Cid"),
	}
}

fn unit_variant(name: &str) -> Value {
	Value::variant(name, Composite::unnamed(Vec::new()))
}

fn walk_layered<F>(prefix: &str, value: &JsonValue, visit: &mut F) -> Result<()>
where
	F: FnMut(String, &JsonValue) -> Result<()>,
{
	match value {
		JsonValue::Object(children) if !children.is_empty() => {
			for (child_key, child_value) in children {
				let next = if prefix.is_empty() {
					child_key.to_string()
				} else {
					format!("{prefix}.{child_key}")
				};
				walk_layered(&next, child_value, visit)?;
			}
			Ok(())
		},
		JsonValue::Array(_) => {
			Err(Error::Params(format!("attribute '{prefix}' cannot be an array")))
		},
		_ => {
			if prefix.is_empty() {
				return Err(Error::Params("attributes payload must be a JSON object".into()));
			}
			visit(prefix.to_string(), value)
		},
	}
}

fn convert_element(schema: &SchemaAttribute, value: &JsonValue) -> Result<ElementJson> {
	if value.is_null() {
		if schema.optional {
			return Ok(ElementJson::None);
		}
		return Err(Error::Params(format!(
			"attribute '{}' is required and cannot be null",
			schema.label
		)));
	}
	match schema.kind {
		ElementType::Raw => convert_raw(value, &schema.label),
		ElementType::Bool => convert_bool(value, &schema.label),
		ElementType::U64 => convert_u64(value, &schema.label),
		ElementType::U128 => convert_u128(value, &schema.label),
		ElementType::Hash => convert_hash(value, &schema.label),
		ElementType::Token => convert_token(value, &schema.label),
		ElementType::Cid => convert_cid(value, &schema.label),
		ElementType::None => Ok(ElementJson::None),
	}
}

fn convert_raw(value: &JsonValue, label: &str) -> Result<ElementJson> {
	match value {
		JsonValue::String(s) => Ok(ElementJson::RawBase64(BASE64.encode(s.as_bytes()))),
		JsonValue::Object(map) => {
			if let Some(hex) = map.get("hex").and_then(|v| v.as_str()) {
				let bytes = crate::types::hex_to_bytes(hex)?;
				return Ok(ElementJson::RawBase64(BASE64.encode(bytes)));
			}
			if let Some(base64) = map.get("base64").and_then(|v| v.as_str()) {
				base64_to_bytes(base64)?;
				return Ok(ElementJson::RawBase64(base64.to_string()));
			}
			Err(Error::Params(format!(
				"raw attribute '{}' expects string or object with 'hex'/'base64'",
				label
			)))
		},
		other => {
			Err(Error::Params(format!("raw attribute '{}' cannot use value {:?}", label, other)))
		},
	}
}

fn convert_bool(value: &JsonValue, label: &str) -> Result<ElementJson> {
	match value {
		JsonValue::Bool(flag) => Ok(ElementJson::Bool(*flag)),
		JsonValue::String(s) => match s.to_ascii_lowercase().as_str() {
			"true" => Ok(ElementJson::Bool(true)),
			"false" => Ok(ElementJson::Bool(false)),
			_ => {
				Err(Error::Params(format!("bool attribute '{}' expects 'true' or 'false'", label)))
			},
		},
		_ => Err(Error::Params(format!("bool attribute '{}' expects boolean", label))),
	}
}

fn convert_u64(value: &JsonValue, label: &str) -> Result<ElementJson> {
	if let Some(num) = value.as_u64() {
		return Ok(ElementJson::U64(num));
	}
	if let Some(text) = value.as_str() {
		let parsed = text
			.parse::<u64>()
			.map_err(|e| Error::Params(format!("attribute '{}' expects u64: {e}", label)))?;
		return Ok(ElementJson::U64(parsed));
	}
	Err(Error::Params(format!("attribute '{}' expects u64", label)))
}

fn convert_u128(value: &JsonValue, label: &str) -> Result<ElementJson> {
	if let Some(num) = value.as_u64() {
		return Ok(ElementJson::U128(num as u128));
	}
	if let Some(text) = value.as_str() {
		let parsed = text
			.parse::<u128>()
			.map_err(|e| Error::Params(format!("attribute '{}' expects u128: {e}", label)))?;
		return Ok(ElementJson::U128(parsed));
	}
	Err(Error::Params(format!("attribute '{}' expects u128", label)))
}

fn convert_hash(value: &JsonValue, label: &str) -> Result<ElementJson> {
	if let Some(text) = value.as_str() {
		let hex = normalize_hex(text)?;
		return Ok(ElementJson::HashHex(hex));
	}
	if let Some(obj) = value.as_object() {
		if let Some(hex) = obj.get("hex").and_then(|v| v.as_str()) {
			let hex = normalize_hex(hex)?;
			return Ok(ElementJson::HashHex(hex));
		}
	}
	Err(Error::Params(format!("hash attribute '{}' expects hex string", label)))
}

fn convert_token(value: &JsonValue, label: &str) -> Result<ElementJson> {
	if let Some(text) = value.as_str() {
		if text.is_empty() {
			return Err(Error::Params(format!("token attribute '{}' cannot be empty", label)));
		}
		return Ok(ElementJson::TokenSs58(text.to_string()));
	}
	Err(Error::Params(format!("token attribute '{}' expects SS58 string", label)))
}

fn convert_cid(value: &JsonValue, label: &str) -> Result<ElementJson> {
	if let Some(text) = value.as_str() {
		return Ok(ElementJson::CidBase58(text.to_string()));
	}
	Err(Error::Params(format!("cid attribute '{}' expects base58 string", label)))
}

fn parse_attribute_key(raw: &str) -> Result<Vec<u8>> {
	if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
		let bytes = crate::types::hex_to_bytes(hex)?;
		if bytes.is_empty() {
			return Err(Error::Params("attribute key hex cannot be empty".into()));
		}
		if bytes.len() > ATTRIBUTE_KEY_MAX {
			return Err(Error::Params(format!("attribute key exceeds {ATTRIBUTE_KEY_MAX} bytes")));
		}
		return Ok(bytes);
	}
	if raw.is_empty() {
		return Err(Error::Params("attribute key cannot be empty".into()));
	}
	let bytes = raw.as_bytes();
	if bytes.len() > ATTRIBUTE_KEY_MAX {
		return Err(Error::Params(format!("attribute key exceeds {ATTRIBUTE_KEY_MAX} bytes")));
	}
	Ok(bytes.to_vec())
}

fn display_key(bytes: &[u8]) -> String {
	match core::str::from_utf8(bytes) {
		Ok(text) if !text.chars().any(|ch| ch.is_control()) => text.to_string(),
		_ => format!("0x{}", hex::encode(bytes)),
	}
}

fn normalize_hex(raw: &str) -> Result<String> {
	let trimmed = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")).unwrap_or(raw);
	let bytes = crate::types::hex_to_bytes(trimmed)?;
	if bytes.len() != 32 {
		return Err(Error::Params(format!("hash must be 32 bytes (found {} bytes)", bytes.len())));
	}
	Ok(format!("0x{}", hex::encode(bytes)))
}

#[cfg(test)]
mod tests {
	use super::*;
	use origin_primitives::{
		identifier::Ss58Identifier,
		registry::{LookupSpecView, RegistryKind as RuntimeRegistryKind, RegistryStatus},
		view::ElementView,
	};
	use serde_json::json;

	fn dummy_identifier() -> Ss58Identifier {
		let digest = [0u8; 32];
		Ss58Identifier::to_encoded(digest, 100, 5, 0).expect("identifier")
	}

	fn sample_view() -> RegistryInfoView {
		RegistryInfoView {
			info: ElementView::Raw(Vec::new()),
			maintainer: dummy_identifier(),
			attributes: vec![
				RegistryAttributeView {
					key: b"record_id".to_vec(),
					kind: ElementType::Raw,
					optional: false,
				},
				RegistryAttributeView {
					key: b"controller".to_vec(),
					kind: ElementType::Token,
					optional: false,
				},
				RegistryAttributeView {
					key: b"address.street".to_vec(),
					kind: ElementType::Raw,
					optional: false,
				},
				RegistryAttributeView {
					key: b"address.house.building".to_vec(),
					kind: ElementType::Raw,
					optional: true,
				},
			],
			token_spec: LookupSpecView::Combo(vec![b"record_id".to_vec(), b"controller".to_vec()]),
			lookup_specs: vec![LookupSpecView::Single(b"record_id".to_vec())],
			kind: RuntimeRegistryKind::Raw,
			status: RegistryStatus::Active,
		}
	}

	#[test]
	fn blueprint_parses_sample_json() {
		let json = json!({
			"info": "demo",
			"kind": "raw",
			"attribute_schema": [
				{"key": "record_id", "type": "raw"},
				{"key": "controller", "type": "token"}
			],
			"token_spec": {"type": "combo", "keys": ["record_id", "controller"]},
			"lookup_specs": [{"type": "single", "key": "record_id"}]
		});
		let blueprint = RegistryBlueprint::from_json(json).expect("blueprint");
		assert_eq!(blueprint.attributes.len(), 2);
		assert_eq!(blueprint.lookup_specs.len(), 1);
	}

	#[test]
	fn schema_validates_layered_payload() {
		let schema = RegistrySchema::from_view(&sample_view());
		let json = json!({
			"record_id": "rec-1",
			"controller": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
			"address": {"street": "main", "house": {"building": "north"}}
		});
		let payload = schema.build_payload(&json, PayloadMode::Full).expect("payload");
		assert!(!payload.is_empty());
	}

	#[test]
	fn schema_rejects_missing_required() {
		let schema = RegistrySchema::from_view(&sample_view());
		let json = json!({"record_id": "only"});
		let err = schema.build_payload(&json, PayloadMode::Full).unwrap_err();
		assert!(format!("{err}").contains("missing required attributes"));
	}
}
