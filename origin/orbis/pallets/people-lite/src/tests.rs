// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
	mock::{
		alias_target, exec_as_lite_alias_with_account_revised_tx,
		exec_as_lite_alias_with_account_tx, exec_as_lite_alias_with_proof_tx,
		exec_as_lite_person_tx, exec_signed_tx, exec_tx, mock_member_service_delete_collection,
		mock_member_service_fail_next_add_members, mock_member_service_members,
		mock_member_service_revision, new_test_ext, Extrinsic, RuntimeCall, RuntimeEvent, Test,
		TransactionExecutionError,
	},
	pallet::{AccountToAlias, AliasToAccount, AttestationAllowance, LitePeople},
	MemberOf, Pallet as PeopleLitePallet, ProofOf, LITE_PEOPLE_AUTH_CONTEXT,
	LITE_PEOPLE_MEMBER_IDENTIFIER, MSG_PREFIX,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::Hooks, weights::WeightMeter};
use frame_system::Pallet;
use indiv_support::traits::{Context, ContextualAlias, RevisedContextualAlias};
use sp_runtime::{transaction_validity::InvalidTransaction, DispatchError};
use verifiable::GenerateVerifiable;

const EXTENSION_VERSION: u8 = 0;

type SecretOfTest = <crate::CryptoOf<Test> as GenerateVerifiable>::Secret;

fn attest_msg(who: u64, key: MemberOf<Test>) -> Vec<u8> {
	[&MSG_PREFIX[..], &who.encode()[..], &key.encode()[..]].concat()
}

fn secret_from_seed(seed: u8) -> SecretOfTest {
	crate::CryptoOf::<Test>::new_secret([seed; 32])
}

fn member_from_secret(sec: &SecretOfTest) -> MemberOf<Test> {
	crate::CryptoOf::<Test>::member_from_secret(sec)
}

fn sign_attest_with_secret(sec: &SecretOfTest, who: u64) -> crate::types::SignatureOf<Test> {
	let key = member_from_secret(sec);
	crate::CryptoOf::<Test>::sign(sec, &attest_msg(who, key)[..]).expect("sign ok")
}

fn run_people_lite_on_poll() {
	let mut meter = WeightMeter::new();
	PeopleLitePallet::<Test>::on_poll(Pallet::<Test>::block_number(), &mut meter);
}

fn register_lite_person(verifier: u64, user: u64, seed: u8) -> SecretOfTest {
	run_people_lite_on_poll();
	AttestationAllowance::<Test>::insert(verifier, 1);
	let secret = secret_from_seed(seed);
	let ring_vrf_key = member_from_secret(&secret);
	let proof = sign_attest_with_secret(&secret, user);
	let candidate_signature: <Test as crate::Config>::AttestationSignature =
		sp_runtime::testing::UintAuthorityId(user);
	let call = RuntimeCall::PeopleLite(crate::Call::<Test>::attest {
		candidate: user,
		candidate_signature,
		ring_vrf_key,
		proof_of_ownership: proof,
		consumer_registration: None,
	});
	assert_ok!(exec_signed_tx(verifier, call));
	secret
}

fn alias_proof_for_call(
	secret: &SecretOfTest,
	call: &RuntimeCall,
	context: Context,
) -> (ProofOf<Test>, ContextualAlias) {
	let member = member_from_secret(secret);
	let members = mock_member_service_members(LITE_PEOPLE_MEMBER_IDENTIFIER);
	let commitment =
		crate::CryptoOf::<Test>::open((), &member, members.into_iter()).expect("open ok");
	let msg = (&EXTENSION_VERSION, call).using_encoded(sp_io::hashing::blake2_256);
	let (proof, alias) =
		crate::CryptoOf::<Test>::create(commitment, secret, &context[..], &msg).expect("proof ok");
	(proof, ContextualAlias { alias, context })
}

fn revised_alias_proof_for_call(
	secret: &SecretOfTest,
	signer: u64,
	nonce: u32,
	call: &RuntimeCall,
	context: Context,
) -> ProofOf<Test> {
	let member = member_from_secret(secret);
	let members = mock_member_service_members(LITE_PEOPLE_MEMBER_IDENTIFIER);
	let commitment =
		crate::CryptoOf::<Test>::open((), &member, members.into_iter()).expect("open ok");
	let inherited_implication = (EXTENSION_VERSION, call);
	let msg = (&inherited_implication, "revise", &signer, &nonce)
		.using_encoded(sp_io::hashing::blake2_256);
	let (proof, _) =
		crate::CryptoOf::<Test>::create(commitment, secret, &context[..], &msg).expect("proof ok");
	proof
}

