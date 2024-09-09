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
use frame_support::{assert_ok, BoundedVec};
use serde_json::json;
use sp_runtime::traits::Hash;
use sp_std::prelude::*;

/// Generates a Registry ID
pub fn generate_registry_id<T: Config>(id_digest: &RegistryHashOf<T>) -> RegistryIdOf {
	let registry_id: RegistryIdOf =
		Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::Registries)
			.expect("Registry Identifier creation failed.");

	registry_id
}

pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);
pub(crate) const ACCOUNT_01: AccountId = AccountId::new([2u8; 32]);
//pub(crate) const ACCOUNT_02: AccountId = AccountId::new([3u8; 32]);

#[test]
fn create_registry_should_work() {
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

	// If blob exists, add it to the identifier
	let mut id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Check if the Registry was created */
		assert!(Registry::<Test>::contains_key(registry_id.clone()));
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();

		/* Check for values stored are correct */
		assert_eq!(registry.digest, digest);
		assert_eq!(registry.state, state);
		assert_eq!(registry.delegates.len(), 1); // Only the creator should be added as delegate

		/* Check that the creator has OWNER permission */
		assert!(registry
			.delegates
			.iter()
			.any(|d| d.delegate == creator && d.permissions == Permissions::OWNER));

		/* Check for successful event emission of RegistryCreated */
		System::assert_last_event(
			Event::RegistryCreated { creator: creator.clone(), registry_id: registry_id.clone() }
				.into(),
		);
	});
}

