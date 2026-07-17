// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

use std::{collections::BTreeSet, marker::PhantomData};

use ciborium::value::Value;
use unicode_normalization::UnicodeNormalization;

use super::generated::{Production, SEMANTIC_TABLE_JSON};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum CodecError {
	#[error("WIRE_SCHEMA_INVALID: {0}")]
	Schema(String),
	#[error("WIRE_NON_CANONICAL: {0}")]
	NonCanonical(String),
}

/// A closed DTO. Its raw value cannot escape this private module and is created only after
/// validation against the generated production schema.
#[derive(Debug, Clone)]
pub(crate) struct Dto<P: Production> {
	value: Value,
	canonical: Vec<u8>,
	_marker: PhantomData<P>,
}

impl<P: Production> Dto<P> {
	pub(crate) fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
		let value: Value = ciborium::from_reader(bytes)
			.map_err(|error| CodecError::Schema(format!("invalid CBOR: {error}")))?;
		let canonical = encode_value(&value)?;
		if canonical != bytes {
			return Err(CodecError::NonCanonical(
				"input is not deterministic RFC 8949 encoding".into(),
			));
		}
		validate_production(P::NAME, &value)?;
		Ok(Self { value, canonical, _marker: PhantomData })
	}

	pub(crate) fn from_value(value: Value) -> Result<Self, CodecError> {
		validate_production(P::NAME, &value)?;
		let canonical = encode_value(&value)?;
		Ok(Self { value, canonical, _marker: PhantomData })
	}

	pub(crate) fn canonical(&self) -> &[u8] {
		&self.canonical
	}

	pub(super) fn value(&self) -> &Value {
		&self.value
	}
}

fn schema_table() -> Result<serde_json::Value, CodecError> {
	serde_json::from_str(SEMANTIC_TABLE_JSON)
		.map_err(|error| CodecError::Schema(format!("generated schema table is invalid: {error}")))
}

fn validate_production(name: &str, value: &Value) -> Result<(), CodecError> {
	let table = schema_table()?;
	let schemas = table
		.get("schemas")
		.and_then(serde_json::Value::as_object)
		.ok_or_else(|| CodecError::Schema("generated schema table has no schemas".into()))?;
	let schema = schemas
		.get(name)
		.ok_or_else(|| CodecError::Schema(format!("unknown production {name}")))?;
	validate_node(schema, value, schemas, name)?;
	if let Some(rules) = table.get("cross_field_rules").and_then(serde_json::Value::as_array) {
		for rule in rules
			.iter()
			.filter(|rule| rule.get("production").and_then(serde_json::Value::as_str) == Some(name))
		{
			validate_rule(rule, value)?;
		}
	}
	Ok(())
}

fn validate_node(
	schema: &serde_json::Value,
	value: &Value,
	schemas: &serde_json::Map<String, serde_json::Value>,
	path: &str,
) -> Result<(), CodecError> {
	let kind = schema
		.get("kind")
		.and_then(serde_json::Value::as_str)
		.ok_or_else(|| CodecError::Schema(format!("{path}: schema kind missing")))?;
	match kind {
		"ref" => {
			let name = schema["name"]
				.as_str()
				.ok_or_else(|| CodecError::Schema(format!("{path}: ref name missing")))?;
			let target = schemas
				.get(name)
				.ok_or_else(|| CodecError::Schema(format!("{path}: ref {name} missing")))?;
			validate_node(target, value, schemas, name)
		},
		"union" => {
			let variants = schema["variants"]
				.as_array()
				.ok_or_else(|| CodecError::Schema(format!("{path}: variants missing")))?;
			if variants
				.iter()
				.any(|variant| validate_node(variant, value, schemas, path).is_ok())
			{
				Ok(())
			} else {
				Err(CodecError::Schema(format!("{path}: no closed union variant matched")))
			}
		},
		"map" => validate_map(schema, value, schemas, path),
		"array" => {
			let Value::Array(items) = value else { return invalid(path, "must be an array") };
			check_len(schema, items.len(), path)?;
			let item_schema = &schema["items"];
			for (index, item) in items.iter().enumerate() {
				validate_node(item_schema, item, schemas, &format!("{path}[{index}]"))?;
			}
			Ok(())
		},
		"bytes" => {
			let Value::Bytes(bytes) = value else { return invalid(path, "must be bytes") };
			check_len(schema, bytes.len(), path)
		},
		"text" => {
			let Value::Text(text) = value else { return invalid(path, "must be text") };
			check_len(schema, text.len(), path)?;
			if schema.get("nfc").and_then(serde_json::Value::as_bool) == Some(true) &&
				text.nfc().ne(text.chars())
			{
				return invalid(path, "must already be NFC")
			}
			Ok(())
		},
		"uint" => {
			let integer = unsigned(value).ok_or_else(|| {
				CodecError::Schema(format!("{path}: must be an unsigned integer"))
			})?;
			let min = parse_bound(schema, "min", 0)?;
			let max = parse_bound(schema, "max", u64::MAX)?;
			if integer < min || integer > max {
				invalid(path, "unsigned integer is out of bounds")
			} else {
				Ok(())
			}
		},
		"bool" =>
			if matches!(value, Value::Bool(_)) {
				Ok(())
			} else {
				invalid(path, "must be boolean")
			},
		"const" => validate_const(&schema["value"], value, path),
		other => invalid(path, &format!("unsupported schema kind {other}")),
	}
}

