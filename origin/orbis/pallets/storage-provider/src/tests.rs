// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
	mock::*, ActivatedAgreements, AgreementRecord, AgreementStatus, Agreements, Challenges,
	DeletionAcknowledgements, EndpointOf, Error, OpenChallengeCount, ProviderCheckpoint,
	ProviderRoots, Providers, ServiceKeyOf,
};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Hash};

const PROVIDER: u64 = 7;
const OWNER: u64 = 9;

fn register() {
	let endpoint: EndpointOf<Test> = b"http://provider".to_vec().try_into().unwrap();
	let key: ServiceKeyOf<Test> = b"service-key".to_vec().try_into().unwrap();
	assert_ok!(StorageProvider::register_provider(
		RuntimeOrigin::root(),
		PROVIDER,
		endpoint,
		key,
		1_000,
	));
}

fn terminal_agreement(agreement: H256, content: H256) {
	Agreements::<Test>::insert(
		agreement,
		AgreementRecord {
			owner: OWNER,
			provider: PROVIDER,
			container_ref: H256::repeat_byte(4),
			content_commitment: content,
			reservation_ref: None,
			bytes: 1,
			created_at: 1,
			expires_at: 2,
			pending_expiry: None,
			status: AgreementStatus::Cancelled,
		},
	);
	ActivatedAgreements::<Test>::insert(agreement, ());
}

fn node(left: H256, right: H256) -> H256 {
	BlakeTwo256::hash_of(&(b"orbis/provider-node/v2", left, right))
}

fn tree(mut leaves: Vec<H256>, mut index: usize) -> (H256, Vec<H256>) {
	let mut proof = Vec::new();
	while leaves.len() > 1 {
		proof.push(if index % 2 == 0 {
			*leaves.get(index + 1).unwrap_or(&leaves[index])
		} else {
			leaves[index - 1]
		});
		index /= 2;
		leaves = leaves
			.chunks(2)
			.map(|pair| node(pair[0], *pair.get(1).unwrap_or(&pair[0])))
			.collect();
	}
	(leaves[0], proof)
}

fn leaf(agreement: H256, content: H256, provider: u64) -> H256 {
	BlakeTwo256::hash_of(&(b"orbis/provider-deletion-leaf/v1", agreement, content, provider, 0u64))
}

fn append(sequence: u64, leaves: Vec<H256>) {
	assert_ok!(StorageProvider::commit_provider_root(
		RuntimeOrigin::signed(PROVIDER),
		sequence,
		leaves.try_into().unwrap(),
	));
}

fn acknowledge_for(count: usize, index: usize) {
	new_test_ext().execute_with(|| {
		register();
		let agreement = H256::repeat_byte(count as u8);
		let content = H256::repeat_byte((count + 10) as u8);
		terminal_agreement(agreement, content);
		let mut leaves: Vec<_> = (0..count).map(|n| H256::repeat_byte((50 + n) as u8)).collect();
		leaves[index] = leaf(agreement, content, PROVIDER);
		let (root, proof) = tree(leaves.clone(), index);
		assert_noop!(
			StorageProvider::acknowledge_deletion(
				RuntimeOrigin::signed(PROVIDER),
				agreement,
				content,
				root,
				1,
				index as u64,
				count as u64,
				proof.clone().try_into().unwrap(),
			),
			Error::<Test>::ProviderRootNotFound
		);
		append(1, leaves);
		assert_ok!(StorageProvider::acknowledge_deletion(
			RuntimeOrigin::signed(PROVIDER),
			agreement,
			content,
			root,
			1,
			index as u64,
			count as u64,
			proof.try_into().unwrap(),
		));
		let accepted = DeletionAcknowledgements::<Test>::get(agreement).unwrap();
		assert_eq!(
			(accepted.root_sequence, accepted.leaf_index, accepted.leaf_count),
			(1, index as u64, count as u64)
		);
	});
}

#[test]
fn odd_and_even_duplicate_last_proofs_require_root_before_ack() {
	acknowledge_for(3, 2);
	acknowledge_for(4, 1);
}

