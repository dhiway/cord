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
use core::convert::TryInto;
use frame_support::{assert_err, assert_ok};
use sp_core::{sr25519, Pair, H256};

fn make_auth(payload: &[u8], pair: &sr25519::Pair) -> ViewAuthorization<Test> {
	let vec_payload = payload.to_vec();
	let signature: Signature = pair.sign(&vec_payload).into();
	let bounded: ViewAuthPayloadOf<Test> = vec_payload.try_into().expect("payload within bounds");
	ViewAuthorization::<Test> { account: pair.public().into(), payload: bounded, signature }
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
		let token = Ss58Identifier::to_encoded(id_digest, 100, 5, 0)
			.expect("Token encoding should succeed");
		let digest = H256::random();

		let action: EventTypeOf =
			vec![1u8; 10].try_into().expect("Should create a valid bounded vector");
		let seal = EventBlock { height: 1, index: 0 };

		assert_ok!(Pallet::<Test>::state_event(&token, digest, action.clone(), seal.clone()));

		let counter = StateVersion::<Test>::get(&token);
		assert_eq!(counter, 1);

		let record = StateHistory::<Test>::get(&token, 0).expect("An activity record should exist");
		assert_eq!(record.action, action);
		assert_eq!(record.digest, digest);
		assert_eq!(record.seal, seal);
	});
}

#[test]
fn timeline_view_requires_valid_authorization() {
	new_test_ext().execute_with(|| {
		let id_digest = vec![2u8; 32];
		let token = Ss58Identifier::to_encoded(id_digest, 200, 7, 0).expect("token encoding ok");
		let digest = H256::random();
		let action: EventTypeOf = b"history".to_vec().try_into().unwrap();
		let seal = EventBlock { height: 5, index: 1 };
		Pallet::<Test>::state_event(&token, digest, action.clone(), seal.clone()).unwrap();

		let signer = sr25519::Pair::from_seed(&[42u8; 32]);
		let auth = make_auth(b"view-history", &signer);
		let bytes = Pallet::<Test>::timeline(auth.clone(), token.clone(), Some(0), Some(10))
			.expect("authorized timeline");
		let body = core::str::from_utf8(&bytes).expect("utf8");
		assert!(body.contains("\"digestHex\""));
		assert!(body.contains(&format!("0x{}", hex::encode(digest))));

		let replay = Pallet::<Test>::timeline(auth, token.clone(), Some(0), Some(10));
		assert!(replay.is_none(), "reused authorizations must be rejected");

		let forge = sr25519::Pair::from_seed(&[99u8; 32]);
		let mut forged = make_auth(b"view-history", &forge);
		forged.account = signer.public().into();
		let rejected = Pallet::<Test>::timeline(forged, token.clone(), Some(0), Some(10));
		assert!(rejected.is_none(), "invalid signature should be rejected");

		let raw = Pallet::<Test>::timeline_entries(&token, Some(0), 10);
		assert_eq!(raw.len(), 1, "helpers remain accessible without auth");
	});
}

#[test]
fn resolve_identifier_view_requires_authorization() {
	new_test_ext().execute_with(|| {
		let digest = vec![3u8; 32];
		let token = Ss58Identifier::to_encoded(digest.clone(), 300, 9, 0).unwrap();
		let signer = sr25519::Pair::from_seed(&[7u8; 32]);
		let auth = make_auth(b"resolve-id", &signer);
		let bytes = Pallet::<Test>::resolve_identifier(auth.clone(), token.clone())
			.expect("authorized view should succeed");
		let body = core::str::from_utf8(&bytes).expect("utf8");
		assert!(body.contains("\"network\":300"));
		assert!(body.contains("\"pallet\":9"));
		assert!(body.contains(&format!("0x{}", hex::encode(digest.clone()))));

		let raw = Pallet::<Test>::resolve_identifier_plain(&token).expect("helper");
		assert_eq!(raw.network, 300);

		assert!(Pallet::<Test>::resolve_identifier(auth, token.clone()).is_none());
	});
}

#[test]
fn resolve_pallet_view_requires_authorization() {
	new_test_ext().execute_with(|| {
		let signer = sr25519::Pair::from_seed(&[11u8; 32]);
		let pallet_name = "TokenView";
		let index = Pallet::<Test>::get_or_add_pallet_index(pallet_name).unwrap();
		let auth = make_auth(b"resolve-pallet", &signer);
		let bytes =
			Pallet::<Test>::resolve_pallet(auth.clone(), index).expect("authorized pallet view");
		let body = core::str::from_utf8(&bytes).expect("utf8");
		assert!(body.contains(pallet_name));

		let raw = Pallet::<Test>::resolve_pallet_plain(index).expect("helper");
		assert_eq!(raw, pallet_name);

		assert!(Pallet::<Test>::resolve_pallet(auth, index).is_none());
	});
}
