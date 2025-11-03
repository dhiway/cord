// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]

use super::*;
use crate::{
	mock::*,
	register::{LookupSpec, RegistryKind, RegistryPermissions},
};
use cord_primitives::packet::ElementType;
use frame_support::{assert_noop, assert_ok, BoundedVec};

fn raw(data: &[u8]) -> Element<MaxRawDataLength> {
	Element::Raw(data.to_vec().try_into().expect("bounded data"))
}

fn attrs<'a>(
	pairs: impl IntoIterator<Item = (&'a [u8], ElementType)>,
) -> AttributeSchemaListOf<Test> {
	let mut list = AttributeSchemaListOf::<Test>::default();
	for (key, value_type) in pairs {
		let bounded_key = Attribute::try_from(key.to_vec()).expect("within key bound");
		list.try_push((bounded_key, value_type)).expect("attribute capacity");
	}
	list
}

fn token_spec(keys: &[&[u8]]) -> TokenSpecOf<Test> {
	match keys.len() {
		0 => LookupSpec::Combo(
			BoundedVec::<Attribute, <Test as Config>::MaxAdditionalAttributes>::default(),
		),
		1 => {
			let attr = Attribute::try_from(keys[0].to_vec()).expect("within key bound");
			LookupSpec::Single(attr)
		},
		_ => {
			let mut combo =
				BoundedVec::<Attribute, <Test as Config>::MaxAdditionalAttributes>::default();
			for key in keys {
				let attr = Attribute::try_from(key.to_vec()).expect("within key bound");
				combo.try_push(attr).expect("combo capacity");
			}
			LookupSpec::Combo(combo)
		},
	}
}

fn lookup_specs(specs: &[&[&[u8]]]) -> LookupSpecListOf<Test> {
	let mut list = LookupSpecListOf::<Test>::default();
	for spec in specs {
		let lookup = token_spec(spec);
		list.try_push(lookup).expect("lookup spec capacity");
	}
	list
}

fn first_registry() -> Ss58Identifier {
	Registries::<Test>::iter_keys().next().expect("registry present")
}

#[test]
fn create_register_happy_path() {
	new_test_ext().execute_with(|| {
		let maintainer = account(1);
		let maintainer_token = bind_account(maintainer);

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			raw(b"My Registry"),
			RegistryKind::Raw,
			true,
			attrs([(b"schema".as_ref(), ElementType::Raw)]),
			token_spec(&[b"schema"]),
			lookup_specs(&[]),
		));

		let registry = first_registry();
		let stored = Registries::<Test>::get(&registry).expect("stored");
		assert_eq!(stored.maintainer(), &maintainer_token);
		assert_eq!(stored.info, raw(b"My Registry"));
		assert_eq!(stored.attribute_type(b"schema"), Some(ElementType::Raw));
		assert_eq!(stored.kind, RegistryKind::Raw);
		assert!(stored.is_active);

		let perms =
			RegistryDelegates::<Test>::get(&registry, maintainer_token).expect("maintainer perms");
		assert!(perms.has_admin());
		assert!(perms.has_view());
	});
}

#[test]
fn registry_attributes_support_u128_element() {
	new_test_ext().execute_with(|| {
		let maintainer = account(99);
		let maintainer_token = bind_account(maintainer);

		let mut attributes = AttributeSchemaListOf::<Test>::default();
		let limit_key = Attribute::try_from(b"limit".to_vec()).expect("bounded key");
		attributes
			.try_push((limit_key.clone(), ElementType::U128))
			.expect("attribute capacity");

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			raw(b"Balances"),
			RegistryKind::Raw,
			true,
			attributes,
			token_spec(&[b"limit"]),
			lookup_specs(&[]),
		));

		let registry = first_registry();
		let stored = Registries::<Test>::get(&registry).expect("stored");
		assert_eq!(stored.maintainer(), &maintainer_token);
		let value = stored.attribute_type(b"limit").expect("attribute exists");
		assert_eq!(value, ElementType::U128);
	});
}

#[test]
fn create_register_requires_attributes_and_valid_token_fields() {
	new_test_ext().execute_with(|| {
		let maintainer = account(2);
		let _ = bind_account(maintainer);

		assert_noop!(
			Pallet::<Test>::create_registry(
				RuntimeOrigin::signed(maintainer),
				raw(b"A"),
				RegistryKind::Token,
				true,
				AttributeSchemaListOf::<Test>::default(),
				token_spec(&[b"foo"]),
				lookup_specs(&[])
			),
			Error::<Test>::NoAttributes
		);

		let attributes = attrs([(b"foo".as_ref(), ElementType::Raw)]);
		let fields = token_spec(&[b"missing"]);
		assert_noop!(
			Pallet::<Test>::create_registry(
				RuntimeOrigin::signed(maintainer),
				raw(b"A"),
				RegistryKind::Token,
				true,
				attributes,
				fields,
				lookup_specs(&[])
			),
			Error::<Test>::UnknownTokenField
		);
	});
}