fn establish_alias(
	secret: &SecretOfTest,
	alias_account: u64,
) -> (RuntimeCall, RevisedContextualAlias) {
	let call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
		account: alias_account,
		valid_at_block: Pallet::<Test>::block_number(),
	});
	let (proof, alias) = alias_proof_for_call(secret, &call, *LITE_PEOPLE_AUTH_CONTEXT);
	assert_ok!(exec_as_lite_alias_with_proof_tx(call.clone(), proof, 0));
	let rev_alias = RevisedContextualAlias {
		revision: mock_member_service_revision(LITE_PEOPLE_MEMBER_IDENTIFIER),
		ring: 0,
		ca: alias,
	};
	(call, rev_alias)
}

#[test]
fn increase_attestation_allowance_fails_for_wrong_origin() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);
		Pallet::<Test>::reset_events();
		let verifier = 42;
		let count = 5;
		let call = RuntimeCall::PeopleLite(crate::Call::<Test>::increase_attestation_allowance {
			account: verifier,
			count,
		});

		let err = exec_signed_tx(1, call).expect_err("non-root origin should fail");

		assert_eq!(err.unwrap_dispatch().error, DispatchError::BadOrigin);
		assert_eq!(AttestationAllowance::<Test>::get(verifier), 0);
		assert!(Pallet::<Test>::events()
			.iter()
			.all(|rec| !matches!(rec.event, crate::mock::RuntimeEvent::PeopleLite(_))));
	});
}

#[test]
fn increase_attestation_allowance_succeeds() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);
		let verifier = 7;
		let count = 10;

		assert_ok!(PeopleLitePallet::<Test>::increase_attestation_allowance(
			frame_system::RawOrigin::Root.into(),
			verifier,
			count,
		));

		assert_eq!(AttestationAllowance::<Test>::get(verifier), count);
		Pallet::<Test>::assert_last_event(
			crate::Event::<Test>::AttestationAllowanceIncreased { account: verifier, count }.into(),
		);
	});
}

#[test]
fn clear_attestation_allowance_fails_for_wrong_origin() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);
		Pallet::<Test>::reset_events();
		let verifier = 11;
		let call = RuntimeCall::PeopleLite(crate::Call::<Test>::clear_attestation_allowance {
			account: verifier,
		});

		let err = exec_signed_tx(1, call).expect_err("non-root origin should fail");

		assert_eq!(err.unwrap_dispatch().error, DispatchError::BadOrigin);
		assert!(Pallet::<Test>::events()
			.iter()
			.all(|rec| !matches!(rec.event, crate::mock::RuntimeEvent::PeopleLite(_))));
	});
}

#[test]
fn clear_attestation_allowance_succeeds() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);
		AttestationAllowance::<Test>::insert(12, 3);

		assert_ok!(PeopleLitePallet::<Test>::clear_attestation_allowance(
			frame_system::RawOrigin::Root.into(),
			12,
		));

		assert_eq!(AttestationAllowance::<Test>::get(12), 0);
		assert!(!AttestationAllowance::<Test>::contains_key(12));
		Pallet::<Test>::assert_last_event(
			crate::Event::<Test>::AllAttestationAllowanceCleared { verifier: 12 }.into(),
		);
	});
}

#[test]
fn attest_fails_when_collection_not_initialized() {
	new_test_ext().execute_with(|| {
		let verifier = 77;
		let candidate = 88;
		AttestationAllowance::<Test>::insert(verifier, 1);

		let secret = secret_from_seed(11);
		let ring_vrf_key = member_from_secret(&secret);
		let proof = sign_attest_with_secret(&secret, candidate);
		let candidate_signature: <Test as crate::Config>::AttestationSignature =
			sp_runtime::testing::UintAuthorityId(candidate);

		assert_noop!(
			PeopleLitePallet::<Test>::attest(
				Some(verifier).into(),
				candidate,
				candidate_signature,
				ring_vrf_key,
				proof,
				None,
			),
			crate::Error::<Test>::LitePeopleCollectionNotCreated
		);
		assert!(!LitePeople::<Test>::contains_key(candidate));
	});
}

