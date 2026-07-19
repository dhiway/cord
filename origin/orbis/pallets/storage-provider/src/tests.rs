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
	mock::*, AgreementCapacityState, AgreementStatus, Agreements, BucketAgreements,
	BucketHostDelegations, BucketIds, BucketSnapshots, Buckets, CanonicalManifests,
	CapabilityCidOf, CapabilityMethodsOf, CapabilityProductIdOf, CapacityReleases,
	ChallengeBacklog, ChallengeStatus, CheckpointClaims, CheckpointDutyCurrent, CheckpointDutyMode,
	CheckpointDutyPending, CheckpointErrorCode, CheckpointFallbackPromotionErrorCode,
	CheckpointFallbackPromotionReceiptByBucket, CheckpointFallbackPromotionV1, CommitmentPayloadV2,
	CommitmentState, CommitmentV1, ConfirmationsOf, DutyAdmissionCount, EquivocationEvidence,
	Error, Event, GovernedFinalizedCheckpoint, GrantNonce, HostDelegations,
	ManifestDeletionAcknowledgements, ManifestDeletionRequirements, MmrLeafV1, MmrProofV1,
	OrganizationRefOf, ProviderAgreements, ProviderAuthorityError, ProviderBucketAssignmentCount,
	ProviderEvidence, ProviderEvidenceOverflow, ProviderOrganizationRefV1, ProviderStatus,
	Providers, ReplicaSignature, ReplicasOf, ServiceKeyOwner,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::Hooks};
use pallet_orbis_storage_control_primitives::CanonicalStorageControl;
use sp_core::{ed25519, Pair, H256};
use sp_runtime::traits::Hash;

const OWNER: u64 = 9;
const DOMAIN: &[u8] = b"cord/storage/checkpoint/v2";

fn pair(id: u8) -> ed25519::Pair {
	ed25519::Pair::from_seed(&[id; 32])
}

fn organization(key: &ed25519::Public) -> OrganizationRefOf<Test> {
	ProviderOrganizationRefV1 {
		entity_id: b"enterprise".to_vec().try_into().unwrap(),
		attestation_id: H256::from(sp_io::hashing::blake2_256(key.as_ref())),
		schema_id: H256::repeat_byte(2),
		sla_commitment: H256::repeat_byte(3),
		sla_version: 1,
		valid_from: 1,
		valid_until: 1_000,
		rotation_predecessor: None,
	}
}

fn register(id: u64, capacity: u64) {
	let pair = pair(id as u8);
	assert_ok!(StorageProvider::register_provider(
		RuntimeOrigin::root(),
		id,
		format!("https://provider-{id}").into_bytes().try_into().unwrap(),
		pair.public(),
		organization(&pair.public()),
		capacity,
	));
}

fn setup_bucket() -> H256 {
	for id in 1..=4 {
		register(id, 10_000);
	}
	let replicas: ReplicasOf<Test> = vec![2, 3].try_into().unwrap();
	assert_ok!(StorageProvider::create_bucket(
		RuntimeOrigin::signed(OWNER),
		H256::repeat_byte(10),
		1,
		replicas,
	));
	BucketIds::<Test>::get()[0]
}

fn product(value: &[u8]) -> CapabilityProductIdOf<Test> {
	value.to_vec().try_into().unwrap()
}

fn methods(value: &[u16]) -> CapabilityMethodsOf<Test> {
	value.to_vec().try_into().unwrap()
}

fn capability_cid(value: &[u8]) -> CapabilityCidOf<Test> {
	value.to_vec().try_into().unwrap()
}

fn payload(
	bucket_id: H256,
	root: H256,
	start_seq: u64,
	leaf_count: u64,
	nonce: u64,
) -> CommitmentPayloadV2<H256, u64> {
	CommitmentPayloadV2 {
		version: 2,
		bucket_id,
		commitment: CommitmentV1 { mmr_root: root, start_seq, leaf_count },
		nonce,
	}
}

fn digest(domain: &[u8], payload: &CommitmentPayloadV2<H256, u64>) -> [u8; 32] {
	let mut bytes = domain.to_vec();
	payload.encode_to(&mut bytes);
	sp_io::hashing::blake2_256(&bytes)
}

fn signed_checkpoint(
	payload: CommitmentPayloadV2<H256, u64>,
) -> (ed25519::Signature, ed25519::Signature, ConfirmationsOf<Test>) {
	signed_checkpoint_for(payload, 1, &[2, 3])
}

fn signed_checkpoint_for(
	payload: CommitmentPayloadV2<H256, u64>,
	primary: u64,
	confirming: &[u64],
) -> (ed25519::Signature, ed25519::Signature, ConfirmationsOf<Test>) {
	let digest = digest(DOMAIN, &payload);
	let context_digest = StorageProvider::checkpoint_context_for(&payload)
		.map(|context| StorageProvider::checkpoint_context_digest(&context))
		.unwrap_or(digest);
	let primary_signature = pair(primary as u8).sign(&digest);
	let confirmations = confirming
		.iter()
		.copied()
		.map(|provider| ReplicaSignature {
			provider,
			service_key: pair(provider as u8).public(),
			signature: pair(provider as u8).sign(&digest),
			context_signature: pair(provider as u8).sign(&context_digest),
		})
		.collect::<Vec<_>>()
		.try_into()
		.unwrap();
	(primary_signature, pair(primary as u8).sign(&context_digest), confirmations)
}

fn signed_promotion(
	bucket_id: H256,
	snapshot_nonce: u64,
	provider: u64,
) -> (CheckpointFallbackPromotionV1<H256, u64>, ed25519::Signature) {
	let duty = StorageProvider::checkpoint_duty_at(bucket_id, snapshot_nonce).unwrap();
	let payload = CheckpointFallbackPromotionV1 {
		version: 1,
		bucket_id,
		snapshot_nonce,
		duty_id: StorageProvider::checkpoint_duty_id(&duty, snapshot_nonce),
	};
	let signature =
		pair(provider as u8).sign(&StorageProvider::checkpoint_promotion_digest(&payload));
	(payload, signature)
}

fn submit(payload: CommitmentPayloadV2<H256, u64>) {
	let (signature, context_signature, confirmations) = signed_checkpoint(payload);
	assert_ok!(StorageProvider::submit_checkpoint(
		RuntimeOrigin::signed(1),
		DOMAIN.to_vec().try_into().unwrap(),
		payload,
		101,
		121,
		pair(1).public(),
		signature,
		context_signature,
		confirmations,
	));
}

#[test]
fn exact_checkpoint_error_codes_are_frozen() {
	new_test_ext().execute_with(|| {
		let cases = [
			(Error::<Test>::StorageCheckpointWrongDomain, 220),
			(Error::<Test>::StorageCheckpointWrongVersion, 221),
			(Error::<Test>::StorageCheckpointWrongBucket, 222),
			(Error::<Test>::StorageCheckpointWrongKey, 223),
			(Error::<Test>::StorageCheckpointStaleNonce, 224),
			(Error::<Test>::StorageCheckpointWrongWindow, 225),
			(Error::<Test>::StorageCheckpointInsufficientQuorum, 239),
			(Error::<Test>::StorageCheckpointSequenceInvalid, 240),
			(Error::<Test>::StorageCheckpointEquivocation, 241),
			(Error::<Test>::StorageCheckpointWrongContext, 242),
		];
		for (error, code) in cases {
			assert_eq!(StorageProvider::checkpoint_error_code(&error), Some(code));
		}
		assert_eq!(CheckpointErrorCode::StorageCheckpointEquivocation as u16, 241);
		assert_eq!(CheckpointErrorCode::StorageCheckpointWrongContext as u16, 242);
		assert_eq!(
			StorageProvider::checkpoint_promotion_error_code(
				&Error::<Test>::StorageCheckpointWrongContext
			),
			None
		);
	});
}

#[test]
fn exact_checkpoint_promotion_error_codes_are_additive_and_disjoint() {
	new_test_ext().execute_with(|| {
		let cases = [
			(Error::<Test>::CheckpointFallbackPromotionWrongVersion, 243),
			(Error::<Test>::CheckpointFallbackPromotionWrongDuty, 244),
			(Error::<Test>::CheckpointFallbackPromotionWrongKey, 245),
			(Error::<Test>::CheckpointFallbackPromotionNotAllowed, 246),
		];
		for (error, code) in cases {
			assert_eq!(StorageProvider::checkpoint_promotion_error_code(&error), Some(code));
			assert_eq!(StorageProvider::checkpoint_error_code(&error), None);
		}
		assert_eq!(CheckpointFallbackPromotionErrorCode::WrongVersion as u16, 243);
		assert_eq!(CheckpointFallbackPromotionErrorCode::WrongDuty as u16, 244);
		assert_eq!(CheckpointFallbackPromotionErrorCode::WrongKey as u16, 245);
		assert_eq!(CheckpointFallbackPromotionErrorCode::NotAllowed as u16, 246);
	});
}

#[test]
fn organization_admission_rejects_every_invalid_authority_case_without_state() {
	let cases = [
		(ProviderAuthorityError::OrganizationUnknown, Error::<Test>::ProviderOrgUnknown),
		(ProviderAuthorityError::AttestationInvalid, Error::<Test>::ProviderAttestationInvalid),
		(ProviderAuthorityError::AttestationExpired, Error::<Test>::ProviderAttestationExpired),
		(ProviderAuthorityError::SlaInvalid, Error::<Test>::ProviderSlaInvalid),
		(ProviderAuthorityError::ServiceKeyInvalid, Error::<Test>::ProviderServiceKeyInvalid),
	];
	for (authority_error, pallet_error) in cases {
		new_test_ext().execute_with(|| {
			set_authority_failure(Some(authority_error));
			let key = pair(1).public();
			assert_noop!(
				StorageProvider::register_provider(
					RuntimeOrigin::root(),
					1,
					b"endpoint".to_vec().try_into().unwrap(),
					key,
					organization(&key),
					100,
				),
				pallet_error
			);
			assert!(!Providers::<Test>::contains_key(1));
			assert!(System::events().is_empty());
		});
	}
}

