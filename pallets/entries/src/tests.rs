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
use frame_support::{assert_err, assert_ok, BoundedVec};
use serde_json::json;
use sp_runtime::traits::Hash;
use sp_std::prelude::*;

use pallet_namespace::{NameSpaceCodeOf, NameSpaceIdOf};
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
pub fn generate_authorization_id<T: Config>(
	digest: &RegistryHashOf<T>,
) -> RegistryAuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::RegistryAuthorization)
		.unwrap()
}

/// Generates a Schema ID
pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::SchemaAccounts)
		.unwrap()
}

/// Generates a Namespace ID
pub fn generate_namespace_id<T: Config>(digest: &NameSpaceCodeOf<T>) -> NameSpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::NameSpace).unwrap()
}

/// Generates a Namespace Authorization ID
pub fn generate_namespace_authorization_id<T: Config>(
	digest: &NameSpaceCodeOf<T>,
) -> NamespaceAuthorizationIdOf {
	Ss58Identifier::create_identifier(
		&(digest).encode()[..],
		IdentifierType::NameSpaceAuthorization,
	)
	.unwrap()
}

pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);
pub(crate) const ACCOUNT_01: AccountId = AccountId::new([2u8; 32]);
pub(crate) const ACCOUNT_02: AccountId = AccountId::new([3u8; 32]);

#[test]
fn create_registry_entry_should_work() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

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

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
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

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

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

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
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

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

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

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
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

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

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

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
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

#[test]
fn update_registry_entry_should_work_for_valid_creator() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
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
			creator_authorization_id.clone(),
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

		/* Entries will be updated from the creator of the Registry Entry */
		assert_ok!(Entries::update(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			creator_authorization_id.clone(),
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
fn update_registry_entry_should_work_for_valid_admin() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

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

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
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

		/* Entries will be updated from the admin of the Registry */
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
fn update_registry_entry_should_fail_for_non_registry_entry_creator() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;
	let non_creator = ACCOUNT_02;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let non_creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &non_creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let non_creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_creator_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			non_creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
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
			creator_authorization_id.clone(),
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

		/* Entries updation by a non-creator of the Registry Entry
		 * but part of the Registry will fail.
		 */
		assert_err!(
			Entries::update(
				frame_system::RawOrigin::Signed(non_creator.clone()).into(),
				registry_entry_id.clone(),
				non_creator_authorization_id.clone(),
				updated_registry_entry_digest,
				Some(updated_registry_entry_blob.clone()),
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn update_registry_entry_should_fail_for_non_registry_admin() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;
	let invalid_admin = ACCOUNT_02;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let invalid_admin_namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &invalid_admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let invalid_admin_namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&invalid_admin_namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let invalid_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &invalid_admin.encode()[..]].concat()[..],
	);

	let invalid_registry_id: RegistryIdOf = generate_registry_id::<Test>(&invalid_id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let invalid_admin_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[
			&invalid_registry_id.encode()[..],
			&invalid_admin.encode()[..],
			&invalid_admin.encode()[..],
		]
		.concat()[..],
	);

	let invalid_admin_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&invalid_admin_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Add a invalid-admin as part of the namespace so can form a different registry */
		assert_ok!(NameSpace::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_id.clone(),
			invalid_admin.clone(),
			namespace_authorization_id.clone()
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id.clone()),
			Some(blob.clone()),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(invalid_admin.clone()).into(),
			registry_digest,
			invalid_admin_namespace_authorization_id.clone(),
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
			creator_authorization_id.clone(),
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

		/* Entries updation by a invalid-admin of the Registry
		 * but a Admin of a different Registry should fail.
		 */
		assert_err!(
			Entries::update(
				frame_system::RawOrigin::Signed(invalid_admin.clone()).into(),
				registry_entry_id.clone(),
				invalid_admin_authorization_id.clone(),
				updated_registry_entry_digest,
				Some(updated_registry_entry_blob.clone()),
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn update_ownership_of_registry_entry_creator_should_work_for_creator() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;
	let new_owner = ACCOUNT_02;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest_admin = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id_admin: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest_admin);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let new_owner_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &new_owner.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let new_owner_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&new_owner_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id_admin.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id_admin.clone(),
			authorization_id.clone(),
		));

		/* Add Registry Entry `new_owner` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			new_owner.clone(),
			namespace_authorization_id_admin.clone(),
			authorization_id.clone(),
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
			creator_authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct after ownership update */
		assert_eq!(entry.creator, creator.clone());
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		/* Change the ownership to newer owner */
		assert_ok!(Entries::update_ownership(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			creator_authorization_id.clone(),
			new_owner.clone(),
			new_owner_authorization_id.clone(),
		));

		/* Check if the Entry was updated */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct after ownership update */
		assert_eq!(entry.creator, new_owner.clone());
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		/* Check for successful event emission of RegistryEntryOwnershipUpdated */
		System::assert_last_event(
			Event::RegistryEntryOwnershipUpdated {
				updater: creator.clone(),
				new_owner: new_owner.clone(),
				registry_entry_id: registry_entry_id.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn update_ownership_of_registry_entry_creator_should_work_for_admin() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;
	let new_owner = ACCOUNT_02;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let new_owner_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &new_owner.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let new_owner_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&new_owner_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		/* Add Registry Entry `new_owner` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			new_owner.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
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
			creator_authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct after ownership update */
		assert_eq!(entry.creator, creator.clone());
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		/* Change the ownership to newer owner, signed by the Registry Admin */
		assert_ok!(Entries::update_ownership(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
			new_owner.clone(),
			new_owner_authorization_id.clone(),
		));

		/* Check if the Entry was updated */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct after ownership update */
		assert_eq!(entry.creator, new_owner.clone());
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		/* Check for successful event emission of RegistryEntryOwnershipUpdated */
		System::assert_last_event(
			Event::RegistryEntryOwnershipUpdated {
				updater: admin.clone(),
				new_owner: new_owner.clone(),
				registry_entry_id: registry_entry_id.clone(),
			}
			.into(),
		);
	});
}

