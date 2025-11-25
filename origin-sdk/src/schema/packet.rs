use core::convert::TryFrom;
use frame_support::BoundedVec;
use origin_primitives::{
	attribute::Attribute,
	element::{ElementType, ElementView},
	packet::PacketAttributeView,
	registry::RegistryAttributeView,
};

use crate::types::{
	error::OriginSdkError,
	packet_input::{MaxRawDataLength, PacketAttributesInput, PacketElementInput},
};

/// Developer-friendly nested packet representation (same shape as view).
#[derive(Clone, Debug, PartialEq)]
pub struct PacketNestedValue {
	pub attributes: Vec<PacketAttributeView>,
}

/// Expand flat view (Vec<PacketAttributeView>) into nested helper type.
pub fn expand_packet(attrs: &[PacketAttributeView]) -> PacketNestedValue {
	PacketNestedValue { attributes: attrs.to_vec() }
}

/// Expand a PacketStateView into nested helper form (attributes only).
pub fn expand_packet_view(view: &origin_primitives::packet::PacketStateView) -> PacketNestedValue {
	let attrs: Vec<PacketAttributeView> = view
		.attributes
		.iter()
		.map(|a| PacketAttributeView { key: a.key.clone(), value: a.value.clone() })
		.collect();
	PacketNestedValue { attributes: attrs }
}

/// Flatten nested packet into pallet-aligned bounded attributes without schema validation.
pub fn flatten_packet(
	nested: &PacketNestedValue,
) -> Result<Vec<(Vec<u8>, origin_primitives::element::ElementView)>, OriginSdkError> {
	let mut attrs = nested.attributes.clone();
	attrs.sort_by(|a, b| a.key.cmp(&b.key));
	Ok(attrs.into_iter().map(|a| (a.key, a.value)).collect())
}

/// Validate packet nested attributes against a registry schema and return bounded attributes.
pub fn validate_and_flatten(
	nested: &PacketNestedValue,
	schema: &[RegistryAttributeView],
) -> Result<PacketAttributesInput, OriginSdkError> {
	let mut out_vec: Vec<(Attribute, PacketElementInput)> = Vec::new();
	let mut seen = std::collections::BTreeSet::new();

	for spec in schema {
		let key_bytes = &spec.key;
		let key: Attribute = Attribute::try_from(key_bytes.clone())
			.map_err(|_| OriginSdkError::InvalidInput("packet attr key too long".into()))?;
		if !seen.insert(key_bytes.clone()) {
			return Err(OriginSdkError::InvalidInput("duplicate packet attribute key".into()));
		}

		// find provided value
		let provided = nested.attributes.iter().find(|a| a.key == *key_bytes);
		match (provided, spec.optional) {
			(Some(attr), _) => {
				ensure_kind(key_bytes, spec.kind, &attr.value)?;
				let val = element_from_view(&attr.value)?;
				out_vec.push((key, val));
			},
			(None, true) => {},
			(None, false) =>
				return Err(OriginSdkError::Schema(format!(
					"missing required attribute '{}'",
					String::from_utf8_lossy(key_bytes)
				))),
		}
	}

	// reject unknown keys
	for attr in &nested.attributes {
		let known = schema.iter().any(|s| s.key == attr.key);
		if !known {
			return Err(OriginSdkError::Schema(format!(
				"attribute '{}' not in registry schema",
				String::from_utf8_lossy(&attr.key)
			)));
		}
	}

	PacketAttributesInput::try_collect(out_vec.into_iter())
		.map_err(|_| OriginSdkError::InvalidInput("too many packet attributes".into()))
}

fn ensure_kind(
	key: &[u8],
	expected: ElementType,
	value: &ElementView,
) -> Result<(), OriginSdkError> {
	let ok = match (expected, value) {
		(ElementType::None, ElementView::None) => true,
		(ElementType::Raw, ElementView::Raw(_)) => true,
		(ElementType::Bool, ElementView::Bool(_)) => true,
		(ElementType::U64, ElementView::U64(_)) => true,
		(ElementType::U128, ElementView::U128(_)) => true,
		(ElementType::Hash, ElementView::Hash(_)) => true,
		(ElementType::Token, ElementView::Token(_)) => true,
		(ElementType::Cid, ElementView::Cid(_)) => true,
		_ => false,
	};
	if ok {
		return Ok(());
	}
	Err(OriginSdkError::Schema(format!(
		"attribute '{}' expected {:?} got {:?}",
		String::from_utf8_lossy(key),
		expected,
		value
	)))
}

fn element_from_view(ev: &ElementView) -> Result<PacketElementInput, OriginSdkError> {
	match ev {
		ElementView::None => Ok(origin_primitives::element::Elum::None),
		ElementView::Raw(bytes) => {
			let bounded = BoundedVec::<u8, MaxRawDataLength>::try_from(bytes.clone())
				.map_err(|_| OriginSdkError::InvalidInput("packet raw too large".into()))?;
			Ok(origin_primitives::element::Elum::Raw(bounded))
		},
		ElementView::Bool(b) => Ok(origin_primitives::element::Elum::Bool(*b as u8)),
		ElementView::U64(v) => Ok(origin_primitives::element::Elum::U64(v.to_le_bytes())),
		ElementView::U128(v) => Ok(origin_primitives::element::Elum::U128(v.to_le_bytes())),
		ElementView::Hash(h) => Ok(origin_primitives::element::Elum::Hash(*h)),
		ElementView::Token(id) => Ok(origin_primitives::element::Elum::Token(id.clone())),
		ElementView::Cid(cid) => {
			let bounded = BoundedVec::<u8, MaxRawDataLength>::try_from(cid.clone())
				.map_err(|_| OriginSdkError::InvalidInput("packet cid too large".into()))?;
			Ok(origin_primitives::element::Elum::CID(bounded))
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use origin_primitives::element::ElementType;

	fn attr(key: &str, kind: ElementType, optional: bool) -> RegistryAttributeView {
		RegistryAttributeView { key: key.as_bytes().to_vec(), kind, optional }
	}

	fn p(key: &str, val: ElementView) -> PacketAttributeView {
		PacketAttributeView { key: key.as_bytes().to_vec(), value: val }
	}

	#[test]
	fn validate_and_flatten_accepts_matching_required() {
		let schema = vec![attr("a", ElementType::Bool, false)];
		let nested = PacketNestedValue { attributes: vec![p("a", ElementView::Bool(true))] };
		let flat = validate_and_flatten(&nested, &schema).expect("valid");
		assert_eq!(flat.len(), 1);
	}

	#[test]
	fn validate_and_flatten_rejects_missing_required() {
		let schema = vec![attr("a", ElementType::Bool, false)];
		let nested = PacketNestedValue { attributes: vec![] };
		let err = validate_and_flatten(&nested, &schema).unwrap_err().to_string();
		assert!(err.contains("missing required attribute"));
	}
}