#[test]
fn governed_finality_is_authorized_monotonic_and_fails_closed_when_uninitialized() {
	new_test_ext().execute_with(|| {
		GovernedFinalizedCheckpoint::<Test>::kill();
		assert_eq!(StorageProvider::governed_finalized_checkpoint(), None);
		let key = pair(1).public();
		assert_noop!(
			StorageProvider::register_provider(
				RuntimeOrigin::root(),
				1,
				b"endpoint".to_vec().try_into().unwrap(),
				key,
				organization(&key),
				100,
			),
			Error::<Test>::FinalizedCheckpointUninitialized
		);
		assert!(!Providers::<Test>::contains_key(1));

		System::set_block_number(10);
		assert_noop!(
			StorageProvider::advance_finalized_checkpoint(RuntimeOrigin::signed(OWNER), 5),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_noop!(
			StorageProvider::advance_finalized_checkpoint(RuntimeOrigin::root(), 11),
			Error::<Test>::FinalizedCheckpointInFuture
		);
		assert_ok!(StorageProvider::advance_finalized_checkpoint(RuntimeOrigin::root(), 5));
		assert_eq!(StorageProvider::governed_finalized_checkpoint(), Some(5));
		assert_noop!(
			StorageProvider::advance_finalized_checkpoint(RuntimeOrigin::root(), 5),
			Error::<Test>::FinalizedCheckpointNotMonotonic
		);
		assert_noop!(
			StorageProvider::advance_finalized_checkpoint(RuntimeOrigin::root(), 4),
			Error::<Test>::FinalizedCheckpointNotMonotonic
		);
	});
}

#[test]
fn service_key_rotation_activates_only_at_finalized_boundary() {
	new_test_ext().execute_with(|| {
		register(1, 100);
		let old = pair(1).public();
		let new = pair(8).public();
		set_finalized(10);
		assert_ok!(StorageProvider::rotate_service_key(RuntimeOrigin::root(), 1, new, 20));
		let scheduled = Providers::<Test>::get(1).unwrap();
		assert_eq!(scheduled.service_key.active, old);
		assert_eq!(scheduled.service_key.pending, Some(new));
		StorageProvider::on_initialize(10);
		assert_eq!(Providers::<Test>::get(1).unwrap().service_key.active, old);
		set_finalized(20);
		StorageProvider::on_initialize(22);
		let active = Providers::<Test>::get(1).unwrap();
		assert_eq!(active.service_key.active, new);
		assert_eq!(active.service_key.previous, Some(old));
		assert_eq!(active.service_key.pending, None);
	});
}

#[test]
fn rejected_service_key_rotation_leaves_no_stale_key_owner() {
	new_test_ext().execute_with(|| {
		register(1, 100);
		let old = pair(1).public();
		let rejected = pair(8).public();
		set_provider_authority_failure(1, Some(ProviderAuthorityError::ServiceKeyInvalid));
		assert_noop!(
			StorageProvider::rotate_service_key(RuntimeOrigin::root(), 1, rejected, 1),
			Error::<Test>::ProviderServiceKeyInvalid
		);
		assert_eq!(Providers::<Test>::get(1).unwrap().service_key.active, old);
		assert_eq!(ServiceKeyOwner::<Test>::get(old), Some(1));
		assert_eq!(ServiceKeyOwner::<Test>::get(rejected), None);
	});
}

#[test]
fn checkpoint_negatives_220_through_225_and_239_240_have_no_state_or_events() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		let good = payload(bucket, H256::repeat_byte(20), 0, 3, 101);
		let (signature, context_signature, confirmations) = signed_checkpoint(good);
		let baseline_events = System::events().len();
		let no_effect = |expected: Error<Test>| {
			assert!(BucketSnapshots::<Test>::get(bucket).is_none());
			assert_eq!(CheckpointClaims::<Test>::iter().count(), 0);
			assert_eq!(System::events().len(), baseline_events);
			assert!(matches!(
				expected,
				Error::<Test>::StorageCheckpointWrongDomain
					| Error::<Test>::StorageCheckpointWrongVersion
					| Error::<Test>::StorageCheckpointWrongBucket
					| Error::<Test>::StorageCheckpointWrongKey
					| Error::<Test>::StorageCheckpointStaleNonce
					| Error::<Test>::StorageCheckpointWrongWindow
					| Error::<Test>::StorageCheckpointInsufficientQuorum
					| Error::<Test>::StorageCheckpointSequenceInvalid
					| Error::<Test>::StorageCheckpointWrongContext
			));
		};

		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				b"wrong".to_vec().try_into().unwrap(),
				good,
				101,
				121,
				pair(1).public(),
				signature,
				context_signature,
				confirmations.clone(),
			),
			Error::<Test>::StorageCheckpointWrongDomain
		);
		no_effect(Error::<Test>::StorageCheckpointWrongDomain);

		let mut wrong_version = good;
		wrong_version.version = 1;
		let (sig, context_sig, conf) = signed_checkpoint(wrong_version);
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				wrong_version,
				101,
				121,
				pair(1).public(),
				sig,
				context_sig,
				conf
			),
			Error::<Test>::StorageCheckpointWrongVersion
		);
		no_effect(Error::<Test>::StorageCheckpointWrongVersion);

		let wrong_bucket = payload(H256::repeat_byte(99), good.commitment.mmr_root, 0, 3, 101);
		let (sig, context_sig, conf) = signed_checkpoint(wrong_bucket);
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				wrong_bucket,
				101,
				121,
				pair(1).public(),
				sig,
				context_sig,
				conf
			),
			Error::<Test>::StorageCheckpointWrongBucket
		);
		no_effect(Error::<Test>::StorageCheckpointWrongBucket);

		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				good,
				101,
				121,
				pair(4).public(),
				signature,
				context_signature,
				confirmations.clone()
			),
			Error::<Test>::StorageCheckpointWrongKey
		);
		no_effect(Error::<Test>::StorageCheckpointWrongKey);

		let stale = payload(bucket, good.commitment.mmr_root, 0, 3, 0);
		set_finalized(200);
		let (sig, context_sig, conf) = signed_checkpoint(stale);
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				stale,
				101,
				121,
				pair(1).public(),
				sig,
				context_sig,
				conf
			),
			Error::<Test>::StorageCheckpointStaleNonce
		);
		set_finalized(101);
		no_effect(Error::<Test>::StorageCheckpointStaleNonce);

		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				good,
				100,
				120,
				pair(1).public(),
				signature,
				context_signature,
				confirmations.clone()
			),
			Error::<Test>::StorageCheckpointWrongWindow
		);
		no_effect(Error::<Test>::StorageCheckpointWrongWindow);

		let empty: ConfirmationsOf<Test> = Default::default();
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				good,
				101,
				121,
				pair(1).public(),
				signature,
				context_signature,
				empty
			),
			Error::<Test>::StorageCheckpointInsufficientQuorum
		);
		no_effect(Error::<Test>::StorageCheckpointInsufficientQuorum);

		let wrong_sequence = payload(bucket, good.commitment.mmr_root, 1, 3, 101);
		let (sig, context_sig, conf) = signed_checkpoint(wrong_sequence);
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				wrong_sequence,
				101,
				121,
				pair(1).public(),
				sig,
				context_sig,
				conf
			),
			Error::<Test>::StorageCheckpointSequenceInvalid
		);
		no_effect(Error::<Test>::StorageCheckpointSequenceInvalid);
	});
}

#[test]
fn checkpoint_equivocation_preserves_two_claims_then_suspends_with_one_event() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		let accepted = payload(bucket, H256::repeat_byte(20), 0, 3, 101);
		submit(accepted);
		let before = System::events().len();
		let conflicting = payload(bucket, H256::repeat_byte(21), 0, 3, 101);
		let (signature, context_signature, confirmations) = signed_checkpoint(conflicting);
		let result = StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(1),
			DOMAIN.to_vec().try_into().unwrap(),
			conflicting,
			101,
			121,
			pair(1).public(),
			signature,
			context_signature,
			confirmations,
		);
		assert_ok!(result);
		assert_eq!(EquivocationEvidence::<Test>::get(1).len(), 2);
		assert_eq!(Providers::<Test>::get(1).unwrap().status, ProviderStatus::Suspended);
		assert_eq!(System::events().len(), before + 1);
		assert!(matches!(
			System::events().last().unwrap().event,
			RuntimeEvent::StorageProvider(Event::CheckpointEquivocation { code: 241, .. })
		));
	});
}

#[test]
fn bucket_membership_agreement_capacity_and_terminal_release_are_bounded() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		for account in 20..24 {
			let version = Buckets::<Test>::get(bucket).unwrap().version;
			assert_ok!(StorageProvider::change_bucket_grant(
				RuntimeOrigin::signed(OWNER),
				bucket,
				version,
				account,
				Some(crate::BucketRole::Writer)
			));
		}
		let version = Buckets::<Test>::get(bucket).unwrap().version;
		assert_noop!(
			StorageProvider::change_bucket_grant(
				RuntimeOrigin::signed(OWNER),
				bucket,
				version,
				24,
				Some(crate::BucketRole::Reader)
			),
			Error::<Test>::BucketMemberLimit
		);

		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			version,
			100,
			50
		));
		let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
		assert_eq!(Providers::<Test>::get(1).unwrap().pending_bytes, 100);
		assert_ok!(StorageProvider::accept_agreement(RuntimeOrigin::signed(1), agreement_id, 1));
		assert_eq!(Providers::<Test>::get(1).unwrap().allocated_bytes, 100);
		assert_ok!(StorageProvider::set_agreement_suspension(
			RuntimeOrigin::root(),
			agreement_id,
			2,
			true
		));
		assert_ok!(StorageProvider::set_agreement_suspension(
			RuntimeOrigin::root(),
			agreement_id,
			3,
			false
		));
		assert_ok!(StorageProvider::terminate_agreement(
			RuntimeOrigin::signed(OWNER),
			agreement_id,
			4
		));
		assert_eq!(Providers::<Test>::get(1).unwrap().allocated_bytes, 100);
		let old_key = pair(1).public();
		let next_key = pair(8).public();
		set_finalized(10);
		assert_ok!(StorageProvider::rotate_service_key(RuntimeOrigin::root(), 1, next_key, 20));
		set_finalized(20);
		System::set_block_number(12);
		StorageProvider::on_initialize(12);
		assert_eq!(Providers::<Test>::get(1).unwrap().allocated_bytes, 0);
		assert_eq!(Providers::<Test>::get(1).unwrap().service_key.active, old_key);
		assert_eq!(
			Agreements::<Test>::get(agreement_id).unwrap().status,
			AgreementStatus::Cancelled
		);
		System::set_block_number(13);
		StorageProvider::on_initialize(13);
		assert_eq!(Providers::<Test>::get(1).unwrap().service_key.active, next_key);
	});
}

#[test]
fn provider_conflict_resolution_matrix_covers_eleven_distinct_conflicts() {
	new_test_ext().execute_with(|| {
		register(1, 100);
		let key2 = pair(2).public();
		let endpoint1: crate::EndpointOf<Test> = b"https://provider-1".to_vec().try_into().unwrap();
		assert_noop!(
			StorageProvider::register_provider(
				RuntimeOrigin::root(),
				2,
				endpoint1,
				key2,
				organization(&key2),
				100
			),
			Error::<Test>::EndpointInUse
		); // 1
		let key1 = pair(1).public();
		assert_noop!(
			StorageProvider::register_provider(
				RuntimeOrigin::root(),
				2,
				b"two".to_vec().try_into().unwrap(),
				key1,
				organization(&key1),
				100
			),
			Error::<Test>::ServiceKeyInUse
		); // 2
		for id in 2..=5 {
			register(id, 100);
		}
		assert_noop!(
			StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::repeat_byte(1),
				1,
				vec![1, 2].try_into().unwrap()
			),
			Error::<Test>::DuplicateProviderAssignment
		); // 3
		assert_noop!(
			StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::repeat_byte(1),
				1,
				vec![2, 2].try_into().unwrap()
			),
			Error::<Test>::DuplicateProviderAssignment
		); // 4
		assert_noop!(
			StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::repeat_byte(1),
				1,
				vec![2].try_into().unwrap()
			),
			Error::<Test>::InvalidReplicaCount
		); // 5
		assert_ok!(StorageProvider::set_provider_status(
			RuntimeOrigin::root(),
			2,
			ProviderStatus::Suspended
		));
		assert_noop!(
			StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::repeat_byte(1),
				1,
				vec![2, 3].try_into().unwrap()
			),
			Error::<Test>::ProviderIneligible
		); // 6
		assert_ok!(StorageProvider::set_provider_status(
			RuntimeOrigin::root(),
			2,
			ProviderStatus::Active
		));
		assert_ok!(StorageProvider::create_bucket(
			RuntimeOrigin::signed(OWNER),
			H256::repeat_byte(1),
			1,
			vec![2, 3].try_into().unwrap()
		));
		let bucket = BucketIds::<Test>::get()[0];
		assert_noop!(
			StorageProvider::change_bucket_grant(
				RuntimeOrigin::signed(OWNER),
				bucket,
				99,
				7,
				Some(crate::BucketRole::Reader)
			),
			Error::<Test>::BucketVersionConflict
		); // 7
		assert_noop!(
			StorageProvider::propose_agreement(RuntimeOrigin::signed(OWNER), bucket, 1, 101, 50),
			Error::<Test>::AgreementCapacityExceeded
		); // 8
		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			10,
			50
		));
		let agreement = Agreements::<Test>::iter_keys().next().unwrap();
		assert_noop!(
			StorageProvider::accept_agreement(RuntimeOrigin::signed(2), agreement, 1),
			Error::<Test>::NotAgreementParty
		); // 9
		assert_noop!(
			StorageProvider::accept_agreement(RuntimeOrigin::signed(1), agreement, 2),
			Error::<Test>::AgreementInvalidState
		); // 10
		assert_ok!(StorageProvider::accept_agreement(RuntimeOrigin::signed(1), agreement, 1));
		assert_noop!(
			StorageProvider::accept_agreement(RuntimeOrigin::signed(1), agreement, 2),
			Error::<Test>::AgreementInvalidState
		); // 11
	});
}

#[test]
fn assigned_provider_requires_bounded_replica_replacement_before_removal() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		assert_noop!(
			StorageProvider::remove_provider(RuntimeOrigin::root(), 2),
			Error::<Test>::ProviderHasBucketAssignments
		);
		set_finalized(2);
		register(5, 10_000);
		assert_ok!(StorageProvider::replace_bucket_replica(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			2,
			5,
		));
		assert_eq!(Buckets::<Test>::get(bucket).unwrap().replicas.as_slice(), &[5, 3]);
		assert_ok!(StorageProvider::remove_provider(RuntimeOrigin::root(), 2));
		assert!(!Providers::<Test>::contains_key(2));
	});
}

#[test]
fn replica_replacement_atomically_rebinds_proposed_active_and_suspended_agreements() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		for (bytes, expires_at) in [(10, 50), (20, 60), (30, 70)] {
			assert_ok!(StorageProvider::propose_agreement(
				RuntimeOrigin::signed(OWNER),
				bucket,
				1,
				bytes,
				expires_at,
			));
		}
		let mut ids: Vec<_> = Agreements::<Test>::iter_keys().collect();
		ids.sort();
		let proposed = ids
			.iter()
			.copied()
			.find(|id| Agreements::<Test>::get(id).unwrap().bytes == 10)
			.unwrap();
		let active = ids
			.iter()
			.copied()
			.find(|id| Agreements::<Test>::get(id).unwrap().bytes == 20)
			.unwrap();
		let suspended = ids
			.iter()
			.copied()
			.find(|id| Agreements::<Test>::get(id).unwrap().bytes == 30)
			.unwrap();
		assert_ok!(StorageProvider::accept_agreement(RuntimeOrigin::signed(1), active, 1));
		assert_ok!(StorageProvider::accept_agreement(RuntimeOrigin::signed(1), suspended, 1));
		assert_ok!(StorageProvider::set_agreement_suspension(
			RuntimeOrigin::root(),
			suspended,
			2,
			true,
		));

		set_finalized(2);
		register(5, 10_000);
		assert_ok!(StorageProvider::replace_bucket_replica(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			2,
			5,
		));
		for agreement_id in [proposed, active, suspended] {
			let agreement = Agreements::<Test>::get(agreement_id).unwrap();
			assert!(!agreement.replicas.contains(&2));
			assert!(agreement.replicas.contains(&5));
			assert!(ProviderAgreements::<Test>::get(5).contains(&agreement_id));
			assert!(!ProviderAgreements::<Test>::get(2).contains(&agreement_id));
		}
		assert_eq!(Providers::<Test>::get(2).unwrap().pending_bytes, 0);
		assert_eq!(Providers::<Test>::get(2).unwrap().allocated_bytes, 0);
		assert_eq!(Providers::<Test>::get(5).unwrap().pending_bytes, 10);
		assert_eq!(Providers::<Test>::get(5).unwrap().allocated_bytes, 50);
		assert_eq!(ProviderBucketAssignmentCount::<Test>::get(2), 0);
		assert_eq!(ProviderBucketAssignmentCount::<Test>::get(5), 1);
	});
}

