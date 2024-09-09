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
use frame_support::{assert_err, assert_ok, BoundedVec};
//use frame_system::RawOrigin;
use sp_runtime::traits::Hash;
use sp_std::prelude::*;
//use cord_utilities::mock::mock_origin::Origin;
use serde_json::json;

use pallet_registries::{RegistryBlobOf, RegistryHashOf, RegistrySupportedStateOf};

/// Generates a Registry ID
pub fn generate_registry_id<T: Config>(id_digest: &RegistryHashOf<T>) -> RegistryIdOf {
	let registry_id: RegistryIdOf =
		Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::Registries)
			.expect("Registry Identifier creation failed.");

	registry_id
}

/// Generates a Registry Entry ID
pub fn generate_registry_entry_id<T: Config>(id_digest: &RegistryHashOf<T>) -> RegistryEntryIdOf {
	let registry_entry_id: RegistryEntryIdOf =
		Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::Entries)
			.expect("Registry Entry Identifier creation failed");

	registry_entry_id
}

pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);
pub(crate) const ACCOUNT_01: AccountId = AccountId::new([2u8; 32]);
pub(crate) const ACCOUNT_02: AccountId = AccountId::new([3u8; 32]);

#[test]
fn create_registry_entry_should_work() {
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
	let registry_id = generate_registry_id::<Test>(&id_digest);

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

		/* Assumed JSON for Registry Entry */
		let registry_entry_json_object = json!({
			"name": "Alice",
			"age": 25,
			"email": "alice@dhiway.com",
			"isActive": true,
			"address": {
				"street": "Koramangala",
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

		let registry_entry_blob: RegistryEntryBlobOf<Test> =
			BoundedVec::try_from(registry_entry_raw_bytes.clone()).expect(
				"Test Blob should fit into the expected input length of BLOB for the test runtime.",
			);

		let registry_entry_digest: RegistryHashOf<Test> =
			<Test as frame_system::Config>::Hashing::hash(&registry_entry_raw_bytes.encode()[..]);

		let registry_entry_id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &registry_entry_digest.encode()[..]].concat()[..],
		);

		let registry_entry_id: RegistryEntryIdOf =
			generate_registry_entry_id::<Test>(&registry_entry_id_digest);

		let registry_entry_state = RegistryEntrySupportedStateOf::ACTIVE;

		assert_ok!(Entries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_entry_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
			Some(registry_entry_state.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct */
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.state, registry_entry_state);
		assert_eq!(entry.registry_id, registry_id);

		/* Check for successful event emission of RegistryCreated */
		System::assert_last_event(
			Event::RegistryEntryCreated {
				creator: creator.clone(),
				registry_id: registry_id.clone(),
				registry_entry_id: registry_entry_id.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn update_registry_entry_should_work() {
	let owner = ACCOUNT_00;
	let creator = ACCOUNT_01;

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
		&[&owner.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&owner.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}
	let registry_id = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(owner.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Add a account with `DELEGATE` permission */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(owner.clone()).into(),
			registry_id.clone(),
			creator.clone(),
		));

		/* Assumed JSON for Registry Entry */
		let registry_entry_json_object = json!({
			"name": "Alice",
			"age": 25,
			"email": "alice@dhiway.com",
			"isActive": true,
			"address": {
				"street": "Koramangala",
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

		let registry_entry_blob: RegistryEntryBlobOf<Test> =
			BoundedVec::try_from(registry_entry_raw_bytes.clone()).expect(
				"Test Blob should fit into the expected input length of BLOB for the test runtime.",
			);

		let registry_entry_digest: RegistryHashOf<Test> =
			<Test as frame_system::Config>::Hashing::hash(&registry_entry_raw_bytes.encode()[..]);

		let registry_entry_id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &registry_entry_digest.encode()[..]].concat()[..],
		);

		let registry_entry_id: RegistryEntryIdOf =
			generate_registry_entry_id::<Test>(&registry_entry_id_digest);

		let registry_entry_state = RegistryEntrySupportedStateOf::ACTIVE;

		assert_ok!(Entries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_entry_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
			Some(registry_entry_state.clone()),
		));

		/* Assumed JSON for Registry Entry */
		let updated_registry_entry_json_object = json!({
			"name": "BOB",
			"age": 21,
			"email": "bob@dhiway.com",
			"isActive": true,
			"address": {
				"street": "Koramangala",
				"city": "Bengaluru",
				"zipcode": "560001"
			},
			"phoneNumbers": [
				"+91-234787311",
				"+91-283746811"
			]
		});

		let updated_registry_entry_json_string =
			serde_json::to_string(&updated_registry_entry_json_object)
				.expect("Failed to serialize JSON");

		let updated_registry_entry_raw_bytes =
			updated_registry_entry_json_string.as_bytes().to_vec();

		let updated_registry_entry_blob: RegistryEntryBlobOf<Test> =
			BoundedVec::try_from(updated_registry_entry_raw_bytes.clone()).expect(
				"Test Blob should fit into the expected input length of BLOB for the test runtime.",
			);

		let updated_registry_entry_digest: RegistryHashOf<Test> =
			<Test as frame_system::Config>::Hashing::hash(
				&updated_registry_entry_raw_bytes.encode()[..],
			);

		let updated_registry_entry_state = RegistryEntrySupportedStateOf::REVOKED;

		assert_ok!(Entries::update(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			updated_registry_entry_digest,
			Some(updated_registry_entry_blob.clone()),
			Some(updated_registry_entry_state.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct */
		assert_eq!(entry.digest, updated_registry_entry_digest);
		assert_eq!(entry.state, updated_registry_entry_state);
		assert_eq!(entry.registry_id, registry_id);

		/* Check for successful event emission of RegistryCreated */
		System::assert_last_event(
			Event::RegistryEntryUpdated {
				updater: creator.clone(),
				registry_entry_id: registry_entry_id.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn update_registry_entry_state_should_work() {
	let owner = ACCOUNT_00;
	let creator = ACCOUNT_01;

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
		&[&owner.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&owner.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}
	let registry_id = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(owner.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Add a account with `DELEGATE` permission */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(owner.clone()).into(),
			registry_id.clone(),
			creator.clone(),
		));

		/* Assumed JSON for Registry Entry */
		let registry_entry_json_object = json!({
			"name": "Alice",
			"age": 25,
			"email": "alice@dhiway.com",
			"isActive": true,
			"address": {
				"street": "Koramangala",
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

		let registry_entry_blob: RegistryEntryBlobOf<Test> =
			BoundedVec::try_from(registry_entry_raw_bytes.clone()).expect(
				"Test Blob should fit into the expected input length of BLOB for the test runtime.",
			);

		let registry_entry_digest: RegistryHashOf<Test> =
			<Test as frame_system::Config>::Hashing::hash(&registry_entry_raw_bytes.encode()[..]);

		let registry_entry_id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &registry_entry_digest.encode()[..]].concat()[..],
		);

		let registry_entry_id: RegistryEntryIdOf =
			generate_registry_entry_id::<Test>(&registry_entry_id_digest);

		let registry_entry_state = RegistryEntrySupportedStateOf::ACTIVE;

		assert_ok!(Entries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_entry_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
			Some(registry_entry_state.clone()),
		));

		let new_state = RegistryEntrySupportedStateOf::REVOKED;

		assert_ok!(Entries::update_state(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			new_state.clone(),
		));

		/* Check if the Entry state is updated */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct */
		assert_eq!(entry.state, new_state);
		assert_eq!(entry.registry_id, registry_id);

		/* Check for successful event emission of Registry Entry State Updation */
		System::assert_last_event(
			Event::RegistryEntryStateChanged {
				who: creator.clone(),
				registry_entry_id: registry_entry_id.clone(),
				new_state: new_state.clone(),
			}
			.into(),
		);
	});
}

/// A `creator` of the Registry Entry (record) or the OWNER/ADMIN of the Registry
/// should only be able to Update the Registry Entry. `creator` of a different entry
/// should not be able to Update the Entry.
#[test]
fn update_registry_entry_state_should_fail_for_a_different_creator() {
	let owner = ACCOUNT_00;
	let creator = ACCOUNT_01;
	let creator2 = ACCOUNT_02;

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
		&[&owner.encode()[..], &digest.encode()[..]].concat()[..],
	);
	if !blob.is_empty() {
		id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&owner.encode()[..], &digest.encode()[..], &blob.encode()[..]].concat()[..],
		);
	}
	let registry_id = generate_registry_id::<Test>(&id_digest);

	let state = RegistrySupportedStateOf::ACTIVE;

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(owner.clone()).into(),
			registry_id.clone(),
			digest,
			None, // No template ID
			Some(blob.clone()),
			Some(state.clone()),
		));

		/* Add a account with `DELEGATE` permission */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(owner.clone()).into(),
			registry_id.clone(),
			creator.clone(),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(owner.clone()).into(),
			registry_id.clone(),
			creator2.clone(),
		));

		/* Assumed JSON for Registry Entry */
		let registry_entry_json_object = json!({
			"name": "Alice",
			"age": 25,
			"email": "alice@dhiway.com",
			"isActive": true,
			"address": {
				"street": "Koramangala",
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

		let registry_entry_blob: RegistryEntryBlobOf<Test> =
			BoundedVec::try_from(registry_entry_raw_bytes.clone()).expect(
				"Test Blob should fit into the expected input length of BLOB for the test runtime.",
			);

		let registry_entry_digest: RegistryHashOf<Test> =
			<Test as frame_system::Config>::Hashing::hash(&registry_entry_raw_bytes.encode()[..]);

		let registry_entry_id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&creator.encode()[..], &registry_entry_digest.encode()[..]].concat()[..],
		);

		let registry_entry_id: RegistryEntryIdOf =
			generate_registry_entry_id::<Test>(&registry_entry_id_digest);

		let registry_entry_state = RegistryEntrySupportedStateOf::ACTIVE;

		assert_ok!(Entries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_entry_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
			Some(registry_entry_state.clone()),
		));

		let registry_entry_id_digest_2 = <Test as frame_system::Config>::Hashing::hash(
			&[&creator2.encode()[..], &registry_entry_digest.encode()[..]].concat()[..],
		);

		let registry_entry_id_2: RegistryEntryIdOf =
			generate_registry_entry_id::<Test>(&registry_entry_id_digest_2);

		let new_state = RegistryEntrySupportedStateOf::REVOKED;

		assert_ok!(Entries::create(
			frame_system::RawOrigin::Signed(creator2.clone()).into(),
			registry_id.clone(),
			registry_entry_id_2.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
			Some(registry_entry_state.clone()),
		));

		assert_err!(
			Entries::update_state(
				frame_system::RawOrigin::Signed(creator2.clone()).into(),
				registry_entry_id.clone(),
				new_state.clone(),
			),
			Error::<Test>::UnauthorizedOperation
		);

		assert_err!(
			Entries::update_state(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_entry_id_2.clone(),
				new_state.clone(),
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}