#[test]
fn new_owner_should_be_able_to_perform_registry_entry_operations_after_ownership_change() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;
	let new_owner = ACCOUNT_02;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let new_owner_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &new_owner.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let new_owner_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&new_owner_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		/* Add Registry Entry `new_owner` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			new_owner.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
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
			creator_authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct after ownership update */
		assert_eq!(entry.creator, creator.clone());
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		/* Change the ownership to newer owner */
		assert_ok!(Entries::update_ownership(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			creator_authorization_id.clone(),
			new_owner.clone(),
			new_owner_authorization_id.clone(),
		));

		/* Check if the Entry was updated */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct after ownership update */
		assert_eq!(entry.creator, new_owner.clone());
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		/* Check for successful event emission of RegistryEntryOwnershipUpdated */
		System::assert_last_event(
			Event::RegistryEntryOwnershipUpdated {
				updater: creator.clone(),
				new_owner: new_owner.clone(),
				registry_entry_id: registry_entry_id.clone(),
			}
			.into(),
		);

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

		/* Update, revoke, restore etc or any registry entry related tasks should
		 * be able to be performed by the new-owner */
		assert_ok!(Entries::update(
			frame_system::RawOrigin::Signed(new_owner.clone()).into(),
			registry_entry_id.clone(),
			new_owner_authorization_id.clone(),
			updated_registry_entry_digest,
			Some(updated_registry_entry_blob.clone()),
		));

		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		assert_eq!(entry.creator, new_owner.clone());
		assert_eq!(entry.digest, updated_registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);
	});
}