#[test]
fn replica_replacement_rebinds_terminal_awaiting_release_and_moves_capacity() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			40,
			200,
		));
		let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
		assert_ok!(StorageProvider::terminate_agreement(
			RuntimeOrigin::signed(OWNER),
			agreement_id,
			1,
		));
		let terminal = Agreements::<Test>::get(agreement_id).unwrap();
		assert_eq!(terminal.status, AgreementStatus::Cancelled);
		assert_eq!(terminal.capacity_state, AgreementCapacityState::Pending);
		assert_eq!(terminal.release_at, Some(12));
		assert_eq!(Providers::<Test>::get(2).unwrap().pending_bytes, 40);

		set_finalized(2);
		register(5, 10_000);
		let bucket_index = BucketAgreements::<Test>::get(bucket);
		assert_ok!(StorageProvider::replace_bucket_replica(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			2,
			5,
		));
		let rebound = Agreements::<Test>::get(agreement_id).unwrap();
		assert_eq!(rebound.status, AgreementStatus::Cancelled);
		assert_eq!(rebound.capacity_state, AgreementCapacityState::Pending);
		assert_eq!(rebound.release_at, Some(12));
		assert_eq!(rebound.version, 3);
		assert!(!rebound.replicas.contains(&2));
		assert!(rebound.replicas.contains(&5));
		assert_eq!(BucketAgreements::<Test>::get(bucket), bucket_index);
		assert_eq!(
			BucketAgreements::<Test>::get(bucket)
				.iter()
				.filter(|id| **id == agreement_id)
				.count(),
			1
		);
		assert!(!ProviderAgreements::<Test>::get(2).contains(&agreement_id));
		assert_eq!(
			ProviderAgreements::<Test>::get(5)
				.iter()
				.filter(|id| **id == agreement_id)
				.count(),
			1
		);
		assert_eq!(Providers::<Test>::get(2).unwrap().pending_bytes, 0);
		assert_eq!(Providers::<Test>::get(5).unwrap().pending_bytes, 40);

		System::set_block_number(12);
		StorageProvider::on_initialize(12);
		assert_eq!(
			Agreements::<Test>::get(agreement_id).unwrap().capacity_state,
			AgreementCapacityState::Released
		);
		assert_eq!(Providers::<Test>::get(5).unwrap().pending_bytes, 0);
		assert!(!BucketAgreements::<Test>::get(bucket).contains(&agreement_id));
		assert!(!ProviderAgreements::<Test>::get(5).contains(&agreement_id));
	});
}

#[test]
fn replica_replacement_capacity_failure_rolls_back_every_index_and_counter() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		register(5, 10);
		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			20,
			50,
		));
		let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
		assert_ok!(StorageProvider::terminate_agreement(
			RuntimeOrigin::signed(OWNER),
			agreement_id,
			1,
		));
		let before_bucket = Buckets::<Test>::get(bucket).unwrap();
		let before_agreement = Agreements::<Test>::get(agreement_id).unwrap();
		assert_eq!(before_agreement.status, AgreementStatus::Cancelled);
		assert_eq!(before_agreement.capacity_state, AgreementCapacityState::Pending);
		let bucket_index = BucketAgreements::<Test>::get(bucket);
		let provider_indexes = (1..=5).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>();
		let provider_records = (1..=5).map(Providers::<Test>::get).collect::<Vec<_>>();
		let assignment_counts =
			(1..=5).map(ProviderBucketAssignmentCount::<Test>::get).collect::<Vec<_>>();
		let release_queue = CapacityReleases::<Test>::get(12);
		let pending_duty = CheckpointDutyPending::<Test>::get(bucket);
		let events = System::events().len();
		assert_noop!(
			StorageProvider::replace_bucket_replica(RuntimeOrigin::signed(OWNER), bucket, 1, 2, 5,),
			Error::<Test>::AgreementCapacityExceeded
		);
		assert_eq!(Buckets::<Test>::get(bucket).unwrap(), before_bucket);
		assert_eq!(Agreements::<Test>::get(agreement_id).unwrap(), before_agreement);
		assert_eq!(BucketAgreements::<Test>::get(bucket), bucket_index);
		assert_eq!(
			(1..=5).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>(),
			provider_indexes
		);
		assert_eq!((1..=5).map(Providers::<Test>::get).collect::<Vec<_>>(), provider_records);
		assert_eq!(
			(1..=5).map(ProviderBucketAssignmentCount::<Test>::get).collect::<Vec<_>>(),
			assignment_counts
		);
		assert_eq!(CapacityReleases::<Test>::get(12), release_queue);
		assert_eq!(CheckpointDutyPending::<Test>::get(bucket), pending_duty);
		assert_eq!(System::events().len(), events);
	});
}

#[test]
fn deterministic_failover_uses_checkpoint_then_encoded_provider_id_tie_break() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
		set_finalized(102);
		Providers::<Test>::mutate(1, |record| {
			record.as_mut().unwrap().status = ProviderStatus::Suspended
		});
		assert_ok!(
			StorageProvider::refresh_bucket_authority(RuntimeOrigin::signed(OWNER), bucket,)
		);
		assert_eq!(Buckets::<Test>::get(bucket).unwrap().primary, 2);
		let events = System::events();
		let selection = events
			.iter()
			.position(|event| {
				matches!(
					event.event,
					RuntimeEvent::StorageProvider(Event::ReplicaSelected { provider: 2, .. })
				)
			})
			.unwrap();
		let promotion = events
			.iter()
			.position(|event| {
				matches!(
					event.event,
					RuntimeEvent::StorageProvider(Event::PrimaryPromoted {
						old_provider: 1,
						new_provider: 2,
						..
					})
				)
			})
			.unwrap();
		assert!(selection < promotion);
	});
}

#[test]
fn finalized_organization_expiry_suspends_then_promotes_in_event_order() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
		Providers::<Test>::mutate(1, |record| {
			record.as_mut().unwrap().organization.valid_until = 101
		});
		set_finalized(102);
		assert_ok!(
			StorageProvider::refresh_bucket_authority(RuntimeOrigin::signed(OWNER), bucket,)
		);
		assert_eq!(Providers::<Test>::get(1).unwrap().status, ProviderStatus::Suspended);
		assert_eq!(Buckets::<Test>::get(bucket).unwrap().primary, 2);
		let events = System::events();
		let suspension = events
			.iter()
			.rposition(|event| {
				matches!(
					event.event,
					RuntimeEvent::StorageProvider(Event::ProviderStatusChanged {
						provider: 1,
						status: ProviderStatus::Suspended
					})
				)
			})
			.unwrap();
		let promotion = events
			.iter()
			.rposition(|event| {
				matches!(
					event.event,
					RuntimeEvent::StorageProvider(Event::PrimaryPromoted {
						old_provider: 1,
						new_provider: 2,
						..
					})
				)
			})
			.unwrap();
		assert!(suspension < promotion);
	});
}

#[test]
fn finalized_revocation_refresh_suspends_and_fails_over_one_bucket() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
		set_finalized(102);
		set_provider_authority_failure(1, Some(ProviderAuthorityError::AttestationInvalid));
		assert_ok!(
			StorageProvider::refresh_bucket_authority(RuntimeOrigin::signed(OWNER), bucket,)
		);
		assert_eq!(Providers::<Test>::get(1).unwrap().status, ProviderStatus::Suspended);
		assert_eq!(Buckets::<Test>::get(bucket).unwrap().primary, 2);
		let events = System::events();
		let suspension = events
			.iter()
			.position(|event| {
				matches!(
					event.event,
					RuntimeEvent::StorageProvider(Event::ProviderStatusChanged {
						provider: 1,
						status: ProviderStatus::Suspended
					})
				)
			})
			.unwrap();
		let promotion = events
			.iter()
			.position(|event| {
				matches!(
					event.event,
					RuntimeEvent::StorageProvider(Event::PrimaryPromoted {
						old_provider: 1,
						new_provider: 2,
						..
					})
				)
			})
			.unwrap();
		assert!(suspension < promotion);
		assert!(events.iter().any(|event| matches!(
			event.event,
			RuntimeEvent::StorageProvider(Event::ProviderAuthorityRefreshed {
				provider: 1,
				bucket_id,
				checkpoint: 102,
				valid: false,
			}) if bucket_id == bucket
		)));
	});
}

#[test]
fn bucket_authority_refresh_rejects_revoked_sla_and_key_invalid_replicas() {
	for authority_error in [
		ProviderAuthorityError::AttestationInvalid,
		ProviderAuthorityError::SlaInvalid,
		ProviderAuthorityError::ServiceKeyInvalid,
	] {
		new_test_ext().execute_with(|| {
			let bucket = setup_bucket();
			set_finalized(101);
			submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
			set_finalized(102);
			set_provider_authority_failure(2, Some(authority_error));
			assert_ok!(StorageProvider::refresh_bucket_authority(
				RuntimeOrigin::signed(OWNER),
				bucket,
			));
			assert_eq!(Providers::<Test>::get(2).unwrap().status, ProviderStatus::Suspended);
			for provider in [1u64, 2, 3] {
				assert_eq!(
					Providers::<Test>::get(provider).unwrap().authority_validated_at,
					Some(102)
				);
			}

			Providers::<Test>::mutate(1, |record| {
				record.as_mut().unwrap().status = ProviderStatus::Suspended
			});
			assert_ok!(StorageProvider::reconcile_bucket(RuntimeOrigin::signed(OWNER), bucket,));
			assert_eq!(Buckets::<Test>::get(bucket).unwrap().primary, 3);
		});
	}
}

#[test]
fn challenge_evidence_is_recorded_before_provider_ineligibility() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
		System::set_block_number(101);
		assert_ok!(StorageProvider::issue_challenge(
			RuntimeOrigin::root(),
			bucket,
			1,
			crate::ChunkLocationV1 { leaf_index: 0, chunk_index: 0 },
			105
		));
		let challenge = crate::Challenges::<Test>::iter_keys().next().unwrap();
		System::set_block_number(106);
		set_finalized(106);
		StorageProvider::on_initialize(106);
		assert_eq!(
			crate::Challenges::<Test>::get(challenge).unwrap().status,
			crate::ChallengeStatus::Open
		);
		System::set_block_number(107);
		StorageProvider::on_initialize(107);
		assert_eq!(
			crate::Challenges::<Test>::get(challenge).unwrap().status,
			crate::ChallengeStatus::TimedOut
		);
		assert_eq!(ProviderEvidence::<Test>::get(1).len(), 1);
		assert_eq!(Providers::<Test>::get(1).unwrap().status, ProviderStatus::Suspended);
		let events = System::events();
		assert!(matches!(
			events[events.len() - 3].event,
			RuntimeEvent::StorageProvider(Event::EvidenceRecorded { .. })
		));
		assert!(matches!(
			events[events.len() - 2].event,
			RuntimeEvent::StorageProvider(Event::ProviderIneligible { .. })
		));
		assert!(matches!(
			events.last().unwrap().event,
			RuntimeEvent::StorageProvider(Event::ChallengeTimedOut { .. })
		));
	});
}