#[test]
fn set_attestation_fails_for_wrong_origin() {
	new_test_ext().execute_with(|| {
		run_people_lite_on_poll();
		Pallet::<Test>::set_block_number(1);
		Pallet::<Test>::reset_events();
		let candidate = 300;

		let secret = secret_from_seed(11);
		let ring_vrf_key = member_from_secret(&secret);
		let proof = sign_attest_with_secret(&secret, candidate);
		let candidate_signature: <Test as crate::Config>::AttestationSignature =
			sp_runtime::testing::UintAuthorityId(candidate);

		let err = PeopleLitePallet::<Test>::attest(
			frame_system::RawOrigin::Root.into(),
			candidate,
			candidate_signature,
			ring_vrf_key,
			proof,
			None,
		)
		.expect_err("root origin is invalid for attest");

		assert_eq!(err, DispatchError::BadOrigin.into());
		assert!(Pallet::<Test>::events()
			.iter()
			.all(|rec| !matches!(rec.event, crate::mock::RuntimeEvent::PeopleLite(_))));
	});
}

#[test]
fn set_attestation_fails_for_no_attestation_allowance() {
	new_test_ext().execute_with(|| {
		run_people_lite_on_poll();
		Pallet::<Test>::set_block_number(1);
		Pallet::<Test>::reset_events();
		let verifier = 13;
		let candidate = 300;

		let secret = secret_from_seed(11);
		let ring_vrf_key = member_from_secret(&secret);
		let proof = sign_attest_with_secret(&secret, candidate);
		let candidate_signature: <Test as crate::Config>::AttestationSignature =
			sp_runtime::testing::UintAuthorityId(candidate);
		let call = RuntimeCall::PeopleLite(crate::Call::<Test>::attest {
			candidate,
			candidate_signature,
			ring_vrf_key,
			proof_of_ownership: proof,
			consumer_registration: None,
		});

		let err = exec_signed_tx(verifier, call).expect_err("missing allowance should fail");

		assert_eq!(
			err.unwrap_dispatch().error,
			crate::Error::<Test>::NoAttestationAllowance.into()
		);
		assert_eq!(AttestationAllowance::<Test>::get(verifier), 0);
		assert!(!LitePeople::<Test>::contains_key(candidate));
		assert!(Pallet::<Test>::events()
			.iter()
			.all(|rec| !matches!(rec.event, crate::mock::RuntimeEvent::PeopleLite(_))));
	});
}

#[test]
fn attest_rejects_invalid_proof_of_ownership() {
	new_test_ext().execute_with(|| {
		run_people_lite_on_poll();

		let candidate = 50;
		let verifier = 51;
		AttestationAllowance::<Test>::insert(verifier, 3);

		let secret_ok = secret_from_seed(1);
		let secret_wrong = secret_from_seed(2);
		let ring_vrf_key = member_from_secret(&secret_ok);
		let bad_proof = sign_attest_with_secret(&secret_wrong, candidate);
		let candidate_signature: <Test as crate::Config>::AttestationSignature =
			sp_runtime::testing::UintAuthorityId(candidate);

		assert_noop!(
			PeopleLitePallet::<Test>::attest(
				Some(verifier).into(),
				candidate,
				candidate_signature,
				ring_vrf_key,
				bad_proof,
				None,
			),
			crate::Error::<Test>::InvalidProofOfOwnership
		);
	});
}

#[test]
fn attest_rejects_invalid_attestation_signature() {
	new_test_ext().execute_with(|| {
		run_people_lite_on_poll();

		let candidate = 60;
		let verifier = 61;
		AttestationAllowance::<Test>::insert(verifier, 3);

		let secret = secret_from_seed(11);
		let ring_vrf_key = member_from_secret(&secret);
		let proof = sign_attest_with_secret(&secret, candidate);
		let bad_attestation_signature: <Test as crate::Config>::AttestationSignature =
			sp_runtime::testing::UintAuthorityId(candidate + 1);

		assert_noop!(
			PeopleLitePallet::<Test>::attest(
				Some(verifier).into(),
				candidate,
				bad_attestation_signature,
				ring_vrf_key,
				proof,
				None,
			),
			crate::Error::<Test>::InvalidAttestationSignature
		);
	});
}