#[test]
fn old_owner_should_not_be_able_to_perform_registry_entry_operations_after_ownership_change() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;
	let new_owner = ACCOUNT_02;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let new_owner_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &new_owner.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let new_owner_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&new_owner_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		/* Add Registry Entry `new_owner` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			new_owner.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
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
			creator_authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct after ownership update */
		assert_eq!(entry.creator, creator.clone());
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		/* Change the ownership to newer owner */
		assert_ok!(Entries::update_ownership(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			creator_authorization_id.clone(),
			new_owner.clone(),
			new_owner_authorization_id.clone(),
		));

		/* Check if the Entry was updated */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct after ownership update */
		assert_eq!(entry.creator, new_owner.clone());
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		/* Check for successful event emission of RegistryEntryOwnershipUpdated */
		System::assert_last_event(
			Event::RegistryEntryOwnershipUpdated {
				updater: creator.clone(),
				new_owner: new_owner.clone(),
				registry_entry_id: registry_entry_id.clone(),
			}
			.into(),
		);

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

		/* Update, revoke, restore etc or any registry entry related tasks should not
		 * be able to be performed by the old-owner (creator) */
		assert_err!(
			Entries::update(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_entry_id.clone(),
				creator_authorization_id.clone(),
				updated_registry_entry_digest,
				Some(updated_registry_entry_blob.clone()),
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn update_ownership_should_fail_for_updating_themselves() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;
	let new_owner = ACCOUNT_02;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		/* Add Registry Entry `new_owner` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			new_owner.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
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
			creator_authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for values stored are correct after ownership update */
		assert_eq!(entry.creator, creator.clone());
		assert_eq!(entry.digest, registry_entry_digest);
		assert_eq!(entry.registry_id, registry_id);
		assert_eq!(entry.revoked, false);

		/* Change the ownership to newer owner */
		assert_err!(
			Entries::update_ownership(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_entry_id.clone(),
				creator_authorization_id.clone(),
				creator.clone(),
				creator_authorization_id.clone(),
			),
			Error::<Test>::NewOwnerCannotBeSameAsExistingOwner
		);
	});
}

#[test]
fn verification_of_registry_entry_identifier_should_succeed() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
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
			creator_authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let _entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		/* Check for verification of Registry Entry */
		assert_ok!(Entries::verify_existence(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
		));

		/* Check for successful event emission of RegistryEntryExistenceVerified */
		System::assert_last_event(
			Event::RegistryEntryExistenceVerified {
				verifier: creator.clone(),
				registry_entry_id: registry_entry_id.clone(),
				registry_entry_digest,
			}
			.into(),
		);
	});
}

#[test]
fn verification_of_non_existing_registry_entry_identifier_should_fail() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
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

		/* Operation should fail given identifier does not exist */
		assert_err!(
			Entries::verify_existence(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_entry_id.clone(),
			),
			Error::<Test>::RegistryEntryIdentifierDoesNotExist
		);
	});
}

#[test]
fn verification_of_revoked_registry_entry_identifier_should_fail() {
	let admin = ACCOUNT_00;
	let creator = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &admin.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let creator_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &admin.encode()[..]].concat()[..],
	);

	let creator_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&creator_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		/* Test creation of a Namespace */
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			namespace_digest,
			None,
		));

		/* Test creation of a Registry */
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));

		/* Add Registry Entry `creator` as a delegate */
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(admin.clone()).into(),
			registry_id.clone(),
			creator.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
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
			creator_authorization_id.clone(),
			registry_entry_digest,
			Some(registry_entry_blob.clone()),
		));

		/* Check if the Entry was created */
		assert!(RegistryEntries::<Test>::contains_key(registry_entry_id.clone()));
		let _entry = RegistryEntries::<Test>::get(registry_entry_id.clone()).unwrap();

		assert_ok!(Entries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			creator_authorization_id.clone(),
		));

		/* Check for verification of Registry Entry */
		assert_err!(
			Entries::verify_existence(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_entry_id.clone(),
			),
			Error::<Test>::RegistryEntryRevoked
		);
	});
}