#[test]
fn canonical_manifest_lifecycle_is_pending_publishable_then_tombstoned() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		let leaf = MmrLeafV1 { data_root: H256::repeat_byte(44), data_size: 1, total_size: 1 };
		let leaf_hash = sp_runtime::traits::BlakeTwo256::hash_of(&leaf);
		set_finalized(101);
		submit(payload(bucket, leaf_hash, 0, 1, 101));
		let manifest = [55u8; 32];
		assert_ok!(StorageProvider::register_manifest(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			manifest
		));
		assert_eq!(StorageProvider::manifest_state(&manifest), CommitmentState::Pending);
		let proof = MmrProofV1 { peaks: vec![leaf_hash], leaf, leaf_proof: vec![] };
		assert_ok!(StorageProvider::publish_manifest(RuntimeOrigin::signed(1), manifest, 0, proof));
		assert_eq!(StorageProvider::manifest_state(&manifest), CommitmentState::Publishable);
		assert!(StorageProvider::provider_commitment_matches(&manifest, &[44u8; 32]));
		assert_ok!(StorageProvider::tombstone_manifest(RuntimeOrigin::signed(OWNER), manifest));
		assert_eq!(
			CanonicalManifests::<Test>::get(manifest).unwrap().state,
			CommitmentState::Tombstoned
		);
		assert!(!StorageProvider::deletion_evidence_satisfied(&manifest));
		set_finalized(111);
		assert!(!StorageProvider::deletion_evidence_satisfied(&manifest));
		assert_eq!(ManifestDeletionRequirements::<Test>::get(manifest).as_slice(), &[1, 2, 3]);
		for provider in [1u64, 2, 3] {
			let evidence_hash = H256::repeat_byte(provider as u8);
			let tombstoned_at =
				CanonicalManifests::<Test>::get(manifest).unwrap().tombstoned_at.unwrap();
			let digest = sp_runtime::traits::BlakeTwo256::hash_of(&(
				b"cord/storage/deletion-ack/v1",
				bucket,
				manifest,
				evidence_hash,
				tombstoned_at,
			));
			let signature = pair(provider as u8).sign(digest.as_bytes());
			assert_ok!(StorageProvider::acknowledge_manifest_deletion(
				RuntimeOrigin::signed(provider),
				manifest,
				evidence_hash,
				pair(provider as u8).public(),
				signature,
			));
			assert!(ManifestDeletionAcknowledgements::<Test>::contains_key(manifest, provider));
			if provider != 3 {
				assert!(!StorageProvider::deletion_evidence_satisfied(&manifest));
			}
		}
		assert!(StorageProvider::deletion_evidence_satisfied(&manifest));
	});
}

#[test]
fn proposed_replica_replacement_releases_pending_capacity_without_version_heuristics() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			40,
			200,
		));
		let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
		set_finalized(2);
		register(5, 10_000);
		assert_ok!(StorageProvider::replace_bucket_replica(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			2,
			5,
		));
		let rebound = Agreements::<Test>::get(agreement_id).unwrap();
		assert_eq!(rebound.version, 2);
		assert_eq!(rebound.capacity_state, AgreementCapacityState::Pending);
		assert_ok!(StorageProvider::terminate_agreement(
			RuntimeOrigin::signed(OWNER),
			agreement_id,
			2,
		));
		System::set_block_number(12);
		StorageProvider::on_initialize(12);
		let released = Agreements::<Test>::get(agreement_id).unwrap();
		assert_eq!(released.capacity_state, AgreementCapacityState::Released);
		assert_eq!(released.release_at, None);
		for provider in [1u64, 3, 5] {
			let record = Providers::<Test>::get(provider).unwrap();
			assert_eq!(record.pending_bytes, 0);
			assert_eq!(record.allocated_bytes, 0);
			assert!(!ProviderAgreements::<Test>::get(provider).contains(&agreement_id));
		}
		assert_eq!(Providers::<Test>::get(2).unwrap().pending_bytes, 0);
	});
}

#[test]
fn primary_failover_atomically_rebinds_every_non_terminal_agreement_role() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		for (bytes, expires_at) in [(10, 200), (20, 210), (30, 220), (40, 230)] {
			assert_ok!(StorageProvider::propose_agreement(
				RuntimeOrigin::signed(OWNER),
				bucket,
				1,
				bytes,
				expires_at,
			));
		}
		let mut ids = Agreements::<Test>::iter_keys().collect::<Vec<_>>();
		ids.sort();
		let active = ids
			.iter()
			.copied()
			.find(|id| Agreements::<Test>::get(id).unwrap().bytes == 20)
			.unwrap();
		let suspended = ids
			.iter()
			.copied()
			.find(|id| Agreements::<Test>::get(id).unwrap().bytes == 30)
			.unwrap();
		assert_ok!(StorageProvider::accept_agreement(RuntimeOrigin::signed(1), active, 1));
		assert_ok!(StorageProvider::accept_agreement(RuntimeOrigin::signed(1), suspended, 1));
		assert_ok!(StorageProvider::set_agreement_suspension(
			RuntimeOrigin::root(),
			suspended,
			2,
			true,
		));
		set_finalized(101);
		submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
		set_finalized(102);
		Providers::<Test>::mutate(1, |record| {
			record.as_mut().unwrap().status = ProviderStatus::Suspended
		});
		assert_ok!(
			StorageProvider::refresh_bucket_authority(RuntimeOrigin::signed(OWNER), bucket,)
		);
		let bucket_record = Buckets::<Test>::get(bucket).unwrap();
		assert_eq!(bucket_record.primary, 2);
		assert!(bucket_record.replicas.contains(&1));
		for agreement_id in ids.iter().copied() {
			let agreement = Agreements::<Test>::get(agreement_id).unwrap();
			assert_eq!(agreement.primary, 2);
			assert!(agreement.replicas.contains(&1));
			assert!(!agreement.replicas.contains(&2));
		}
		for agreement_id in ids {
			let agreement = Agreements::<Test>::get(agreement_id).unwrap();
			let caller = if agreement.status == AgreementStatus::Proposed { OWNER } else { 2 };
			assert_ok!(StorageProvider::terminate_agreement(
				RuntimeOrigin::signed(caller),
				agreement_id,
				agreement.version,
			));
		}
	});
}

#[test]
fn primary_failover_rebinds_terminal_awaiting_release_without_moving_capacity() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			40,
			200,
		));
		let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
		assert_ok!(StorageProvider::terminate_agreement(
			RuntimeOrigin::signed(OWNER),
			agreement_id,
			1,
		));
		let terminal = Agreements::<Test>::get(agreement_id).unwrap();
		assert_eq!(terminal.status, AgreementStatus::Cancelled);
		assert_eq!(terminal.capacity_state, AgreementCapacityState::Pending);
		assert_eq!(terminal.release_at, Some(12));
		let bucket_index = BucketAgreements::<Test>::get(bucket);
		let provider_indexes = (1..=3).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>();
		let provider_capacity = (1..=3)
			.map(|provider| {
				let record = Providers::<Test>::get(provider).unwrap();
				(record.pending_bytes, record.allocated_bytes)
			})
			.collect::<Vec<_>>();

		set_finalized(101);
		submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
		set_finalized(102);
		Providers::<Test>::mutate(1, |record| {
			record.as_mut().unwrap().status = ProviderStatus::Suspended
		});
		assert_ok!(
			StorageProvider::refresh_bucket_authority(RuntimeOrigin::signed(OWNER), bucket,)
		);
		let rebound = Agreements::<Test>::get(agreement_id).unwrap();
		assert_eq!(rebound.status, AgreementStatus::Cancelled);
		assert_eq!(rebound.capacity_state, AgreementCapacityState::Pending);
		assert_eq!(rebound.release_at, Some(12));
		assert_eq!(rebound.version, 3);
		assert_eq!(rebound.primary, 2);
		assert!(rebound.replicas.contains(&1));
		assert!(!rebound.replicas.contains(&2));
		assert_eq!(BucketAgreements::<Test>::get(bucket), bucket_index);
		assert_eq!(
			(1..=3).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>(),
			provider_indexes
		);
		assert_eq!(
			(1..=3)
				.map(|provider| {
					let record = Providers::<Test>::get(provider).unwrap();
					(record.pending_bytes, record.allocated_bytes)
				})
				.collect::<Vec<_>>(),
			provider_capacity
		);

		System::set_block_number(12);
		StorageProvider::on_initialize(12);
		assert_eq!(
			Agreements::<Test>::get(agreement_id).unwrap().capacity_state,
			AgreementCapacityState::Released
		);
		for provider in 1..=3 {
			assert_eq!(Providers::<Test>::get(provider).unwrap().pending_bytes, 0);
			assert!(!ProviderAgreements::<Test>::get(provider).contains(&agreement_id));
		}
		assert!(!BucketAgreements::<Test>::get(bucket).contains(&agreement_id));
	});
}

#[test]
fn primary_failover_terminal_rebind_invariant_failure_rolls_back_every_surface() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			40,
			200,
		));
		let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
		assert_ok!(StorageProvider::terminate_agreement(
			RuntimeOrigin::signed(OWNER),
			agreement_id,
			1,
		));
		set_finalized(101);
		submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
		Agreements::<Test>::mutate(agreement_id, |record| record.as_mut().unwrap().primary = 3);
		set_finalized(102);
		Providers::<Test>::mutate(1, |record| {
			record.as_mut().unwrap().status = ProviderStatus::Suspended
		});
		let bucket_before = Buckets::<Test>::get(bucket).unwrap();
		let agreement_before = Agreements::<Test>::get(agreement_id).unwrap();
		let bucket_index_before = BucketAgreements::<Test>::get(bucket);
		let provider_indexes_before =
			(1..=3).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>();
		let providers_before = (1..=3).map(Providers::<Test>::get).collect::<Vec<_>>();
		let assignment_counts_before =
			(1..=3).map(ProviderBucketAssignmentCount::<Test>::get).collect::<Vec<_>>();
		let release_queue_before = CapacityReleases::<Test>::get(12);
		let pending_before = CheckpointDutyPending::<Test>::get(bucket);
		let current_before = CheckpointDutyCurrent::<Test>::get(bucket);
		let events = System::events().len();
		assert_noop!(
			StorageProvider::refresh_bucket_authority(RuntimeOrigin::signed(OWNER), bucket),
			Error::<Test>::AgreementInvalidState
		);
		assert_eq!(Buckets::<Test>::get(bucket).unwrap(), bucket_before);
		assert_eq!(Agreements::<Test>::get(agreement_id).unwrap(), agreement_before);
		assert_eq!(BucketAgreements::<Test>::get(bucket), bucket_index_before);
		assert_eq!(
			(1..=3).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>(),
			provider_indexes_before
		);
		assert_eq!((1..=3).map(Providers::<Test>::get).collect::<Vec<_>>(), providers_before);
		assert_eq!(
			(1..=3).map(ProviderBucketAssignmentCount::<Test>::get).collect::<Vec<_>>(),
			assignment_counts_before
		);
		assert_eq!(CapacityReleases::<Test>::get(12), release_queue_before);
		assert_eq!(CheckpointDutyPending::<Test>::get(bucket), pending_before);
		assert_eq!(CheckpointDutyCurrent::<Test>::get(bucket), current_before);
		assert_eq!(System::events().len(), events);
	});
}

#[test]
fn challenge_backlog_is_bounded_lossless_and_non_starving() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
		System::set_block_number(101);
		for chunk_index in 0..8 {
			assert_ok!(StorageProvider::issue_challenge(
				RuntimeOrigin::root(),
				bucket,
				1,
				crate::ChunkLocationV1 { leaf_index: 0, chunk_index },
				105,
			));
		}
		assert_eq!(ChallengeBacklog::<Test>::get().len(), 8);
		assert_noop!(
			StorageProvider::issue_challenge(
				RuntimeOrigin::root(),
				bucket,
				1,
				crate::ChunkLocationV1 { leaf_index: 0, chunk_index: 9 },
				105,
			),
			Error::<Test>::ChallengeDutyLimit
		);
		System::set_block_number(104);
		let deferred_weight = StorageProvider::on_initialize(104);
		assert_eq!(
			deferred_weight,
			<crate::weights::SubstrateWeight<Test> as crate::weights::WeightInfo>::on_initialize_challenges(4,)
		);
		assert_eq!(ChallengeBacklog::<Test>::get().len(), 8);
		assert_eq!(crate::ChallengeBacklogCursor::<Test>::get(), 4);
		assert!(crate::Challenges::<Test>::iter_values()
			.all(|challenge| challenge.status == ChallengeStatus::Open));
		set_finalized(106);
		System::set_block_number(107);
		StorageProvider::on_initialize(107);
		assert_eq!(ChallengeBacklog::<Test>::get().len(), 4);
		assert_eq!(
			crate::Challenges::<Test>::iter_values()
				.filter(|challenge| challenge.status == ChallengeStatus::TimedOut)
				.count(),
			4
		);
		System::set_block_number(110);
		StorageProvider::on_initialize(110);
		assert!(ChallengeBacklog::<Test>::get().is_empty());
		assert!(crate::Challenges::<Test>::iter_values()
			.all(|challenge| challenge.status == ChallengeStatus::TimedOut));
	});
}

