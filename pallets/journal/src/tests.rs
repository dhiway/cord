// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Marks: Handles #MARKs on chain,
//! adding and revoking #MARKs.

use frame_support::{assert_noop, assert_ok};
use sp_core::Pair;

use crate::{self as mark, mock::*};
use pallet_delegation::mock as delegation_mock;
use pallet_mtype::mock as mtype_mock;

// submit_mark_creation_operation

#[test]
fn anchor_no_delegation_successful() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let stream_hash = get_stream_hash(true);
	let mark = generate_base_mark(issuer.clone());

	let operation = generate_base_mark_creation_details(stream_hash, mark);

	let mut ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(operation.mtype_hash, issuer.clone())])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Mark::anchor(
			get_origin(issuer.clone()),
			operation.stream_hash,
			operation.mtype_hash,
			operation.delegation_id
		));
	});

	let stored_mark = ext.execute_with(|| Mark::marks(&stream_hash).expect("Mark should be present on chain."));

	assert_eq!(stored_mark.mtype_hash, operation.mtype_hash);
	assert_eq!(stored_mark.issuer, issuer);
	assert_eq!(stored_mark.delegation_id, operation.delegation_id);
	assert!(!stored_mark.revoked);
}

#[test]
fn anchor_with_delegation_successful() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let stream_hash = get_stream_hash(true);
	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(issuer.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, issuer.clone()),
	);
	delegation_node.permissions = pallet_delegation::Permissions::ANCHOR;
	let mut mark = generate_base_mark(issuer.clone());
	mark.delegation_id = Some(delegation_id);

	let operation = generate_base_mark_creation_details(stream_hash, mark);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(operation.mtype_hash, issuer.clone())])
		.build(None);
	let mut ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Mark::anchor(
			get_origin(issuer.clone()),
			operation.stream_hash,
			operation.mtype_hash,
			operation.delegation_id
		));
	});

	let stored_mark = ext.execute_with(|| Mark::marks(&stream_hash).expect("Mark should be present on chain."));

	assert_eq!(stored_mark.mtype_hash, operation.mtype_hash);
	assert_eq!(stored_mark.issuer, issuer);
	assert_eq!(stored_mark.delegation_id, operation.delegation_id);
	assert!(!stored_mark.revoked);

	let delegated_marks = ext.execute_with(|| {
		Mark::delegated_marks(&delegation_id).expect("Attested delegation should be present on chain.")
	});

	assert_eq!(delegated_marks, vec![stream_hash]);
}

#[test]
fn mtype_not_present_anchor_error() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let stream_hash = get_stream_hash(true);
	let mark = generate_base_mark(issuer.clone());

	let operation = generate_base_mark_creation_details(stream_hash, mark);

	// No MTYPE stored
	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Mark::anchor(
				get_origin(issuer.clone()),
				operation.stream_hash,
				operation.mtype_hash,
				operation.delegation_id
			),
			pallet_mtype::Error::<Test>::MTypeNotFound
		);
	});
}

#[test]
fn duplicate_anchor_error() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let stream_hash = get_stream_hash(true);
	let mark = generate_base_mark(issuer.clone());

	let operation = generate_base_mark_creation_details(stream_hash, mark.clone());

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(operation.mtype_hash, issuer.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default().build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::anchor(
				get_origin(issuer.clone()),
				operation.stream_hash,
				operation.mtype_hash,
				operation.delegation_id
			),
			mark::Error::<Test>::AlreadyAnchored
		);
	});
}

#[test]
fn delegation_not_found_anchor_error() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let stream_hash = get_stream_hash(true);
	let delegation_id = delegation_mock::get_delegation_id(true);
	let mut mark = generate_base_mark(issuer.clone());
	mark.delegation_id = Some(delegation_id);

	let operation = generate_base_mark_creation_details(stream_hash, mark);

	let mut ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(operation.mtype_hash, issuer.clone())])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Mark::anchor(
				get_origin(issuer.clone()),
				operation.stream_hash,
				operation.mtype_hash,
				operation.delegation_id
			),
			pallet_delegation::Error::<Test>::DelegationNotFound
		);
	});
}

#[test]
fn delegation_revoked_anchor_error() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let stream_hash = get_stream_hash(true);
	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(issuer.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, issuer.clone()),
	);
	delegation_node.permissions = pallet_delegation::Permissions::ANCHOR;
	delegation_node.revoked = true;
	let mut mark = generate_base_mark(issuer.clone());
	mark.delegation_id = Some(delegation_id);

	let operation = generate_base_mark_creation_details(stream_hash, mark);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(operation.mtype_hash, issuer.clone())])
		.build(None);
	let mut ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::anchor(
				get_origin(issuer.clone()),
				operation.stream_hash,
				operation.mtype_hash,
				operation.delegation_id
			),
			mark::Error::<Test>::DelegationRevoked
		);
	});
}

