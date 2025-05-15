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

#[cfg(test)]
use crate::mock::*;
use crate::{
	BoundedVec, Delegates, Error, PermissionVariant, Permissions, Registries, RegistryIdentifierOf,
	Status,
};
use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_core::H256;

// Helper: Create a profile for an account
fn create_profile(account: u64) {
	let mut data = vec![];
	let key = b"pub_name".to_vec();
	let value = BoundedVec::try_from(b"test".to_vec()).expect("Value should fit");
	data.push((key.try_into().expect("Key should fit"), value));
	assert_ok!(Profile::set_profile(RawOrigin::Signed(account).into(), data,));
}

// Helper: Create a registry and verify its contents
fn create_registry(creator: u64) -> RegistryIdentifierOf {
	create_profile(creator);
	let creator_profile_id = pallet_profile::Pallet::<Test>::get_profile_id(&creator)
		.expect("Creator profile ID should exist");
	let tx_hash = H256::random();
	let doc_id = b"doc_id".to_vec();
	let doc_node_id = b"doc_node_id".to_vec();
	assert_ok!(Registry::create_store(
		RawOrigin::Signed(creator).into(),
		tx_hash,
		doc_id.clone(),
		creator,
		doc_node_id.clone()
	));
	let registry_id = Registries::<Test>::iter().next().expect("Registry should exist").0;

	// Restore creator permissions to Permissions::all()
	Delegates::<Test>::insert(&registry_id, &creator_profile_id, Permissions::all());

	// Assert registry contents
	let registry = Registries::<Test>::get(&registry_id).expect("Registry should exist");
	assert_eq!(registry.status, Status::Active, "Registry should be active");
	assert_eq!(
		registry.doc_id,
		Some(BoundedVec::try_from(doc_id).expect("Doc ID should fit")),
		"Doc ID should match"
	);
	assert_eq!(
		registry.doc_node_id,
		Some(BoundedVec::try_from(doc_node_id).expect("Doc node ID should fit")),
		"Doc node ID should match"
	);
	assert_eq!(registry.tx_hash, tx_hash, "Transaction hash should match");
	assert_eq!(registry.creator, creator_profile_id, "Creator should match");
	assert_eq!(
		registry.doc_author_profile_id,
		Some(creator_profile_id.clone()),
		"Doc author should match"
	);

	// Assert creator permissions
	assert!(
		Delegates::<Test>::contains_key(&registry_id, &creator_profile_id),
		"Creator should have permissions"
	);
	assert_eq!(
		Delegates::<Test>::get(&registry_id, &creator_profile_id).unwrap(),
		Permissions::all(),
		"Creator should have all permissions"
	);

	registry_id
}

// Helper: Assign delegate permission to a given profile on a registry
fn set_delegate_permission(registry_id: &RegistryIdentifierOf, delegate: u64) {
	create_profile(delegate);
	let delegate_profile_id =
		pallet_profile::Pallet::<Test>::get_profile_id(&delegate).expect("Profile ID should exist");
	let delegate_perms = Permissions::from_variants(&[PermissionVariant::Delegate]);
	Delegates::<Test>::insert(registry_id, &delegate_profile_id, delegate_perms);
}

#[test]
fn add_delegate_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let delegate: u64 = 2;
		let reg_id = create_registry(admin);
		let permission_variants = vec![PermissionVariant::Entry, PermissionVariant::Delegate];

		create_profile(delegate);
		let delegate_profile_id = pallet_profile::Pallet::<Test>::get_profile_id(&delegate)
			.expect("Profile ID should exist");

		assert_ok!(Registry::add_delegate(
			RawOrigin::Signed(admin).into(),
			reg_id.clone(),
			delegate,
			permission_variants
		));
		assert!(Delegates::<Test>::contains_key(reg_id, &delegate_profile_id));
	});
}

#[test]
fn add_delegate_negative_unauthorized() {
	new_test_ext().execute_with(|| {
		let non_admin: u64 = 3;
		let delegate: u64 = 2;
		let admin: u64 = 1;
		let reg_id = create_registry(admin);
		let permission_variants = vec![PermissionVariant::Entry];

		create_profile(delegate);
		create_profile(non_admin);

		assert_noop!(
			Registry::add_delegate(
				RawOrigin::Signed(non_admin).into(),
				reg_id.clone(),
				delegate,
				permission_variants
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn add_delegate_negative_no_profile() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let delegate: u64 = 2;
		let reg_id = create_registry(admin);
		let permission_variants = vec![PermissionVariant::Entry];

		// Do not create profile for delegate
		assert_noop!(
			Registry::add_delegate(
				RawOrigin::Signed(admin).into(),
				reg_id.clone(),
				delegate,
				permission_variants
			),
			pallet_profile::Error::<Test>::ProfileNotFound
		);
	});
}

#[test]
fn add_delegate_negative_already_exists() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let delegate: u64 = 2;
		let reg_id = create_registry(admin);
		let permission_variants = vec![PermissionVariant::Entry];

		create_profile(delegate);
		assert_ok!(Registry::add_delegate(
			RawOrigin::Signed(admin).into(),
			reg_id.clone(),
			delegate,
			permission_variants.clone()
		));

		assert_err!(
			Registry::add_delegate(
				RawOrigin::Signed(admin).into(),
				reg_id.clone(),
				delegate,
				permission_variants
			),
			Error::<Test>::DelegateAlreadyExists
		);
	});
}

#[test]
fn remove_delegate_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let delegate: u64 = 2;
		let reg_id = create_registry(admin);
		set_delegate_permission(&reg_id, delegate);
		let delegate_profile_id = pallet_profile::Pallet::<Test>::get_profile_id(&delegate)
			.expect("Profile ID should exist");

		assert_ok!(Registry::remove_delegate(
			RawOrigin::Signed(admin).into(),
			reg_id.clone(),
			delegate
		));
		assert!(!Delegates::<Test>::contains_key(reg_id, &delegate_profile_id));
	});
}

#[test]
fn remove_delegate_negative_not_found() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let delegate: u64 = 2;
		let reg_id = create_registry(admin);

		create_profile(delegate);

		assert_noop!(
			Registry::remove_delegate(RawOrigin::Signed(admin).into(), reg_id.clone(), delegate),
			Error::<Test>::DelegateNotFound
		);
	});
}

#[test]
fn remove_delegate_negative_unauthorized() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let non_admin: u64 = 3;
		let delegate: u64 = 2;
		let reg_id = create_registry(admin);
		set_delegate_permission(&reg_id, delegate);

		create_profile(non_admin);

		assert_noop!(
			Registry::remove_delegate(
				RawOrigin::Signed(non_admin).into(),
				reg_id.clone(),
				delegate
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn remove_delegate_negative_no_profile() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let delegate: u64 = 2;
		let reg_id = create_registry(admin);

		// Do not create profile for delegate
		assert_noop!(
			Registry::remove_delegate(RawOrigin::Signed(admin).into(), reg_id.clone(), delegate),
			pallet_profile::Error::<Test>::ProfileNotFound
		);
	});
}