#[test]
fn challenge_timeout_fails_closed_when_evidence_journal_is_saturated() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
		ProviderEvidence::<Test>::mutate(1, |items| {
			for index in 0..8 {
				items
					.try_push(crate::EvidenceRecord {
						provider: 1,
						bucket_id: bucket,
						evidence_hash: H256::repeat_byte(index),
						recorded_at: 101,
					})
					.unwrap();
			}
		});
		System::set_block_number(101);
		assert_ok!(StorageProvider::issue_challenge(
			RuntimeOrigin::root(),
			bucket,
			1,
			crate::ChunkLocationV1 { leaf_index: 0, chunk_index: 0 },
			105,
		));
		let challenge_id = crate::Challenges::<Test>::iter_keys().next().unwrap();
		set_finalized(106);
		System::set_block_number(107);
		StorageProvider::on_initialize(107);
		assert_eq!(
			crate::Challenges::<Test>::get(challenge_id).unwrap().status,
			ChallengeStatus::TimedOut
		);
		assert_eq!(ProviderEvidence::<Test>::get(1).len(), 8);
		assert_eq!(ProviderEvidenceOverflow::<Test>::get(1), 1);
		assert_eq!(crate::OverdueChallenges::<Test>::get(1), 1);
		assert_eq!(Providers::<Test>::get(1).unwrap().status, ProviderStatus::Suspended);
		assert!(ChallengeBacklog::<Test>::get().is_empty());
	});
}

#[test]
fn checkpoint_duties_are_initially_scheduled_and_frozen_until_next_finalized_snapshot() {
	new_test_ext().execute_with(|| {
		for id in 1..=4 {
			register(id, 10_000);
		}
		System::set_block_number(2);
		assert_ok!(StorageProvider::create_bucket(
			RuntimeOrigin::signed(OWNER),
			H256::repeat_byte(10),
			1,
			vec![2, 3].try_into().unwrap(),
		));
		let bucket = BucketIds::<Test>::get()[0];
		assert!(CheckpointDutyCurrent::<Test>::get(bucket).is_none());
		assert!(CheckpointDutyPending::<Test>::get(bucket).is_some());
		assert!(StorageProvider::checkpoint_duty_at(bucket, 1).is_none());

		System::set_block_number(3);
		assert_ok!(StorageProvider::advance_finalized_checkpoint(RuntimeOrigin::root(), 2));
		let initial = StorageProvider::checkpoint_duty_at(bucket, 2).unwrap();
		assert_eq!(initial.previous_checkpoint, 2);
		assert_eq!(initial.due_at, 102);

		System::set_block_number(103);
		assert_ok!(StorageProvider::advance_finalized_checkpoint(RuntimeOrigin::root(), 102));
		let next_payload = payload(bucket, H256::repeat_byte(20), 0, 3, 102);
		let (signature, context_signature, confirmations) = signed_checkpoint(next_payload);
		assert_ok!(StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(1),
			DOMAIN.to_vec().try_into().unwrap(),
			next_payload,
			102,
			122,
			pair(1).public(),
			signature,
			context_signature,
			confirmations,
		));
		assert_eq!(StorageProvider::checkpoint_duty_at(bucket, 102).unwrap(), initial);

		System::set_block_number(104);
		assert_ok!(StorageProvider::advance_finalized_checkpoint(RuntimeOrigin::root(), 103));
		let next = StorageProvider::checkpoint_duty_at(bucket, 103).unwrap();
		assert_eq!(next.previous_checkpoint, 102);
		assert_eq!(next.due_at, 202);
		assert_eq!(next.grace_until, 222);
	});
}

#[test]
fn checkpoint_duty_admission_is_exactly_bounded_per_block() {
	new_test_ext().execute_with(|| {
		for id in 1..=4 {
			register(id, 10_000);
		}
		let replicas: ReplicasOf<Test> = vec![2, 3].try_into().unwrap();
		for index in 0..8 {
			assert_ok!(StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::repeat_byte(index),
				1,
				replicas.clone(),
			));
		}
		assert_eq!(DutyAdmissionCount::<Test>::get(), 8);
		assert_eq!(CheckpointDutyPending::<Test>::iter().count(), 8);
		assert_noop!(
			StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::repeat_byte(9),
				1,
				replicas,
			),
			Error::<Test>::CheckpointDutyLimit
		);
		assert_eq!(BucketIds::<Test>::get().len(), 8);
		System::set_block_number(2);
		assert_ok!(StorageProvider::create_bucket(
			RuntimeOrigin::signed(OWNER),
			H256::repeat_byte(10),
			1,
			vec![2, 3].try_into().unwrap(),
		));
		assert_eq!(DutyAdmissionCount::<Test>::get(), 1);
		assert_eq!(CheckpointDutyPending::<Test>::iter().count(), 9);
	});
}

#[test]
fn checkpoint_and_challenge_admissions_share_one_exact_bound() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		BucketSnapshots::<Test>::insert(
			bucket,
			crate::BucketSnapshot {
				commitment: CommitmentV1 {
					mmr_root: H256::repeat_byte(9),
					start_seq: 0,
					leaf_count: 1,
				},
				checkpoint_block: 1,
				primary_signers: 1,
				commitment_nonce: 1,
				replica_confirmations: vec![2, 3].try_into().unwrap(),
			},
		);
		for index in 1..4 {
			assert_ok!(StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::from_low_u64_be(index),
				1,
				vec![2, 3].try_into().unwrap(),
			));
		}
		for index in 0..4 {
			assert_ok!(StorageProvider::issue_challenge(
				RuntimeOrigin::root(),
				bucket,
				1,
				crate::ChunkLocationV1 { leaf_index: index, chunk_index: 0 },
				10,
			));
		}
		assert_eq!(DutyAdmissionCount::<Test>::get(), 8);
		let buckets = BucketIds::<Test>::get();
		let backlog = ChallengeBacklog::<Test>::get();
		let events = System::events().len();
		assert_noop!(
			StorageProvider::issue_challenge(
				RuntimeOrigin::root(),
				bucket,
				1,
				crate::ChunkLocationV1 { leaf_index: 4, chunk_index: 0 },
				10,
			),
			Error::<Test>::ChallengeDutyLimit
		);
		assert_noop!(
			StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::from_low_u64_be(9),
				1,
				vec![2, 3].try_into().unwrap(),
			),
			Error::<Test>::CheckpointDutyLimit
		);
		assert_eq!(DutyAdmissionCount::<Test>::get(), 8);
		assert_eq!(BucketIds::<Test>::get(), buckets);
		assert_eq!(ChallengeBacklog::<Test>::get(), backlog);
		assert_eq!(System::events().len(), events);
	});
}

#[test]
fn outstanding_challenge_and_capacity_release_bounds_are_additive() {
	new_test_ext().execute_with(|| {
		let ids = (0..8).map(H256::from_low_u64_be).collect::<Vec<_>>();
		let challenge_backlog: frame_support::BoundedVec<H256, MaxChallengeBacklog> =
			ids.clone().try_into().unwrap();
		let releases: frame_support::BoundedVec<H256, MaxCapacityReleasesPerBlock> =
			ids.try_into().unwrap();
		ChallengeBacklog::<Test>::put(challenge_backlog);
		CapacityReleases::<Test>::insert(12, releases);
		assert_eq!(ChallengeBacklog::<Test>::get().len(), 8);
		assert_eq!(CapacityReleases::<Test>::get(12).len(), 8);
		assert_eq!(DutyAdmissionCount::<Test>::get(), 0);
	});
}

#[test]
fn checkpoint_nonce_age_has_exact_127_128_129_boundary() {
	for (age, accepted) in [(127u64, true), (128, true), (129, false)] {
		new_test_ext().execute_with(|| {
			System::set_block_number(100);
			set_finalized(100);
			let bucket = setup_bucket();
			set_finalized(200);
			let mut duty = CheckpointDutyPending::<Test>::get(bucket).unwrap();
			duty.scheduled_at = 0;
			duty.due_at = 200;
			duty.grace_until = 220;
			CheckpointDutyCurrent::<Test>::insert(bucket, duty);
			CheckpointDutyPending::<Test>::remove(bucket);
			let checkpoint = payload(bucket, H256::repeat_byte(age as u8), 0, 1, 200 - age);
			let (signature, context_signature, confirmations) = signed_checkpoint(checkpoint);
			let event_count = System::events().len();
			let result = StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				checkpoint,
				200,
				220,
				pair(1).public(),
				signature,
				context_signature,
				confirmations,
			);
			if accepted {
				assert_ok!(result);
				assert!(BucketSnapshots::<Test>::contains_key(bucket));
			} else {
				assert_eq!(result, Err(Error::<Test>::StorageCheckpointStaleNonce.into()));
				assert!(!BucketSnapshots::<Test>::contains_key(bucket));
				assert_eq!(System::events().len(), event_count);
			}
		});
	}
}

#[test]
fn exact_checkpoint_replay_remains_idempotent_after_nonce_and_duty_expire() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		let checkpoint = payload(bucket, H256::repeat_byte(20), 0, 3, 101);
		let (signature, context_signature, confirmations) = signed_checkpoint(checkpoint);
		assert_ok!(StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(1),
			DOMAIN.to_vec().try_into().unwrap(),
			checkpoint,
			101,
			121,
			pair(1).public(),
			signature.clone(),
			context_signature.clone(),
			confirmations.clone(),
		));
		let snapshot = BucketSnapshots::<Test>::get(bucket).unwrap();
		let events = System::events().len();
		set_finalized(230);

		assert_ok!(StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(1),
			DOMAIN.to_vec().try_into().unwrap(),
			checkpoint,
			101,
			121,
			pair(1).public(),
			signature.clone(),
			context_signature.clone(),
			confirmations.clone(),
		));
		assert_eq!(BucketSnapshots::<Test>::get(bucket).unwrap(), snapshot);
		assert_eq!(System::events().len(), events);
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				checkpoint,
				102,
				121,
				pair(1).public(),
				signature.clone(),
				context_signature.clone(),
				confirmations.clone(),
			),
			Error::<Test>::StorageCheckpointStaleNonce
		);
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				checkpoint,
				101,
				121,
				pair(2).public(),
				signature,
				context_signature,
				confirmations,
			),
			Error::<Test>::StorageCheckpointStaleNonce
		);
		assert_eq!(BucketSnapshots::<Test>::get(bucket).unwrap(), snapshot);
		assert_eq!(System::events().len(), events);
	});
}

#[test]
fn checkpoint_sequence_accepts_contiguous_and_rejects_gap_and_overlap() {
	for (start_seq, accepted) in [(3u64, true), (4, false), (2, false)] {
		new_test_ext().execute_with(|| {
			let bucket = setup_bucket();
			set_finalized(101);
			submit(payload(bucket, H256::repeat_byte(20), 0, 3, 101));
			let first = BucketSnapshots::<Test>::get(bucket).unwrap();
			set_finalized(201);
			let checkpoint = payload(bucket, H256::repeat_byte(start_seq as u8), start_seq, 1, 201);
			let (signature, context_signature, confirmations) = signed_checkpoint(checkpoint);
			let event_count = System::events().len();
			let result = StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				checkpoint,
				201,
				221,
				pair(1).public(),
				signature,
				context_signature,
				confirmations,
			);
			if accepted {
				assert_ok!(result);
				assert_eq!(BucketSnapshots::<Test>::get(bucket).unwrap().commitment.start_seq, 3);
			} else {
				assert_eq!(result, Err(Error::<Test>::StorageCheckpointSequenceInvalid.into()));
				assert_eq!(BucketSnapshots::<Test>::get(bucket).unwrap(), first);
				assert_eq!(System::events().len(), event_count);
			}
		});
	}
}

#[test]
fn checkpoint_window_acceptance_has_exact_due_and_grace_boundaries() {
	for (finalized, expected) in [
		(100u64, Some(Error::<Test>::StorageCheckpointWrongWindow)),
		(101, None),
		(120, None),
		(121, Some(Error::<Test>::StorageCheckpointWrongKey)),
	] {
		new_test_ext().execute_with(|| {
			let bucket = setup_bucket();
			set_finalized(finalized);
			let checkpoint = payload(bucket, H256::repeat_byte(finalized as u8), 0, 1, finalized);
			let (signature, context_signature, confirmations) = signed_checkpoint(checkpoint);
			let events = System::events().len();
			let result = StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				checkpoint,
				101,
				121,
				pair(1).public(),
				signature,
				context_signature,
				confirmations,
			);
			if expected.is_none() {
				assert_ok!(result);
				assert_eq!(
					BucketSnapshots::<Test>::get(bucket).unwrap().checkpoint_block,
					finalized
				);
			} else {
				assert_eq!(result, Err(expected.unwrap().into()));
				assert!(BucketSnapshots::<Test>::get(bucket).is_none());
				assert_eq!(CheckpointClaims::<Test>::iter().count(), 0);
				assert_eq!(System::events().len(), events);
			}
		});
	}
}

