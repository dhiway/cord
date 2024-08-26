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

use super::*;
use crate::mock::*;
use codec::Encode;
//use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_ok, BoundedVec};
//use frame_system::RawOrigin;
use sp_runtime::traits::Hash;
use sp_std::prelude::*;
//use cord_utilities::mock::mock_origin::Origin;
use serde_json::json;

/// Generates a Registry ID
pub fn generate_registry_id<T: Config>(id_digest: &RegistryHashOf<T>) -> RegistryIdOf {
	let registry_id: RegistryIdOf =
		Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::DeDir)
			.expect("Registry Identifier creation failed.");

	registry_id
}

/// Generates a Registry Entry ID
pub fn generate_registry_entry_id<T: Config>(id_digest: &RegistryHashOf<T>) -> RegistryEntryIdOf {
	let registry_entry_id: RegistryEntryIdOf =
		Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::DeDir)
			.expect("Registry Entry Identifier creation failed");

	registry_entry_id
}

pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);
pub(crate) const ACCOUNT_01: AccountId = AccountId::new([2u8; 32]);

#[test]
fn create_registry_should_work() {
	new_test_ext().execute_with(|| {
		let creator = ACCOUNT_00;

		let json_object = json!({
			"name": "String",
			"age": "Number",
			"email": "String",
			"isActive": "Boolean",
			"address": {
				"street": "String",
				"city": "String",
				"zipcode": "Number"
			},
			"phoneNumbers": [
				"Number",
				"Number"
			],
		});

		let json_string = serde_json::to_string(&json_object).expect("Failed to serialize JSON");

		let raw_bytes = json_string.as_bytes().to_vec();

		let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_bytes.clone()).expect(
			"Test Blob should fit into the expected input length of BLOB for the test runtime.",
		);

		let digest: RegistryHashOf<Test> =
			<Test as frame_system::Config>::Hashing::hash(&raw_bytes.encode()[..]);

		let id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
		);

		let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

		/* Test creation of a Registry */
		assert_ok!(DeDir::create_registry(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			digest,
			Some(blob.clone()),
		));

		/* Check if the Registry was created */
		assert!(Registries::<Test>::contains_key(registry_id.clone()));
		let registry = Registries::<Test>::get(registry_id.clone()).unwrap();

		/* Check for values stored are correct */
		assert_eq!(registry.digest, digest);
		assert_eq!(registry.blob, blob.clone());
		assert_eq!(registry.owner, creator);

		/* Check for Delegates */
		let delegates = DelegatesList::<Test>::get(registry_id.clone());
		assert!(delegates
			.unwrap()
			.entries
			.iter()
			.any(|d| d.delegate == creator && d.permissions == Permissions::OWNER));

		/* Check for successfull event emission of CreatedRegistry */
		System::assert_last_event(
			Event::CreatedRegistry { creator: creator.clone(), registry_id: registry_id.clone() }
				.into(),
		);
	});
}

