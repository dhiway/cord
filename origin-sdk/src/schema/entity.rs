use crate::types::error::OriginSdkError;
use crate::types::{
	entity_input::{build_attributes, ElementInput, MaxRawDataLength},
	EntityInfoInput,
};
use core::convert::TryFrom;
use origin_primitives::{
	attribute::{Attribute, Element},
	AttributeValueView, ElementView, EntityInfoView, EntityStateView, Ss58Identifier,
};
use std::collections::BTreeSet;

/// Developer-friendly nested representation for entity info.
///
/// At the moment this mirrors `EntityInfoView`; it stays separate so
/// we can evolve nested forms (e.g. JSON maps) without touching the
/// pallet-facing flat type.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityNestedValue {
	pub display: ElementView,
	pub web: ElementView,
	pub email: ElementView,
	pub attributes: Option<Vec<AttributeValueView>>,
}

/// Flat entity info exactly as the pallet view returns.
pub type EntityFlatValue = EntityInfoView;

/// Expand a flat view result into the nested representation.
pub fn expand_entity(flat: &EntityFlatValue) -> EntityNestedValue {
	EntityNestedValue {
		display: flat.display.clone(),
		web: flat.web.clone(),
		email: flat.email.clone(),
		attributes: flat.attributes.clone(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use origin_primitives::element::ElementView;

	fn make_nested() -> EntityNestedValue {
		EntityNestedValue {
			display: ElementView::Raw(b"display".to_vec()),
			web: ElementView::Raw(b"web".to_vec()),
			email: ElementView::Raw(b"mail".to_vec()),
			attributes: Some(vec![AttributeValueView {
				key: b"k".to_vec(),
				value: ElementView::Bool(true),
			}]),
		}
	}

	#[test]
	fn flatten_expand_roundtrip() {
		let nested = make_nested();
		let flat = flatten_entity(&nested);
		let round = expand_entity(&flat);
		assert_eq!(nested, round);
	}

	#[test]
	fn nested_to_input_respects_bounds() {
		let nested = make_nested();
		let input = to_entity_input(&nested).expect("convert");
		// ensure attributes included
		assert!(input.attributes.is_some());
	}
}

/// Flatten a nested representation into the pallet-aligned flat view.
pub fn flatten_entity(nested: &EntityNestedValue) -> EntityFlatValue {
	EntityFlatValue {
		display: nested.display.clone(),
		web: nested.web.clone(),
		email: nested.email.clone(),
		attributes: nested.attributes.as_ref().map(|attrs| {
			let mut sorted = attrs.clone();
			sorted.sort_by(|a, b| a.key.cmp(&b.key));
			sorted
		}),
	}
}

/// Helper to expand full entity state (info + metadata).
pub fn expand_entity_state(
	state: &EntityStateView<subxt::utils::AccountId32>,
) -> (EntityNestedValue, Option<Vec<AttributeValueView>>, Option<Ss58Identifier>) {
	(
		expand_entity(&state.info),
		state.info.attributes.clone(),
		state.nym.as_ref().and_then(|b| Ss58Identifier::try_from(b.clone()).ok()),
	)
}

/// Convert nested entity info into the pallet-aligned extrinsic input.
pub fn to_entity_input(nested: &EntityNestedValue) -> Result<EntityInfoInput, OriginSdkError> {
	let display = element_from_view(&nested.display)?;
	let web = element_from_view(&nested.web)?;
	let email = element_from_view(&nested.email)?;

	let attributes = if let Some(attrs) = &nested.attributes {
		let mut pairs = Vec::with_capacity(attrs.len());
		let mut seen = BTreeSet::new();
		for attr in attrs {
			if attr.key == b"display" || attr.key == b"web" || attr.key == b"email" {
				return Err(OriginSdkError::InvalidInput(
					"attribute key collides with reserved fields display/web/email".into(),
				));
			}
			if !seen.insert(attr.key.clone()) {
				return Err(OriginSdkError::InvalidInput("duplicate attribute key".into()));
			}
			let key: Attribute = Attribute::try_from(attr.key.clone())
				.map_err(|_| OriginSdkError::InvalidInput("attribute key too long".into()))?;
			let val = element_from_view(&attr.value)?;
			pairs.push((key, val));
		}
		Some(build_attributes(&pairs).map_err(|e| OriginSdkError::InvalidInput(format!("{e:?}")))?)
	} else {
		None
	};

	Ok(EntityInfoInput { display, web, email, attributes })
}

/// Convert ElementView into pallet Element with bounded capacity.
pub fn element_from_view(ev: &ElementView) -> Result<ElementInput, OriginSdkError> {
	match ev {
		ElementView::None => Ok(Element::None),
		ElementView::Raw(bytes) => Ok(Element::Raw(
			frame_support::BoundedVec::<u8, MaxRawDataLength>::try_from(bytes.clone()).map_err(
				|_| OriginSdkError::InvalidInput("raw element exceeds max length".into()),
			)?,
		)),
		ElementView::Bool(flag) => Ok(Element::Bool(*flag as u8)),
		ElementView::U64(v) => Ok(Element::U64(v.to_le_bytes())),
		ElementView::U128(v) => Ok(Element::U128(v.to_le_bytes())),
		ElementView::Hash(h) => Ok(Element::Hash(*h)),
		ElementView::Token(t) => Ok(Element::Token(t.clone())),
		ElementView::Cid(cid) => Ok(Element::CID(
			frame_support::BoundedVec::<u8, MaxRawDataLength>::try_from(cid.clone())
				.map_err(|_| OriginSdkError::InvalidInput("cid exceeds max length".into()))?,
		)),
	}
}