#[test]
fn create_registry_fails_for_empty_token_spec() {
	new_test_ext().execute_with(|| {
		let maintainer = account(10);
		let _ = bind_account(maintainer);

		assert_noop!(
			Pallet::<Test>::create_registry(
				RuntimeOrigin::signed(maintainer),
				raw(b"A"),
				RegistryKind::Raw,
				true,
				attrs([(b"k".as_ref(), ElementType::Raw)]),
				token_spec(&[]),
				lookup_specs(&[])
			),
			Error::<Test>::NoTokenFields
		);
	});
}

#[test]
fn update_info_requires_admin() {
	new_test_ext().execute_with(|| {
		let maintainer = account(3);
		let _ = bind_account(maintainer);

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			raw(b"Initial"),
			RegistryKind::Raw,
			true,
			attrs([(b"a".as_ref(), ElementType::Raw)]),
			token_spec(&[b"a"]),
			lookup_specs(&[]),
		));

		let registry = first_registry();
		assert_ok!(Pallet::<Test>::update_registry_info(
			RuntimeOrigin::signed(maintainer),
			registry.clone(),
			raw(b"Updated")
		));

		let stored = Registries::<Test>::get(&registry).expect("stored");
		assert_eq!(stored.info, raw(b"Updated"));
	});
}

#[test]
fn status_may_be_toggled_by_admin_delegate_or_root() {
	new_test_ext().execute_with(|| {
		let maintainer = account(4);
		let maintainer_token = bind_account(maintainer);
		let delegate = account(5);
		let delegate_token = bind_account(delegate);

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			raw(b"Status"),
			RegistryKind::Hash,
			true,
			attrs([(b"x".as_ref(), ElementType::Raw)]),
			token_spec(&[b"x"]),
			lookup_specs(&[]),
		));

		let registry = first_registry();

		// Maintainer can archive
		assert_ok!(Pallet::<Test>::set_registry_status(
			RuntimeOrigin::signed(maintainer),
			registry.clone(),
			false
		));
		assert!(!Registries::<Test>::get(&registry).unwrap().is_active);

		// Delegate without admin cannot archive
		assert_ok!(Pallet::<Test>::set_registry_delegate(
			RuntimeOrigin::signed(maintainer),
			registry.clone(),
			delegate,
			vec![RegistryPermissions::ENTRY]
		));
		let delegate_perms = RegistryDelegates::<Test>::get(&registry, &delegate_token)
			.expect("delegate perms present");
		assert!(delegate_perms.has_entry());
		assert!(delegate_perms.has_view());
		assert_noop!(
			Pallet::<Test>::set_registry_status(
				RuntimeOrigin::signed(delegate),
				registry.clone(),
				true
			),
			Error::<Test>::PermissionDenied
		);

		// Grant admin and toggle
		assert_ok!(Pallet::<Test>::set_registry_delegate(
			RuntimeOrigin::signed(maintainer),
			registry.clone(),
			delegate,
			vec![RegistryPermissions::ADMIN]
		));
		assert_ok!(Pallet::<Test>::set_registry_status(
			RuntimeOrigin::signed(delegate),
			registry.clone(),
			true
		));
		assert!(Registries::<Test>::get(&registry).unwrap().is_active);

		// Root can archive regardless of delegates
		assert_ok!(Pallet::<Test>::set_registry_status(
			RuntimeOrigin::root(),
			registry.clone(),
			false
		));
		assert!(!Registries::<Test>::get(&registry).unwrap().is_active);

		// Verify admin permissions remain
		let perms =
			RegistryDelegates::<Test>::get(&registry, maintainer_token).expect("maintainer perms");
		assert!(perms.has_admin());
		assert!(perms.has_view());
		let updated_delegate_perms =
			RegistryDelegates::<Test>::get(&registry, &delegate_token).expect("delegate perms");
		assert!(updated_delegate_perms.has_admin());
		assert!(updated_delegate_perms.has_view());
	});
}

