// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Tests for hop-promotion pallet.

use crate::{mock::*, signing_payload};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::Authorize};
use sp_io::hashing::blake2_256;
use sp_keyring::Sr25519Keyring;
use sp_runtime::{
	transaction_validity::{InvalidTransaction, TransactionSource},
	AccountId32, MultiSignature, MultiSigner,
};

const TEST_TIMESTAMP_MS: u64 = 1_700_000_000_000;

fn authorized_origin() -> RuntimeOrigin {
	frame_system::Origin::<Test>::Authorized.into()
}

/// Build a `(signer, signature)` pair where `keyring` signs the payload for `(data, ts)`.
fn signed_by(
	keyring: Sr25519Keyring,
	data: &[u8],
	submit_timestamp: u64,
) -> (MultiSigner, MultiSignature) {
	let payload = signing_payload(&blake2_256(data), submit_timestamp);
	let sig = keyring.sign(&payload);
	(MultiSigner::Sr25519(keyring.public()), MultiSignature::Sr25519(sig))
}

fn dummy_signer_and_sig() -> (MultiSigner, MultiSignature) {
	(
		MultiSigner::Sr25519(Sr25519Keyring::Alice.public()),
		MultiSignature::Sr25519(Default::default()),
	)
}

fn authorize_account(who: AccountId32, transactions: u32, bytes: u64) {
	assert_ok!(pallet_bulletin_transaction_storage::Pallet::<Test>::authorize_account(
		RuntimeOrigin::root(),
		who,
		transactions,
		bytes,
	));
}

fn set_now(ms: u64) {
	pallet_timestamp::Pallet::<Test>::set_timestamp(ms);
}

fn make_promote_call(
	data: Vec<u8>,
	signer: MultiSigner,
	signature: MultiSignature,
	submit_timestamp: u64,
) -> RuntimeCall {
	RuntimeCall::HopPromotion(crate::Call::promote { data, signer, signature, submit_timestamp })
}

// ---- Dispatch tests ----

#[test]
fn promote_succeeds_with_valid_data() {
	new_test_ext().execute_with(|| {
		System::run_to_block::<AllPalletsWithSystem>(1);
		frame_system::Pallet::<Test>::set_extrinsic_index(0);
		let data = vec![42u8; 100];
		let (signer, sig) = dummy_signer_and_sig();
		assert_ok!(HopPromotion::promote(authorized_origin(), signer, sig, 0, data));
	});
}

#[test]
fn promote_rejects_empty_data() {
	new_test_ext().execute_with(|| {
		System::run_to_block::<AllPalletsWithSystem>(1);
		frame_system::Pallet::<Test>::set_extrinsic_index(0);
		let (signer, sig) = dummy_signer_and_sig();
		assert_noop!(
			HopPromotion::promote(authorized_origin(), signer, sig, 0, vec![]),
			pallet_bulletin_transaction_storage::Error::<Test>::BadDataSize,
		);
	});
}

#[test]
fn promote_rejects_oversized_data() {
	new_test_ext().execute_with(|| {
		System::run_to_block::<AllPalletsWithSystem>(1);
		frame_system::Pallet::<Test>::set_extrinsic_index(0);
		let (signer, sig) = dummy_signer_and_sig();
		assert_noop!(
			HopPromotion::promote(
				authorized_origin(),
				signer,
				sig,
				0,
				vec![0u8; TEST_MAX_TRANSACTION_SIZE as usize + 1],
			),
			pallet_bulletin_transaction_storage::Error::<Test>::BadDataSize,
		);
	});
}

#[test]
fn promote_rejects_non_authorized_origins() {
	new_test_ext().execute_with(|| {
		System::run_to_block::<AllPalletsWithSystem>(1);
		let data = vec![42u8; 100];
		let (signer, sig) = dummy_signer_and_sig();
		assert_noop!(
			HopPromotion::promote(
				RuntimeOrigin::none(),
				signer.clone(),
				sig.clone(),
				0,
				data.clone(),
			),
			sp_runtime::traits::BadOrigin,
		);
		assert_noop!(
			HopPromotion::promote(
				RuntimeOrigin::signed(Sr25519Keyring::Alice.to_account_id()),
				signer.clone(),
				sig.clone(),
				0,
				data.clone(),
			),
			sp_runtime::traits::BadOrigin,
		);
		assert_noop!(
			HopPromotion::promote(RuntimeOrigin::root(), signer, sig, 0, data),
			sp_runtime::traits::BadOrigin,
		);
	});
}

// ---- Authorize closure: source / data size ----

#[test]
fn authorize_rejects_external_source() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		let data = vec![1u8; 100];
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, TEST_TIMESTAMP_MS);
		let call = make_promote_call(data, signer, sig, TEST_TIMESTAMP_MS);
		assert_eq!(
			call.authorize(TransactionSource::External),
			Some(Err(InvalidTransaction::Call.into())),
		);
	});
}