#[test]
fn checkpoint_context_v1_binds_every_field_and_rejects_wrong_signatures_without_effects() {
	for mutation in 0..7 {
		new_test_ext().execute_with(|| {
			let bucket = setup_bucket();
			set_finalized(101);
			let checkpoint = payload(bucket, H256::repeat_byte(70 + mutation), 0, 1, 101);
			let (signature, _, confirmations) = signed_checkpoint(checkpoint);
			let mut context = StorageProvider::checkpoint_context_for(&checkpoint).unwrap();
			assert_eq!(context.version, 1);
			assert_eq!(context.genesis_hash, H256::repeat_byte(0x11));
			assert_eq!(context.spec_version, 1);
			assert_eq!(context.transaction_version, 1);
			assert_eq!(context.metadata_hash, H256::repeat_byte(0x22));
			assert_eq!(context.finalized_hash, H256::from_low_u64_be(101));
			match mutation {
				0 => context.genesis_hash = H256::repeat_byte(1),
				1 => context.spec_version = 2,
				2 => context.transaction_version = 2,
				3 => context.metadata_hash = H256::repeat_byte(3),
				4 => context.finalized_hash = H256::repeat_byte(4),
				5 => context.duty_id = H256::repeat_byte(5),
				_ => context.v2_digest = [6; 32],
			}
			let wrong_context_signature =
				pair(1).sign(&StorageProvider::checkpoint_context_digest(&context));
			let events = System::events().len();
			assert_noop!(
				StorageProvider::submit_checkpoint(
					RuntimeOrigin::signed(1),
					DOMAIN.to_vec().try_into().unwrap(),
					checkpoint,
					101,
					121,
					pair(1).public(),
					signature,
					wrong_context_signature,
					confirmations,
				),
				Error::<Test>::StorageCheckpointWrongContext
			);
			assert!(BucketSnapshots::<Test>::get(bucket).is_none());
			assert_eq!(CheckpointClaims::<Test>::iter().count(), 0);
			assert_eq!(System::events().len(), events);
		});
	}

	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(101);
		let checkpoint = payload(bucket, H256::repeat_byte(88), 0, 1, 101);
		let (signature, context_signature, mut confirmations) = signed_checkpoint(checkpoint);
		confirmations[0].context_signature = ed25519::Signature::from_raw([0; 64]);
		let events = System::events().len();
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(1),
				DOMAIN.to_vec().try_into().unwrap(),
				checkpoint,
				101,
				121,
				pair(1).public(),
				signature,
				context_signature,
				confirmations,
			),
			Error::<Test>::StorageCheckpointWrongContext
		);
		assert!(BucketSnapshots::<Test>::get(bucket).is_none());
		assert_eq!(CheckpointClaims::<Test>::iter().count(), 0);
		assert_eq!(System::events().len(), events);
	});
}

#[test]
fn checkpoint_context_duty_and_promotion_exact_golden_vector_is_stable() {
	new_test_ext().execute_with(|| {
		let hex = |bytes: &[u8]| {
			bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>()
		};
		let bucket = setup_bucket();
		set_finalized(101);
		let checkpoint = payload(bucket, H256::repeat_byte(88), 0, 1, 101);
		let duty = StorageProvider::checkpoint_duty_at(bucket, 101).unwrap();
		let duty_preimage = StorageProvider::checkpoint_duty_preimage(&duty, 101);
		let duty_id = StorageProvider::checkpoint_duty_id(&duty, 101);
		let context = StorageProvider::checkpoint_context_for(&checkpoint).unwrap();
		let context_scale = context.encode();
		let mut context_message = b"cord/storage/checkpoint-context/v1".to_vec();
		context_message.extend_from_slice(&context_scale);
		let promotion = CheckpointFallbackPromotionV1 {
			version: 1,
			bucket_id: bucket,
			snapshot_nonce: 101,
			duty_id,
		};
		assert_eq!(
			hex(bucket.as_bytes()),
			"e47d01b8abf1ab7be46c1c985ddd7d1cd4d198ddf533f5a322a885c383f7d105"
		);
		assert_eq!(
			hex(&duty_preimage),
			"636f72642f73746f726167652f636865636b706f696e742d647574792f763211111111111111111111111111111111111111111111111111111111111111110100000001000000222222222222222222222222222222222222222222222222222222222222222265000000000000000000000000000000000000000000000000000000000000000000000000000065e47d01b8abf1ab7be46c1c985ddd7d1cd4d198ddf533f5a322a885c383f7d105010000000000000008020000000000000003000000000000000100000000000000000000000000000000650000000000000079000000000000000000"
		);
		assert_eq!(
			hex(duty_id.as_bytes()),
			"8973f76c873b69489cff1ee35f4f1c8c3f61d6f111e2919f2de53cbdb7c7e171"
		);
		assert_eq!(
			hex(&context_scale),
			"0111111111111111111111111111111111111111111111111111111111111111110100000001000000222222222222222222222222222222222222222222222222222222222222222200000000000000000000000000000000000000000000000000000000000000658973f76c873b69489cff1ee35f4f1c8c3f61d6f111e2919f2de53cbdb7c7e171e54285176cae46b1765a808fa812a1e09203f54337656e9ab3b6bd7d9514485b"
		);
		assert_eq!(
			hex(&context_message),
			"636f72642f73746f726167652f636865636b706f696e742d636f6e746578742f76310111111111111111111111111111111111111111111111111111111111111111110100000001000000222222222222222222222222222222222222222222222222222222222222222200000000000000000000000000000000000000000000000000000000000000658973f76c873b69489cff1ee35f4f1c8c3f61d6f111e2919f2de53cbdb7c7e171e54285176cae46b1765a808fa812a1e09203f54337656e9ab3b6bd7d9514485b"
		);
		let context_digest = StorageProvider::checkpoint_context_digest(&context);
		assert_eq!(
			hex(&context_digest),
			"66f0ce2876c7ebc5c57c3388e826750eef59b6950e96ab57c7d8e3f2ef81992a"
		);
		assert_eq!(
			hex(pair(1).public().as_ref()),
			"8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c"
		);
		let context_signature = pair(1).sign(&context_digest);
		assert_eq!(
			hex(context_signature.as_ref()),
			"b5c269cf6b084071e45731e1f4477dfc62e13b726e0df49a69fac882d41ff7e99072894a9f8111dbb8b907d72fa9f8471c3167e280d0ba492a8e3b81f764b80a"
		);
		assert!(sp_io::crypto::ed25519_verify(
			&context_signature,
			&context_digest,
			&pair(1).public()
		));
		assert_eq!(
			hex(&promotion.encode()),
			"01e47d01b8abf1ab7be46c1c985ddd7d1cd4d198ddf533f5a322a885c383f7d10565000000000000008973f76c873b69489cff1ee35f4f1c8c3f61d6f111e2919f2de53cbdb7c7e171"
		);
		assert_eq!(
			hex(&StorageProvider::checkpoint_promotion_digest(&promotion)),
			"c70899b451eac1e5bf509fa33dfc71448f0ed1078b67c04424b070530e4fc988"
		);
	});
}

#[test]
fn checkpoint_quorum_rejects_reversed_surplus_and_initiator_confirmations_without_effects() {
	for confirming in [&[3u64, 2][..], &[2, 3, 4][..], &[1, 2][..]] {
		new_test_ext().execute_with(|| {
			let bucket = setup_bucket();
			set_finalized(101);
			let checkpoint = payload(bucket, H256::repeat_byte(confirming.len() as u8), 0, 1, 101);
			let (signature, context_signature, confirmations) =
				signed_checkpoint_for(checkpoint, 1, confirming);
			let events = System::events().len();
			assert_noop!(
				StorageProvider::submit_checkpoint(
					RuntimeOrigin::signed(1),
					DOMAIN.to_vec().try_into().unwrap(),
					checkpoint,
					101,
					121,
					pair(1).public(),
					signature,
					context_signature,
					confirmations,
				),
				Error::<Test>::StorageCheckpointInsufficientQuorum
			);
			assert!(BucketSnapshots::<Test>::get(bucket).is_none());
			assert_eq!(CheckpointClaims::<Test>::iter().count(), 0);
			assert_eq!(System::events().len(), events);
		});
	}
}

#[test]
fn grace_fallback_is_typed_when_two_post_promotion_confirmers_are_unavailable() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		set_finalized(121);
		Providers::<Test>::mutate(1, |record| {
			record.as_mut().unwrap().status = ProviderStatus::Suspended
		});
		let before_bucket = Buckets::<Test>::get(bucket).unwrap();
		let before_events = System::events().len();
		let checkpoint = payload(bucket, H256::repeat_byte(31), 0, 1, 121);
		let (signature, context_signature, confirmations) =
			signed_checkpoint_for(checkpoint, 2, &[1, 3]);
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(2),
				DOMAIN.to_vec().try_into().unwrap(),
				checkpoint,
				101,
				121,
				pair(2).public(),
				signature,
				context_signature,
				confirmations,
			),
			Error::<Test>::StorageCheckpointInsufficientQuorum
		);
		assert_eq!(Buckets::<Test>::get(bucket).unwrap(), before_bucket);
		assert!(BucketSnapshots::<Test>::get(bucket).is_none());
		assert_eq!(System::events().len(), before_events);
	});
}

#[test]
fn authority_only_fallback_promotion_is_idempotent_and_requires_repair_before_publish() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			10,
			200,
		));
		let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
		set_finalized(121);
		Providers::<Test>::mutate(1, |record| {
			record.as_mut().unwrap().status = ProviderStatus::Suspended
		});
		let (promotion, signature) = signed_promotion(bucket, 121, 2);
		assert_ok!(StorageProvider::promote_checkpoint_fallback(
			RuntimeOrigin::signed(2),
			promotion,
			pair(2).public(),
			signature.clone(),
		));
		let promoted = Buckets::<Test>::get(bucket).unwrap();
		assert_eq!(promoted.primary, 2);
		assert_eq!(promoted.replicas.as_slice(), &[1, 3]);
		let agreement = Agreements::<Test>::get(agreement_id).unwrap();
		assert_eq!(agreement.primary, 2);
		assert_eq!(agreement.replicas.as_slice(), &[1, 3]);
		assert!(BucketSnapshots::<Test>::get(bucket).is_none());
		assert_eq!(CheckpointClaims::<Test>::iter().count(), 0);
		let recovery = CheckpointDutyPending::<Test>::get(bucket).unwrap();
		assert_eq!(recovery.mode, CheckpointDutyMode::PromotionPending);
		assert_eq!(recovery.promotion_predecessor, Some(1));
		assert_eq!(recovery.scheduled_at, 121);
		assert!(StorageProvider::checkpoint_duty_at(bucket, 121)
			.is_some_and(|duty| duty.mode == CheckpointDutyMode::Standard));
		assert!(CheckpointFallbackPromotionReceiptByBucket::<Test>::contains_key(bucket));

		let events = System::events().len();
		assert_ok!(StorageProvider::promote_checkpoint_fallback(
			RuntimeOrigin::signed(2),
			promotion,
			pair(2).public(),
			signature,
		));
		assert_eq!(Buckets::<Test>::get(bucket).unwrap(), promoted);
		assert_eq!(System::events().len(), events);

		Providers::<Test>::mutate(4, |record| {
			record.as_mut().unwrap().authority_validated_at = Some(121)
		});
		assert_noop!(
			StorageProvider::replace_bucket_replica(
				RuntimeOrigin::signed(OWNER),
				bucket,
				promoted.version,
				1,
				4,
			),
			Error::<Test>::CheckpointDutyPending
		);
		System::set_block_number(122);
		set_finalized(122);
		Providers::<Test>::mutate(4, |record| {
			record.as_mut().unwrap().authority_validated_at = Some(122)
		});
		assert_ok!(StorageProvider::replace_bucket_replica(
			RuntimeOrigin::signed(OWNER),
			bucket,
			promoted.version,
			1,
			4,
		));
		let repaired = Buckets::<Test>::get(bucket).unwrap();
		assert_eq!(repaired.replicas.as_slice(), &[4, 3]);
		assert_eq!(
			StorageProvider::checkpoint_duty_at(bucket, 122).unwrap().mode,
			CheckpointDutyMode::PromotionPending
		);
		System::set_block_number(123);
		set_finalized(123);
		let repaired_duty = StorageProvider::checkpoint_duty_at(bucket, 123).unwrap();
		assert_eq!(repaired_duty.replicas.as_slice(), &[4, 3]);
		assert_eq!(repaired_duty.mode, CheckpointDutyMode::PromotionPending);
		let checkpoint = payload(bucket, H256::repeat_byte(90), 0, 1, 123);
		let (checkpoint_signature, context_signature, confirmations) =
			signed_checkpoint_for(checkpoint, 2, &[3, 4]);
		assert_ok!(StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(2),
			DOMAIN.to_vec().try_into().unwrap(),
			checkpoint,
			repaired_duty.due_at,
			repaired_duty.grace_until,
			pair(2).public(),
			checkpoint_signature,
			context_signature,
			confirmations,
		));
		assert_eq!(BucketSnapshots::<Test>::get(bucket).unwrap().checkpoint_block, 123);
	});
}

