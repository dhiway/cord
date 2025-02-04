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
use sp_std::prelude::*;

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
		assert_err!(result, IdentifierError::PalletNameTooLong);
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
		assert_err!(result, IdentifierError::PalletNotFound);
	});
}

/// Test that setting and then retrieving the network id works correctly.
#[test]
fn set_and_get_network_id() {
	new_test_ext().execute_with(|| {
		let network_id = NetworkId::from(42u32);
		Pallet::<Test>::set_network_id(network_id);
		let retrieved = Pallet::<Test>::get_network_id();
		assert_eq!(retrieved, network_id);
	});
}

/// Test that a valid activity record is stored correctly.
#[test]
fn record_activity_positive() {
	new_test_ext().execute_with(|| {
		let raw_identifier = vec![1u8; 10];
		let identifier =
			Ss58Identifier::try_from(raw_identifier).expect("Should create a valid Ss58Identifier");
		let entry: EntryTypeOf =
			vec![1u8; 10].try_into().expect("Should create a valid bounded vector");
		let stamp = EventStamp { height: 1, index: 0 };

		assert_ok!(Pallet::<Test>::record_activity(&identifier, entry.clone(), stamp.clone()));

		let counter = ActivityCounter::<Test>::get(&identifier);
		assert_eq!(counter, 1);
		let record =
			ActivityChain::<Test>::get(&identifier, 0).expect("An activity record should exist");
		assert_eq!(record.entry, entry);
		assert_eq!(record.event_stamp, stamp);
	});
}
