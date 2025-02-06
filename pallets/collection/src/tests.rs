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
use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_core::H256;
use sp_std::prelude::*;

/// Helper function to extract the collection identifier.
fn get_collection_id() -> CollectionIdentifierOf {
	Collections::<Test>::iter().next().expect("A collection should exist").0
}

// Helper: Create a registry and return its identifier.
fn create_collection() -> CollectionIdentifierOf {
	let creator: u64 = 1;
	assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
	Collections::<Test>::iter().next().expect("Registry should exist").0
}

/// Helper: Assign delegate permission (without admin) to a given account on a registry.
fn set_delegate_permission(registry_id: &CollectionIdentifierOf, account: u64) {
	let delegate_perms = Permissions::from_variants(&[PermissionVariant::Delegate]);
	Delegates::<Test>::insert(registry_id, &account, delegate_perms);
}

#[test]
fn create_collection_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		assert_eq!(Collections::<Test>::iter().count(), 1);
		let collection_id = get_collection_id();
		let perm = Delegates::<Test>::get(&collection_id, &creator);
		assert!(perm.is_some());
	});
}

#[test]
fn create_collection_should_fail_if_duplicate() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		assert_noop!(
			Collection::create(RawOrigin::Signed(creator).into()),
			Error::<Test>::CollectionAlreadyExists
		);
	});
}

#[test]
fn add_collection_delegate_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let delegate: u64 = 2;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		let permission_variants =
			vec![PermissionVariant::Entry, PermissionVariant::Delegate, PermissionVariant::Admin];
		assert_ok!(Collection::add_delegate(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			delegate,
			permission_variants
		));
		assert!(Delegates::<Test>::contains_key(&collection_id, &delegate));
	});
}

#[test]
fn add_collection_delegate_should_fail_for_unauthorized() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let non_authorized: u64 = 3;
		let delegate: u64 = 2;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		let permission_variants =
			vec![PermissionVariant::Entry, PermissionVariant::Delegate, PermissionVariant::Admin];
		assert_noop!(
			Collection::add_delegate(
				RawOrigin::Signed(non_authorized).into(),
				collection_id.clone(),
				delegate,
				permission_variants
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn remove_collection_delegate_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let delegate: u64 = 2;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		let permission_variants =
			vec![PermissionVariant::Entry, PermissionVariant::Delegate, PermissionVariant::Admin];
		assert_ok!(Collection::add_delegate(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			delegate,
			permission_variants
		));
		assert_ok!(Collection::remove_delegate(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			delegate
		));
		assert!(Delegates::<Test>::get(&collection_id, &delegate).is_none());
	});
}

#[test]
fn remove_collection_delegate_should_fail_if_delegate_not_found() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let delegate: u64 = 2;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_noop!(
			Collection::remove_delegate(
				RawOrigin::Signed(creator).into(),
				collection_id.clone(),
				delegate
			),
			Error::<Test>::DelegateNotFound
		);
	});
}

#[test]
fn add_delegate_should_work_with_delegate_permission() {
	new_test_ext().execute_with(|| {
		let collection_id = create_collection();
		let delegate_caller: u64 = 2;
		set_delegate_permission(&collection_id, delegate_caller);

		let new_delegate: u64 = 3;
		let permission_variants = vec![PermissionVariant::Entry];
		assert_ok!(Collection::add_delegate(
			RawOrigin::Signed(delegate_caller).into(),
			collection_id.clone(),
			new_delegate,
			permission_variants
		));
		assert!(Delegates::<Test>::contains_key(&collection_id, &new_delegate));
	});
}

#[test]
fn archive_collection_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_ok!(Collection::archive(RawOrigin::Signed(creator).into(), collection_id.clone()));
		let collection = Collections::<Test>::get(&collection_id).unwrap();
		assert_eq!(collection.status, Status::Archived);
	});
}

#[test]
fn archive_collection_should_fail_for_unauthorized() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let unauthorized: u64 = 3;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_noop!(
			Collection::archive(RawOrigin::Signed(unauthorized).into(), collection_id.clone()),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn restore_collection_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_ok!(Collection::archive(RawOrigin::Signed(creator).into(), collection_id.clone()));
		assert_ok!(Collection::restore(RawOrigin::Signed(creator).into(), collection_id.clone()));
		let collection = Collections::<Test>::get(&collection_id).unwrap();
		assert_eq!(collection.status, Status::Active);
	});
}

#[test]
fn restore_collection_should_fail_if_not_archived() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_noop!(
			Collection::restore(RawOrigin::Signed(creator).into(), collection_id.clone()),
			Error::<Test>::CollectionNotArchived
		);
	});
}

#[test]
fn add_registry_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		let registry_id: RegistryIdentifierOf =
			H256::random().as_fixed_bytes().to_vec().try_into().unwrap();
		assert_ok!(Collection::add_registry(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			registry_id.clone()
		));
		assert!(CollectionRegistries::<Test>::contains_key(&collection_id, &registry_id));
	});
}

#[test]
fn add_registry_should_fail_if_duplicate() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		let registry_id: RegistryIdentifierOf =
			H256::random().as_fixed_bytes().to_vec().try_into().unwrap();
		assert_ok!(Collection::add_registry(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			registry_id.clone()
		));
		assert_noop!(
			Collection::add_registry(
				RawOrigin::Signed(creator).into(),
				collection_id.clone(),
				registry_id.clone()
			),
			Error::<Test>::RegistryAlreadyExists
		);
	});
}

#[test]
fn remove_registry_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		let registry_id: RegistryIdentifierOf =
			H256::random().as_fixed_bytes().to_vec().try_into().unwrap();
		assert_ok!(Collection::add_registry(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			registry_id.clone()
		));
		assert_ok!(Collection::remove_registry(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			registry_id.clone()
		));
		assert!(!CollectionRegistries::<Test>::contains_key(&collection_id, &registry_id));
	});
}

#[test]
fn remove_registry_should_fail_if_nonexistent() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		let registry_id: RegistryIdentifierOf =
			H256::random().as_fixed_bytes().to_vec().try_into().unwrap();
		assert_noop!(
			Collection::remove_registry(
				RawOrigin::Signed(creator).into(),
				collection_id.clone(),
				registry_id.clone()
			),
			Error::<Test>::RegistryNotFound
		);
	});
}