#[test]
fn authority_only_fallback_promotion_rolls_back_every_surface_on_hostile_invariants() {
	for case in 0..6 {
		new_test_ext().execute_with(|| {
			for provider in 1..=4 {
				register(provider, 10_000);
			}
			assert_ok!(StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::repeat_byte(10),
				1,
				vec![2, 3, 4].try_into().unwrap(),
			));
			let bucket = BucketIds::<Test>::get()[0];
			assert_ok!(StorageProvider::propose_agreement(
				RuntimeOrigin::signed(OWNER),
				bucket,
				1,
				10,
				200,
			));
			let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
			set_finalized(121);
			Providers::<Test>::mutate(1, |record| {
				record.as_mut().unwrap().status = ProviderStatus::Suspended
			});
			let (promotion, mut signature) = signed_promotion(bucket, 121, 2);
			let expected = match case {
				0 => {
					Agreements::<Test>::remove(agreement_id);
					Error::<Test>::AgreementNotFound
				},
				1 => {
					Agreements::<Test>::mutate(agreement_id, |record| {
						record.as_mut().unwrap().bucket_id = H256::repeat_byte(99)
					});
					Error::<Test>::AgreementInvalidState
				},
				2 => {
					Agreements::<Test>::mutate(agreement_id, |record| {
						record.as_mut().unwrap().primary = 4
					});
					Error::<Test>::AgreementInvalidState
				},
				3 => {
					Agreements::<Test>::mutate(agreement_id, |record| {
						record.as_mut().unwrap().replicas = vec![3, 4].try_into().unwrap()
					});
					Error::<Test>::ProviderIneligible
				},
				4 => {
					let initial = CheckpointDutyPending::<Test>::take(bucket).unwrap();
					CheckpointDutyCurrent::<Test>::insert(bucket, initial.clone());
					let mut unfinalized = initial;
					unfinalized.scheduled_at = 121;
					CheckpointDutyPending::<Test>::insert(bucket, unfinalized);
					Error::<Test>::CheckpointDutyPending
				},
				_ => {
					signature = ed25519::Signature::from_raw([0; 64]);
					Error::<Test>::CheckpointFallbackPromotionWrongKey
				},
			};
			let bucket_before = Buckets::<Test>::get(bucket).unwrap();
			let agreement_before = Agreements::<Test>::get(agreement_id);
			let bucket_index_before = BucketAgreements::<Test>::get(bucket);
			let provider_indexes_before =
				(1..=4).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>();
			let assignment_counts_before =
				(1..=4).map(ProviderBucketAssignmentCount::<Test>::get).collect::<Vec<_>>();
			let pending_before = CheckpointDutyPending::<Test>::get(bucket);
			let current_before = CheckpointDutyCurrent::<Test>::get(bucket);
			let providers_before = (1..=4)
				.map(|provider| Providers::<Test>::get(provider).unwrap())
				.collect::<Vec<_>>();
			let events = System::events().len();
			assert_noop!(
				StorageProvider::promote_checkpoint_fallback(
					RuntimeOrigin::signed(2),
					promotion,
					pair(2).public(),
					signature,
				),
				expected
			);
			assert_eq!(Buckets::<Test>::get(bucket).unwrap(), bucket_before);
			assert_eq!(Agreements::<Test>::get(agreement_id), agreement_before);
			assert_eq!(BucketAgreements::<Test>::get(bucket), bucket_index_before);
			assert_eq!(
				(1..=4).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>(),
				provider_indexes_before
			);
			assert_eq!(
				(1..=4).map(ProviderBucketAssignmentCount::<Test>::get).collect::<Vec<_>>(),
				assignment_counts_before
			);
			assert_eq!(CheckpointDutyPending::<Test>::get(bucket), pending_before);
			assert_eq!(CheckpointDutyCurrent::<Test>::get(bucket), current_before);
			assert_eq!(
				(1..=4)
					.map(|provider| Providers::<Test>::get(provider).unwrap())
					.collect::<Vec<_>>(),
				providers_before
			);
			assert!(!CheckpointFallbackPromotionReceiptByBucket::<Test>::contains_key(bucket));
			assert_eq!(System::events().len(), events);
		});
	}
}

#[test]
fn grace_fallback_atomically_promotes_with_two_eligible_post_promotion_confirmers() {
	new_test_ext().execute_with(|| {
		for provider in 1..=4 {
			register(provider, 10_000);
		}
		let replicas: ReplicasOf<Test> = vec![2, 3, 4].try_into().unwrap();
		assert_ok!(StorageProvider::create_bucket(
			RuntimeOrigin::signed(OWNER),
			H256::repeat_byte(10),
			1,
			replicas,
		));
		let bucket = BucketIds::<Test>::get()[0];
		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			10,
			200,
		));
		let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
		assert_ok!(StorageProvider::rotate_service_key(
			RuntimeOrigin::root(),
			2,
			pair(8).public(),
			121,
		));
		assert_ok!(StorageProvider::rotate_service_key(
			RuntimeOrigin::root(),
			3,
			pair(7).public(),
			121,
		));
		set_finalized(121);
		Providers::<Test>::mutate(1, |record| {
			record.as_mut().unwrap().status = ProviderStatus::Suspended
		});
		let checkpoint = payload(bucket, H256::repeat_byte(32), 0, 1, 121);
		let (_, _, mut confirmations) = signed_checkpoint_for(checkpoint, 2, &[3, 4]);
		let checkpoint_digest = digest(DOMAIN, &checkpoint);
		let context_digest = StorageProvider::checkpoint_context_for(&checkpoint)
			.map(|context| StorageProvider::checkpoint_context_digest(&context))
			.unwrap();
		let signature = pair(8).sign(&checkpoint_digest);
		let context_signature = pair(8).sign(&context_digest);
		confirmations[0].service_key = pair(7).public();
		confirmations[0].signature = pair(7).sign(&checkpoint_digest);
		confirmations[0].context_signature = pair(7).sign(&context_digest);
		assert_ok!(StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(2),
			DOMAIN.to_vec().try_into().unwrap(),
			checkpoint,
			101,
			121,
			pair(8).public(),
			signature.clone(),
			context_signature.clone(),
			confirmations.clone(),
		));
		let promoted = Buckets::<Test>::get(bucket).unwrap();
		assert_eq!(promoted.primary, 2);
		assert_eq!(promoted.replicas.as_slice(), &[1, 3, 4]);
		assert_eq!(BucketSnapshots::<Test>::get(bucket).unwrap().commitment_nonce, 121);
		let rebound = Agreements::<Test>::get(agreement_id).unwrap();
		assert_eq!(rebound.primary, 2);
		assert_eq!(rebound.replicas.as_slice(), &[1, 3, 4]);
		assert_eq!(Providers::<Test>::get(2).unwrap().service_key.active, pair(8).public());
		assert_eq!(Providers::<Test>::get(3).unwrap().service_key.active, pair(7).public());
		let events = System::events().len();
		let bucket_after = promoted;
		let snapshot_after = BucketSnapshots::<Test>::get(bucket).unwrap();
		assert_ok!(StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(2),
			DOMAIN.to_vec().try_into().unwrap(),
			checkpoint,
			101,
			121,
			pair(8).public(),
			signature,
			context_signature,
			confirmations,
		));
		assert_eq!(Buckets::<Test>::get(bucket).unwrap(), bucket_after);
		assert_eq!(BucketSnapshots::<Test>::get(bucket).unwrap(), snapshot_after);
		assert_eq!(System::events().len(), events);

		let stale = payload(bucket, H256::repeat_byte(33), 0, 1, 121);
		let (stale_signature, stale_context_signature, stale_confirmations) =
			signed_checkpoint_for(stale, 3, &[2, 4]);
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(3),
				DOMAIN.to_vec().try_into().unwrap(),
				stale,
				101,
				121,
				pair(3).public(),
				stale_signature,
				stale_context_signature,
				stale_confirmations,
			),
			Error::<Test>::StorageCheckpointWrongWindow
		);
		assert_eq!(Buckets::<Test>::get(bucket).unwrap(), bucket_after);
		assert_eq!(BucketSnapshots::<Test>::get(bucket).unwrap(), snapshot_after);
		assert_eq!(System::events().len(), events);
	});
}

#[test]
fn grace_fallback_rolls_back_every_surface_on_hostile_invariants() {
	for case in 0..5 {
		new_test_ext().execute_with(|| {
			for provider in 1..=4 {
				register(provider, 10_000);
			}
			let replicas: ReplicasOf<Test> = vec![2, 3, 4].try_into().unwrap();
			assert_ok!(StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::repeat_byte(10),
				1,
				replicas,
			));
			let bucket = BucketIds::<Test>::get()[0];
			assert_ok!(StorageProvider::propose_agreement(
				RuntimeOrigin::signed(OWNER),
				bucket,
				1,
				10,
				200,
			));
			let agreement_id = Agreements::<Test>::iter_keys().next().unwrap();
			set_finalized(121);
			Providers::<Test>::mutate(1, |record| {
				record.as_mut().unwrap().status = ProviderStatus::Suspended
			});
			let expected = match case {
				0 => {
					Agreements::<Test>::remove(agreement_id);
					Error::<Test>::AgreementNotFound
				},
				1 => {
					Agreements::<Test>::mutate(agreement_id, |record| {
						record.as_mut().unwrap().bucket_id = H256::repeat_byte(99)
					});
					Error::<Test>::AgreementInvalidState
				},
				2 => {
					Agreements::<Test>::mutate(agreement_id, |record| {
						record.as_mut().unwrap().primary = 4
					});
					Error::<Test>::AgreementInvalidState
				},
				3 => {
					Agreements::<Test>::mutate(agreement_id, |record| {
						record.as_mut().unwrap().replicas = vec![3, 4].try_into().unwrap()
					});
					Error::<Test>::ProviderIneligible
				},
				_ => {
					let initial = CheckpointDutyPending::<Test>::take(bucket).unwrap();
					CheckpointDutyCurrent::<Test>::insert(bucket, initial.clone());
					let mut unfinalized = initial;
					unfinalized.scheduled_at = 121;
					CheckpointDutyPending::<Test>::insert(bucket, unfinalized);
					Error::<Test>::CheckpointDutyPending
				},
			};
			let checkpoint = payload(bucket, H256::repeat_byte(40 + case), 0, 1, 121);
			let (signature, context_signature, confirmations) =
				signed_checkpoint_for(checkpoint, 2, &[3, 4]);
			let bucket_before = Buckets::<Test>::get(bucket).unwrap();
			let agreement_before = Agreements::<Test>::get(agreement_id);
			let bucket_index_before = BucketAgreements::<Test>::get(bucket);
			let provider_indexes_before =
				(1..=4).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>();
			let assignment_counts_before =
				(1..=4).map(ProviderBucketAssignmentCount::<Test>::get).collect::<Vec<_>>();
			let pending_before = CheckpointDutyPending::<Test>::get(bucket);
			let current_before = CheckpointDutyCurrent::<Test>::get(bucket);
			let providers_before = (1..=4)
				.map(|provider| Providers::<Test>::get(provider).unwrap())
				.collect::<Vec<_>>();
			let events = System::events().len();
			assert_noop!(
				StorageProvider::submit_checkpoint(
					RuntimeOrigin::signed(2),
					DOMAIN.to_vec().try_into().unwrap(),
					checkpoint,
					101,
					121,
					pair(2).public(),
					signature,
					context_signature,
					confirmations,
				),
				expected
			);
			assert_eq!(Buckets::<Test>::get(bucket).unwrap(), bucket_before);
			assert_eq!(Agreements::<Test>::get(agreement_id), agreement_before);
			assert_eq!(BucketAgreements::<Test>::get(bucket), bucket_index_before);
			assert_eq!(
				(1..=4).map(ProviderAgreements::<Test>::get).collect::<Vec<_>>(),
				provider_indexes_before
			);
			assert_eq!(
				(1..=4).map(ProviderBucketAssignmentCount::<Test>::get).collect::<Vec<_>>(),
				assignment_counts_before
			);
			assert_eq!(CheckpointDutyPending::<Test>::get(bucket), pending_before);
			assert_eq!(CheckpointDutyCurrent::<Test>::get(bucket), current_before);
			assert_eq!(
				(1..=4)
					.map(|provider| Providers::<Test>::get(provider).unwrap())
					.collect::<Vec<_>>(),
				providers_before
			);
			assert!(BucketSnapshots::<Test>::get(bucket).is_none());
			assert_eq!(CheckpointClaims::<Test>::iter().count(), 0);
			assert_eq!(System::events().len(), events);
		});
	}
}

#[test]
fn bucket_agreement_admission_is_exact_and_terminal_release_frees_slot() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		for bytes in 1..=4u64 {
			assert_ok!(StorageProvider::propose_agreement(
				RuntimeOrigin::signed(OWNER),
				bucket,
				1,
				bytes,
				200,
			));
		}
		assert_eq!(BucketAgreements::<Test>::get(bucket).len(), 4);
		assert_noop!(
			StorageProvider::propose_agreement(RuntimeOrigin::signed(OWNER), bucket, 1, 5, 200,),
			Error::<Test>::AgreementIndexFull
		);
		let agreement_id = BucketAgreements::<Test>::get(bucket)[0];
		assert_ok!(StorageProvider::terminate_agreement(
			RuntimeOrigin::signed(OWNER),
			agreement_id,
			1,
		));
		System::set_block_number(12);
		StorageProvider::on_initialize(12);
		assert_eq!(BucketAgreements::<Test>::get(bucket).len(), 3);
		assert_ok!(StorageProvider::propose_agreement(
			RuntimeOrigin::signed(OWNER),
			bucket,
			1,
			5,
			200,
		));
		assert_eq!(BucketAgreements::<Test>::get(bucket).len(), 4);
	});
}