#[test]
fn not_delegation_owner_anchor_error() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let alternative_owner_keypair = get_bob_ed25519();
	let alternative_owner = get_ed25519_account(alternative_owner_keypair.public());
	let stream_hash = get_stream_hash(true);
	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(alternative_owner.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, alternative_owner),
	);
	delegation_node.permissions = pallet_delegation::Permissions::ANCHOR;
	let mut mark = generate_base_mark(issuer.clone());
	mark.delegation_id = Some(delegation_id);

	let operation = generate_base_mark_creation_details(stream_hash, mark);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(operation.mtype_hash, issuer.clone())])
		.build(None);
	let mut ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::anchor(
				get_origin(issuer.clone()),
				operation.stream_hash,
				operation.mtype_hash,
				operation.delegation_id
			),
			mark::Error::<Test>::NotDelegatedToMarker
		);
	});
}

#[test]
fn unauthorised_permissions_anchor_error() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let stream_hash = get_stream_hash(true);
	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(issuer.clone()),
	);
	let (delegation_id, delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, issuer.clone()),
	);
	let mut mark = generate_base_mark(issuer.clone());
	mark.delegation_id = Some(delegation_id);

	let operation = generate_base_mark_creation_details(stream_hash, mark);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(operation.mtype_hash, issuer.clone())])
		.build(None);
	let mut ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::anchor(
				get_origin(issuer.clone()),
				operation.stream_hash,
				operation.mtype_hash,
				operation.delegation_id
			),
			mark::Error::<Test>::DelegationUnauthorizedToAnchor
		);
	});
}

#[test]
fn root_not_present_anchor_error() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let stream_hash = get_stream_hash(true);
	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(issuer.clone()),
	);
	let alternative_root_id = delegation_mock::get_delegation_root_id(false);
	let (delegation_id, mut delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, issuer.clone()),
	);
	delegation_node.permissions = pallet_delegation::Permissions::ANCHOR;
	let mut mark = generate_base_mark(issuer.clone());
	mark.delegation_id = Some(delegation_id);

	let operation = generate_base_mark_creation_details(stream_hash, mark);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(operation.mtype_hash, issuer.clone())])
		.build(None);
	let mut ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(alternative_root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(alternative_root_id, vec![delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::anchor(
				get_origin(issuer.clone()),
				operation.stream_hash,
				operation.mtype_hash,
				operation.delegation_id
			),
			pallet_delegation::Error::<Test>::RootNotFound
		);
	});
}

#[test]
fn root_mtype_mismatch_anchor_error() {
	let issuer_keypair = get_alice_ed25519();
	let issuer = get_ed25519_account(issuer_keypair.public());
	let stream_hash = get_stream_hash(true);
	let alternative_mtype_hash = mtype_mock::get_mtype_hash(false);
	let (root_id, mut root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(issuer.clone()),
	);
	root_node.mtype_hash = alternative_mtype_hash;
	let (delegation_id, mut delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, issuer.clone()),
	);
	delegation_node.permissions = pallet_delegation::Permissions::ANCHOR;
	let mut mark = generate_base_mark(issuer.clone());
	mark.delegation_id = Some(delegation_id);

	let operation = generate_base_mark_creation_details(stream_hash, mark);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(operation.mtype_hash, issuer.clone())])
		.build(None);
	let mut ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::anchor(
				get_origin(issuer.clone()),
				operation.stream_hash,
				operation.mtype_hash,
				operation.delegation_id
			),
			mark::Error::<Test>::MTypeMismatch
		);
	});
}

// submit_mark_revocation_operation

#[test]
fn revoke_direct_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let stream_hash = get_stream_hash(true);
	let mark = generate_base_mark(revoker.clone());

	let operation = generate_base_mark_revocation_details(stream_hash);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default().build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(operation.stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Mark::revoke(
			get_origin(revoker.clone()),
			operation.stream_hash,
			operation.max_parent_checks
		));
	});

	let stored_mark = ext.execute_with(|| Mark::marks(stream_hash).expect("Mark should be present on chain."));

	assert!(stored_mark.revoked);
}

