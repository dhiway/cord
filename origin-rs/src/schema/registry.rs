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

use crate::types::{
	error::OriginSdkError,
	registry::{
		MaxAdditionalAttributes, MaxRawDataLength, RegistryAttributeInput, RegistryCreateInput,
		RegistryLookupInput,
	},
};
use codec::Encode;
use frame_support::BoundedVec;
use origin_primitives::{
	attribute::{Attribute, Element},
	element::ElementView,
	registry::{RegistryAttributeView, RegistryKind, RegistryStateView, RegistryStatus},
	Ss58Identifier,
};
use std::collections::BTreeSet;

/// Nested developer-facing registry schema.
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryNestedSchema {
	pub registry: Ss58Identifier,
	pub info: ElementView,
	pub kind: RegistryKind,
	pub status: RegistryStatus,
	pub attributes: Vec<RegistryAttributeView>,
	pub token_spec: Vec<Vec<u8>>,
	/// Lookup specs in the view form (Vec<Vec<u8>> combos)
	pub lookup_specs: Vec<Vec<Vec<u8>>>,
	pub maintainer: Ss58Identifier,
}

/// Flat registry view as returned by the pallet.
pub type RegistryFlatSchema = RegistryStateView;

pub fn expand_registry(flat: &RegistryFlatSchema) -> RegistryNestedSchema {
	RegistryNestedSchema {
		registry: flat.registry.clone(),
		info: flat.info.clone(),
		kind: flat.kind.clone(),
		status: flat.status,
		attributes: flat.attributes.clone(),
		token_spec: flat.token_spec.clone(),
		lookup_specs: flat.lookup_specs.clone(),
		maintainer: flat.maintainer.clone(),
	}
}

pub fn flatten_registry(nested: &RegistryNestedSchema) -> RegistryFlatSchema {
	RegistryFlatSchema {
		registry: nested.registry.clone(),
		maintainer: nested.maintainer.clone(),
		info: nested.info.clone(),
		kind: nested.kind.clone(),
		status: nested.status,
		attributes: {
			let mut attrs = nested.attributes.clone();
			attrs.sort_by(|a, b| a.key.cmp(&b.key));
			attrs
		},
		token_spec: {
			let mut ts = nested.token_spec.clone();
			ts.sort();
			ts
		},
		lookup_specs: {
			let mut ls = nested.lookup_specs.clone();
			ls.iter_mut().for_each(|spec| spec.sort());
			ls.sort();
			ls
		},
	}
}

/// Convert a nested schema into the SDK input struct for `create_registry`.
pub fn to_create_input(
	nested: &RegistryNestedSchema,
) -> Result<RegistryCreateInput, OriginSdkError> {
	let info = element_from_view(&nested.info)?;

	let mut attrs = BoundedVec::<RegistryAttributeInput, MaxAdditionalAttributes>::new();
	let mut attr_keys = BTreeSet::new();
	for a in &nested.attributes {
		let key: Attribute = Attribute::try_from(a.key.clone())
			.map_err(|_| OriginSdkError::InvalidInput("registry attribute key too long".into()))?;
		if !attr_keys.insert(a.key.clone()) {
			return Err(OriginSdkError::InvalidInput("duplicate registry attribute key".into()));
		}
		attrs
			.try_push(RegistryAttributeInput { key, kind: a.kind, optional: a.optional })
			.map_err(|_| OriginSdkError::InvalidInput("too many registry attributes".into()))?;
	}

	let token_spec = lookup_from_vecs(&nested.token_spec)?;
	for k in &nested.token_spec {
		if !attr_keys.contains(k) {
			return Err(OriginSdkError::Schema(format!(
				"token_spec key '{}' not in attributes",
				String::from_utf8_lossy(k)
			)));
		}
	}

	let mut lookup_specs = BoundedVec::<RegistryLookupInput, MaxAdditionalAttributes>::new();
	for spec in &nested.lookup_specs {
		for k in spec {
			if !attr_keys.contains(k) {
				return Err(OriginSdkError::Schema(format!(
					"lookup_spec key '{}' not in attributes",
					String::from_utf8_lossy(k)
				)));
			}
		}
		lookup_specs
			.try_push(lookup_from_vecs(spec)?)
			.map_err(|_| OriginSdkError::InvalidInput("too many lookup specs".into()))?;
	}

	Ok(RegistryCreateInput {
		info,
		kind: nested.kind.clone(),
		attributes: attrs,
		token_spec,
		lookup_specs,
	})
}

pub(crate) fn element_from_view(
	ev: &ElementView,
) -> Result<Element<MaxRawDataLength>, OriginSdkError> {
	match ev {
		ElementView::Raw(bytes) => Ok(Element::Raw(
			BoundedVec::<u8, MaxRawDataLength>::try_from(bytes.clone())
				.map_err(|_| OriginSdkError::InvalidInput("registry info too large".into()))?,
		)),
		ElementView::None => Ok(Element::None),
		_ => Ok(Element::Raw(
			BoundedVec::<u8, MaxRawDataLength>::try_from(ev.encode())
				.map_err(|_| OriginSdkError::InvalidInput("registry info too large".into()))?,
		)),
	}
}

fn lookup_from_vecs(keys: &[Vec<u8>]) -> Result<RegistryLookupInput, OriginSdkError> {
	let mut bounded_keys = BoundedVec::<Attribute, MaxAdditionalAttributes>::new();
	for k in keys {
		let attr: Attribute = Attribute::try_from(k.clone())
			.map_err(|_| OriginSdkError::InvalidInput("lookup key too long".into()))?;
		bounded_keys
			.try_push(attr)
			.map_err(|_| OriginSdkError::InvalidInput("too many lookup keys".into()))?;
	}

	Ok(match bounded_keys.len() {
		0 => RegistryLookupInput::Combo(bounded_keys),
		1 => RegistryLookupInput::Single(bounded_keys[0].clone()),
		_ => RegistryLookupInput::Combo(bounded_keys),
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use origin_primitives::{element::ElementType, registry::RegistryAttributeView};

	fn base_schema() -> RegistryNestedSchema {
		RegistryNestedSchema {
			registry: Ss58Identifier::to_encoded([1u8; 32], 42, 1, 1).unwrap(),
			info: ElementView::Raw(b"info".to_vec()),
			kind: RegistryKind::Raw,
			status: RegistryStatus::Active,
			attributes: vec![RegistryAttributeView {
				key: b"id".to_vec(),
				kind: ElementType::Raw,
				optional: false,
			}],
			token_spec: vec![b"id".to_vec()],
			lookup_specs: vec![vec![b"id".to_vec()]],
			maintainer: Ss58Identifier::to_encoded([2u8; 32], 42, 1, 1).unwrap(),
		}
	}

	#[test]
	fn flatten_expand_roundtrip() {
		let nested = base_schema();
		let flat = flatten_registry(&nested);
		let round = expand_registry(&flat);
		assert_eq!(nested, round);
	}

	#[test]
	fn create_input_from_nested_validates_lengths() {
		let nested = base_schema();
		let input = to_create_input(&nested).expect("convert");
		assert_eq!(input.attributes.len(), 1);
	}
}