#[test]
fn attest_rolls_back_when_add_members_fails() {
	new_test_ext().execute_with(|| {
		run_people_lite_on_poll();

		let verifier = 68;
		let candidate = 69;
		AttestationAllowance::<Test>::insert(verifier, 1);

		let secret = secret_from_seed(12);
		let ring_vrf_key = member_from_secret(&secret);
		let proof = sign_attest_with_secret(&secret, candidate);
		let candidate_signature: <Test as crate::Config>::AttestationSignature =
			sp_runtime::testing::UintAuthorityId(candidate);

		// Force add_members to fail. assert_noop! checks the error and that nothing
		// changed. Allowance still there, no person registered, no sufficients bump.
		mock_member_service_fail_next_add_members();
		assert_noop!(
			PeopleLitePallet::<Test>::attest(
				Some(verifier).into(),
				candidate,
				candidate_signature,
				ring_vrf_key,
				proof,
				None,
			),
			DispatchError::Other("mock add_members failed")
		);
	});
}

#[test]
fn attest_rejects_already_registered_candidate() {
	new_test_ext().execute_with(|| {
		let candidate = 70;
		let verifier = 71;
		register_lite_person(verifier, candidate, 7);
		assert!(LitePeople::<Test>::contains_key(candidate));
		AttestationAllowance::<Test>::insert(verifier, 3);

		let secret = secret_from_seed(8);
		let ring_vrf_key = member_from_secret(&secret);
		let proof = sign_attest_with_secret(&secret, candidate);
		let candidate_signature: <Test as crate::Config>::AttestationSignature =
			sp_runtime::testing::UintAuthorityId(candidate);

		assert_noop!(
			PeopleLitePallet::<Test>::attest(
				Some(verifier).into(),
				candidate,
				candidate_signature,
				ring_vrf_key,
				proof,
				None,
			),
			crate::Error::<Test>::AlreadyRegistered
		);
	});
}

#[test]
fn attest_rejects_duplicate_ring_vrf_key() {
	new_test_ext().execute_with(|| {
		let original_verifier = 80;
		let original_candidate = 81;
		let secret = register_lite_person(original_verifier, original_candidate, 13);
		let ring_vrf_key = member_from_secret(&secret);

		let verifier = 82;
		let candidate = 83;
		AttestationAllowance::<Test>::insert(verifier, 1);
		let proof = sign_attest_with_secret(&secret, candidate);
		let candidate_signature: <Test as crate::Config>::AttestationSignature =
			sp_runtime::testing::UintAuthorityId(candidate);

		assert_noop!(
			PeopleLitePallet::<Test>::attest(
				Some(verifier).into(),
				candidate,
				candidate_signature,
				ring_vrf_key,
				proof,
				None,
			),
			crate::Error::<Test>::KeyAlreadyInUse
		);
		assert_eq!(AttestationAllowance::<Test>::get(verifier), 1);
		assert!(!LitePeople::<Test>::contains_key(candidate));
	});
}

#[test]
fn as_lite_person_matches_main_behavior() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);

		let lite_account = 500;
		register_lite_person(1100, lite_account, 41);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(lite_account), 0);

		let nested = RuntimeCall::System(frame_system::Call::<Test>::remark_with_event {
			remark: b"lite-ok".to_vec(),
		});
		let outer = RuntimeCall::PeopleLite(crate::Call::<Test>::dispatch_as_signer {
			call: Box::new(nested),
		});
		assert_ok!(exec_as_lite_person_tx(lite_account, outer, 0));

		let found = Pallet::<Test>::events().iter().any(|rec| {
			matches!(
				rec.event,
				crate::mock::RuntimeEvent::System(frame_system::Event::Remarked { ref sender, .. })
					if *sender == lite_account
			)
		});
		assert!(found, "expected canonical lite account as downstream sender");
		assert_eq!(Pallet::<Test>::account_nonce(lite_account), 1);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(lite_account), 0);
		assert!(AccountToAlias::<Test>::get(lite_account).is_none());
	});
}