#[test]
fn authorize_rejects_empty_data() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &[], TEST_TIMESTAMP_MS);
		let call = make_promote_call(vec![], signer, sig, TEST_TIMESTAMP_MS);
		assert_eq!(
			call.authorize(TransactionSource::Local),
			Some(Err(InvalidTransaction::Custom(0).into())),
		);
	});
}

#[test]
fn authorize_rejects_oversized_data() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		let data = vec![0u8; TEST_MAX_TRANSACTION_SIZE as usize + 1];
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, TEST_TIMESTAMP_MS);
		let call = make_promote_call(data, signer, sig, TEST_TIMESTAMP_MS);
		assert_eq!(
			call.authorize(TransactionSource::Local),
			Some(Err(InvalidTransaction::Custom(0).into())),
		);
	});
}

// ---- Authorize closure: signature, account, timestamp ----

#[test]
fn authorize_accepts_valid_signature_and_active_auth() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		authorize_account(Sr25519Keyring::Alice.to_account_id(), 1, data.len() as u64);

		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, TEST_TIMESTAMP_MS);
		let call = make_promote_call(data, signer, sig, TEST_TIMESTAMP_MS);
		assert!(matches!(call.authorize(TransactionSource::Local), Some(Ok(_))));
	});
}

#[test]
fn authorize_rejects_bad_signature() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		authorize_account(Sr25519Keyring::Alice.to_account_id(), 1, data.len() as u64);

		// Sign different data, then submit with the original data.
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &[7u8; 50], TEST_TIMESTAMP_MS);
		let call = make_promote_call(data, signer, sig, TEST_TIMESTAMP_MS);
		assert_eq!(
			call.authorize(TransactionSource::Local),
			Some(Err(InvalidTransaction::BadProof.into())),
		);
	});
}

#[test]
fn authorize_rejects_signer_mismatch() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		authorize_account(Sr25519Keyring::Alice.to_account_id(), 1, data.len() as u64);

		// Bob signs, but the call advertises Alice as the signer.
		let bob_sig =
			Sr25519Keyring::Bob.sign(&signing_payload(&blake2_256(&data), TEST_TIMESTAMP_MS));
		let call = make_promote_call(
			data,
			MultiSigner::Sr25519(Sr25519Keyring::Alice.public()),
			MultiSignature::Sr25519(bob_sig),
			TEST_TIMESTAMP_MS,
		);
		assert_eq!(
			call.authorize(TransactionSource::Local),
			Some(Err(InvalidTransaction::BadProof.into())),
		);
	});
}

#[test]
fn authorize_rejects_unauthorized_account() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		// Note: no authorize_account for Alice.
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, TEST_TIMESTAMP_MS);
		let call = make_promote_call(data, signer, sig, TEST_TIMESTAMP_MS);
		assert_eq!(
			call.authorize(TransactionSource::Local),
			Some(Err(InvalidTransaction::BadSigner.into())),
		);
	});
}

#[test]
fn authorize_accepts_fully_consumed_unexpired_authorization() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);
		frame_system::Pallet::<Test>::set_extrinsic_index(0);

		let alice = Sr25519Keyring::Alice.to_account_id();
		let data = vec![1u8; 100];
		// Authorize exactly enough for one store call, then spend it.
		authorize_account(alice.clone(), 1, data.len() as u64);
		let store_call =
			pallet_bulletin_transaction_storage::Call::<Test>::store { data: data.clone() };
		assert_ok!(pallet_bulletin_transaction_storage::Pallet::<Test>::pre_dispatch_signed(
			&alice,
			&store_call,
		));

		// Allowance is fully spent but the entry is still in storage and unexpired,
		// so HOP promotion is still permitted.
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, TEST_TIMESTAMP_MS);
		let call = make_promote_call(data, signer, sig, TEST_TIMESTAMP_MS);
		assert!(matches!(call.authorize(TransactionSource::Local), Some(Ok(_))));
	});
}

#[test]
fn authorize_rejects_expired_account_authorization() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		authorize_account(Sr25519Keyring::Alice.to_account_id(), 1, data.len() as u64);

		// Run past the auth period (10 blocks in mock).
		run_to_block(20);

		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, TEST_TIMESTAMP_MS);
		let call = make_promote_call(data, signer, sig, TEST_TIMESTAMP_MS);
		assert_eq!(
			call.authorize(TransactionSource::Local),
			Some(Err(InvalidTransaction::BadSigner.into())),
		);
	});
}

#[test]
fn authorize_rejects_timestamp_too_old() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		authorize_account(Sr25519Keyring::Alice.to_account_id(), 1, data.len() as u64);

		let stale_ts = TEST_TIMESTAMP_MS - TEST_SUBMIT_TIMESTAMP_TOLERANCE_MS - 1;
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, stale_ts);
		let call = make_promote_call(data, signer, sig, stale_ts);
		assert_eq!(
			call.authorize(TransactionSource::Local),
			Some(Err(InvalidTransaction::Stale.into())),
		);
	});
}

