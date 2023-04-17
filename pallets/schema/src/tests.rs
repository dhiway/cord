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
#[cfg(any(feature = "mock", test))]
use crate::mock as crate_mock;
use crate::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_core::H256;
use sp_runtime::{traits::Hash, AccountId32};
use sp_std::prelude::*;
use cord_primitives::AccountId;
const DEFAULT_SCHEMA_HASH_SEED: u64 = 1u64;
const ALTERNATIVE_SCHEMA_HASH_SEED: u64 = 2u64;

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

pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	let identifier = Ss58Identifier::to_schema_id(&(digest).encode()[..]).unwrap();
	identifier
}

// submit_schema_creation_operation
pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

#[test]
fn check_successful_schema_creation() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<crate_mock::Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let digest: SchemaHashOf<crate_mock::Test> = <crate_mock::Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let id_digest = <crate_mock::Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<crate_mock::Test>(&id_digest);

	crate_mock::new_test_ext().execute_with(|| {
		// Author Transaction
		assert_ok!(crate_mock::Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		// Storage Checks
		let stored_schema = Schemas::<crate_mock::Test>::get(&schema_id)
			.expect("Schema Identifier should be present on chain.");

		// Verify the Schema has the right owner
		assert_eq!(stored_schema.creator, creator);
		// Verify the Schema digest is mapped correctly
		assert_eq!(stored_schema.digest, digest);
	});
}

#[test]
fn check_duplicate_schema_creation() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_schema = [9u8; 256].to_vec();
	let schema: InputSchemaOf<crate_mock::Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	crate_mock::new_test_ext().execute_with(|| {
		// Author Transaction
		assert_ok!(crate_mock::Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));
		// Try Author the same schema again. should fail.
		assert_noop!(
			crate_mock::Schema::create(DoubleOrigin(author, creator).into(), schema),
			Error::<crate_mock::Test>::SchemaAlreadyAnchored
		);
	});
}