#[test]
fn set_alias_account_only_works_under_lite_alias_origin() {
	new_test_ext().execute_with(|| {
		let lite_account = 600;
		register_lite_person(1200, lite_account, 51);

		assert_noop!(
			PeopleLitePallet::<Test>::set_alias_account(
				frame_system::RawOrigin::Signed(700).into(),
				701,
				Pallet::<Test>::block_number(),
			),
			DispatchError::BadOrigin
		);
		assert_noop!(
			PeopleLitePallet::<Test>::set_alias_account(
				crate::Origin::<Test>::LitePerson(lite_account).into(),
				701,
				Pallet::<Test>::block_number(),
			),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn alias_establishment_via_proof_works() {
	new_test_ext().execute_with(|| {
		let lite_account = 700;
		let alias_account = 701;
		let secret = register_lite_person(1300, lite_account, 61);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(lite_account), 0);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(alias_account), 0);

		let (_, rev_alias) = establish_alias(&secret, alias_account);

		assert_eq!(AliasToAccount::<Test>::get(&rev_alias.ca), Some(alias_account));
		assert_eq!(AccountToAlias::<Test>::get(alias_account), Some(rev_alias.clone()));
		assert!(AccountToAlias::<Test>::get(lite_account).is_none());
		assert_ne!(AliasToAccount::<Test>::get(&rev_alias.ca), Some(lite_account));
		assert_eq!(Pallet::<Test>::account(alias_account).sufficients, 1);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(lite_account), 0);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(alias_account), 0);
	});
}

#[test]
fn unset_alias_account_clears_mappings_and_decrements_sufficients() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);

		let lite_account = 702;
		let alias_account = 703;
		let secret = register_lite_person(1305, lite_account, 66);
		let (_, rev_alias) = establish_alias(&secret, alias_account);

		assert_ok!(PeopleLitePallet::<Test>::unset_alias_account(
			crate::Origin::<Test>::LiteAlias(rev_alias.clone()).into(),
		));

		assert!(AliasToAccount::<Test>::get(&rev_alias.ca).is_none());
		assert!(AccountToAlias::<Test>::get(alias_account).is_none());
		assert_eq!(Pallet::<Test>::account(alias_account).sufficients, 0);
		Pallet::<Test>::assert_last_event(
			crate::Event::<Test>::AliasAccountUnset { alias: rev_alias.ca, account: alias_account }
				.into(),
		);
	});
}

#[test]
fn alias_proof_rejects_wrong_context() {
	new_test_ext().execute_with(|| {
		let lite_account = 710;
		let alias_account = 711;
		let secret = register_lite_person(1301, lite_account, 62);
		let invalid_context: Context = *b"pop:invalid.context/notallowed  ";

		let call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
			account: alias_account,
			valid_at_block: Pallet::<Test>::block_number(),
		});
		let (proof, _) = alias_proof_for_call(&secret, &call, invalid_context);
		let tx = Extrinsic::new_transaction(
			call,
			crate::PeopleLiteAuth::<Test>::new(Some(
				crate::PeopleLiteAuthData::AsLiteAliasWithProof(proof, 0, invalid_context),
			)),
		);

		let err = exec_tx(tx).expect_err("invalid context should fail");
		assert_eq!(err, TransactionExecutionError::from(InvalidTransaction::Call));
		assert!(AccountToAlias::<Test>::get(alias_account).is_none());
	});
}

#[test]
fn alias_proof_rejects_non_set_alias_account_calls() {
	new_test_ext().execute_with(|| {
		let lite_account = 712;
		let secret = register_lite_person(1303, lite_account, 64);
		let call = RuntimeCall::System(frame_system::Call::<Test>::remark_with_event {
			remark: b"not-alias-setup".to_vec(),
		});
		let (proof, _) = alias_proof_for_call(&secret, &call, *LITE_PEOPLE_AUTH_CONTEXT);

		let err = exec_as_lite_alias_with_proof_tx(call, proof, 0)
			.expect_err("proof auth must only allow set_alias_account");

		assert_eq!(err, TransactionExecutionError::from(InvalidTransaction::Call));
	});
}

