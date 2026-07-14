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

use crate::{
	delegated_revoke_signing_payload, delegated_signing_payload, mock::*, AttestationCount,
	AttestationIdBatchOf, AttestationInput, Attestations, BatchOf, CreatorSchemas, DelegatedAction,
	DelegatedIntent, DelegatedIssueBatchOf, DelegatedRevokeIntent, EmergencyPaused, Error,
	ExternalStatuses, IndexPolicy, IssuerAttestations, NextDelegatedNonce,
	NextIssuerAttestationNonce, Pallet, SchemaCount, SchemaDefinitionOf, SchemaStatus, Schemas,
	SignedDelegatedIntent, SubjectSchemaAttestations, UniquenessIndex,
};
use frame_support::{assert_noop, assert_ok, traits::StorageVersion, BoundedVec};
use sp_keyring::Sr25519Keyring;
use sp_runtime::{traits::Hash as _, MultiSignature};

fn account(key: Sr25519Keyring) -> sp_runtime::AccountId32 {
	key.to_account_id()
}

fn hash(value: &[u8]) -> <Test as frame_system::Config>::Hash {
	<Test as frame_system::Config>::Hashing::hash(value)
}

fn create_schema(
	creator: Sr25519Keyring,
	issuers: &[Sr25519Keyring],
) -> <Test as frame_system::Config>::Hash {
	create_schema_with_flags(creator, issuers, true, true, IndexPolicy::IssuerAndSubjectSchema)
}

fn create_schema_with_flags(
	creator: Sr25519Keyring,
	issuers: &[Sr25519Keyring],
	revocable: bool,
	unique: bool,
	index_policy: IndexPolicy,
) -> <Test as frame_system::Config>::Hash {
	let definition: SchemaDefinitionOf<Test> = b"immutable-schema-v1".to_vec().try_into().unwrap();
	let authorized: BoundedVec<_, MaxAuthorizedIssuers> =
		issuers.iter().copied().map(account).collect::<Vec<_>>().try_into().unwrap();
	let definition_commitment = hash(definition.as_slice());
	let schema = Pallet::<Test>::schema_id(
		&account(creator),
		&definition_commitment,
		revocable,
		unique,
		index_policy,
	);
	assert_ok!(Attestation::create_schema(
		RuntimeOrigin::signed(account(creator)),
		definition,
		authorized,
		revocable,
		unique,
		index_policy,
	));
	schema
}

#[test]
fn empty_batches_are_rejected_before_mutation() {
	new_test_ext().execute_with(|| {
		let empty_issues: BatchOf<Test> = vec![].try_into().unwrap();
		assert_noop!(
			Attestation::issue_batch(
				RuntimeOrigin::signed(account(Sr25519Keyring::Alice)),
				empty_issues,
			),
			Error::<Test>::EmptyBatch
		);
		let empty_revokes: AttestationIdBatchOf<Test> = vec![].try_into().unwrap();
		assert_noop!(
			Attestation::revoke_batch(
				RuntimeOrigin::signed(account(Sr25519Keyring::Alice)),
				empty_revokes,
			),
			Error::<Test>::EmptyBatch
		);
	});
}

#[test]
fn schema_flags_enforce_uniqueness_revocability_and_index_policy() {
	new_test_ext().execute_with(|| {
		let schema =
			create_schema_with_flags(Sr25519Keyring::Alice, &[], false, false, IndexPolicy::None);
		let issuer = account(Sr25519Keyring::Alice);
		let mut claim = input(schema, 42);
		assert_noop!(
			Attestation::issue(RuntimeOrigin::signed(issuer.clone()), claim.clone()),
			Error::<Test>::RevocableMismatch
		);
		claim.revocable = false;
		assert_noop!(
			Attestation::issue(RuntimeOrigin::signed(issuer.clone()), claim.clone()),
			Error::<Test>::UnexpectedUniquenessCommitment
		);
		claim.uniqueness_commitment = None;
		let id = Pallet::<Test>::attestation_id(&issuer, &claim, 0);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(issuer.clone()), claim));
		assert!(IssuerAttestations::<Test>::get(&issuer).is_empty());
		assert_noop!(
			Attestation::revoke(RuntimeOrigin::signed(issuer), id),
			Error::<Test>::Irrevocable
		);
	});
}

