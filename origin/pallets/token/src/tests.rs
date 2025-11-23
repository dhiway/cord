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
use codec::Encode;
use core::convert::TryInto;
use frame_support::{assert_err, assert_ok};
use sp_core::{sr25519, Pair, H256};
use sp_io::hashing::twox_128;
use sp_runtime::traits::UniqueSaturatedInto;

fn current_block_u32() -> u32 {
	frame_system::Pallet::<Test>::block_number().unique_saturated_into()
}

fn view_payload(
	account: &<Test as frame_system::Config>::AccountId,
	nonce: &[u8],
	reference_block: u32,
) -> Vec<u8> {
	let account_bytes = account.encode();
	let mut preimage = Vec::with_capacity(
		nonce.len() + account_bytes.len() + core::mem::size_of::<u32>() + b"token::tests".len(),
	);
	preimage.extend_from_slice(nonce);
	preimage.extend_from_slice(b"token::tests");
	preimage.extend_from_slice(&account_bytes);
	preimage.extend_from_slice(&reference_block.to_le_bytes());
	let digest = twox_128(&preimage);
	let mut payload = Vec::with_capacity(digest.len() + account_bytes.len() + 4);
	payload.extend_from_slice(&digest);
	payload.extend_from_slice(&account_bytes);
	payload.extend_from_slice(&reference_block.to_le_bytes());
	payload
}

fn make_auth_with_reference_block(
	payload: &[u8],
	pair: &sr25519::Pair,
	reference_block: u32,
) -> Authorization<Test> {
	let account: <Test as frame_system::Config>::AccountId = pair.public().into();
	let nonce = payload.to_vec();
	let vec_payload = view_payload(&account, &nonce, reference_block);
	let signature: Signature = pair.sign(&vec_payload).into();
	let bounded: AuthorizationPayloadOf<Test> =
		vec_payload.try_into().expect("payload within bounds");
	Authorization::<Test> { account, payload: bounded, signature }
}