#[test]
fn as_lite_alias_with_account_resolves_fresh_account_to_alias() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);

		let lite_account = 720;
		let first_alias_account = 721;
		let second_alias_account = 722;
		let secret = register_lite_person(1302, lite_account, 63);
		let (_, rev_alias) = establish_alias(&secret, first_alias_account);

		let rotate_call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
			account: second_alias_account,
			valid_at_block: Pallet::<Test>::block_number(),
		});
		assert_ok!(exec_as_lite_alias_with_account_tx(first_alias_account, rotate_call, 0,));

		assert_eq!(AliasToAccount::<Test>::get(&rev_alias.ca), Some(second_alias_account));
		assert!(AccountToAlias::<Test>::get(first_alias_account).is_none());
		assert_eq!(AccountToAlias::<Test>::get(second_alias_account), Some(rev_alias));
		// Nonce went to 1 in prepare, but rotating the alias drops sufficients to 0,
		// the account gets reaped, and the nonce resets.
		assert_eq!(Pallet::<Test>::account_nonce(first_alias_account), 0);
		assert_eq!(Pallet::<Test>::account_nonce(lite_account), 0);
		assert_eq!(Pallet::<Test>::account(first_alias_account).sufficients, 0);
		assert_eq!(Pallet::<Test>::account(second_alias_account).sufficients, 1);
	});
}

#[test]
fn as_lite_alias_with_account_dispatches_general_call_under_alias_origin() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);

		let lite_account = 723;
		let alias_account = 724;
		let secret = register_lite_person(1304, lite_account, 65);
		let (_, rev_alias) = establish_alias(&secret, alias_account);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(alias_account), 0);
		let payload = b"alias-general-call".to_vec();
		let call = RuntimeCall::AliasTarget(alias_target::Call::<Test>::record {
			payload: payload.clone(),
		});

		assert_ok!(exec_as_lite_alias_with_account_tx(alias_account, call, 0));

		let found = Pallet::<Test>::events().iter().any(|rec| {
			matches!(
				rec.event,
				RuntimeEvent::AliasTarget(alias_target::Event::Recorded { ref alias, payload: ref event_payload })
					if *alias == rev_alias.ca && *event_payload == payload
			)
		});
		assert!(found, "expected downstream pallet call under lite alias origin");
		assert_eq!(Pallet::<Test>::account_nonce(alias_account), 1);
		assert_eq!(Pallet::<Test>::account_nonce(lite_account), 0);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(alias_account), 0);
		assert_eq!(AccountToAlias::<Test>::get(alias_account), Some(rev_alias));
	});
}

#[test]
fn alias_account_without_binding_is_rejected() {
	new_test_ext().execute_with(|| {
		let call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
			account: 801,
			valid_at_block: Pallet::<Test>::block_number(),
		});

		let err = exec_as_lite_alias_with_account_tx(800, call, 0)
			.expect_err("unbound signer should fail");
		assert_eq!(err, TransactionExecutionError::from(InvalidTransaction::Custom(175)));
	});
}

#[test]
fn stale_alias_requires_revised_variant() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);

		let lite_account = 900;
		let old_alias_account = 901;
		let new_alias_account = 902;
		let secret = register_lite_person(1400, lite_account, 71);
		let (_, rev_alias) = establish_alias(&secret, old_alias_account);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(old_alias_account), 0);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(new_alias_account), 0);

		// Adding another lite person bumps the ring revision, so the old alias is now stale.
		register_lite_person(1401, 903, 72);

		let rotate_call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
			account: new_alias_account,
			valid_at_block: Pallet::<Test>::block_number(),
		});
		let stale = exec_as_lite_alias_with_account_tx(old_alias_account, rotate_call.clone(), 0)
			.expect_err("stale alias should fail");
		assert_eq!(stale, TransactionExecutionError::from(InvalidTransaction::Custom(172)));

		let proof = revised_alias_proof_for_call(
			&secret,
			old_alias_account,
			0,
			&rotate_call,
			*LITE_PEOPLE_AUTH_CONTEXT,
		);
		assert_ok!(exec_as_lite_alias_with_account_revised_tx(
			old_alias_account,
			rotate_call,
			0,
			proof,
			0,
		));

		let refreshed = RevisedContextualAlias {
			revision: mock_member_service_revision(LITE_PEOPLE_MEMBER_IDENTIFIER),
			ring: 0,
			ca: rev_alias.ca.clone(),
		};
		assert_eq!(AliasToAccount::<Test>::get(&rev_alias.ca), Some(new_alias_account));
		assert!(AccountToAlias::<Test>::get(old_alias_account).is_none());
		assert_eq!(AccountToAlias::<Test>::get(new_alias_account), Some(refreshed));
		assert_eq!(Pallet::<Test>::account_nonce(old_alias_account), 0);
		assert_eq!(Pallet::<Test>::account(new_alias_account).sufficients, 1);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(old_alias_account), 0);
		assert_eq!(pallet_balances::Pallet::<Test>::free_balance(new_alias_account), 0);
	});
}