#[test]
fn delegated_revoke_consumes_nonce_and_rejects_replay() {
	new_test_ext().execute_with(|| {
		let schema = create_schema(Sr25519Keyring::Alice, &[Sr25519Keyring::Bob]);
		let issuer = account(Sr25519Keyring::Bob);
		let delegate = account(Sr25519Keyring::Charlie);
		let claim = input(schema, 50);
		let id = Pallet::<Test>::attestation_id(&issuer, &claim, 0);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(issuer.clone()), claim));
		let intent = DelegatedRevokeIntent::<Test> {
			genesis_hash: System::block_hash(0),
			spec_version: 0,
			action: DelegatedAction::Revoke,
			revoker: issuer.clone(),
			delegate: delegate.clone(),
			attestation: id,
			nonce: 0,
			deadline: 10,
		};
		let signature = MultiSignature::Sr25519(
			Sr25519Keyring::Bob.sign(&delegated_revoke_signing_payload::<Test>(&intent)),
		);
		assert_ok!(Attestation::revoke_delegated(
			RuntimeOrigin::signed(delegate.clone()),
			intent.clone(),
			signature.clone(),
		));
		assert_eq!(NextDelegatedNonce::<Test>::get(&issuer), 1);
		assert_noop!(
			Attestation::revoke_delegated(RuntimeOrigin::signed(delegate), intent, signature,),
			Error::<Test>::InvalidNonce
		);
	});
}

#[test]
fn delegated_issue_batch_verifies_every_signature_before_mutation() {
	new_test_ext().execute_with(|| {
		let schema = create_schema(Sr25519Keyring::Alice, &[Sr25519Keyring::Bob]);
		let issuer = account(Sr25519Keyring::Bob);
		let delegate = account(Sr25519Keyring::Charlie);
		let make_intent = |claim: AttestationInput<Test>, nonce| DelegatedIntent::<Test> {
			genesis_hash: System::block_hash(0),
			spec_version: 0,
			action: DelegatedAction::Issue,
			issuer: issuer.clone(),
			delegate: delegate.clone(),
			schema,
			subject_commitment: claim.subject_commitment,
			payload_commitment: claim.payload_commitment,
			status_commitment: claim.status_commitment,
			parent: claim.parent,
			expiry: claim.expiry,
			uniqueness_commitment: claim.uniqueness_commitment,
			revocable: claim.revocable,
			nonce,
			deadline: 10,
		};
		let first = make_intent(input(schema, 60), 0);
		let second = make_intent(input(schema, 61), 1);
		let items: DelegatedIssueBatchOf<Test> = vec![
			SignedDelegatedIntent {
				signature: MultiSignature::Sr25519(
					Sr25519Keyring::Bob.sign(&delegated_signing_payload::<Test>(&first)),
				),
				intent: first,
			},
			SignedDelegatedIntent {
				signature: MultiSignature::Sr25519(
					Sr25519Keyring::Charlie.sign(&delegated_signing_payload::<Test>(&second)),
				),
				intent: second,
			},
		]
		.try_into()
		.unwrap();
		assert_noop!(
			Attestation::issue_delegated_batch(RuntimeOrigin::signed(delegate), items),
			Error::<Test>::InvalidSignature
		);
		assert_eq!(NextDelegatedNonce::<Test>::get(issuer), 0);
		assert_eq!(Attestations::<Test>::iter().count(), 0);
	});
}

fn input(schema: <Test as frame_system::Config>::Hash, tag: u8) -> AttestationInput<Test> {
	AttestationInput {
		schema,
		subject_commitment: hash(&[tag, 1]),
		payload_commitment: hash(&[tag, 2]),
		status_commitment: hash(&[tag, 3]),
		parent: None,
		expiry: Some(System::block_number() + 10),
		uniqueness_commitment: Some(hash(&[tag, 4])),
		revocable: true,
	}
}