fn validate_map(
	schema: &serde_json::Value,
	value: &Value,
	schemas: &serde_json::Map<String, serde_json::Value>,
	path: &str,
) -> Result<(), CodecError> {
	let Value::Map(entries) = value else { return invalid(path, "must be a closed map") };
	let fields = schema["fields"]
		.as_array()
		.ok_or_else(|| CodecError::Schema(format!("{path}: fields missing")))?;
	let mut present = BTreeSet::new();
	for (key, item) in entries {
		let key = unsigned(key)
			.ok_or_else(|| CodecError::Schema(format!("{path}: map key must be unsigned")))?;
		if !present.insert(key) {
			return invalid(path, "duplicate map key")
		}
		let field = fields
			.iter()
			.find(|field| field["key"].as_u64() == Some(key))
			.ok_or_else(|| CodecError::Schema(format!("{path}: unknown closed-map key {key}")))?;
		validate_node(&field["schema"], item, schemas, &format!("{path}.{key}"))?;
	}
	for field in fields {
		if field["required"].as_bool() == Some(true) {
			let key = field["key"]
				.as_u64()
				.ok_or_else(|| CodecError::Schema(format!("{path}: invalid field key")))?;
			if !present.contains(&key) {
				return invalid(path, &format!("required key {key} missing"))
			}
		}
	}
	Ok(())
}

fn validate_const(
	expected: &serde_json::Value,
	actual: &Value,
	path: &str,
) -> Result<(), CodecError> {
	let matches = match expected {
		serde_json::Value::Bool(value) => matches!(actual, Value::Bool(actual) if actual == value),
		serde_json::Value::Number(value) =>
			value.as_u64().is_some_and(|value| unsigned(actual) == Some(value)),
		serde_json::Value::String(value) =>
			matches!(actual, Value::Text(actual) if actual == value),
		_ => false,
	};
	if matches {
		Ok(())
	} else {
		invalid(path, "constant does not match")
	}
}

fn validate_rule(rule: &serde_json::Value, value: &Value) -> Result<(), CodecError> {
	let kind = rule["kind"].as_str().unwrap_or("");
	let id = rule["id"].as_str().unwrap_or("cross-field-rule");
	let get = |key: u64| {
		map_get(value, key).ok_or_else(|| CodecError::Schema(format!("{id}: key {key} missing")))
	};
	let key = |name: &str| {
		rule[name]
			.as_u64()
			.ok_or_else(|| CodecError::Schema(format!("{id}: rule key missing")))
	};
	let valid = match kind {
		"uint-positive" => unsigned(get(key("key")?)?).is_some_and(|value| value > 0),
		"uint-greater" => unsigned(get(key("left")?)?)
			.zip(unsigned(get(key("right")?)?))
			.is_some_and(|(left, right)| left > right),
		"uint-delta-max" => {
			let left = unsigned(get(key("left")?)?);
			let right = unsigned(get(key("right")?)?);
			let max = rule["max"].as_str().and_then(|value| value.parse::<u64>().ok());
			left.zip(right)
				.zip(max)
				.is_some_and(|((left, right), max)| left >= right && left - right <= max)
		},
		"bytes-nonzero" =>
			matches!(get(key("key")?)?, Value::Bytes(bytes) if bytes.iter().any(|byte| *byte != 0)),
		"bytes-no-nul" => matches!(get(key("key")?)?, Value::Bytes(bytes) if !bytes.contains(&0)),
		"bytes-array-sorted-unique" => match get(key("key")?)? {
			Value::Array(items) => items.windows(2).all(|pair| match (&pair[0], &pair[1]) {
				(Value::Bytes(left), Value::Bytes(right)) => left < right,
				(Value::Text(left), Value::Text(right)) => left < right,
				_ => false,
			}),
			_ => false,
		},
		_ => return invalid(id, &format!("unsupported cross-field rule {kind}")),
	};
	if valid {
		Ok(())
	} else {
		invalid(id, "cross-field rule failed")
	}
}