#[test]
fn create_registry_entry_should_work() {
	let creator = ACCOUNT_00;

	/* Assumed Json for Registry (schema) */
	let registry_json_object = json!({
		"name": "String",
		"age": "Number",
		"email": "String",
		"isActive": "Boolean",
		"address": {
			"street": "String",
			"city": "String",
			"zipcode": "Number"
		},
		"phoneNumbers": [
			"Number",
			"Number"
		],
	});

	let registry_json_string =
		serde_json::to_string(&registry_json_object).expect("Failed to serialize JSON");

	let registry_raw_bytes = registry_json_string.as_bytes().to_vec();

	let registry_blob: RegistryBlobOf<Test> = BoundedVec::try_from(registry_raw_bytes.clone())
		.expect(
			"Test Blob should fit into the expected input length of BLOB for the test runtime.",
		);

	let registry_digest: RegistryHashOf<Test> =
		<Test as frame_system::Config>::Hashing::hash(&registry_raw_bytes.encode()[..]);

	let registry_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &registry_digest.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&registry_id_digest);

	/* Assumed JSON for Registry Entry (record) */
	let registry_entry_json_object = json!({
		"name": "Alice",
		"age": 25,
		"email": "alice@dhiway.com",
		"isActive": true,
		"address": {
			"street": "M.G ROAD",
			"city": "Bengaluru",
			"zipcode": "560001"
		},
		"phoneNumbers": [
			"+91-234787324",
			"+91-283746823"
		]
	});

	let registry_entry_json_string =
		serde_json::to_string(&registry_entry_json_object).expect("Failed to serialize JSON");

	let registry_entry_raw_bytes = registry_entry_json_string.as_bytes().to_vec();

	let registry_entry_blob: RegistryBlobOf<Test> = BoundedVec::try_from(
		registry_entry_raw_bytes.clone(),
	)
	.expect("Test Blob should fit into the expected input length of BLOB for the test runtime.");

	let registry_entry_digest: RegistryHashOf<Test> =
		<Test as frame_system::Config>::Hashing::hash(&registry_entry_raw_bytes.encode()[..]);

	let registry_entry_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &registry_entry_digest.encode()[..]].concat()[..],
	);

	let registry_entry_id: RegistryIdOf =
		generate_registry_entry_id::<Test>(&registry_entry_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(DeDir::create_registry(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(registry_blob.clone()),
		));

		/* Test creation of a Registry Entry */
		assert_ok!(DeDir::create_registry_entry(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_entry_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
			None,
		));

		/* Check if the registry entry was created */
		let registry_entry =
			RegistryEntries::<Test>::get(registry_id.clone(), registry_entry_id.clone());
		assert!(registry_entry.is_some());

		/* Check the stored values in TEST chain-state */
		let registry_entry = registry_entry.unwrap();
		assert_eq!(registry_entry.digest, registry_entry_digest);
		assert_eq!(registry_entry.blob, registry_entry_blob);
		assert_eq!(registry_entry.current_state, RegistrySupportedStateOf::DRAFT);

		/* Check for successfull event emission of CreatedRegistryEntry */
		System::assert_last_event(
			Event::CreatedRegistryEntry {
				creator: creator.clone(),
				registry_id: registry_id.clone(),
				registry_entry_id: registry_entry_id.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn registry_entry_state_updation_should_work() {
	let creator = ACCOUNT_00;

	/* Assumed Json for Registry (schema) */
	let registry_json_object = json!({
		"name": "String",
		"age": "Number",
		"email": "String",
		"isActive": "Boolean",
		"address": {
			"street": "String",
			"city": "String",
			"zipcode": "Number"
		},
		"phoneNumbers": [
			"Number",
			"Number"
		],
	});

	let registry_json_string =
		serde_json::to_string(&registry_json_object).expect("Failed to serialize JSON");

	let registry_raw_bytes = registry_json_string.as_bytes().to_vec();

	let registry_blob: RegistryBlobOf<Test> = BoundedVec::try_from(registry_raw_bytes.clone())
		.expect(
			"Test Blob should fit into the expected input length of BLOB for the test runtime.",
		);

	let registry_digest: RegistryHashOf<Test> =
		<Test as frame_system::Config>::Hashing::hash(&registry_raw_bytes.encode()[..]);

	let registry_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &registry_digest.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&registry_id_digest);

	/* Assumed JSON for Registry Entry (record) */
	let registry_entry_json_object = json!({
		"name": "Alice",
		"age": 25,
		"email": "alice@dhiway.com",
		"isActive": true,
		"address": {
			"street": "M.G ROAD",
			"city": "Bengaluru",
			"zipcode": "560001"
		},
		"phoneNumbers": [
			"+91-234787324",
			"+91-283746823"
		]
	});

	let registry_entry_json_string =
		serde_json::to_string(&registry_entry_json_object).expect("Failed to serialize JSON");

	let registry_entry_raw_bytes = registry_entry_json_string.as_bytes().to_vec();

	let registry_entry_blob: RegistryBlobOf<Test> = BoundedVec::try_from(
		registry_entry_raw_bytes.clone(),
	)
	.expect("Test Blob should fit into the expected input length of BLOB for the test runtime.");

	let registry_entry_digest: RegistryHashOf<Test> =
		<Test as frame_system::Config>::Hashing::hash(&registry_entry_raw_bytes.encode()[..]);

	let registry_entry_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &registry_entry_digest.encode()[..]].concat()[..],
	);

	let registry_entry_id: RegistryIdOf =
		generate_registry_entry_id::<Test>(&registry_entry_id_digest);

	let new_state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(DeDir::create_registry(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(registry_blob.clone()),
		));

		/* Test creation of a Registry Entry */
		assert_ok!(DeDir::create_registry_entry(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_entry_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
			None,
		));

		/* Check if the registry entry was created */
		let registry_entry =
			RegistryEntries::<Test>::get(registry_id.clone(), registry_entry_id.clone());
		assert!(registry_entry.is_some());

		/* Check the stored values in TEST chain-state */
		let registry_entry = registry_entry.unwrap();
		assert_eq!(registry_entry.digest, registry_entry_digest);
		assert_eq!(registry_entry.blob, registry_entry_blob);
		assert_eq!(registry_entry.current_state, RegistrySupportedStateOf::DRAFT);

		/* Test change of a Registry Entry State from DRAFT to ACTIVE */
		assert_ok!(DeDir::registry_entry_state_change(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_entry_id.clone(),
			new_state.clone(),
		));

		/* Test if state of Registry Entry has been updated to ACTIVE */
		let updated_registry_entry =
			RegistryEntries::<Test>::get(registry_id.clone(), registry_entry_id.clone());
		assert!(updated_registry_entry.is_some());

		let updated_registry_entry = updated_registry_entry.unwrap();
		assert_eq!(updated_registry_entry.current_state, new_state);

		/* Check for successfull event emission of CreatedRegistryEntry */
		System::assert_last_event(
			Event::RegistryEntryStateChanged {
				who: creator.clone(),
				registry_id: registry_id.clone(),
				registry_entry_id: registry_entry_id.clone(),
				new_state: new_state.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn add_delegate_should_work() {
	let creator = ACCOUNT_00;

	/* Assumed Json for Registry (schema) */
	let registry_json_object = json!({
		"name": "String",
		"age": "Number",
		"email": "String",
		"isActive": "Boolean",
		"address": {
			"street": "String",
			"city": "String",
			"zipcode": "Number"
		},
		"phoneNumbers": [
			"Number",
			"Number"
		],
	});

	let registry_json_string =
		serde_json::to_string(&registry_json_object).expect("Failed to serialize JSON");

	let registry_raw_bytes = registry_json_string.as_bytes().to_vec();

	let registry_blob: RegistryBlobOf<Test> = BoundedVec::try_from(registry_raw_bytes.clone())
		.expect(
			"Test Blob should fit into the expected input length of BLOB for the test runtime.",
		);

	let registry_digest: RegistryHashOf<Test> =
		<Test as frame_system::Config>::Hashing::hash(&registry_raw_bytes.encode()[..]);

	let registry_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &registry_digest.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&registry_id_digest);

	let delegator = creator.clone();

	let delegate = ACCOUNT_01;

	let permission = Permissions::ADMIN;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(DeDir::create_registry(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(registry_blob.clone()),
		));

		/* Test for addition of a delegate */
		assert_ok!(DeDir::add_delegate(
			frame_system::RawOrigin::Signed(delegator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			permission,
		));

		/* Check if the delegate was added successfully */
		let delegates = DelegatesList::<Test>::get(registry_id.clone());
		assert!(delegates.is_some());

		let delegates = delegates.unwrap();
		assert!(delegates
			.entries
			.iter()
			.any(|d| d.delegate == delegate && d.permissions == permission));

		/* Check for successfull event emission of RegistryDelegateAdded */
		System::assert_last_event(
			Event::RegistryDelegateAdded {
				delegator: delegator.clone(),
				registry_id: registry_id.clone(),
				delegate: delegate.clone(),
				permission,
			}
			.into(),
		);
	});
}

#[test]
fn remove_delegate_should_work() {
	let creator = ACCOUNT_00;

	/* Assumed Json for Registry (schema) */
	let registry_json_object = json!({
		"name": "String",
		"age": "Number",
		"email": "String",
		"isActive": "Boolean",
		"address": {
			"street": "String",
			"city": "String",
			"zipcode": "Number"
		},
		"phoneNumbers": [
			"Number",
			"Number"
		],
	});

	let registry_json_string =
		serde_json::to_string(&registry_json_object).expect("Failed to serialize JSON");

	let registry_raw_bytes = registry_json_string.as_bytes().to_vec();

	let registry_blob: RegistryBlobOf<Test> = BoundedVec::try_from(registry_raw_bytes.clone())
		.expect(
			"Test Blob should fit into the expected input length of BLOB for the test runtime.",
		);

	let registry_digest: RegistryHashOf<Test> =
		<Test as frame_system::Config>::Hashing::hash(&registry_raw_bytes.encode()[..]);

	let registry_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &registry_digest.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&registry_id_digest);

	let delegator = creator.clone();

	let delegate = ACCOUNT_01;

	let permission = Permissions::ADMIN;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(DeDir::create_registry(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(registry_blob.clone()),
		));

		/* Test for addition of a delegate */
		assert_ok!(DeDir::add_delegate(
			frame_system::RawOrigin::Signed(delegator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			permission,
		));

		/* Test for removal of delegate */
		assert_ok!(DeDir::remove_delegate(
			frame_system::RawOrigin::Signed(delegator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));

		/* Check if the delegate was removed successfully */
		let delegates = DelegatesList::<Test>::get(registry_id.clone());
		assert!(delegates.is_some());
		let delegates = delegates.unwrap();

		assert!(!delegates.entries.iter().any(|d| d.delegate == delegate));

		/* Check for successfull event emission of RegistryDelegateRemoved */
		System::assert_last_event(
			Event::RegistryDelegateRemoved {
				delegator: delegator.clone(),
				registry_id: registry_id.clone(),
				delegate: delegate.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn update_delegate_permission_should_work() {
	let creator = ACCOUNT_00;

	/* Assumed Json for Registry (schema) */
	let registry_json_object = json!({
		"name": "String",
		"age": "Number",
		"email": "String",
		"isActive": "Boolean",
		"address": {
			"street": "String",
			"city": "String",
			"zipcode": "Number"
		},
		"phoneNumbers": [
			"Number",
			"Number"
		],
	});

	let registry_json_string =
		serde_json::to_string(&registry_json_object).expect("Failed to serialize JSON");

	let registry_raw_bytes = registry_json_string.as_bytes().to_vec();

	let registry_blob: RegistryBlobOf<Test> = BoundedVec::try_from(registry_raw_bytes.clone())
		.expect(
			"Test Blob should fit into the expected input length of BLOB for the test runtime.",
		);

	let registry_digest: RegistryHashOf<Test> =
		<Test as frame_system::Config>::Hashing::hash(&registry_raw_bytes.encode()[..]);

	let registry_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &registry_digest.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&registry_id_digest);

	let delegator = creator.clone();

	let delegate = ACCOUNT_01;

	let permission = Permissions::DELEGATE;

	let new_permission = Permissions::ADMIN;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(DeDir::create_registry(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(registry_blob.clone()),
		));

		/* Test for addition of a delegate */
		assert_ok!(DeDir::add_delegate(
			frame_system::RawOrigin::Signed(delegator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			permission,
		));

		/* Test for addition of a delegate */
		assert_ok!(DeDir::update_delegate_permission(
			frame_system::RawOrigin::Signed(delegator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			new_permission,
		));

		/* Check if the delegate was added successfully */
		let delegates = DelegatesList::<Test>::get(registry_id.clone());
		assert!(delegates.is_some());

		/* Check if the permission of the delegate was updated to ADMIN */
		let delegates = delegates.unwrap();
		assert!(delegates
			.entries
			.iter()
			.any(|d| d.delegate == delegate && d.permissions == new_permission));

		/* Check for successfull event emission of RegistryDelegateAdded */
		System::assert_last_event(
			Event::RegistryDelegatePermissionUpdated {
				delegator: delegator.clone(),
				registry_id: registry_id.clone(),
				delegate: delegate.clone(),
				new_permission,
			}
			.into(),
		);
	});
}