#[test]
fn genesis_is_empty_at_storage_version_one_and_direct_issue_is_indexed() {
	new_test_ext().execute_with(|| {
		assert_eq!(StorageVersion::get::<Attestation>(), StorageVersion::new(1));
		assert_eq!(Schemas::<Test>::iter().count(), 0);
		assert_eq!(Attestations::<Test>::iter().count(), 0);
		assert_eq!(SchemaCount::<Test>::get(), 0);
		assert_eq!(AttestationCount::<Test>::get(), 0);

		let schema = create_schema(Sr25519Keyring::Alice, &[Sr25519Keyring::Bob]);
		let issuer = account(Sr25519Keyring::Bob);
		let claim = input(schema, 7);
		let id = Pallet::<Test>::attestation_id(&issuer, &claim, 0);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(issuer.clone()), claim.clone()));

		let record = Attestations::<Test>::get(id).unwrap();
		assert_eq!(record.payload_commitment, claim.payload_commitment);
		assert_eq!(
			CreatorSchemas::<Test>::get(account(Sr25519Keyring::Alice)).as_slice(),
			&[schema]
		);
		assert_eq!(IssuerAttestations::<Test>::get(&issuer).as_slice(), &[id]);
		assert_eq!(
			SubjectSchemaAttestations::<Test>::get(schema, claim.subject_commitment).as_slice(),
			&[id]
		);
		assert_eq!(
			UniquenessIndex::<Test>::get(schema, claim.uniqueness_commitment.unwrap()),
			Some(id)
		);
		assert!(Pallet::<Test>::is_live(id));
		assert_eq!(SchemaCount::<Test>::get(), 1);
		assert_eq!(AttestationCount::<Test>::get(), 1);
		assert_eq!(NextIssuerAttestationNonce::<Test>::get(&issuer), 1);
		assert_eq!(record.issuance_nonce, 0);
		assert!(System::events().iter().any(|record| matches!(
			&record.event,
			RuntimeEvent::Attestation(crate::Event::AttestationIssued {
				attestation,
				schema: event_schema,
				issuer: event_issuer,
				subject_commitment,
			}) if *attestation == id && *event_schema == schema && event_issuer == &issuer && *subject_commitment == claim.subject_commitment
		)));
	});
}

#[test]
fn non_unique_schema_allows_identical_repeat_issuance_with_nonce_ids() {
	new_test_ext().execute_with(|| {
		let schema = create_schema_with_flags(
			Sr25519Keyring::Alice,
			&[],
			true,
			false,
			IndexPolicy::IssuerAndSubjectSchema,
		);
		let issuer = account(Sr25519Keyring::Alice);
		let mut claim = input(schema, 70);
		claim.uniqueness_commitment = None;
		let first_id = Pallet::<Test>::attestation_id(&issuer, &claim, 0);
		let second_id = Pallet::<Test>::attestation_id(&issuer, &claim, 1);
		assert_ne!(first_id, second_id);

		assert_ok!(Attestation::issue(RuntimeOrigin::signed(issuer.clone()), claim.clone()));
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(issuer.clone()), claim));

		assert_eq!(Attestations::<Test>::get(first_id).unwrap().issuance_nonce, 0);
		assert_eq!(Attestations::<Test>::get(second_id).unwrap().issuance_nonce, 1);
		assert_eq!(NextIssuerAttestationNonce::<Test>::get(&issuer), 2);
		assert_eq!(AttestationCount::<Test>::get(), 2);
		assert_eq!(IssuerAttestations::<Test>::get(&issuer).as_slice(), &[first_id, second_id]);
	});
}

#[test]
fn unique_attestation_id_uses_unique_key_not_issuance_nonce() {
	new_test_ext().execute_with(|| {
		let schema = create_schema(Sr25519Keyring::Alice, &[]);
		let issuer = account(Sr25519Keyring::Alice);
		let claim = input(schema, 71);
		assert_eq!(
			Pallet::<Test>::attestation_id(&issuer, &claim, 0),
			Pallet::<Test>::attestation_id(&issuer, &claim, 99),
		);
	});
}