fn map_get(value: &Value, wanted: u64) -> Option<&Value> {
	let Value::Map(entries) = value else { return None };
	entries
		.iter()
		.find_map(|(key, value)| (unsigned(key) == Some(wanted)).then_some(value))
}

fn unsigned(value: &Value) -> Option<u64> {
	let Value::Integer(integer) = value else { return None };
	u64::try_from(*integer).ok()
}

fn parse_bound(schema: &serde_json::Value, name: &str, fallback: u64) -> Result<u64, CodecError> {
	match schema.get(name) {
		None => Ok(fallback),
		Some(serde_json::Value::String(value)) =>
			value.parse().map_err(|_| CodecError::Schema(format!("invalid {name} bound"))),
		Some(serde_json::Value::Number(value)) => value
			.as_u64()
			.ok_or_else(|| CodecError::Schema(format!("invalid {name} bound"))),
		_ => Err(CodecError::Schema(format!("invalid {name} bound"))),
	}
}

fn check_len(schema: &serde_json::Value, length: usize, path: &str) -> Result<(), CodecError> {
	let min = schema.get("min").and_then(serde_json::Value::as_u64).unwrap_or(0) as usize;
	let max = schema.get("max").and_then(serde_json::Value::as_u64).unwrap_or(u64::MAX) as usize;
	if length < min || length > max {
		invalid(path, "length is out of bounds")
	} else {
		Ok(())
	}
}

fn invalid<T>(path: &str, message: &str) -> Result<T, CodecError> {
	Err(CodecError::Schema(format!("{path}: {message}")))
}

fn head(major: u8, value: u64, output: &mut Vec<u8>) {
	match value {
		0..=23 => output.push((major << 5) | value as u8),
		24..=0xff => output.extend([(major << 5) | 24, value as u8]),
		0x100..=0xffff => {
			output.push((major << 5) | 25);
			output.extend((value as u16).to_be_bytes());
		},
		0x1_0000..=0xffff_ffff => {
			output.push((major << 5) | 26);
			output.extend((value as u32).to_be_bytes());
		},
		_ => {
			output.push((major << 5) | 27);
			output.extend(value.to_be_bytes());
		},
	}
}

pub(super) fn encode_value(value: &Value) -> Result<Vec<u8>, CodecError> {
	let mut output = Vec::new();
	encode_into(value, &mut output)?;
	Ok(output)
}

fn encode_into(value: &Value, output: &mut Vec<u8>) -> Result<(), CodecError> {
	match value {
		Value::Integer(_) => head(
			0,
			unsigned(value)
				.ok_or_else(|| CodecError::Schema("negative integer is forbidden".into()))?,
			output,
		),
		Value::Bytes(bytes) => {
			head(2, bytes.len() as u64, output);
			output.extend(bytes);
		},
		Value::Text(text) => {
			if text.nfc().ne(text.chars()) {
				return invalid("text", "must already be NFC")
			}
			head(3, text.len() as u64, output);
			output.extend(text.as_bytes());
		},
		Value::Array(items) => {
			head(4, items.len() as u64, output);
			for item in items {
				encode_into(item, output)?;
			}
		},
		Value::Map(entries) => {
			let mut encoded = Vec::with_capacity(entries.len());
			let mut keys = BTreeSet::new();
			for (key, value) in entries {
				let key_number = unsigned(key).ok_or_else(|| {
					CodecError::Schema("map keys must be unsigned integers".into())
				})?;
				if !keys.insert(key_number) {
					return invalid("map", "duplicate key")
				}
				let mut key_bytes = Vec::new();
				head(0, key_number, &mut key_bytes);
				encoded.push((key_bytes, encode_value(value)?));
			}
			encoded.sort_by(|left, right| {
				left.0.len().cmp(&right.0.len()).then_with(|| left.0.cmp(&right.0))
			});
			head(5, encoded.len() as u64, output);
			for (key, value) in encoded {
				output.extend(key);
				output.extend(value);
			}
		},
		Value::Bool(value) => output.push(if *value { 0xf5 } else { 0xf4 }),
		Value::Tag(_, _) => return Err(CodecError::NonCanonical("CBOR tags are forbidden".into())),
		Value::Float(_) | Value::Null => return invalid("value", "floats and null are forbidden"),
		_ => return invalid("value", "unsupported CBOR value"),
	}
	Ok(())
}