#[test]
fn alias_account_rejects_missing_ring_revision() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);

		let lite_account = 805;
		let alias_account = 806;
		let secret = register_lite_person(1402, lite_account, 69);
		establish_alias(&secret, alias_account);
		// Nuke the collection so the ring is gone. Can't happen through normal operations,
		// but we need to make sure the tx gets rejected if it does.
		mock_member_service_delete_collection(LITE_PEOPLE_MEMBER_IDENTIFIER);

		let call = RuntimeCall::System(frame_system::Call::<Test>::remark_with_event {
			remark: b"stale-ring".to_vec(),
		});
		let err = exec_as_lite_alias_with_account_tx(alias_account, call, 0)
			.expect_err("missing ring revision should be treated as stale alias");

		assert_eq!(err, TransactionExecutionError::from(InvalidTransaction::Custom(172)));
	});
}

#[test]
fn revised_alias_mismatch_is_rejected() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);

		let lite_account = 950;
		let alias_account = 951;
		let secret = register_lite_person(1450, lite_account, 73);
		establish_alias(&secret, alias_account);

		let other_secret = register_lite_person(1451, 952, 74);

		let rotate_call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
			account: 953,
			valid_at_block: Pallet::<Test>::block_number(),
		});
		let mismatched_proof = revised_alias_proof_for_call(
			&other_secret,
			alias_account,
			0,
			&rotate_call,
			*LITE_PEOPLE_AUTH_CONTEXT,
		);

		let err = exec_as_lite_alias_with_account_revised_tx(
			alias_account,
			rotate_call,
			0,
			mismatched_proof,
			0,
		)
		.expect_err("mismatched proof should fail");
		assert_eq!(err, TransactionExecutionError::from(InvalidTransaction::Custom(174)));
	});
}

#[test]
fn set_alias_account_rejects_canonical_lite_account_as_alias_account() {
	new_test_ext().execute_with(|| {
		let lite_account = 980;
		let secret = register_lite_person(1480, lite_account, 76);

		let call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
			account: lite_account,
			valid_at_block: Pallet::<Test>::block_number(),
		});
		let (proof, _) = alias_proof_for_call(&secret, &call, *LITE_PEOPLE_AUTH_CONTEXT);
		let err = exec_as_lite_alias_with_proof_tx(call, proof, 0)
			.expect_err("canonical lite account must not enter alias storage");

		assert_eq!(err.unwrap_dispatch().error, crate::Error::<Test>::AccountInUse.into());
		assert!(AccountToAlias::<Test>::get(lite_account).is_none());
		assert!(AliasToAccount::<Test>::iter().next().is_none());
	});
}

#[test]
fn set_alias_account_rejects_idempotent_rebinding() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(1);

		let lite_account = 981;
		let alias_account = 982;
		let secret = register_lite_person(1481, lite_account, 79);
		let (_, rev_alias) = establish_alias(&secret, alias_account);

		assert_noop!(
			PeopleLitePallet::<Test>::set_alias_account(
				crate::Origin::<Test>::LiteAlias(rev_alias).into(),
				alias_account,
				Pallet::<Test>::block_number(),
			),
			crate::Error::<Test>::AliasAccountAlreadySet
		);
	});
}