#[test]
fn external_status_revocations_are_issuer_scoped_immutable_and_atomic() {
	new_test_ext().execute_with(|| {
		let alice = account(Sr25519Keyring::Alice);
		let bob = account(Sr25519Keyring::Bob);
		let first = hash(b"external-status-1");
		let second = hash(b"external-status-2");
		let third = hash(b"external-status-3");
		let alice_first = Pallet::<Test>::external_status_key(&alice, first);
		let bob_first = Pallet::<Test>::external_status_key(&bob, first);
		assert_ne!(alice_first, bob_first);

		assert_ok!(Attestation::revoke_external_status(
			RuntimeOrigin::signed(alice.clone()),
			first
		));
		let first_record = ExternalStatuses::<Test>::get(alice_first).unwrap();
		assert_eq!(first_record.issuer, alice);
		assert_eq!(first_record.status_commitment, first);
		assert_eq!(first_record.revoked_at, 1);
		assert!(System::events().iter().any(|record| matches!(
			&record.event,
			RuntimeEvent::Attestation(crate::Event::ExternalStatusRevoked {
				key,
				issuer,
				status_commitment,
				revoked_at,
			}) if *key == alice_first && issuer == &alice && *status_commitment == first && *revoked_at == 1
		)));
		assert_noop!(
			Attestation::revoke_external_status(RuntimeOrigin::signed(alice.clone()), first),
			Error::<Test>::ExternalStatusAlreadyRevoked
		);
		assert_ok!(Attestation::revoke_external_status(RuntimeOrigin::signed(bob), first));

		let batch: AttestationIdBatchOf<Test> = vec![second, first].try_into().unwrap();
		assert_noop!(
			Attestation::revoke_external_status_batch(RuntimeOrigin::signed(alice.clone()), batch),
			Error::<Test>::ExternalStatusAlreadyRevoked
		);
		assert!(!ExternalStatuses::<Test>::contains_key(Pallet::<Test>::external_status_key(
			&alice, second
		)));

		let batch: AttestationIdBatchOf<Test> = vec![second, third].try_into().unwrap();
		assert_ok!(Attestation::revoke_external_status_batch(
			RuntimeOrigin::signed(alice.clone()),
			batch
		));
		assert!(ExternalStatuses::<Test>::contains_key(Pallet::<Test>::external_status_key(
			&alice, second
		)));
		assert!(ExternalStatuses::<Test>::contains_key(Pallet::<Test>::external_status_key(
			&alice, third
		)));
	});
}

#[test]
fn delegated_intent_binds_chain_action_parties_claim_nonce_and_deadline() {
	new_test_ext().execute_with(|| {
		let schema = create_schema(Sr25519Keyring::Alice, &[Sr25519Keyring::Bob]);
		let issuer = account(Sr25519Keyring::Bob);
		let delegate = account(Sr25519Keyring::Charlie);
		let claim = input(schema, 9);
		let intent = DelegatedIntent::<Test> {
			genesis_hash: System::block_hash(0),
			spec_version: 0,
			action: DelegatedAction::Issue,
			issuer: issuer.clone(),
			delegate: delegate.clone(),
			schema,
			subject_commitment: claim.subject_commitment,
			payload_commitment: claim.payload_commitment,
			status_commitment: claim.status_commitment,
			parent: claim.parent,
			expiry: claim.expiry,
			uniqueness_commitment: claim.uniqueness_commitment,
			revocable: claim.revocable,
			nonce: 0,
			deadline: System::block_number() + 5,
		};
		let signature = MultiSignature::Sr25519(
			Sr25519Keyring::Bob.sign(&delegated_signing_payload::<Test>(&intent)),
		);
		assert_ok!(Attestation::issue_delegated(
			RuntimeOrigin::signed(delegate.clone()),
			intent.clone(),
			signature.clone(),
		));
		assert_eq!(NextDelegatedNonce::<Test>::get(&issuer), 1);
		assert_noop!(
			Attestation::issue_delegated(
				RuntimeOrigin::signed(delegate.clone()),
				intent,
				signature
			),
			Error::<Test>::InvalidNonce
		);

		let mut tampered = DelegatedIntent::<Test> {
			genesis_hash: System::block_hash(0),
			spec_version: 0,
			action: DelegatedAction::Issue,
			issuer,
			delegate: delegate.clone(),
			schema,
			subject_commitment: hash(b"different-subject"),
			payload_commitment: hash(b"payload"),
			status_commitment: hash(b"status"),
			parent: None,
			expiry: Some(20),
			uniqueness_commitment: None,
			revocable: true,
			nonce: 1,
			deadline: 10,
		};
		let bound_signature = MultiSignature::Sr25519(
			Sr25519Keyring::Bob.sign(&delegated_signing_payload::<Test>(&tampered)),
		);
		tampered.status_commitment = hash(b"tampered-after-signing");
		assert_noop!(
			Attestation::issue_delegated(
				RuntimeOrigin::signed(delegate.clone()),
				tampered,
				bound_signature,
			),
			Error::<Test>::InvalidSignature
		);

		System::set_block_number(11);
		let expired = DelegatedIntent::<Test> {
			genesis_hash: System::block_hash(0),
			spec_version: 0,
			action: DelegatedAction::Issue,
			issuer: account(Sr25519Keyring::Bob),
			delegate,
			schema,
			subject_commitment: hash(b"subject-2"),
			payload_commitment: hash(b"payload-2"),
			status_commitment: hash(b"status-2"),
			parent: None,
			expiry: Some(20),
			uniqueness_commitment: None,
			revocable: true,
			nonce: 1,
			deadline: 10,
		};
		let expired_signature = MultiSignature::Sr25519(
			Sr25519Keyring::Bob.sign(&delegated_signing_payload::<Test>(&expired)),
		);
		assert_noop!(
			Attestation::issue_delegated(
				RuntimeOrigin::signed(account(Sr25519Keyring::Charlie)),
				expired,
				expired_signature,
			),
			Error::<Test>::IntentExpired
		);
	});
}