#[test]
fn update_registry_state_should_work() {
	let creator = ACCOUNT_00;
	let admin = ACCOUNT_01;

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

	// If blob exists, add it to the identifier
	let mut id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		let mut new_state = RegistrySupportedStateOf::REVOKED;

		/* Test updation of state for a Registry */
		assert_ok!(Registries::update_state(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_state.clone(),
		));

		/* Verify the registry state has been updated */
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();
		assert_eq!(registry.state, new_state);

		/* Add a admin delegate and test updation of Registry State */
		assert_ok!(Registries::add_admin_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			admin.clone()
		));

		new_state = RegistrySupportedStateOf::ACTIVE;
		assert_ok!(Registries::update_state(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			new_state.clone(),
		));

		/* Verify the registry state has been updated back to ACTIVE */
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();
		assert_eq!(registry.state, new_state);

		/* Check successful event emission */
		System::assert_last_event(
			Event::RegistryStateChanged {
				who: admin.clone(),
				registry_id: registry_id.clone(),
				new_state: new_state.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn update_registry_should_work() {
	let creator = ACCOUNT_00;
	let admin = ACCOUNT_01;

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

	// If blob exists, add it to the identifier
	let mut id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		let updated_json_object = json!({
			"firstName": "String",
			"lastName": "String",
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

		let updated_json_string =
			serde_json::to_string(&updated_json_object).expect("Failed to serialize JSON");

		let updated_raw_bytes = updated_json_string.as_bytes().to_vec();

		let updated_blob: RegistryBlobOf<Test> = BoundedVec::try_from(updated_raw_bytes.clone())
			.expect(
				"Test Blob should fit into the expected input length of BLOB for the test runtime.",
			);

		let updated_digest: RegistryHashOf<Test> =
			<Test as frame_system::Config>::Hashing::hash(&updated_raw_bytes.encode()[..]);

		let updated_state = RegistrySupportedStateOf::REVOKED;

		/* Update registry by OWNER Permissioned account */
		assert_ok!(Registries::update(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			updated_digest,
			Some(updated_blob.clone()),
			Some(updated_state.clone()),
		));

		/* Verify the registry updated by the OWNER */
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();
		assert_eq!(registry.digest, updated_digest);
		assert_eq!(registry.state, updated_state);

		/* Add a admin delegate and test updation of Registry */
		assert_ok!(Registries::add_admin_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			admin.clone()
		));

		/* Update the Registry from a ADMIN permissioned account */
		assert_ok!(Registries::update(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			digest,
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Verify the registry updated by the ADMIN */
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();
		assert_eq!(registry.digest, digest);
		assert_eq!(registry.state, state);

		/* Check successful event emission */
		System::assert_last_event(
			Event::RegistryUpdated { updater: admin.clone(), registry_id: registry_id.clone() }
				.into(),
		);
	});
}

#[test]
fn add_owner_delegate_should_work() {
	let creator = ACCOUNT_00;
	let new_owner = ACCOUNT_01;

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

	// If blob exists, add it to the identifier
	let mut id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Test addition of a OWNER delegate */
		assert_ok!(Registries::add_owner_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_owner.clone(),
		));

		/* Verify the registry state has been updated back to ACTIVE */
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();
		let delegate_info = registry
			.delegates
			.iter()
			.find(|info| info.delegate == new_owner.clone())
			.unwrap();
		assert!(delegate_info.permissions.contains(Permissions::OWNER));

		/* Check successful event emission */
		System::assert_last_event(
			Event::RegistryOwnerDelegateAdded {
				delegator: creator.clone(),
				registry_id: registry_id.clone(),
				delegate: new_owner.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn add_admin_delegate_should_work() {
	let creator = ACCOUNT_00;
	let new_admin = ACCOUNT_01;

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

	// If blob exists, add it to the identifier
	let mut id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Test addition of a ADMIN delegate */
		assert_ok!(Registries::add_admin_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_admin.clone(),
		));

		/* Verify the registry state has been updated back to ACTIVE */
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();
		let delegate_info = registry
			.delegates
			.iter()
			.find(|info| info.delegate == new_admin.clone())
			.unwrap();
		assert!(delegate_info.permissions.contains(Permissions::ADMIN));

		/* Check successful event emission */
		System::assert_last_event(
			Event::RegistryAdminDelegateAdded {
				delegator: creator.clone(),
				registry_id: registry_id.clone(),
				delegate: new_admin.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn add_delegate_should_work() {
	let creator = ACCOUNT_00;
	let new_delegate = ACCOUNT_01;

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

	// If blob exists, add it to the identifier
	let mut id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Test addition of a delegate with `DELEGATE` permission */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_delegate.clone(),
		));

		/* Verify the registry state has been updated back to ACTIVE */
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();
		let delegate_info = registry
			.delegates
			.iter()
			.find(|info| info.delegate == new_delegate.clone())
			.unwrap();
		assert!(delegate_info.permissions.contains(Permissions::DELEGATE));

		/* Check successful event emission */
		System::assert_last_event(
			Event::RegistryDelegateAdded {
				delegator: creator.clone(),
				registry_id: registry_id.clone(),
				delegate: new_delegate.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn remove_admin_delegate_should_work() {
	let creator = ACCOUNT_00;
	let new_admin = ACCOUNT_01;

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

	// If blob exists, add it to the identifier
	let mut id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Test addition of a delegate with `ADMIN` permission */
		assert_ok!(Registries::add_admin_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_admin.clone(),
		));

		/* Test removal of a delegate with `ADMIN` permission */
		assert_ok!(Registries::remove_admin_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_admin.clone(),
		));

		/* Verify that the delegate no longer has the ADMIN permission */
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();
		let delegate_info =
			registry.delegates.iter().find(|info| info.delegate == new_admin.clone());

		/* Assert that the delegate no longer exists in the registry */
		assert!(delegate_info.is_none());

		/* Check successful event emission */
		System::assert_last_event(
			Event::RegistryAdminDelegateRemoved {
				remover: creator.clone(),
				registry_id: registry_id.clone(),
				delegate: new_admin.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn remove_delegate_should_work() {
	let creator = ACCOUNT_00;
	let new_delegate = ACCOUNT_01;

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

	// If blob exists, add it to the identifier
	let mut id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&creator.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Test addition of a delegate with `DELEGATE` permission */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_delegate.clone(),
		));

		/* Test removal of a delegate with `DELEGATE` permission */
		assert_ok!(Registries::remove_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_delegate.clone(),
		));

		/* Verify that the delegate no longer has the ADMIN permission */
		let registry = Registry::<Test>::get(registry_id.clone()).unwrap();
		let delegate_info =
			registry.delegates.iter().find(|info| info.delegate == new_delegate.clone());

		/* Assert that the delegate no longer exists in the registry */
		assert!(delegate_info.is_none());

		/* Check successful event emission */
		System::assert_last_event(
			Event::RegistryDelegateRemoved {
				remover: creator.clone(),
				registry_id: registry_id.clone(),
				delegate: new_delegate.clone(),
			}
			.into(),
		);
	});
}