#[test]
fn revoke_with_delegation_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let mark_owner_keypair = get_bob_ed25519();
	let mark_owner = get_ed25519_account(mark_owner_keypair.public());
	let stream_hash = get_stream_hash(true);

	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(revoker.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, revoker.clone()),
	);
	delegation_node.permissions = pallet_delegation::Permissions::ANCHOR;
	// Mark owned by a different user, but delegation owned by the user
	// submitting the operation.
	let mut mark = generate_base_mark(mark_owner);
	mark.delegation_id = Some(delegation_id);

	let mut operation = generate_base_mark_revocation_details(stream_hash);
	// Set to 0 as we only need to check the delegation node itself and no parent.
	operation.max_parent_checks = 0u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![delegation_id])])
		.build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(operation.stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Mark::revoke(
			get_origin(revoker.clone()),
			operation.stream_hash,
			operation.max_parent_checks
		));
	});

	let stored_mark =
		ext.execute_with(|| Mark::marks(operation.stream_hash).expect("Mark should be present on chain."));

	assert!(stored_mark.revoked);
}

#[test]
fn revoke_with_parent_delegation_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let mark_owner_keypair = get_bob_ed25519();
	let mark_owner = get_ed25519_account(mark_owner_keypair.public());
	let stream_hash = get_stream_hash(true);

	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(revoker.clone()),
	);
	let (parent_id, mut parent_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, revoker.clone()),
	);
	parent_node.permissions = pallet_delegation::Permissions::ANCHOR;
	let (delegation_id, delegation_node) = (
		delegation_mock::get_delegation_id(false),
		delegation_mock::generate_base_delegation_node(root_id, mark_owner.clone()),
	);
	let mut mark = generate_base_mark(mark_owner);
	mark.delegation_id = Some(delegation_id);

	let mut operation = generate_base_mark_revocation_details(stream_hash);
	// Set to 1 as the delegation referenced in the mark is the child of the
	// node we want to use
	operation.max_parent_checks = 1u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(parent_id, parent_node), (delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![parent_id]), (parent_id, vec![delegation_id])])
		.build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(operation.stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Mark::revoke(
			get_origin(revoker.clone()),
			operation.stream_hash,
			operation.max_parent_checks
		));
	});

	let stored_mark = ext.execute_with(|| Mark::marks(stream_hash).expect("Mark should be present on chain."));

	assert!(stored_mark.revoked);
}

#[test]
fn revoke_parent_delegation_no_mark_permissions_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let mark_owner_keypair = get_bob_ed25519();
	let mark_owner = get_ed25519_account(mark_owner_keypair.public());
	let stream_hash = get_stream_hash(true);

	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(revoker.clone()),
	);
	let (parent_id, mut parent_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, revoker.clone()),
	);
	parent_node.permissions = pallet_delegation::Permissions::DELEGATE;
	let (delegation_id, delegation_node) = (
		delegation_mock::get_delegation_id(false),
		delegation_mock::generate_base_delegation_node(root_id, mark_owner.clone()),
	);
	let mut mark = generate_base_mark(mark_owner);
	mark.delegation_id = Some(delegation_id);

	let mut operation = generate_base_mark_revocation_details(stream_hash);
	// Set to 1 as the delegation referenced in the mark is the child of the
	// node we want to use
	operation.max_parent_checks = 1u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(parent_id, parent_node), (delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![parent_id]), (parent_id, vec![delegation_id])])
		.build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(operation.stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Mark::revoke(
			get_origin(revoker.clone()),
			operation.stream_hash,
			operation.max_parent_checks
		));
	});

	let stored_mark = ext.execute_with(|| Mark::marks(stream_hash).expect("Mark should be present on chain."));

	assert!(stored_mark.revoked);
}

#[test]
fn revoke_parent_delegation_with_direct_delegation_revoked_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let mark_owner_keypair = get_bob_ed25519();
	let mark_owner = get_ed25519_account(mark_owner_keypair.public());
	let stream_hash = get_stream_hash(true);

	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(revoker.clone()),
	);
	let (parent_id, mut parent_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, revoker.clone()),
	);
	parent_node.permissions = pallet_delegation::Permissions::ANCHOR;
	let (delegation_id, mut delegation_node) = (
		delegation_mock::get_delegation_id(false),
		delegation_mock::generate_base_delegation_node(root_id, mark_owner.clone()),
	);
	delegation_node.revoked = true;
	let mut mark = generate_base_mark(mark_owner);
	mark.delegation_id = Some(delegation_id);

	let mut operation = generate_base_mark_revocation_details(stream_hash);
	// Set to 1 as the delegation referenced in the mark is the child of the
	// node we want to use
	operation.max_parent_checks = 1u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(parent_id, parent_node), (delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![parent_id]), (parent_id, vec![delegation_id])])
		.build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(operation.stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Mark::revoke(
			get_origin(revoker.clone()),
			operation.stream_hash,
			operation.max_parent_checks
		));
	});

	let stored_mark = ext.execute_with(|| Mark::marks(stream_hash).expect("Mark should be present on chain."));

	assert!(stored_mark.revoked);
}