#[test]
fn bounded_batch_is_atomic_on_uniqueness_collision() {
	new_test_ext().execute_with(|| {
		let schema = create_schema(Sr25519Keyring::Alice, &[]);
		let issuer = account(Sr25519Keyring::Alice);
		let first = input(schema, 1);
		let mut second = input(schema, 2);
		second.uniqueness_commitment = first.uniqueness_commitment;
		let batch: BatchOf<Test> = vec![first.clone(), second].try_into().unwrap();
		assert_noop!(
			Attestation::issue_batch(RuntimeOrigin::signed(issuer.clone()), batch),
			Error::<Test>::UniquenessAlreadyUsed
		);
		assert_eq!(Attestations::<Test>::iter().count(), 0);
		assert!(IssuerAttestations::<Test>::get(issuer).is_empty());
		assert!(SubjectSchemaAttestations::<Test>::get(schema, first.subject_commitment).is_empty());
		assert!(!UniquenessIndex::<Test>::contains_key(
			schema,
			first.uniqueness_commitment.unwrap()
		));
	});
}

#[test]
fn parent_expiry_revocation_and_emergency_controls_fail_closed() {
	new_test_ext().execute_with(|| {
		let schema = create_schema(Sr25519Keyring::Alice, &[Sr25519Keyring::Bob]);
		let issuer = account(Sr25519Keyring::Bob);
		let parent = input(schema, 3);
		let parent_id = Pallet::<Test>::attestation_id(&issuer, &parent, 0);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(issuer.clone()), parent));

		let mut child = input(schema, 4);
		child.parent = Some(parent_id);
		assert_ok!(Attestation::issue(RuntimeOrigin::signed(issuer.clone()), child));
		assert_ok!(Attestation::revoke(RuntimeOrigin::signed(issuer.clone()), parent_id));
		assert!(!Pallet::<Test>::is_live(parent_id));
		let mut rejected_child = input(schema, 5);
		rejected_child.parent = Some(parent_id);
		assert_noop!(
			Attestation::issue(RuntimeOrigin::signed(issuer.clone()), rejected_child),
			Error::<Test>::AttestationRevoked
		);

		assert_ok!(Attestation::set_emergency_pause(RuntimeOrigin::root(), true));
		assert!(EmergencyPaused::<Test>::get());
		assert_noop!(
			Attestation::issue(RuntimeOrigin::signed(issuer), input(schema, 6)),
			Error::<Test>::EmergencyPaused
		);
		assert_ok!(Attestation::force_schema_status(
			RuntimeOrigin::root(),
			schema,
			SchemaStatus::Retired,
		));
		assert_eq!(Schemas::<Test>::get(schema).unwrap().status, SchemaStatus::Retired);
	});
}