#[test]
fn frontier_matches_duplicate_last_roots_at_honest_boundaries() {
	new_test_ext().execute_with(|| {
		register();
		let mut leaves = Vec::new();
		for sequence in 1..=15u64 {
			let next = H256::repeat_byte(sequence as u8);
			leaves.push(next);
			append(sequence, vec![next]);
			let record = ProviderRoots::<Test>::get(PROVIDER).unwrap();
			assert_eq!(record.sequence, sequence);
			assert_eq!(record.leaf_count, sequence);
			assert_eq!(record.root, tree(leaves.clone(), 0).0);
		}
	});
}

#[test]
fn bounded_multiappend_verifies_every_leaf_and_matches_single_append_history() {
	let leaves: Vec<_> = (1..=9).map(H256::repeat_byte).collect();
	let expected = tree(leaves.clone(), 0).0;
	let batched = new_test_ext().execute_with(|| {
		register();
		append(1, leaves[..3].to_vec());
		append(2, leaves[3..].to_vec());
		ProviderRoots::<Test>::get(PROVIDER).unwrap().root
	});
	let singly = new_test_ext().execute_with(|| {
		register();
		for (index, leaf) in leaves.into_iter().enumerate() {
			append(index as u64 + 1, vec![leaf]);
		}
		ProviderRoots::<Test>::get(PROVIDER).unwrap().root
	});
	assert_eq!(batched, expected);
	assert_eq!(singly, expected);
}

#[test]
fn root_sequence_replay_skip_rollback_and_replacement_fail_closed() {
	new_test_ext().execute_with(|| {
		register();
		let first = vec![H256::repeat_byte(1), H256::repeat_byte(2), H256::repeat_byte(3)];
		append(1, first.clone());
		let accepted = ProviderRoots::<Test>::get(PROVIDER).unwrap();
		// Exact replay is the only same-sequence crash recovery operation.
		append(1, first);
		assert_eq!(ProviderRoots::<Test>::get(PROVIDER).unwrap(), accepted);
		assert_noop!(
			StorageProvider::commit_provider_root(
				RuntimeOrigin::signed(PROVIDER),
				1,
				vec![H256::repeat_byte(99)].try_into().unwrap(),
			),
			Error::<Test>::InvalidRootSequence
		);
		assert_noop!(
			StorageProvider::commit_provider_root(
				RuntimeOrigin::signed(PROVIDER),
				0,
				vec![H256::repeat_byte(4)].try_into().unwrap(),
			),
			Error::<Test>::InvalidRootSequence
		);
		assert_noop!(
			StorageProvider::commit_provider_root(
				RuntimeOrigin::signed(PROVIDER),
				3,
				vec![H256::repeat_byte(4)].try_into().unwrap(),
			),
			Error::<Test>::InvalidRootSequence
		);
		let empty: BoundedVec<H256, MaxRootAppendBatch> = BoundedVec::default();
		assert_noop!(
			StorageProvider::commit_provider_root(RuntimeOrigin::signed(PROVIDER), 2, empty),
			Error::<Test>::RootAppendBatchEmpty
		);
		assert_eq!(ProviderRoots::<Test>::get(PROVIDER).unwrap(), accepted);
	});
}

#[test]
fn modified_prior_frontier_is_detected_instead_of_rewritten() {
	new_test_ext().execute_with(|| {
		register();
		append(1, vec![H256::repeat_byte(1), H256::repeat_byte(2)]);
		ProviderRoots::<Test>::mutate(PROVIDER, |record| {
			record.as_mut().unwrap().frontier[1] = None;
		});
		assert_noop!(
			StorageProvider::commit_provider_root(
				RuntimeOrigin::signed(PROVIDER),
				2,
				vec![H256::repeat_byte(3)].try_into().unwrap(),
			),
			Error::<Test>::InvalidProviderFrontier
		);
	});
}

