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
use crate::mock::{new_test_ext, Test};
use frame_support::{assert_err, assert_ok};
use sp_core::H256;

/// Test that a valid pallet name can be stored and returns a consistent index.
#[test]
fn get_or_add_pallet_index_positive() {
	new_test_ext().execute_with(|| {
		let pallet_name = "TestPallet";
		let index = Pallet::<Test>::get_or_add_pallet_index(pallet_name)
			.expect("Should create a valid index");
		// Calling the function again returns the same index.
		let index2 = Pallet::<Test>::get_or_add_pallet_index(pallet_name)
			.expect("Should retrieve the same index");
		assert_eq!(index, index2);
	});
}

/// Test that a pallet name that is too long returns an error.
#[test]
fn get_or_add_pallet_index_negative() {
	new_test_ext().execute_with(|| {
		let long_name = "A".repeat(65);
		let result = Pallet::<Test>::get_or_add_pallet_index(&long_name);
		assert_err!(result, Error::<Test>::PalletNameTooLong);
	});
}

/// Test that resolving a valid pallet index returns the original pallet name.
#[test]
fn resolve_pallet_name_positive() {
	new_test_ext().execute_with(|| {
		let pallet_name = "TestPallet";
		let index = Pallet::<Test>::get_or_add_pallet_index(pallet_name)
			.expect("Should create a valid index");
		let resolved =
			Pallet::<Test>::resolve_pallet_name(index).expect("Should resolve the pallet name");
		assert_eq!(resolved, pallet_name);
	});
}

/// Test that resolving an invalid (nonexistent) pallet index returns an error.
#[test]
fn resolve_pallet_name_negative() {
	new_test_ext().execute_with(|| {
		let invalid_index = 9999;
		let result = Pallet::<Test>::resolve_pallet_name(invalid_index);
		assert_err!(result, Error::<Test>::PalletNotFound);
	});
}

/// Test that setting and then retrieving the network id works correctly.
#[test]
fn set_and_get_network_id() {
	new_test_ext().execute_with(|| {
		let chain_id: u16 = 12_001;
		GenesisNetworkId::<Test>::put(chain_id);
		let retrieved = Pallet::<Test>::get_network_id();
		assert_eq!(retrieved, chain_id);
	});
}

#[test]
fn record_activity_positive() {
	new_test_ext().execute_with(|| {
		let id_digest = vec![1u8; 32];
		let doken = Ss58Identifier::to_encoded(id_digest, 100, 5, 0)
			.expect("Doken encoding should succeed");
		let digest = H256::random();

		let action: EventTypeOf =
			vec![1u8; 10].try_into().expect("Should create a valid bounded vector");
		let seal = EventBlock { height: 1, index: 0 };

		assert_ok!(Pallet::<Test>::state_event(&doken, digest, action.clone(), seal.clone()));

		let counter = StateVersion::<Test>::get(&doken);
		assert_eq!(counter, 1);

		let record = StateHistory::<Test>::get(&doken, 0).expect("An activity record should exist");
		assert_eq!(record.action, action);
		assert_eq!(record.digest, digest);
		assert_eq!(record.seal, seal);
	});
}
