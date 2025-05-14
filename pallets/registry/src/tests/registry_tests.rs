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
use crate::{BoundedVec, Delegates, Error, Permissions, Registries, RegistryIdentifierOf, Status};
use frame_support::{assert_noop, assert_ok};
use frame_system::{self as system, RawOrigin};
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
fn create_registry(creator: u64, doc_author: u64) -> RegistryIdentifierOf {
	create_profile(creator);
	create_profile(doc_author);
	let creator_profile_id = pallet_profile::Pallet::<Test>::get_profile_id(&creator)
		.expect("Creator profile ID should exist");
	let doc_author_profile_id = pallet_profile::Pallet::<Test>::get_profile_id(&doc_author)
		.expect("Doc author profile ID should exist");
	let tx_hash = H256::random();
	let doc_id = b"doc_id".to_vec();
	let doc_node_id = b"doc_node_id".to_vec();
	assert_ok!(Registry::create_store(
		RawOrigin::Signed(creator).into(),
		tx_hash,
		doc_id.clone(),
		doc_author,
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
		Some(doc_author_profile_id.clone()),
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

	// Assert doc author permissions (if different from creator)
	if creator != doc_author {
		assert!(
			Delegates::<Test>::contains_key(&registry_id, &doc_author_profile_id),
			"Doc author should have permissions"
		);
		assert_eq!(
			Delegates::<Test>::get(&registry_id, &doc_author_profile_id).unwrap(),
			Permissions::ENTRY,
			"Doc author should have ENTRY permissions"
		);
	}

	// Assert event emission
	system::Pallet::<Test>::assert_last_event(
		crate::Event::RegistryStoreCreated {
			registry: registry_id.clone(),
			creator,
			profile_id: creator_profile_id,
		}
		.into(),
	);

	registry_id
}

#[test]
fn create_registry_positive() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let doc_author: u64 = 2;
		let _registry_id = create_registry(creator, doc_author);
		assert_eq!(Registries::<Test>::iter().count(), 1);
	});
}

#[test]
fn create_registry_duplicate_should_fail() {
	new_test_ext().execute_with(|| {
		let creator: u64 = 1;
		let doc_author: u64 = 2;
		let tx_hash = H256::random();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();

		create_profile(creator);
		create_profile(doc_author);
		assert_ok!(Registry::create_store(
			RawOrigin::Signed(creator).into(),
			tx_hash,
			doc_id.clone(),
			doc_author,
			doc_node_id.clone()
		));
		assert_noop!(
			Registry::create_store(
				RawOrigin::Signed(creator).into(),
				tx_hash,
				doc_id,
				doc_author,
				doc_node_id
			),
			Error::<Test>::RegistryAlreadyExists
		);
	});
}

#[test]
fn archive_registry_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let doc_author: u64 = 2;
		let registry_id = create_registry(admin, doc_author);
		assert_ok!(Registry::archive(RawOrigin::Signed(admin).into(), registry_id.clone()));
		let registry = Registries::<Test>::get(registry_id).unwrap();
		assert_eq!(registry.status, Status::Archived);
	});
}

#[test]
fn archive_registry_negative_unauthorized() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let non_admin: u64 = 3;
		let doc_author: u64 = 2;
		let registry_id = create_registry(admin, doc_author);
		create_profile(non_admin);
		assert_noop!(
			Registry::archive(RawOrigin::Signed(non_admin).into(), registry_id.clone()),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn restore_registry_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let doc_author: u64 = 2;
		let registry_id = create_registry(admin, doc_author);
		assert_ok!(Registry::archive(RawOrigin::Signed(admin).into(), registry_id.clone()));
		assert_ok!(Registry::restore(RawOrigin::Signed(admin).into(), registry_id.clone()));
		let registry = Registries::<Test>::get(registry_id).unwrap();
		assert_eq!(registry.status, Status::Active);
	});
}

#[test]
fn restore_registry_negative_if_not_archived() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let doc_author: u64 = 2;
		let registry_id = create_registry(admin, doc_author);
		assert_noop!(
			Registry::restore(RawOrigin::Signed(admin).into(), registry_id.clone()),
			Error::<Test>::RegistryNotArchived
		);
	});
}

#[test]
fn update_registry_author_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let doc_author: u64 = 2;
		let new_author: u64 = 3;
		let registry_id = create_registry(admin, doc_author);
		create_profile(new_author);
		let new_author_profile_id = pallet_profile::Pallet::<Test>::get_profile_id(&new_author)
			.expect("New author profile ID should exist");
		assert_ok!(Registry::update_author(
			RawOrigin::Signed(admin).into(),
			registry_id.clone(),
			new_author
		));
		let registry = Registries::<Test>::get(registry_id).unwrap();
		assert_eq!(registry.doc_author_profile_id, Some(new_author_profile_id));
	});
}

#[test]
fn update_registry_author_negative_unauthorized() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let doc_author: u64 = 2;
		let non_admin: u64 = 3;
		let registry_id = create_registry(admin, doc_author);
		create_profile(non_admin);
		assert_noop!(
			Registry::update_author(
				RawOrigin::Signed(non_admin).into(),
				registry_id.clone(),
				non_admin
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn update_registry_creator_positive() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let doc_author: u64 = 2;
		let new_creator: u64 = 3;
		let registry_id = create_registry(admin, doc_author);
		create_profile(new_creator);
		let new_creator_profile_id = pallet_profile::Pallet::<Test>::get_profile_id(&new_creator)
			.expect("New creator profile ID should exist");
		assert_ok!(Registry::update_creator(
			RawOrigin::Signed(admin).into(),
			registry_id.clone(),
			new_creator
		));
		let registry = Registries::<Test>::get(registry_id).unwrap();
		assert_eq!(registry.creator, new_creator_profile_id);
	});
}

#[test]
fn update_registry_creator_negative_unauthorized() {
	new_test_ext().execute_with(|| {
		let admin: u64 = 1;
		let doc_author: u64 = 2;
		let non_admin: u64 = 3;
		let registry_id = create_registry(admin, doc_author);
		create_profile(non_admin);
		assert_noop!(
			Registry::update_creator(
				RawOrigin::Signed(non_admin).into(),
				registry_id.clone(),
				non_admin
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}
