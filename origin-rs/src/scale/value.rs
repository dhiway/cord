use crate::error::{Error, Result};
use origin_primitives::{
	identifier::Ss58Identifier,
	view::{dev_attr_from, dev_element_from, AttributeValueView, DevAttr, ElementView},
};
use scale_value::{Composite, Value, ValueDef, Variant};
use std::collections::BTreeMap;

fn flatten<'a>(value: &'a Value<u32>) -> &'a Value<u32> {
	match &value.value {
		ValueDef::Composite(Composite::Named(fields)) if fields.len() == 1 => flatten(&fields[0].1),
		ValueDef::Composite(Composite::Unnamed(items)) if items.len() == 1 => flatten(&items[0]),
		_ => value,
	}
}

fn variant<'a>(value: &'a Value<u32>) -> Option<&'a Variant<u32>> {
	match &flatten(value).value {
		ValueDef::Variant(v) => Some(v),
		_ => None,
	}
}

fn first_field<'a>(composite: &'a Composite<u32>) -> Option<&'a Value<u32>> {
	match composite {
		Composite::Named(fields) => fields.first().map(|(_, v)| v),
		Composite::Unnamed(items) => items.first(),
	}
}

fn option_inner<'a>(value: &'a Value<u32>) -> Option<&'a Value<u32>> {
	match variant(value) {
		Some(var) if var.name == "None" => None,
		Some(var) if var.name == "Some" => first_field(&var.values),
		_ => Some(value),
	}
}

fn sequence_items<'a>(value: &'a Value<u32>) -> Option<Vec<&'a Value<u32>>> {
	match &flatten(value).value {
		ValueDef::Composite(Composite::Unnamed(items)) => Some(items.iter().collect()),
		_ => None,
	}
}

fn tuple2<'a>(value: &'a Value<u32>) -> Option<(&'a Value<u32>, &'a Value<u32>)> {
	let items = sequence_items(value)?;
	if items.len() == 2 {
		Some((items[0], items[1]))
	} else {
		None
	}
}

fn value_bytes(value: &Value<u32>) -> Option<Vec<u8>> {
	match &flatten(value).value {
		ValueDef::Composite(Composite::Unnamed(items))
			if items.iter().all(|item| item.as_u128().is_some()) =>
		{
			Some(items.iter().map(|item| item.as_u128().unwrap() as u8).collect())
		},
		ValueDef::Variant(var) => first_field(&var.values).and_then(value_bytes),
		ValueDef::Primitive(_) => value.as_u128().map(|n| vec![n as u8]),
		ValueDef::BitSequence(bits) => {
			let mut out = Vec::new();
			let mut current = 0u8;
			let mut count = 0u8;
			for bit in bits.iter() {
				if bit {
					current |= 1 << count;
				}
				count += 1;
				if count == 8 {
					out.push(current);
					current = 0;
					count = 0;
				}
			}
			if count > 0 {
				out.push(current);
			}
			Some(out)
		},
		_ => None,
	}
}

fn bool_from_field(field: Option<&Value<u32>>) -> Option<bool> {
	field.and_then(|f| f.as_u128()).map(|num| num != 0)
}

fn bytes_to_array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N]> {
	bytes.try_into().map_err(|_| Error::Codec(format!("expected {N} bytes")))
}

fn bytes_to_identifier(bytes: Vec<u8>) -> Result<Ss58Identifier> {
	Ss58Identifier::try_from(bytes)
		.map_err(|e| Error::Codec(format!("invalid ss58 identifier: {e:?}")))
}

fn bytes_from_field(field: Option<&Value<u32>>, label: &str) -> Result<Vec<u8>> {
	let value = field.ok_or_else(|| Error::Codec(format!("missing {label}")))?;
	value_bytes(value).ok_or_else(|| Error::Codec(format!("invalid {label}")))
}

/// Decode a dynamic `Value` representing an on-chain `Element` into its typed view form.
pub fn decode_element_view(value: &Value<u32>) -> Result<ElementView> {
	let var = variant(value).ok_or_else(|| Error::Codec("expected element variant".into()))?;
	let field = first_field(&var.values);
	match var.name.as_str() {
		"None" => Ok(ElementView::None),
		"Raw" => Ok(ElementView::Raw(bytes_from_field(field, "raw value")?)),
		"Bool" => Ok(ElementView::Bool(bool_from_field(field).unwrap_or(false))),
		"U64" => {
			let bytes = bytes_from_field(field, "u64 bytes")?;
			Ok(ElementView::U64(u64::from_le_bytes(bytes_to_array(bytes)?)))
		},
		"U128" => {
			let bytes = bytes_from_field(field, "u128 bytes")?;
			Ok(ElementView::U128(u128::from_le_bytes(bytes_to_array(bytes)?)))
		},
		"Hash" => {
			let bytes = bytes_from_field(field, "hash bytes")?;
			Ok(ElementView::Hash(bytes_to_array(bytes)?))
		},
		"Token" => {
			let bytes = bytes_from_field(field, "token bytes")?;
			Ok(ElementView::Token(bytes_to_identifier(bytes)?))
		},
		"CID" => {
			let bytes = bytes_from_field(field, "cid bytes")?;
			Ok(ElementView::Cid(bytes))
		},
		"Localized" => {
			let Some(entries) = sequence_items(field.ok_or_else(|| Error::Codec("missing localized entries".into()))?) else {
				return Ok(ElementView::Localized(Vec::new()));
			};
			let mut pairs = Vec::new();
			for entry in entries {
				if let Some((locale_value, element_value)) = tuple2(entry) {
					let locale = value_bytes(locale_value).unwrap_or_default();
					let element = decode_element_view(element_value)?;
					pairs.push((locale, element));
				}
			}
			Ok(ElementView::Localized(pairs))
		},
		other => Err(Error::Codec(format!("unsupported element variant {other}"))),
	}
}

/// Decode an optional list of `(key, Element)` pairs into developer-friendly attributes.
pub fn decode_dev_attributes(value: &Value<u32>) -> Result<Vec<DevAttr>> {
	let Some(inner) = option_inner(value) else {
		return Ok(Vec::new());
	};
	let Some(entries) = sequence_items(inner) else {
		return Ok(Vec::new());
	};
	let mut result = Vec::new();
	for entry in entries {
		if let Some((key_value, element_value)) = tuple2(entry) {
			if let Some(bytes) = value_bytes(key_value) {
				let element = decode_element_view(element_value)?;
				let attr_view = AttributeValueView { key: bytes, value: element };
				result.push(dev_attr_from(&attr_view));
			}
		}
	}
	Ok(result)
}

/// Decode an `Element` value directly into its printable developer form.
pub fn decode_dev_element(value: &Value<u32>) -> Result<origin_primitives::view::DevElement> {
	let view = decode_element_view(value)?;
	Ok(dev_element_from(&view))
}

/// Helper returning a map of UTF-8-ish keys to developer attributes.
pub fn decode_dev_attribute_map(value: &Value<u32>) -> Result<BTreeMap<String, DevAttr>> {
	let mut map = BTreeMap::new();
	for attr in decode_dev_attributes(value)? {
		let key = attr.key_utf8.clone().unwrap_or_else(|| attr.key_hex.clone());
		map.insert(key, attr);
	}
	Ok(map)
}
