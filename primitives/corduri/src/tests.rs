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
use alloc::vec::Vec;
use core::convert::TryFrom;
use frame_support::{assert_err, assert_ok};

// Helper function to generate a valid 32-byte digest.
fn valid_digest() -> [u8; 32] {
	[0xAB; 32]
}

/// Test that an identifier built with valid input can be encoded and decoded correctly.
#[test]
fn encode_decode_roundtrip() {
	let digest = valid_digest();
	let nid: u16 = 100;
	let pid: u16 = 5;
	let identifier = Ss58Identifier::to_encoded(digest.clone(), nid, pid)
		.expect("Identifier encoding should succeed");
	let decoded = identifier.to_decoded().expect("Identifier decoding should succeed");

	// Verify that the decoded network and pallet identifiers match the input.
	assert_eq!(decoded.nid, nid, "The network id should match");
	assert_eq!(decoded.pid, pid, "The pallet id should match");
	// Verify that the genesis hash (digest) matches.
	assert_eq!(decoded.gen, format!("0x{}", hex::encode(digest)), "The digest should match");
}

/// Test that creating an identifier with an invalid digest length fails.
#[test]
fn fails_invalid_digest_length() {
	let short_digest = vec![0xAB; 31]; // 31 bytes, which is invalid.
	let nid: u16 = 100;
	let pid: u16 = 5;
	let result = Ss58Identifier::to_encoded(short_digest, nid, pid);
	assert!(result.is_err(), "Encoding should fail when digest is not 32 bytes long");
}

/// Test that an identifier with a tampered checksum fails to decode.
#[test]
fn fails_tampered_checksum() {
	let digest = valid_digest();
	let nid: u16 = 100;
	let pid: u16 = 5;
	let identifier =
		Ss58Identifier::to_encoded(digest, nid, pid).expect("Identifier encoding should succeed");

	// Base58-decode the identifier to obtain the raw bytes.
	let mut decoded_bytes =
		bs58::decode(&identifier.0).into_vec().expect("Base58 decoding should succeed");
	// Tamper with the checksum (flip a bit in the last byte).
	let last_index = decoded_bytes.len() - 1;
	decoded_bytes[last_index] ^= 1;
	// Re-encode the tampered bytes.
	let tampered = bs58::encode(&decoded_bytes).into_string();
	// Convert the tampered Base58 string into an identifier.
	let result: Result<Ss58Identifier, _> = tampered.try_into();
	assert!(result.is_err(), "Decoding should fail for an identifier with a tampered checksum");
}

/// Test conversion from Vec<u8> using the TryFrom implementation.
#[test]
fn try_from_vec_success() {
	let digest = valid_digest();
	let nid: u16 = 100;
	let pid: u16 = 5;
	let identifier =
		Ss58Identifier::to_encoded(digest, nid, pid).expect("Identifier encoding should succeed");
	// Get the raw Base58-encoded vector.
	let raw: Vec<u8> = identifier.0.clone().into();
	let identifier2 =
		Ss58Identifier::try_from(raw).expect("Conversion from Vec<u8> should succeed");
	let decoded = identifier2.to_decoded().expect("Decoding should succeed after conversion");
	assert_eq!(decoded.nid, nid, "Network id should match");
	assert_eq!(decoded.pid, pid, "Pallet id should match");
}

/// Test conversion from a Base58-encoded String using the TryFrom implementation.
#[test]
fn try_from_string_success() {
	let digest = valid_digest();
	let nid: u16 = 100;
	let pid: u16 = 5;
	let identifier =
		Ss58Identifier::to_encoded(digest, nid, pid).expect("Identifier encoding should succeed");
	// Convert the identifier's inner BoundedVec to a Base58 string.
	let as_string = String::from_utf8(identifier.0.clone().into())
		.expect("Base58 string should be valid UTF-8");
	let identifier2 =
		Ss58Identifier::try_from(as_string).expect("Conversion from String should succeed");
	let decoded = identifier2.to_decoded().expect("Decoding should succeed after conversion");
	assert_eq!(decoded.nid, nid, "Network id should match");
	assert_eq!(decoded.pid, pid, "Pallet id should match");
}

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
