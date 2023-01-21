// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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

use cord_primitives::IdentifierOf;
use cord_utilities::mock::mock_origin::DoubleOrigin;
use frame_support::{assert_noop, assert_ok, sp_runtime::traits::Hash};

use crate::{
	self as schema,
	mock::{generate_schema_id, runtime::*},
};

// submit_schema_creation_operation

#[test]
fn check_successful_schema_creation() {
	let creator = DID_00;
	let deposit_owner = ACCOUNT_00;
	let schema = [9u8; 256].to_vec();
	let schema_hash = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id: IdentifierOf = generate_schema_id::<Test>(&schema_hash);
	let initial_balance = <Test as schema::Config>::Fee::get() * 2;
	ExtBuilder::default()
		.with_balances(vec![(deposit_owner.clone(), initial_balance)])
		.build()
		.execute_with(|| {
			assert_ok!(Schema::create(
				DoubleOrigin(deposit_owner.clone(), creator.clone()).into(),
				schema
			));
			let stored_schema_creator =
				Schema::schemas(&schema_id).expect("Schema Identifier should be present on chain.");
			let stored_schema_identifier = Schema::schema_hashes(&schema_hash)
				.expect("Schema Hash should be present on chain.");
			// Verify the Schema has the right owner
			assert_eq!(stored_schema_creator, creator);
			// Verify the Schema hash is mapped to an identifier
			assert_eq!(stored_schema_identifier, schema_id);
			assert_eq!(
				Balances::free_balance(deposit_owner),
				initial_balance.saturating_sub(<Test as schema::Config>::Fee::get())
			);
		});
}

#[test]
fn insufficient_funds() {
	let creator = DID_00;
	let deposit_owner = ACCOUNT_00;
	let schema = [9u8; 256].to_vec();

	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Schema::create(DoubleOrigin(deposit_owner, creator).into(), schema),
			schema::Error::<Test>::UnableToPayFees
		);
	});
}

#[test]
fn check_duplicate_schema_creation() {
	let creator = DID_00;
	let deposit_owner = ACCOUNT_00;
	let schema = [9u8; 256].to_vec();
	let schema_hash = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id = generate_schema_id::<Test>(&schema_hash);

	ExtBuilder::default()
		.with_schemas(vec![(schema_id, creator.clone())])
		.with_balances(vec![(deposit_owner.clone(), <Test as schema::Config>::Fee::get() * 2)])
		.build()
		.execute_with(|| {
			assert_noop!(
				Schema::create(DoubleOrigin(deposit_owner, creator).into(), schema),
				schema::Error::<Test>::SchemaAlreadyAnchored
			);
		});
}
