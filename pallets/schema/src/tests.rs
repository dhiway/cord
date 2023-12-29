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
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use frame_system::RawOrigin;
use sp_core::H256;
use sp_runtime::{traits::Hash, AccountId32};
use sp_std::prelude::*;
const DEFAULT_SCHEMA_HASH_SEED: u64 = 1u64;
const ALTERNATIVE_SCHEMA_HASH_SEED: u64 = 2u64;

/// Retrieves the schema hash based on a specified condition.
///
/// This function returns the schema hash of type `SchemaHashOf<T>` based on the
/// value of the `default` parameter. If `default` is `true`, the default schema
/// hash seed is used to generate the schema hash. If `default` is `false`,
/// an alternative schema hash seed is used instead. The resulting schema hash
/// is returned.
///
/// # Arguments
///
/// * `default` - A boolean value indicating whether to use the default schema hash seed or an
///   alternative one.
///
/// # Type Parameters
///
/// * `T` - The runtime configuration trait.
///
/// # Returns
///
/// A `SchemaHashOf<T>` representing the generated schema hash.
pub fn get_schema_hash<T>(default: bool) -> SchemaHashOf<T>
where
	T: Config,
	T::Hash: From<H256>,
{
	if default {
		H256::from_low_u64_be(DEFAULT_SCHEMA_HASH_SEED).into()
	} else {
		H256::from_low_u64_be(ALTERNATIVE_SCHEMA_HASH_SEED).into()
	}
}

/// Generates a schema ID from a schema digest.
///
/// This function takes a schema digest and converts it into a schema ID using
/// the SS58 encoding scheme. The resulting schema ID is returned.
///
/// # Arguments
///
/// * `digest` - A reference to the schema digest.
///
/// # Type Parameters
///
/// * `T` - The runtime configuration trait.
///
/// # Returns
///
/// A `SchemaIdOf` representing the generated schema ID.
///
/// # Panics
///
/// This function will panic if the conversion from digest to schema ID fails.
pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Schema).unwrap()
}

/// Generates a space ID from a digest.
pub fn generate_space_id<T: Config>(digest: &SchemaHashOf<T>) -> SpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Space).unwrap()
}

/// Generates an authorization ID from a digest.
pub fn generate_authorization_id<T: Config>(digest: &SchemaHashOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Authorization)
		.unwrap()
}

// submit_schema_creation_operation
pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

// This test verifies the successful creation of a schema.
// It creates a schema with a given creator, author, and schema data, and then
// checks if the schema is stored correctly in the storage. The test passes if
// all the assertions are successful.
#[test]
fn check_successful_schema_creation() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let capacity = 3u64;

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier = generate_authorization_id::<Test>(&auth_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, capacity));

		// Author Transaction
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone(),
			authorization_id
		));

		// Storage Checks
		let stored_schema = Schemas::<Test>::get(&schema_id)
			.expect("Schema Identifier should be present on chain.");

		// Verify the Schema has the right owner
		assert_eq!(stored_schema.creator, creator);
		// Verify the Schema digest is mapped correctly
		assert_eq!(stored_schema.digest, digest);
	});
}

// This test checks the behavior when trying to create a duplicate schema. It
// creates a schema with a given creator and author, and then attempts to create
// the same schema again. The test expects the second creation to fail with the
// `SchemaAlreadyAnchored` error.
#[test]
fn check_duplicate_schema_creation() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let capacity = 3u64;
	let raw_schema = [9u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier = generate_authorization_id::<Test>(&auth_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, capacity));

		// Author Transaction
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone(),
			authorization_id.clone()
		));
		// Try Author the same schema again. should fail.
		assert_noop!(
			Schema::create(DoubleOrigin(author, creator).into(), schema, authorization_id),
			Error::<Test>::SchemaAlreadyAnchored
		);
	});
}

// This test case ensures that the creation of a schema with empty schema data
// fails with the EmptyTransaction error.
#[test]
fn check_empty_schema_creation() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let capacity = 3u64;
	let empty_schema: InputSchemaOf<Test> = BoundedVec::default();

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier = generate_authorization_id::<Test>(&auth_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, capacity));

		// Author Transaction
		assert_noop!(
			Schema::create(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				empty_schema,
				authorization_id
			),
			Error::<Test>::EmptyTransaction
		);
	});
}

// This test creates multiple schemas, stores them on the chain, and then
// retrieves each schema based on its identifier. It verifies that the retrieved
// schemas match the expected values.
#[test]
fn test_schema_lookup() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let capacity = 5u64;

	// Create multiple schemas
	let schema1: InputSchemaOf<Test> = BoundedVec::try_from([1u8; 256].to_vec()).unwrap();
	let schema2: InputSchemaOf<Test> = BoundedVec::try_from([2u8; 256].to_vec()).unwrap();
	let schema3: InputSchemaOf<Test> = BoundedVec::try_from([3u8; 256].to_vec()).unwrap();

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier = generate_authorization_id::<Test>(&auth_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		// Create the schemas
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema1.clone(),
			authorization_id.clone()
		));
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema2.clone(),
			authorization_id.clone()
		));
		assert_ok!(Schema::create(
			DoubleOrigin(author, creator.clone()).into(),
			schema3.clone(),
			authorization_id
		));

		// Retrieve and verify each schema
		let schemas = vec![schema1, schema2, schema3];
		for schema in schemas {
			let digest: SchemaHashOf<Test> =
				<Test as frame_system::Config>::Hashing::hash(&schema[..]);
			let id_digest = <Test as frame_system::Config>::Hashing::hash(
				&[&schema.encode()[..], &space_id.encode()[..], &creator.encode()[..]].concat()[..],
			);
			let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);

			let stored_schema = Schemas::<Test>::get(&schema_id)
				.expect("Schema Identifier should be present on chain.");

			assert_eq!(stored_schema.schema, schema);
			assert_eq!(stored_schema.digest, digest);
			assert_eq!(stored_schema.creator, creator);
		}
	});
}