#[test]
fn second_unfinalized_duty_update_fails_closed_without_overwriting_first() {
	new_test_ext().execute_with(|| {
		for id in 1..=5 {
			register(id, 10_000);
		}
		System::set_block_number(2);
		assert_ok!(StorageProvider::create_bucket(
			RuntimeOrigin::signed(OWNER),
			H256::repeat_byte(10),
			1,
			vec![2, 3].try_into().unwrap(),
		));
		let bucket = BucketIds::<Test>::get()[0];
		let pending = CheckpointDutyPending::<Test>::get(bucket).unwrap();
		let before = Buckets::<Test>::get(bucket).unwrap();
		assert_noop!(
			StorageProvider::replace_bucket_replica(RuntimeOrigin::signed(OWNER), bucket, 1, 2, 5,),
			Error::<Test>::CheckpointDutyPending
		);
		assert_eq!(CheckpointDutyPending::<Test>::get(bucket).unwrap(), pending);
		assert_eq!(Buckets::<Test>::get(bucket).unwrap(), before);
		assert_eq!(ProviderBucketAssignmentCount::<Test>::get(2), 1);
		assert_eq!(ProviderBucketAssignmentCount::<Test>::get(5), 0);
	});
}

#[test]
fn duplicate_challenge_is_rejected_and_proof_prunes_every_work_index() {
	new_test_ext().execute_with(|| {
		let bucket = setup_bucket();
		let leaf = MmrLeafV1 { data_root: H256::repeat_byte(44), data_size: 1, total_size: 1 };
		let root = sp_runtime::traits::BlakeTwo256::hash_of(&leaf);
		set_finalized(101);
		submit(payload(bucket, root, 0, 1, 101));
		System::set_block_number(101);
		let location = crate::ChunkLocationV1 { leaf_index: 0, chunk_index: 0 };
		assert_ok!(StorageProvider::issue_challenge(
			RuntimeOrigin::root(),
			bucket,
			1,
			location,
			105,
		));
		let challenge_id = crate::Challenges::<Test>::iter_keys().next().unwrap();
		assert_noop!(
			StorageProvider::issue_challenge(RuntimeOrigin::root(), bucket, 1, location, 105,),
			Error::<Test>::ChallengeAlreadyExists
		);
		assert_eq!(ChallengeBacklog::<Test>::get().as_slice(), &[challenge_id]);
		assert_ok!(StorageProvider::submit_challenge_proof(
			RuntimeOrigin::signed(1),
			challenge_id,
			MmrProofV1 { peaks: vec![root], leaf, leaf_proof: vec![] },
		));
		assert!(ChallengeBacklog::<Test>::get().is_empty());
		assert_eq!(
			crate::Challenges::<Test>::get(challenge_id).unwrap().status,
			ChallengeStatus::Proved
		);
	});
}

#[test]
fn checkpoint_dispatch_weights_cover_atomic_fallback_at_configured_bounds() {
	use crate::weights::{SubstrateWeight, WeightInfo};
	use frame_support::{dispatch::GetDispatchInfo, traits::Get};

	new_test_ext().execute_with(|| {
		let max_replicas = <<Test as crate::Config>::MaxReplicas as Get<u32>>::get();
		let max_agreements = <MaxAgreements as Get<u32>>::get();
		for id in 1..=5 {
			register(id, 10_000);
		}
		let replicas: ReplicasOf<Test> = vec![2, 3, 4, 5].try_into().unwrap();
		assert_eq!(replicas.len() as u32, max_replicas);
		assert_ok!(StorageProvider::create_bucket(
			RuntimeOrigin::signed(OWNER),
			H256::repeat_byte(10),
			1,
			replicas,
		));
		let bucket = BucketIds::<Test>::get()[0];
		let agreement_ids: frame_support::BoundedVec<H256, MaxAgreements> = (0..max_agreements)
			.map(|index| H256::from_low_u64_be(index as u64 + 1))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		BucketAgreements::<Test>::insert(bucket, agreement_ids);

		let checkpoint_payload = payload(bucket, H256::repeat_byte(44), 0, 1, 1);
		let confirmations: ConfirmationsOf<Test> = (2..=5)
			.map(|provider| ReplicaSignature {
				provider,
				service_key: pair(provider as u8).public(),
				signature: pair(provider as u8).sign(&[0u8; 32]),
				context_signature: pair(provider as u8).sign(&[1u8; 32]),
			})
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		let checkpoint = crate::Call::<Test>::submit_checkpoint {
			domain: DOMAIN.to_vec().try_into().unwrap(),
			payload: checkpoint_payload,
			window_start: 1,
			window_end: 1,
			service_key: pair(1).public(),
			primary_signature: pair(1).sign(&[0u8; 32]),
			primary_context_signature: pair(1).sign(&[1u8; 32]),
			confirmations,
		};
		let expected_checkpoint = SubstrateWeight::<Test>::submit_checkpoint(max_replicas)
			.saturating_add(SubstrateWeight::<Test>::promote_checkpoint_fallback(max_agreements))
			.saturating_add(frame_support::weights::Weight::from_parts(5_000_000, 64));
		assert_eq!(checkpoint.get_dispatch_info().call_weight, expected_checkpoint);

		let promotion = crate::Call::<Test>::promote_checkpoint_fallback {
			payload: CheckpointFallbackPromotionV1 {
				version: 1,
				bucket_id: bucket,
				snapshot_nonce: 1,
				duty_id: H256::repeat_byte(45),
			},
			service_key: pair(2).public(),
			signature: pair(2).sign(&[2u8; 32]),
		};
		assert_eq!(
			promotion.get_dispatch_info().call_weight,
			SubstrateWeight::<Test>::promote_checkpoint_fallback(max_agreements)
		);
	});
}

#[test]
fn host_delegation_is_the_owner_controlled_no_fallback_capability_authority() {
	new_test_ext().execute_with(|| {
		let bucket_id = setup_bucket();
		assert_ok!(StorageProvider::change_bucket_grant(
			RuntimeOrigin::signed(OWNER),
			bucket_id,
			1,
			77,
			Some(crate::BucketRole::Admin),
		));
		assert!(HostDelegations::<Test>::iter().next().is_none());
		assert_noop!(
			StorageProvider::create_host_delegation(
				RuntimeOrigin::signed(77),
				bucket_id,
				H256::repeat_byte(1),
				pair(70).public(),
				product(b"festival"),
				methods(&[1010, 1011]),
				None,
				1024,
				100,
			),
			Error::<Test>::NotBucketOwner
		);

		assert_ok!(StorageProvider::create_host_delegation(
			RuntimeOrigin::signed(OWNER),
			bucket_id,
			H256::repeat_byte(1),
			pair(70).public(),
			product(b"festival"),
			methods(&[1010, 1011]),
			Some(capability_cid(b"bafk-test")),
			1024,
			100,
		));
		let grant_id = StorageProvider::host_delegation_id(&OWNER, bucket_id, 0);
		let created = HostDelegations::<Test>::get(grant_id).unwrap();
		assert_eq!(created.issuance_nonce, 0);
		assert_eq!(created.key_version, 1);
		assert_eq!(created.state_version, 1);
		assert_eq!(created.key_activated_at, 1);
		assert_eq!(GrantNonce::<Test>::get(OWNER), 1);
		assert_eq!(BucketHostDelegations::<Test>::get(bucket_id).as_slice(), &[grant_id]);

		System::set_block_number(5);
		let new_key = pair(71).public();
		assert_noop!(
			StorageProvider::rotate_host_delegation(
				RuntimeOrigin::signed(OWNER),
				grant_id,
				2,
				H256::repeat_byte(2),
				new_key,
			),
			Error::<Test>::HostDelegationVersionConflict
		);
		assert_ok!(StorageProvider::rotate_host_delegation(
			RuntimeOrigin::signed(OWNER),
			grant_id,
			1,
			H256::repeat_byte(2),
			new_key,
		));
		let rotated = HostDelegations::<Test>::get(grant_id).unwrap();
		assert_eq!(rotated.issuer_public_key, new_key);
		assert_ne!(rotated.issuer_public_key, created.issuer_public_key);
		assert_eq!(rotated.key_version, 2);
		assert_eq!(rotated.state_version, 2);
		assert_eq!(rotated.key_activated_at, 5);
		assert_eq!(rotated.issued_at, 1);

		assert_noop!(
			StorageProvider::revoke_host_delegation(RuntimeOrigin::signed(77), grant_id, 2),
			Error::<Test>::NotBucketOwner
		);
		assert_ok!(StorageProvider::revoke_host_delegation(
			RuntimeOrigin::signed(OWNER),
			grant_id,
			2,
		));
		let revoked = HostDelegations::<Test>::get(grant_id).unwrap();
		assert_eq!(revoked.state_version, 3);
		assert_eq!(revoked.revoked_at, Some(5));
		assert!(BucketHostDelegations::<Test>::get(bucket_id).is_empty());
		assert_noop!(
			StorageProvider::rotate_host_delegation(
				RuntimeOrigin::signed(OWNER),
				grant_id,
				3,
				H256::repeat_byte(3),
				pair(72).public(),
			),
			Error::<Test>::HostDelegationRevoked
		);
		assert_ok!(StorageProvider::create_host_delegation(
			RuntimeOrigin::signed(OWNER),
			bucket_id,
			H256::repeat_byte(3),
			pair(72).public(),
			product(b"festival"),
			methods(&[1010]),
			None,
			1024,
			100,
		));
		let successor = StorageProvider::host_delegation_id(&OWNER, bucket_id, 1);
		assert_ne!(successor, grant_id);
		assert_eq!(HostDelegations::<Test>::get(grant_id).unwrap(), revoked);
	});
}

#[test]
fn host_delegation_scope_lifetime_active_bound_and_nonce_are_fail_closed() {
	new_test_ext().execute_with(|| {
		let bucket_id = setup_bucket();
		let create = |product_id, methods, cid, max_bytes, expires_at| {
			StorageProvider::create_host_delegation(
				RuntimeOrigin::signed(OWNER),
				bucket_id,
				H256::repeat_byte(1),
				pair(80).public(),
				product_id,
				methods,
				cid,
				max_bytes,
				expires_at,
			)
		};
		assert_noop!(
			create(product(b""), methods(&[1]), None, 1, 2),
			Error::<Test>::InvalidCapabilityScope
		);
		assert_noop!(
			create(product(&[0xff]), methods(&[1]), None, 1, 2),
			Error::<Test>::InvalidCapabilityScope
		);
		assert_noop!(
			create(product(b"app"), methods(&[]), None, 1, 2),
			Error::<Test>::InvalidCapabilityScope
		);
		assert_noop!(
			create(product(b"app"), methods(&[2, 1]), None, 1, 2),
			Error::<Test>::InvalidCapabilityScope
		);
		assert_noop!(
			create(product(b"app"), methods(&[1, 1]), None, 1, 2),
			Error::<Test>::InvalidCapabilityScope
		);
		assert_noop!(
			create(product(b"app"), methods(&[1]), Some(capability_cid(b"")), 1, 2),
			Error::<Test>::InvalidCapabilityScope
		);
		assert_noop!(
			create(product(b"app"), methods(&[1]), Some(capability_cid(&[0xff])), 1, 2),
			Error::<Test>::InvalidCapabilityScope
		);
		assert_noop!(
			create(product(b"app"), methods(&[1]), None, 0, 2),
			Error::<Test>::InvalidCapabilityScope
		);
		assert_noop!(
			create(product(b"app"), methods(&[1]), None, 1, 1),
			Error::<Test>::InvalidCapabilityLifetime
		);
		assert_noop!(
			create(product(b"app"), methods(&[1]), None, 1, 130),
			Error::<Test>::InvalidCapabilityLifetime
		);

		assert_ok!(create(product(b"one"), methods(&[1]), None, 1, 129));
		assert_ok!(create(product(b"two"), methods(&[2]), None, 1, 129));
		assert_noop!(
			create(product(b"three"), methods(&[3]), None, 1, 129),
			Error::<Test>::HostDelegationLimit
		);
		assert_eq!(GrantNonce::<Test>::get(OWNER), 2);

		GrantNonce::<Test>::insert(OWNER, u64::MAX);
		let other_bucket = {
			let replicas: ReplicasOf<Test> = vec![2, 3].try_into().unwrap();
			assert_ok!(StorageProvider::create_bucket(
				RuntimeOrigin::signed(OWNER),
				H256::repeat_byte(11),
				1,
				replicas,
			));
			BucketIds::<Test>::get()[1]
		};
		assert_noop!(
			StorageProvider::create_host_delegation(
				RuntimeOrigin::signed(OWNER),
				other_bucket,
				H256::repeat_byte(4),
				pair(81).public(),
				product(b"overflow"),
				methods(&[1]),
				None,
				1,
				129,
			),
			Error::<Test>::GrantNonceOverflow
		);
		assert_eq!(GrantNonce::<Test>::get(OWNER), u64::MAX);
	});
}
