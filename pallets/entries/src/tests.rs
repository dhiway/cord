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

use pallet_registries::{RegistryBlobOf, RegistryHashOf};
use pallet_schema_accounts::{InputSchemaOf, SchemaHashOf, SchemaIdOf};

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

/// Generates a Authorization ID
pub fn generate_authorization_id<T: Config>(digest: &RegistryHashOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::RegistryAuthorization)
		.unwrap()
}

/// Generates a Schema ID
pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::SchemaAccounts)
		.unwrap()
}

pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

#[test]
fn create_registry_entry_should_work() {
	let creator = ACCOUNT_00;
	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob),
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
			&[
				&registry_entry_digest.encode()[..],
				&registry_id.encode()[..],
				&creator.encode()[..],
			]
			.concat()[..],
		);

		let registry_entry_id: RegistryEntryIdOf =
			generate_registry_entry_id::<Test>(&registry_entry_id_digest);

		assert_ok!(Entries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct */
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.revoked, false);
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
	let creator = ACCOUNT_00;
	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob),
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
			&[
				&registry_entry_digest.encode()[..],
				&registry_id.encode()[..],
				&creator.encode()[..],
			]
			.concat()[..],
		);

		let registry_entry_id: RegistryEntryIdOf =
			generate_registry_entry_id::<Test>(&registry_entry_id_digest);

		assert_ok!(Entries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
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

		assert_ok!(Entries::update(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
			updated_registry_entry_digest,
			Some(updated_registry_entry_blob.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct */
		assert_eq!(entry.digest, updated_registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

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
fn revoke_registry_entry_should_work() {
	let creator = ACCOUNT_00;
	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob),
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
			&[
				&registry_entry_digest.encode()[..],
				&registry_id.encode()[..],
				&creator.encode()[..],
			]
			.concat()[..],
		);

		let registry_entry_id: RegistryEntryIdOf =
			generate_registry_entry_id::<Test>(&registry_entry_id_digest);

		assert_ok!(Entries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		assert_ok!(Entries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
		));

		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, true);

		System::assert_last_event(
			Event::RegistryEntryRevoked {
				updater: creator.clone(),
				registry_entry_id: registry_entry_id.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn reinstating_revoked_registry_entry_should_work() {
	let creator = ACCOUNT_00;
	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob),
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
			&[
				&registry_entry_digest.encode()[..],
				&registry_id.encode()[..],
				&creator.encode()[..],
			]
			.concat()[..],
		);

		let registry_entry_id: RegistryEntryIdOf =
			generate_registry_entry_id::<Test>(&registry_entry_id_digest);

		assert_ok!(Entries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		assert_ok!(Entries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
		));

		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, true);

		assert_ok!(Entries::reinstate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
		));

		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		System::assert_last_event(
			Event::RegistryEntryReinstated {
				updater: creator.clone(),
				registry_entry_id: registry_entry_id.clone(),
			}
			.into(),
		);
	});
}