#[test]
fn historical_root_rejected_and_exact_ack_replay_survives_root_advance_and_prune() {
	new_test_ext().execute_with(|| {
		register();
		let agreement = H256::repeat_byte(11);
		let content = H256::repeat_byte(12);
		terminal_agreement(agreement, content);
		let leaves = vec![H256::repeat_byte(13), leaf(agreement, content, PROVIDER)];
		let (root, proof) = tree(leaves.clone(), 1);
		append(1, leaves);
		append(2, vec![H256::repeat_byte(14)]);
		assert_noop!(
			StorageProvider::acknowledge_deletion(
				RuntimeOrigin::signed(PROVIDER),
				agreement,
				content,
				root,
				1,
				1,
				2,
				proof.clone().try_into().unwrap(),
			),
			Error::<Test>::ProviderRootMismatch
		);
	});

	new_test_ext().execute_with(|| {
		register();
		let agreement = H256::repeat_byte(21);
		let content = H256::repeat_byte(22);
		terminal_agreement(agreement, content);
		let leaves = vec![H256::repeat_byte(23), leaf(agreement, content, PROVIDER)];
		let (root, proof) = tree(leaves.clone(), 1);
		append(1, leaves);
		assert_ok!(StorageProvider::acknowledge_deletion(
			RuntimeOrigin::signed(PROVIDER),
			agreement,
			content,
			root,
			1,
			1,
			2,
			proof.clone().try_into().unwrap(),
		));
		append(2, vec![H256::repeat_byte(24)]);
		let events_after_advance = System::events().len();
		// The outbox consumer may have finalized the call before its receipt was fsynced. An exact
		// replay remains a no-op even though the provider has since advanced its current root.
		assert_ok!(StorageProvider::acknowledge_deletion(
			RuntimeOrigin::signed(PROVIDER),
			agreement,
			content,
			root,
			1,
			1,
			2,
			proof.clone().try_into().unwrap(),
		));
		assert_eq!(System::events().len(), events_after_advance);
		let mut mismatched = proof.clone();
		mismatched[0] = H256::repeat_byte(99);
		assert_noop!(
			StorageProvider::acknowledge_deletion(
				RuntimeOrigin::signed(PROVIDER),
				agreement,
				content,
				root,
				1,
				1,
				2,
				mismatched.try_into().unwrap(),
			),
			Error::<Test>::DeletionAlreadyAcknowledged
		);
		let accepted = DeletionAcknowledgements::<Test>::get(agreement).unwrap();
		assert_eq!(accepted.provider, PROVIDER);
		assert_eq!(accepted.root_sequence, 1);
		assert_eq!(accepted.tombstone_root, root);
		assert_eq!(ProviderRoots::<Test>::get(PROVIDER).unwrap().sequence, 2);
		assert_ok!(StorageProvider::prune_agreement(RuntimeOrigin::signed(OWNER), agreement,));
		assert!(!Agreements::<Test>::contains_key(agreement));
		assert!(DeletionAcknowledgements::<Test>::contains_key(agreement));
		let events_after_prune = System::events().len();
		assert_ok!(StorageProvider::acknowledge_deletion(
			RuntimeOrigin::signed(PROVIDER),
			agreement,
			content,
			root,
			1,
			1,
			2,
			proof.clone().try_into().unwrap(),
		));
		assert_eq!(System::events().len(), events_after_prune);
		assert_noop!(
			StorageProvider::acknowledge_deletion(
				RuntimeOrigin::signed(OWNER),
				agreement,
				content,
				root,
				1,
				1,
				2,
				proof.try_into().unwrap(),
			),
			Error::<Test>::NotAgreementParty
		);
		assert_eq!(System::events().len(), events_after_prune);
	});
}