#[test]
fn delegate_lifecycle() {
	new_test_ext().execute_with(|| {
		let maintainer = account(6);
		let _ = bind_account(maintainer);
		let delegate = account(7);
		let delegate_token = bind_account(delegate);

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			raw(b"Delegation"),
			RegistryKind::Token,
			true,
			attrs([(b"k".as_ref(), ElementType::Raw)]),
			token_spec(&[b"k"]),
			lookup_specs(&[]),
		));

		let registry = first_registry();
		assert_ok!(Pallet::<Test>::set_registry_delegate(
			RuntimeOrigin::signed(maintainer),
			registry.clone(),
			delegate,
			vec![RegistryPermissions::ENTRY, RegistryPermissions::DELEGATE]
		));

		assert!(RegistryDelegates::<Test>::get(&registry, &delegate_token)
			.expect("delegate stored")
			.has_delegate());
		assert!(RegistryDelegates::<Test>::get(&registry, &delegate_token)
			.expect("delegate stored")
			.has_view());
		assert_ok!(Pallet::<Test>::remove_registry_delegate(
			RuntimeOrigin::signed(maintainer),
			registry.clone(),
			delegate_token.clone()
		));
		assert!(RegistryDelegates::<Test>::get(&registry, &delegate_token).is_none());
	});
}

#[test]
fn delegate_with_view_permission_can_view_registry() {
	new_test_ext().execute_with(|| {
		let maintainer = account(11);
		let maintainer_token = bind_account(maintainer);
		let viewer = account(12);
		let _viewer_token = bind_account(viewer);
		let info = raw(b"Visible");

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			info.clone(),
			RegistryKind::Raw,
			true,
			attrs([(b"foo".as_ref(), ElementType::Raw)]),
			token_spec(&[b"foo"]),
			lookup_specs(&[]),
		));

		let registry = first_registry();

		assert!(RegistryDelegates::<Test>::get(&registry, maintainer_token)
			.expect("maintainer perms")
			.has_view());

		assert_ok!(Pallet::<Test>::set_registry_delegate(
			RuntimeOrigin::signed(maintainer),
			registry.clone(),
			viewer,
			vec![RegistryPermissions::VIEW]
		));

		let packet = Pallet::<Test>::info(registry.clone()).expect("registry viewable");
		assert_eq!(packet.info, info);
		assert!(packet.is_active);
		assert_eq!(packet.kind, RegistryKind::Raw);

		let attribute =
			Pallet::<Test>::attribute(registry.clone(), b"foo".to_vec()).expect("attribute type");
		assert_eq!(attribute, ElementType::Raw);
	});
}

#[test]
fn info_and_attribute_available_without_delegate_permission() {
	new_test_ext().execute_with(|| {
		let maintainer = account(13);
		let _ = bind_account(maintainer);
		let delegate = account(14);
		let _delegate_token = bind_account(delegate);

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			raw(b"Hidden"),
			RegistryKind::Raw,
			true,
			attrs([(b"foo".as_ref(), ElementType::Raw)]),
			token_spec(&[b"foo"]),
			lookup_specs(&[]),
		));
		let registry = first_registry();

		assert_ok!(Pallet::<Test>::set_registry_delegate(
			RuntimeOrigin::signed(maintainer),
			registry.clone(),
			delegate,
			vec![RegistryPermissions::DELEGATE]
		));

		assert!(Pallet::<Test>::info(registry.clone()).is_some());
		assert_eq!(
			Pallet::<Test>::attribute(registry.clone(), b"foo".to_vec()),
			Some(ElementType::Raw)
		);
		// Attribute for missing key should return None.
		assert!(Pallet::<Test>::attribute(registry.clone(), b"unknown".to_vec()).is_none());
		// Any account can call these helpers; delegate permission does not gate the access.
		assert!(Pallet::<Test>::info(registry.clone()).is_some());
		assert!(Pallet::<Test>::attribute(registry, b"foo".to_vec()).is_some());
	});
}

#[test]
fn attributes_function_returns_schema() {
	new_test_ext().execute_with(|| {
		let maintainer = account(31);
		let _ = bind_account(maintainer);

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			raw(b"SchemaList"),
			RegistryKind::Raw,
			true,
			attrs([(b"alpha".as_ref(), ElementType::Raw), (b"beta".as_ref(), ElementType::Bool),]),
			token_spec(&[b"alpha"]),
			lookup_specs(&[]),
		));

		let registry = first_registry();
		let attributes = Pallet::<Test>::attributes(registry.clone()).expect("attributes present");
		assert_eq!(
			attributes,
			vec![(b"alpha".to_vec(), ElementType::Raw), (b"beta".to_vec(), ElementType::Bool)]
		);
		let tokens = Pallet::<Test>::token(registry.clone()).expect("token fields");
		assert_eq!(tokens, vec![b"alpha".to_vec()]);
		let lookups = Pallet::<Test>::lookup_specs(registry.clone()).unwrap();
		assert!(lookups.is_empty());
	});
}

