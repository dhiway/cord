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

use oc::schema::{entity, packet, registry};
use proptest::prelude::*;

#[test]
fn entity_roundtrip_ordering_stable() {
	let nested = entity::EntityNestedValue {
		display: origin_primitives::element::ElementView::Raw(b"d".to_vec()),
		web: origin_primitives::element::ElementView::Raw(b"w".to_vec()),
		email: origin_primitives::element::ElementView::Raw(b"e".to_vec()),
		attributes: Some(vec![
			origin_primitives::attribute::AttributeValueView {
				key: b"b".to_vec(),
				value: origin_primitives::element::ElementView::Bool(true),
			},
			origin_primitives::attribute::AttributeValueView {
				key: b"a".to_vec(),
				value: origin_primitives::element::ElementView::Bool(false),
			},
		]),
	};
	let flat = entity::flatten_entity(&nested);
	assert_eq!(flat.attributes.as_ref().unwrap()[0].key, b"a");
	let round = entity::expand_entity(&flat);
	assert_eq!(round.attributes.unwrap()[0].key, b"a");
}

#[test]
fn registry_roundtrip_sorts_keys() {
	let nested = registry::RegistryNestedSchema {
		registry: origin_primitives::Ss58Identifier::to_encoded(vec![1u8; 32], 29, 1, 0).unwrap(),
		info: origin_primitives::element::ElementView::Raw(b"info".to_vec()),
		kind: origin_primitives::registry::RegistryKind::Raw,
		status: origin_primitives::registry::RegistryStatus::Active,
		attributes: vec![
			origin_primitives::registry::RegistryAttributeView {
				key: b"b".to_vec(),
				kind: origin_primitives::element::ElementType::Raw,
				optional: false,
			},
			origin_primitives::registry::RegistryAttributeView {
				key: b"a".to_vec(),
				kind: origin_primitives::element::ElementType::Raw,
				optional: false,
			},
		],
		token_spec: vec![b"b".to_vec(), b"a".to_vec()],
		lookup_specs: vec![vec![b"b".to_vec(), b"a".to_vec()]],
		maintainer: origin_primitives::Ss58Identifier::to_encoded(vec![2u8; 32], 29, 1, 0).unwrap(),
	};
	let flat = registry::flatten_registry(&nested);
	assert_eq!(flat.attributes[0].key, b"a");
	assert_eq!(flat.token_spec[0], b"a");
	assert_eq!(flat.lookup_specs[0][0], b"a");
	let round = registry::expand_registry(&flat);
	assert_eq!(round.attributes[0].key, b"a");

	// ensure create input builds
	let _ = registry::to_create_input(&round).expect("create input ok");
}

#[test]
fn packet_flatten_sorts_keys() {
	let nested = packet::PacketNestedValue {
		attributes: vec![
			origin_primitives::packet::PacketAttributeView {
				key: b"z".to_vec(),
				value: origin_primitives::element::ElementView::Bool(true),
			},
			origin_primitives::packet::PacketAttributeView {
				key: b"a".to_vec(),
				value: origin_primitives::element::ElementView::Bool(false),
			},
		],
	};
	let flat = packet::flatten_packet(&nested).expect("flatten");
	assert_eq!(flat[0].0.as_slice(), b"a");
}

proptest! {
	#[test]
	fn entity_flatten_expand_prop(mut keys in proptest::collection::vec("[a-z]{1,6}".prop_map(|s| s.into_bytes()), 0..10)) {
		keys.sort(); keys.dedup();
		let attrs = keys.iter().map(|k| origin_primitives::attribute::AttributeValueView { key: k.clone(), value: origin_primitives::element::ElementView::Raw(vec![1,2,3])}).collect::<Vec<_>>();
		let nested = entity::EntityNestedValue {
			display: origin_primitives::element::ElementView::Raw(b"d".to_vec()),
			web: origin_primitives::element::ElementView::Raw(b"w".to_vec()),
			email: origin_primitives::element::ElementView::Raw(b"e".to_vec()),
			attributes: if attrs.is_empty() { None } else { Some(attrs.clone()) },
		};
		let flat = entity::flatten_entity(&nested);
		if let Some(a) = &flat.attributes {
			assert!(a.windows(2).all(|w| w[0].key <= w[1].key));
		}
		let round = entity::expand_entity(&flat);
		assert_eq!(round.display, nested.display);
	}

	#[test]
	fn registry_create_input_validates_duplicates(mut keys in proptest::collection::vec("[a-z]{1,6}".prop_map(|s| s.into_bytes()), 1..6)) {
		keys.sort();
		keys.dedup();
		let attrs = keys.iter().map(|k| origin_primitives::registry::RegistryAttributeView {
			key: k.clone(), kind: origin_primitives::element::ElementType::Raw, optional: false
		}).collect::<Vec<_>>();

		let nested = registry::RegistryNestedSchema {
			registry: origin_primitives::Ss58Identifier::to_encoded(vec![1u8; 32], 29, 1, 0).unwrap(),
			info: origin_primitives::element::ElementView::Raw(b"info".to_vec()),
			kind: origin_primitives::registry::RegistryKind::Raw,
			status: origin_primitives::registry::RegistryStatus::Active,
			attributes: attrs.clone(),
			token_spec: vec![keys[0].clone()],
			lookup_specs: vec![keys.clone()],
			maintainer: origin_primitives::Ss58Identifier::to_encoded(vec![2u8; 32], 29, 1, 0).unwrap(),
		};
		let _ = registry::to_create_input(&nested).expect("valid schema");
	}
}