fn make_auth(payload: &[u8], pair: &sr25519::Pair) -> Authorization<Test> {
	let reference_block = current_block_u32();
	make_auth_with_reference_block(payload, pair, reference_block)
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

#[test]
fn pallet_indexes_increment_without_gap() {
	new_test_ext().execute_with(|| {
		let first = Pallet::<Test>::get_or_add_pallet_index("First").unwrap();
		let second = Pallet::<Test>::get_or_add_pallet_index("Second").unwrap();
		assert_eq!(first + 1, second);
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

		assert_ok!(Pallet::<Test>::update_token_state(
			&token,
			digest,
			action.clone(),
			seal.clone()
		));

		let counter = StateVersion::<Test>::get(&token);
		assert_eq!(counter, 1);

		let record = StateHistory::<Test>::get(&token, 0).expect("An activity record should exist");
		assert_eq!(record.action, action);
		assert_eq!(record.digest, digest);
		assert_eq!(record.seal, seal);
	});
}

#[test]
fn history_requires_authorization() {
	new_test_ext().execute_with(|| {
		let token =
			Ss58Identifier::to_encoded(vec![5u8; 32], 250, 11, 0).expect("token encoding ok");
		let digest = H256::random();
		let action: EventTypeOf = b"log".to_vec().try_into().unwrap();
		Pallet::<Test>::update_token_state(
			&token,
			digest,
			action,
			EventBlock { height: 2, index: 0 },
		)
		.unwrap();

		let signer = sr25519::Pair::from_seed(&[21u8; 32]);
		let reference_block = current_block_u32();
		let auth = make_auth_with_reference_block(b"history", &signer, reference_block);
		let entries =
			Pallet::<Test>::history(auth.clone(), token.clone(), Some(0), 8).expect("entries");
		assert_eq!(entries.len(), 1);

		let ttl = <Test as Config>::MaxAuthorizationTTL::get();
		frame_system::Pallet::<Test>::set_block_number((reference_block + ttl + 1).into());
		let expired = Pallet::<Test>::history(auth, token.clone(), Some(0), 8);
		assert!(matches!(expired, Err(AuthorizationError::Expired)));
	});
}

#[test]
fn timeline_requires_valid_authorization() {
	new_test_ext().execute_with(|| {
		let id_digest = vec![2u8; 32];
		let token = Ss58Identifier::to_encoded(id_digest, 200, 7, 0).expect("token encoding ok");
		let digest = H256::random();
		let action: EventTypeOf = b"history".to_vec().try_into().unwrap();
		let seal = EventBlock { height: 5, index: 1 };
		Pallet::<Test>::update_token_state(&token, digest, action.clone(), seal.clone()).unwrap();

		let signer = sr25519::Pair::from_seed(&[42u8; 32]);
		let reference_block = current_block_u32();
		let auth = make_auth_with_reference_block(b"view-history", &signer, reference_block);
		let (entries, next) =
			Pallet::<Test>::timeline(auth.clone(), token.clone(), Some(0), Some(10))
				.expect("authorized timeline");
		assert_eq!(entries.len(), 1);
		assert_eq!(entries[0].digest, digest);
		assert!(next.is_none());

		let ttl = <Test as Config>::MaxAuthorizationTTL::get();
		frame_system::Pallet::<Test>::set_block_number((reference_block + ttl + 1).into());
		let expired = Pallet::<Test>::timeline(auth, token.clone(), Some(0), Some(10));
		assert!(matches!(expired, Err(AuthorizationError::Expired)));

		let forge = sr25519::Pair::from_seed(&[99u8; 32]);
		let mut forged = make_auth(b"view-history", &forge);
		forged.account = signer.public().into();
		let rejected = Pallet::<Test>::timeline(forged, token.clone(), Some(0), Some(10));
		assert!(
			matches!(rejected, Err(AuthorizationError::Unauthorized)),
			"invalid signature should be rejected"
		);

		let (raw, cursor) = Pallet::<Test>::timeline_entries(&token, Some(0), 10);
		assert_eq!(raw.len(), 1, "helpers remain accessible without auth");
		assert!(cursor.is_none());
	});
}

#[test]
fn resolve_identifier_requires_authorization() {
	new_test_ext().execute_with(|| {
		let digest = vec![3u8; 32];
		let token = Ss58Identifier::to_encoded(digest.clone(), 300, 9, 0).unwrap();
		let signer = sr25519::Pair::from_seed(&[7u8; 32]);
		let reference_block = current_block_u32();
		let auth = make_auth_with_reference_block(b"resolve-id", &signer, reference_block);
		let decoded = Pallet::<Test>::resolve_identifier(auth.clone(), token.clone())
			.expect("authorized view should succeed");
		assert_eq!(decoded.network, 300);
		assert_eq!(decoded.pallet, 9);
		assert_eq!(decoded.genesis, format!("0x{}", hex::encode(digest.clone())));

		let raw = Pallet::<Test>::resolve_identifier_plain(&token).expect("helper");
		assert_eq!(raw.network, 300);

		let ttl = <Test as Config>::MaxAuthorizationTTL::get();
		frame_system::Pallet::<Test>::set_block_number((reference_block + ttl + 1).into());
		assert!(matches!(
			Pallet::<Test>::resolve_identifier(auth, token.clone()),
			Err(AuthorizationError::Expired)
		));
	});
}

#[test]
fn resolve_identifier_query_expires_after_valid_until() {
	new_test_ext().execute_with(|| {
		let token = Ss58Identifier::to_encoded(vec![6u8; 32], 310, 12, 0).unwrap();
		let signer = sr25519::Pair::from_seed(&[8u8; 32]);
		let reference_block = current_block_u32();
		let auth = make_auth_with_reference_block(b"resolve-helper", &signer, reference_block);
		let decoded =
			Pallet::<Test>::resolve_identifier_query(auth.clone(), token.clone()).expect("helper");
		assert_eq!(decoded.pallet, 12);

		let ttl = <Test as Config>::MaxAuthorizationTTL::get();
		frame_system::Pallet::<Test>::set_block_number((reference_block + ttl + 1).into());
		let expired = Pallet::<Test>::resolve_identifier_query(auth, token.clone());
		assert!(matches!(expired, Err(AuthorizationError::Expired)));
	});
}

#[test]
fn resolve_pallet_query_enforces_signature() {
	new_test_ext().execute_with(|| {
		let index = Pallet::<Test>::get_or_add_pallet_index("Guarded").unwrap();
		let signer = sr25519::Pair::from_seed(&[55u8; 32]);
		let auth = make_auth(b"pallet-helper", &signer);
		let fetched =
			Pallet::<Test>::resolve_pallet_query(auth.clone(), index).expect("helper succeeds");
		assert_eq!(fetched, "Guarded");

		let mut forged = auth;
		forged.signature =
			sr25519::Pair::from_seed(&[99u8; 32]).sign(forged.payload.as_slice()).into();
		assert!(matches!(
			Pallet::<Test>::resolve_pallet_query(forged, index),
			Err(AuthorizationError::Unauthorized)
		));
	});
}

#[test]
fn resolve_pallet_requires_authorization() {
	new_test_ext().execute_with(|| {
		let signer = sr25519::Pair::from_seed(&[11u8; 32]);
		let pallet_name = "TokenView";
		let index = Pallet::<Test>::get_or_add_pallet_index(pallet_name).unwrap();
		let reference_block = current_block_u32();
		let auth = make_auth_with_reference_block(b"resolve-pallet", &signer, reference_block);
		let name =
			Pallet::<Test>::resolve_pallet(auth.clone(), index).expect("authorized pallet query");
		assert_eq!(name, pallet_name);

		let raw = Pallet::<Test>::resolve_pallet_plain(index).expect("helper");
		assert_eq!(raw, pallet_name);

		let ttl = <Test as Config>::MaxAuthorizationTTL::get();
		frame_system::Pallet::<Test>::set_block_number((reference_block + ttl + 1).into());
		assert!(matches!(
			Pallet::<Test>::resolve_pallet(auth, index),
			Err(AuthorizationError::Expired)
		));
	});
}

#[test]
fn pallet_index_views_roundtrip() {
	new_test_ext().execute_with(|| {
		let pair = sr25519::Pair::from_seed(&[13u8; 32]);
		let pallet_name = "IndexView";
		let index = Pallet::<Test>::get_or_add_pallet_index(pallet_name).unwrap();
		let fetched = Pallet::<Test>::pallet_index_of(
			make_auth(b"view-index", &pair),
			pallet_name.as_bytes().to_vec(),
		)
		.expect("pallet index view");
		assert_eq!(fetched, index);

		let name_bytes =
			Pallet::<Test>::pallet_name(make_auth(b"view-name", &pair), index).expect("name view");
		assert_eq!(core::str::from_utf8(&name_bytes).unwrap(), pallet_name);
	});
}

#[test]
fn network_metadata_views_return_values() {
	new_test_ext().execute_with(|| {
		let pair = sr25519::Pair::from_seed(&[14u8; 32]);
		NextPalletIndex::<Test>::put(9);
		GenesisNetworkId::<Test>::put(4321);

		let next =
			Pallet::<Test>::next_pallet_index(make_auth(b"next-index", &pair)).expect("next index");
		assert_eq!(next, 9);

		let nid =
			Pallet::<Test>::genesis_network_id(make_auth(b"net-id", &pair)).expect("network id");
		assert_eq!(nid, 4321);
	});
}

#[test]
fn state_views_roundtrip_with_authorization() {
	new_test_ext().execute_with(|| {
		let pair = sr25519::Pair::from_seed(&[15u8; 32]);
		let digest = H256::random();
		let token = Ss58Identifier::to_encoded(vec![4u8; 32], 400, 3, 0).unwrap();
		let action: EventTypeOf = b"state".to_vec().try_into().unwrap();
		let seal = EventBlock { height: 10, index: 0 };
		Pallet::<Test>::update_token_state(&token, digest, action.clone(), seal.clone()).unwrap();

		let version =
			Pallet::<Test>::state_version(make_auth(b"state-version", &pair), token.clone())
				.expect("state version view");
		assert_eq!(version, 1);

		let event = Pallet::<Test>::state_event(make_auth(b"state-event", &pair), token.clone(), 0)
			.expect("state event view");
		assert_eq!(event.action, action.to_vec());
		assert_eq!(event.seal.height, seal.height);
		assert_eq!(event.seal.index, seal.index);

		let (batch, cursor) = Pallet::<Test>::timeline(
			make_auth(b"state-events", &pair),
			token.clone(),
			Some(0),
			Some(5),
		)
		.expect("state events");
		assert_eq!(batch.len(), 1);
		assert_eq!(batch[0].digest, digest);
		assert_eq!(batch[0].action, action.to_vec());
		assert_eq!(batch[0].seal.height, seal.height);
		assert!(cursor.is_none());
	});
}