#[test]
fn finalized_checkpoint_replay_is_exact_and_has_no_duplicate_side_effects() {
	new_test_ext().execute_with(|| {
		register();
		let agreement = H256::repeat_byte(41);
		let content = H256::repeat_byte(42);
		Agreements::<Test>::insert(
			agreement,
			AgreementRecord {
				owner: OWNER,
				provider: PROVIDER,
				container_ref: H256::repeat_byte(43),
				content_commitment: content,
				reservation_ref: None,
				bytes: 1,
				created_at: 1,
				expires_at: 100,
				pending_expiry: None,
				status: AgreementStatus::Active,
			},
		);
		append(1, vec![H256::repeat_byte(44)]);
		let root = ProviderRoots::<Test>::get(PROVIDER).unwrap().root;
		let due_at = 10;
		assert_ok!(StorageProvider::issue_challenge(
			RuntimeOrigin::root(),
			agreement,
			root,
			due_at,
		));
		let challenge =
			BlakeTwo256::hash_of(&(b"orbis/provider-challenge/v1", agreement, root, due_at));
		let proof = StorageProvider::expected_challenge_proof(
			challenge,
			&Challenges::<Test>::get(challenge).unwrap(),
		)
		.unwrap();
		assert_ok!(StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(PROVIDER),
			challenge,
			proof,
		));
		let checkpoint = ProviderCheckpoint::<Test>::get(PROVIDER).unwrap();
		let reputation = Providers::<Test>::get(PROVIDER).unwrap().reputation;
		let events = System::events().len();
		assert_eq!(OpenChallengeCount::<Test>::get(agreement), 0);

		System::set_block_number(20);
		assert_ok!(StorageProvider::submit_checkpoint(
			RuntimeOrigin::signed(PROVIDER),
			challenge,
			proof,
		));
		assert_eq!(ProviderCheckpoint::<Test>::get(PROVIDER).unwrap(), checkpoint);
		assert_eq!(Providers::<Test>::get(PROVIDER).unwrap().reputation, reputation);
		assert_eq!(OpenChallengeCount::<Test>::get(agreement), 0);
		assert_eq!(System::events().len(), events);
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(PROVIDER),
				challenge,
				H256::repeat_byte(99),
			),
			Error::<Test>::InvalidProofCommitment
		);
		assert_eq!(System::events().len(), events);

		let second_due = 30;
		assert_ok!(StorageProvider::issue_challenge(
			RuntimeOrigin::root(),
			agreement,
			root,
			second_due,
		));
		let second =
			BlakeTwo256::hash_of(&(b"orbis/provider-challenge/v1", agreement, root, second_due));
		let second_proof = StorageProvider::expected_challenge_proof(
			second,
			&Challenges::<Test>::get(second).unwrap(),
		)
		.unwrap();
		System::set_block_number(31);
		assert_ok!(StorageProvider::timeout_challenge(RuntimeOrigin::signed(OWNER), second,));
		let events_after_timeout = System::events().len();
		assert_noop!(
			StorageProvider::submit_checkpoint(
				RuntimeOrigin::signed(PROVIDER),
				second,
				second_proof,
			),
			Error::<Test>::ChallengeNotOpen
		);
		assert_eq!(System::events().len(), events_after_timeout);
	});
}

#[test]
fn tampered_deletion_proofs_and_odd_terminal_malleability_fail_closed() {
	new_test_ext().execute_with(|| {
		register();
		let agreement = H256::repeat_byte(31);
		let content = H256::repeat_byte(32);
		terminal_agreement(agreement, content);
		let tombstone = leaf(agreement, content, PROVIDER);
		let leaves = vec![H256::repeat_byte(33), H256::repeat_byte(34), tombstone];
		let (root, mut proof) = tree(leaves.clone(), 2);
		append(1, leaves);
		assert_noop!(
			StorageProvider::acknowledge_deletion(
				RuntimeOrigin::signed(PROVIDER),
				agreement,
				content,
				root,
				1,
				3,
				3,
				proof.clone().try_into().unwrap(),
			),
			Error::<Test>::InvalidLeafCount
		);
		let short_proof = proof[..1].to_vec();
		assert_noop!(
			StorageProvider::acknowledge_deletion(
				RuntimeOrigin::signed(PROVIDER),
				agreement,
				content,
				root,
				1,
				2,
				3,
				short_proof.try_into().unwrap(),
			),
			Error::<Test>::InvalidProofDepth
		);
		proof[0] = H256::repeat_byte(99);
		assert_noop!(
			StorageProvider::acknowledge_deletion(
				RuntimeOrigin::signed(PROVIDER),
				agreement,
				content,
				root,
				1,
				2,
				3,
				proof.try_into().unwrap(),
			),
			Error::<Test>::InvalidInclusionProof
		);
	});
}