#[test]
fn create_registry_rejects_duplicate_attribute_keys() {
	new_test_ext().execute_with(|| {
		let maintainer = account(32);
		let _ = bind_account(maintainer);

		let mut attributes = AttributeSchemaListOf::<Test>::default();
		let key = Attribute::try_from(b"dup".to_vec()).expect("bounded key");
		attributes.try_push((key.clone(), ElementType::Raw)).expect("first push");
		attributes.try_push((key, ElementType::Bool)).expect("second push");

		assert_noop!(
			Pallet::<Test>::create_registry(
				RuntimeOrigin::signed(maintainer),
				raw(b"Duplicate"),
				RegistryKind::Raw,
				true,
				attributes,
				token_spec(&[b"dup"]),
				lookup_specs(&[])
			),
			Error::<Test>::AttributeExists
		);
	});
}

#[test]
fn create_registry_rejects_empty_attribute_key() {
	new_test_ext().execute_with(|| {
		let maintainer = account(33);
		let _ = bind_account(maintainer);

		let mut attributes = AttributeSchemaListOf::<Test>::default();
		let empty_key = Attribute::try_from(Vec::new()).expect("bounded conversion");
		attributes.try_push((empty_key, ElementType::Raw)).expect("push");

		let mut attributes = AttributeSchemaListOf::<Test>::default();
		let empty_key = Attribute::try_from(Vec::new()).expect("bounded conversion");
		attributes.try_push((empty_key, ElementType::Raw)).expect("push");

		assert_noop!(
			Pallet::<Test>::create_registry(
				RuntimeOrigin::signed(maintainer),
				raw(b"Empty"),
				RegistryKind::Raw,
				true,
				attributes,
				token_spec(&[b"x"]),
				lookup_specs(&[])
			),
			Error::<Test>::AttributeNotFound
		);
	});
}

#[test]
fn inspector_helpers_surface_data() {
	new_test_ext().execute_with(|| {
		let maintainer = account(8);
		let maintainer_token = bind_account(maintainer);

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			raw(b"Inspector"),
			RegistryKind::Raw,
			true,
			attrs([(b"foo".as_ref(), ElementType::Raw)]),
			token_spec(&[b"foo"]),
			lookup_specs(&[]),
		));

		let registry = first_registry();
		let packet = <Pallet<Test> as RegistryInspector<Test>>::registry_packet(&registry)
			.expect("packet exists");
		assert_eq!(packet.maintainer(), &maintainer_token);

		let keys = <Pallet<Test> as RegistryInspector<Test>>::attribute_keys(&registry).unwrap();
		assert_eq!(keys, vec![b"foo".to_vec()]);

		let fields = <Pallet<Test> as RegistryInspector<Test>>::token_fields(&registry).unwrap();
		assert_eq!(fields, vec![b"foo".to_vec()]);
		let lookups = <Pallet<Test> as RegistryInspector<Test>>::lookup_specs(&registry).unwrap();
		assert!(lookups.is_empty());
	});
}

#[test]
fn registry_supports_lookup_combinations() {
	new_test_ext().execute_with(|| {
		let maintainer = account(9);
		let _ = bind_account(maintainer);

		assert_ok!(Pallet::<Test>::create_registry(
			RuntimeOrigin::signed(maintainer),
			raw(b"Combos"),
			RegistryKind::Raw,
			true,
			attrs([(b"foo".as_ref(), ElementType::Raw), (b"baz".as_ref(), ElementType::Raw),]),
			token_spec(&[b"foo", b"baz"]),
			lookup_specs(&[&[b"foo"], &[b"foo", b"baz"]]),
		));

		let registry = first_registry();
		let lookups = Pallet::<Test>::lookup_specs(registry.clone()).unwrap();
		assert_eq!(lookups, vec![vec![b"foo".to_vec()], vec![b"foo".to_vec(), b"baz".to_vec()]]);
		let token_keys = Pallet::<Test>::token(registry).unwrap();
		assert_eq!(token_keys, vec![b"foo".to_vec(), b"baz".to_vec()]);
	});
}