#[test]
fn mark_not_present_revoke_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let stream_hash = get_stream_hash(true);

	let mark = generate_base_mark(revoker.clone());

	let operation = generate_base_mark_revocation_details(stream_hash);

	let mut ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Mark::revoke(
				get_origin(revoker.clone()),
				operation.stream_hash,
				operation.max_parent_checks
			),
			mark::Error::<Test>::MarkNotFound
		);
	});
}

#[test]
fn already_revoked_revoke_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let stream_hash = get_stream_hash(true);

	// Mark already revoked
	let mut mark = generate_base_mark(revoker.clone());
	mark.revoked = true;

	let operation = generate_base_mark_revocation_details(stream_hash);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default().build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(operation.stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::revoke(
				get_origin(revoker.clone()),
				operation.stream_hash,
				operation.max_parent_checks
			),
			mark::Error::<Test>::AlreadyRevoked
		);
	});
}

#[test]
fn unauthorised_mark_revoke_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let mark_owner_keypair = get_bob_ed25519();
	let mark_owner = get_ed25519_account(mark_owner_keypair.public());
	let stream_hash = get_stream_hash(true);

	// Mark owned by a different user
	let mark = generate_base_mark(mark_owner);

	let operation = generate_base_mark_revocation_details(stream_hash);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default().build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(operation.stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::revoke(
				get_origin(revoker.clone()),
				operation.stream_hash,
				operation.max_parent_checks
			),
			mark::Error::<Test>::UnauthorizedRevocation
		);
	});
}

#[test]
fn max_parent_lookups_revoke_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let mark_owner_keypair = get_bob_ed25519();
	let mark_owner = get_ed25519_account(mark_owner_keypair.public());
	let stream_hash = get_stream_hash(true);

	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(revoker.clone()),
	);
	let (parent_delegation_id, parent_delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, revoker.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, mark_owner.clone()),
	);
	delegation_node.permissions = pallet_delegation::Permissions::ANCHOR;
	delegation_node.parent = Some(parent_delegation_id);
	let mut mark = generate_base_mark(mark_owner);
	mark.delegation_id = Some(delegation_id);

	let mut operation = generate_base_mark_revocation_details(stream_hash);
	operation.max_parent_checks = 0u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(parent_delegation_id, parent_delegation_node),
			(delegation_id, delegation_node),
		])
		.with_children(vec![
			(root_id, vec![parent_delegation_id]),
			(parent_delegation_id, vec![delegation_id]),
		])
		.build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(operation.stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::revoke(
				get_origin(revoker.clone()),
				operation.stream_hash,
				operation.max_parent_checks
			),
			pallet_delegation::Error::<Test>::MaxSearchDepthReached
		);
	});
}

#[test]
fn revoked_delegation_revoke_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let mark_owner_keypair = get_bob_ed25519();
	let mark_owner = get_ed25519_account(mark_owner_keypair.public());
	let stream_hash = get_stream_hash(true);

	let (root_id, root_node) = (
		delegation_mock::get_delegation_root_id(true),
		delegation_mock::generate_base_delegation_root(revoker.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		delegation_mock::get_delegation_id(true),
		delegation_mock::generate_base_delegation_node(root_id, revoker.clone()),
	);
	delegation_node.permissions = pallet_delegation::Permissions::ANCHOR;
	delegation_node.revoked = true;
	let mut mark = generate_base_mark(mark_owner);
	mark.delegation_id = Some(delegation_id);

	let operation = generate_base_mark_revocation_details(stream_hash);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(mark.mtype_hash, revoker.clone())])
		.build(None);
	let ext = delegation_mock::ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![delegation_id])])
		.build(Some(ext));
	let mut ext = ExtBuilder::default()
		.with_marks(vec![(operation.stream_hash, mark)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Mark::revoke(
				get_origin(revoker.clone()),
				operation.stream_hash,
				operation.max_parent_checks
			),
			mark::Error::<Test>::UnauthorizedRevocation
		);
	});
}
