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

#[test]
fn create_collection_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		assert_eq!(Collections::<Test>::iter().count(), 1);
		let collection_id = get_collection_id();
		let perm = CollectionDelegates::<Test>::get(&collection_id, &creator);
		assert!(perm.is_some());
	});
}

#[test]
fn create_collection_should_fail_if_duplicate() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		assert_noop!(
			Collection::create_collection(RawOrigin::Signed(creator).into()),
			Error::<Test>::CollectionAlreadyExists
		);
	});
}

#[test]
fn add_collection_delegate_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let delegate: u64 = 2;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_ok!(Collection::add_collection_delegate(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			delegate,
			Permissions::all()
		));
		let perm = CollectionDelegates::<Test>::get(&collection_id, &delegate);
		assert!(perm.is_some());
	});
}

#[test]
fn add_collection_delegate_should_fail_for_unauthorized() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let non_authorized: u64 = 3;
		let delegate: u64 = 2;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_noop!(
			Collection::add_collection_delegate(
				RawOrigin::Signed(non_authorized).into(),
				collection_id.clone(),
				delegate,
				Permissions::all()
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
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_ok!(Collection::add_collection_delegate(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			delegate,
			Permissions::all()
		));
		assert_ok!(Collection::remove_collection_delegate(
			RawOrigin::Signed(creator).into(),
			collection_id.clone(),
			delegate
		));
		assert!(CollectionDelegates::<Test>::get(&collection_id, &delegate).is_none());
	});
}

#[test]
fn remove_collection_delegate_should_fail_if_delegate_not_found() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let delegate: u64 = 2;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_noop!(
			Collection::remove_collection_delegate(
				RawOrigin::Signed(creator).into(),
				collection_id.clone(),
				delegate
			),
			Error::<Test>::DelegateNotFound
		);
	});
}

#[test]
fn archive_collection_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_ok!(Collection::archive_collection(
			RawOrigin::Signed(creator).into(),
			collection_id.clone()
		));
		let collection = Collections::<Test>::get(&collection_id).unwrap();
		assert_eq!(collection.status, Status::Archived);
	});
}

#[test]
fn archive_collection_should_fail_for_unauthorized() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let unauthorized: u64 = 3;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_noop!(
			Collection::archive_collection(
				RawOrigin::Signed(unauthorized).into(),
				collection_id.clone()
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn restore_collection_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_ok!(Collection::archive_collection(
			RawOrigin::Signed(creator).into(),
			collection_id.clone()
		));
		assert_ok!(Collection::restore_collection(
			RawOrigin::Signed(creator).into(),
			collection_id.clone()
		));
		let collection = Collections::<Test>::get(&collection_id).unwrap();
		assert_eq!(collection.status, Status::Active);
	});
}

#[test]
fn restore_collection_should_fail_if_not_archived() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
		let collection_id = get_collection_id();
		assert_noop!(
			Collection::restore_collection(
				RawOrigin::Signed(creator).into(),
				collection_id.clone()
			),
			Error::<Test>::CollectionNotArchived
		);
	});
}

#[test]
fn add_registry_should_work() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
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
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
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
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
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
		assert_ok!(Collection::create_collection(RawOrigin::Signed(creator).into()));
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