#[test]
fn set_alias_account_enforces_time_window() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(10);

		let lite_account = 990;
		let initial_alias_account = 991;
		let next_alias_account = 992;
		let secret = register_lite_person(1490, lite_account, 77);
		let (_, rev_alias) = establish_alias(&secret, initial_alias_account);

		assert_noop!(
			PeopleLitePallet::<Test>::set_alias_account(
				crate::Origin::<Test>::LiteAlias(rev_alias.clone()).into(),
				next_alias_account,
				Pallet::<Test>::block_number() + 1,
			),
			crate::Error::<Test>::CallBlockOutOfRange
		);

		Pallet::<Test>::set_block_number(
			PeopleLitePallet::<Test>::account_setup_block_tolerance() + 1,
		);
		assert_noop!(
			PeopleLitePallet::<Test>::set_alias_account(
				crate::Origin::<Test>::LiteAlias(rev_alias.clone()).into(),
				next_alias_account,
				0,
			),
			crate::Error::<Test>::CallBlockOutOfRange
		);

		assert_eq!(AliasToAccount::<Test>::get(&rev_alias.ca), Some(initial_alias_account));
		assert!(AccountToAlias::<Test>::get(next_alias_account).is_none());
	});
}

#[test]
fn alias_proof_rejects_future_and_stale_setup_calls() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(10);

		let lite_account = 993;
		let secret = register_lite_person(1491, lite_account, 78);

		let future_call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
			account: 994,
			valid_at_block: Pallet::<Test>::block_number() + 1,
		});
		let (future_proof, _) =
			alias_proof_for_call(&secret, &future_call, *LITE_PEOPLE_AUTH_CONTEXT);
		let future_err = exec_as_lite_alias_with_proof_tx(future_call, future_proof, 0)
			.expect_err("future alias setup should fail validation");
		assert_eq!(future_err, TransactionExecutionError::from(InvalidTransaction::Future));

		Pallet::<Test>::set_block_number(
			PeopleLitePallet::<Test>::account_setup_block_tolerance() + 1,
		);
		let stale_call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
			account: 995,
			valid_at_block: 0,
		});
		let (stale_proof, _) =
			alias_proof_for_call(&secret, &stale_call, *LITE_PEOPLE_AUTH_CONTEXT);
		let stale_err = exec_as_lite_alias_with_proof_tx(stale_call, stale_proof, 0)
			.expect_err("stale alias setup should fail validation");
		assert_eq!(stale_err, TransactionExecutionError::from(InvalidTransaction::Stale));
	});
}

#[test]
fn alias_proof_replay_is_rejected_as_stale() {
	new_test_ext().execute_with(|| {
		Pallet::<Test>::set_block_number(10);

		let lite_account = 996;
		let alias_account = 997;
		let secret = register_lite_person(1492, lite_account, 80);
		let call = RuntimeCall::PeopleLite(crate::Call::<Test>::set_alias_account {
			account: alias_account,
			valid_at_block: Pallet::<Test>::block_number(),
		});
		let (proof, _) = alias_proof_for_call(&secret, &call, *LITE_PEOPLE_AUTH_CONTEXT);
		let replayed_proof = proof.clone();

		assert_ok!(exec_as_lite_alias_with_proof_tx(call.clone(), proof, 0));

		let replay_err = exec_as_lite_alias_with_proof_tx(call, replayed_proof, 0)
			.expect_err("replaying the same alias proof should be stale");
		assert_eq!(replay_err, TransactionExecutionError::from(InvalidTransaction::Stale));
	});
}

#[test]
fn dispatch_as_signer_succeeds_for_lite_person_origin() {
	new_test_ext().execute_with(|| {
		let lite_account = 870;
		let nested = RuntimeCall::System(frame_system::Call::<Test>::remark_with_event {
			remark: b"lite-origin-ok".to_vec(),
		});

		assert_ok!(PeopleLitePallet::<Test>::dispatch_as_signer(
			crate::Origin::<Test>::LitePerson(lite_account).into(),
			Box::new(nested),
		));
	});
}

#[test]
fn dispatch_as_signer_fails_for_wrong_origin() {
	new_test_ext().execute_with(|| {
		let nested = RuntimeCall::System(frame_system::Call::<Test>::remark_with_event {
			remark: b"nope".to_vec(),
		});

		let err = PeopleLitePallet::<Test>::dispatch_as_signer(
			frame_system::RawOrigin::Root.into(),
			Box::new(nested),
		)
		.expect_err("root origin should fail");

		assert_eq!(err.error, DispatchError::BadOrigin);
	});
}