#[test]
fn authorize_rejects_timestamp_too_far_in_future() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		authorize_account(Sr25519Keyring::Alice.to_account_id(), 1, data.len() as u64);

		let future_ts = TEST_TIMESTAMP_MS + TEST_SUBMIT_TIMESTAMP_TOLERANCE_MS + 1;
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, future_ts);
		let call = make_promote_call(data, signer, sig, future_ts);
		assert_eq!(
			call.authorize(TransactionSource::Local),
			Some(Err(InvalidTransaction::Stale.into())),
		);
	});
}

#[test]
fn authorize_accepts_timestamp_at_window_boundary() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		authorize_account(Sr25519Keyring::Alice.to_account_id(), 1, data.len() as u64);

		let edge_ts = TEST_TIMESTAMP_MS - TEST_SUBMIT_TIMESTAMP_TOLERANCE_MS;
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, edge_ts);
		let call = make_promote_call(data, signer, sig, edge_ts);
		assert!(matches!(call.authorize(TransactionSource::Local), Some(Ok(_))));
	});
}

#[test]
fn authorize_rejects_signature_for_different_timestamp() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		authorize_account(Sr25519Keyring::Alice.to_account_id(), 1, data.len() as u64);

		// Sign for one timestamp, submit with another (both within window).
		let signed_ts = TEST_TIMESTAMP_MS;
		let claimed_ts = TEST_TIMESTAMP_MS - 1_000;
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, signed_ts);
		let call = make_promote_call(data, signer, sig, claimed_ts);
		assert_eq!(
			call.authorize(TransactionSource::Local),
			Some(Err(InvalidTransaction::BadProof.into())),
		);
	});
}

#[test]
fn authorize_valid_transaction_properties() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);

		let data = vec![1u8; 100];
		authorize_account(Sr25519Keyring::Alice.to_account_id(), 1, data.len() as u64);

		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, TEST_TIMESTAMP_MS);
		let call = make_promote_call(data.clone(), signer, sig, TEST_TIMESTAMP_MS);
		let (valid_tx, weight) = call.authorize(TransactionSource::Local).unwrap().unwrap();
		assert_eq!(valid_tx.priority, 0);
		assert_eq!(valid_tx.longevity, 5);
		assert!(!valid_tx.propagate);
		assert_eq!(weight, frame_support::weights::Weight::zero());
		let hash = sp_io::hashing::blake2_256(&data);
		let expected_tag = ("HopPromotion", hash).encode();
		assert!(valid_tx.provides.contains(&expected_tag));
	});
}

#[test]
fn promote_has_lower_priority_than_store_and_renew() {
	new_test_ext().execute_with(|| {
		set_now(TEST_TIMESTAMP_MS);
		System::run_to_block::<AllPalletsWithSystem>(1);
		frame_system::Pallet::<Test>::set_extrinsic_index(0);

		// Authorize Alice for store + renew + promote.
		let alice = Sr25519Keyring::Alice.to_account_id();
		let data = vec![2u8; 100];
		authorize_account(alice.clone(), 2, 2 * data.len() as u64);

		// Get promote priority.
		let (signer, sig) = signed_by(Sr25519Keyring::Alice, &data, TEST_TIMESTAMP_MS);
		let promote_call = make_promote_call(data.clone(), signer, sig, TEST_TIMESTAMP_MS);
		let (promote_tx, _) = promote_call.authorize(TransactionSource::Local).unwrap().unwrap();

		// Get store priority.
		let store_call =
			pallet_bulletin_transaction_storage::Call::<Test>::store { data: data.clone() };
		let (store_tx, _) = pallet_bulletin_transaction_storage::Pallet::<Test>::validate_signed(
			&alice,
			&store_call,
		)
		.unwrap();

		// Store data so we can renew it.
		assert_ok!(pallet_bulletin_transaction_storage::Pallet::<Test>::store(
			RuntimeOrigin::none(),
			data,
		));

		// Advance so the stored transaction is available for renew.
		run_to_block(3);

		let renew_call = pallet_bulletin_transaction_storage::Call::<Test>::renew {
			entry: pallet_bulletin_transaction_storage::TransactionRef::Position {
				block: 1,
				index: 0,
			},
		};
		let (renew_tx, _) = pallet_bulletin_transaction_storage::Pallet::<Test>::validate_signed(
			&alice,
			&renew_call,
		)
		.unwrap();

		assert!(
			promote_tx.priority < store_tx.priority,
			"promote priority ({}) must be strictly less than store priority ({})",
			promote_tx.priority,
			store_tx.priority,
		);
		assert!(
			promote_tx.priority < renew_tx.priority,
			"promote priority ({}) must be strictly less than renew priority ({})",
			promote_tx.priority,
			renew_tx.priority,
		);
	});
}
