// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! testing Delegation

use codec::Encode;
use frame_support::{assert_err, assert_noop, assert_ok};
use sp_core::Pair;

use crate::{self as delegation, mock::*};
use pallet_mtype::mock as mtype_mock;

// submit_delegation_root_creation_operation()

#[test]
fn create_root_delegation_successful() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);

	let operation = generate_base_delegation_root_creation_details(root_id, root_node.clone());

	let mut ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);

	ext.execute_with(|| {
		assert_ok!(Delegation::create_root(
			get_origin(creator.clone()),
			operation.root_id,
			operation.mtype_hash
		));
	});

	let stored_delegation_root = ext
		.execute_with(|| Delegation::roots(&operation.root_id).expect("Delegation root should be present on chain."));

	assert_eq!(stored_delegation_root.owner, creator);
	assert_eq!(stored_delegation_root.mtype_hash, operation.mtype_hash);
	assert!(!stored_delegation_root.revoked);
}

#[test]
fn duplicate_create_root_delegation_error() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);

	let operation = generate_base_delegation_root_creation_details(root_id, root_node.clone());

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_err!(
			Delegation::create_root(get_origin(creator.clone()), operation.root_id, operation.mtype_hash),
			delegation::Error::<Test>::RootAlreadyExists
		);
	});
}

#[test]
fn mtype_not_found_create_root_delegation_error() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);

	let operation = generate_base_delegation_root_creation_details(root_id, root_node);

	// No MTYPE stored,
	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_err!(
			Delegation::create_root(get_origin(creator.clone()), operation.root_id, operation.mtype_hash),
			pallet_mtype::Error::<Test>::MTypeNotFound
		);
	});
}

// submit_delegation_creation_operation()

#[test]
fn create_delegation_no_parent_successful() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());
	let delegate_keypair = get_bob_sr25519();
	let delegate = get_sr25519_account(delegate_keypair.public());
	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);
	let (delegation_id, delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, delegate),
	);

	let delegation_info = Delegation::calculate_hash(
		&delegation_id,
		&delegation_node.root_id,
		&delegation_node.parent,
		&delegation_node.permissions,
	);

	let delegate_signature = delegate_keypair.sign(&hash_to_u8(delegation_info));

	let operation =
		generate_base_delegation_creation_details(delegation_id, delegate_signature.into(), delegation_node);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Delegation::add_delegation(
			get_origin(creator.clone()),
			operation.delegation_id,
			operation.root_id,
			operation.parent_id,
			operation.delegate.clone(),
			operation.permissions,
			operation.delegate_signature.clone().encode(),
		));
	});

	let stored_delegation = ext.execute_with(|| {
		Delegation::delegations(&operation.delegation_id).expect("Delegation should be present on chain.")
	});

	assert_eq!(stored_delegation.root_id, operation.root_id);
	assert_eq!(stored_delegation.parent, operation.parent_id);
	assert_eq!(stored_delegation.owner, operation.delegate);
	assert_eq!(stored_delegation.permissions, operation.permissions);
	assert!(!stored_delegation.revoked);

	// Verify that the root has the new delegation among its children
	let stored_root_children = ext.execute_with(|| {
		Delegation::children(&operation.root_id).expect("Delegation root children should be present on chain.")
	});

	assert_eq!(stored_root_children, vec![operation.delegation_id]);
}

#[test]
fn create_delegation_with_parent_successful() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());
	let delegate_keypair = get_bob_sr25519();
	let delegate = get_sr25519_account(delegate_keypair.public());
	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);
	let (parent_delegation_id, parent_delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, creator.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate.clone()),
	);
	delegation_node.parent = Some(parent_delegation_id);

	let delegate_signature = delegate_keypair.sign(&hash_to_u8(Delegation::calculate_hash(
		&delegation_id,
		&delegation_node.root_id,
		&delegation_node.parent,
		&delegation_node.permissions,
	)));

	let operation =
		generate_base_delegation_creation_details(delegation_id, delegate_signature.into(), delegation_node);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(parent_delegation_id, parent_delegation_node)])
		.with_children(vec![(root_id, vec![(parent_delegation_id)])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Delegation::add_delegation(
			get_origin(creator.clone()),
			operation.delegation_id,
			operation.root_id,
			operation.parent_id,
			delegate.clone(),
			operation.permissions,
			operation.delegate_signature.clone().encode(),
		));
	});

	let stored_delegation = ext.execute_with(|| {
		Delegation::delegations(&operation.delegation_id).expect("Delegation should be present on chain.")
	});

	assert_eq!(stored_delegation.root_id, operation.root_id);
	assert_eq!(stored_delegation.parent, operation.parent_id);
	assert_eq!(stored_delegation.owner, operation.delegate);
	assert_eq!(stored_delegation.permissions, operation.permissions);
	assert!(!stored_delegation.revoked);

	// Verify that the parent has the new delegation among its children
	let stored_parent_children = ext.execute_with(|| {
		Delegation::children(&operation.parent_id.unwrap())
			.expect("Delegation parent children should be present on chain.")
	});

	assert_eq!(stored_parent_children, vec![delegation_id]);
}

#[test]
fn invalid_delegate_signature_create_delegation_error() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());
	let alternative_keypair = get_alice_sr25519();
	let delegate_keypair = get_bob_sr25519();
	let delegate = get_sr25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);
	let (delegation_id, delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, delegate.clone()),
	);

	let delegate_signature = alternative_keypair.sign(&hash_to_u8(Delegation::calculate_hash(
		&delegation_id,
		&delegation_node.root_id,
		&delegation_node.parent,
		&delegation_node.permissions,
	)));

	let operation =
		generate_base_delegation_creation_details(delegation_id, delegate_signature.into(), delegation_node);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_err!(
			Delegation::add_delegation(
				get_origin(creator.clone()),
				operation.delegation_id,
				operation.root_id,
				operation.parent_id,
				delegate.clone(),
				operation.permissions,
				operation.delegate_signature.clone().encode(),
			),
			delegation::Error::<Test>::InvalidDelegateSignature
		);
	});
}

#[test]
fn duplicate_delegation_create_delegation_error() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());
	let delegate_keypair = get_bob_sr25519();
	let delegate = get_sr25519_account(delegate_keypair.public());
	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);
	let (delegation_id, delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, delegate.clone()),
	);

	let delegate_signature = delegate_keypair.sign(&hash_to_u8(Delegation::calculate_hash(
		&delegation_id,
		&delegation_node.root_id,
		&delegation_node.parent,
		&delegation_node.permissions,
	)));

	let operation =
		generate_base_delegation_creation_details(delegation_id, delegate_signature.into(), delegation_node.clone());

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_err!(
			Delegation::add_delegation(
				get_origin(creator.clone()),
				operation.delegation_id,
				operation.root_id,
				operation.parent_id,
				delegate.clone(),
				operation.permissions,
				operation.delegate_signature.clone().encode(),
			),
			delegation::Error::<Test>::DelegationAlreadyExists
		);
	});
}

#[test]
fn root_not_existing_create_delegation_error() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());
	let delegate_keypair = get_bob_sr25519();
	let delegate = get_sr25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);
	let (delegation_id, delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, delegate.clone()),
	);

	let delegate_signature = delegate_keypair.sign(&hash_to_u8(Delegation::calculate_hash(
		&delegation_id,
		&delegation_node.root_id,
		&delegation_node.parent,
		&delegation_node.permissions,
	)));

	let operation =
		generate_base_delegation_creation_details(delegation_id, delegate_signature.into(), delegation_node);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	// No delegations added to the pallet storage
	let mut ext = ExtBuilder::default().build(Some(ext));

	ext.execute_with(|| {
		assert_err!(
			Delegation::add_delegation(
				get_origin(creator.clone()),
				operation.delegation_id,
				operation.root_id,
				operation.parent_id,
				delegate.clone(),
				operation.permissions,
				operation.delegate_signature.clone().encode(),
			),
			delegation::Error::<Test>::RootNotFound
		);
	});
}

#[test]
fn parent_not_existing_create_delegation_error() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());
	let delegate_keypair = get_bob_sr25519();
	let delegate = get_sr25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);
	let alternative_parent_id = get_delegation_id(false);
	let (delegation_id, mut delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, delegate.clone()),
	);
	delegation_node.parent = Some(alternative_parent_id);

	let delegate_signature = delegate_keypair.sign(&hash_to_u8(Delegation::calculate_hash(
		&delegation_id,
		&delegation_node.root_id,
		&delegation_node.parent,
		&delegation_node.permissions,
	)));

	let operation =
		generate_base_delegation_creation_details(delegation_id, delegate_signature.into(), delegation_node);
	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_err!(
			Delegation::add_delegation(
				get_origin(creator.clone()),
				operation.delegation_id,
				operation.root_id,
				operation.parent_id,
				delegate.clone(),
				operation.permissions,
				operation.delegate_signature.clone().encode(),
			),
			delegation::Error::<Test>::ParentDelegationNotFound
		);
	});
}

#[test]
fn not_owner_of_parent_create_delegation_error() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());
	let alternative_owner_keypair = get_charlie_ed25519();
	let alternative_owner = get_ed25519_account(alternative_owner_keypair.public());
	let delegate_keypair = get_bob_sr25519();
	let delegate = get_sr25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);
	let (parent_delegation_id, parent_delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, alternative_owner),
	);
	let (delegation_id, mut delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate.clone()),
	);
	delegation_node.parent = Some(parent_delegation_id);

	let delegate_signature = delegate_keypair.sign(&hash_to_u8(Delegation::calculate_hash(
		&delegation_id,
		&delegation_node.root_id,
		&delegation_node.parent,
		&delegation_node.permissions,
	)));

	let operation =
		generate_base_delegation_creation_details(delegation_id, delegate_signature.into(), delegation_node);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(parent_delegation_id, parent_delegation_node)])
		.with_children(vec![(root_id, vec![parent_delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_err!(
			Delegation::add_delegation(
				get_origin(creator.clone()),
				operation.delegation_id,
				operation.root_id,
				operation.parent_id,
				delegate.clone(),
				operation.permissions,
				operation.delegate_signature.clone().encode(),
			),
			delegation::Error::<Test>::NotOwnerOfParentDelegation
		);
	});
}

#[test]
fn unauthorised_delegation_create_delegation_error() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());
	let delegate_keypair = get_bob_sr25519();
	let delegate = get_sr25519_account(delegate_keypair.public());
	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(creator.clone()),
	);
	let (parent_delegation_id, mut parent_delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, creator.clone()),
	);
	parent_delegation_node.permissions = delegation::Permissions::ANCHOR;
	let (delegation_id, mut delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate.clone()),
	);
	delegation_node.parent = Some(parent_delegation_id);

	let delegate_signature = delegate_keypair.sign(&hash_to_u8(Delegation::calculate_hash(
		&delegation_id,
		&delegation_node.root_id,
		&delegation_node.parent,
		&delegation_node.permissions,
	)));

	let operation =
		generate_base_delegation_creation_details(delegation_id, delegate_signature.into(), delegation_node);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(parent_delegation_id, parent_delegation_node)])
		.with_children(vec![(root_id, vec![parent_delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_err!(
			Delegation::add_delegation(
				get_origin(creator.clone()),
				operation.delegation_id,
				operation.root_id,
				operation.parent_id,
				delegate.clone(),
				operation.permissions,
				operation.delegate_signature.clone().encode(),
			),
			delegation::Error::<Test>::UnauthorizedDelegation
		);
	});
}

#[test]
fn not_owner_of_root_create_delegation_error() {
	let creator_keypair = get_alice_ed25519();
	let creator = get_ed25519_account(creator_keypair.public());
	let alternative_owner_keypair = get_charlie_ed25519();
	let alternative_owner = get_ed25519_account(alternative_owner_keypair.public());
	let delegate_keypair = get_bob_sr25519();
	let delegate = get_sr25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(alternative_owner),
	);
	let (delegation_id, delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, delegate.clone()),
	);

	let delegate_signature = delegate_keypair.sign(&hash_to_u8(Delegation::calculate_hash(
		&delegation_id,
		&delegation_node.root_id,
		&delegation_node.parent,
		&delegation_node.permissions,
	)));

	let operation =
		generate_base_delegation_creation_details(delegation_id, delegate_signature.into(), delegation_node);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, creator.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Delegation::add_delegation(
				get_origin(creator.clone()),
				operation.delegation_id,
				operation.root_id,
				operation.parent_id,
				delegate.clone(),
				operation.permissions,
				operation.delegate_signature.clone().encode(),
			),
			delegation::Error::<Test>::NotOwnerOfRootDelegation
		);
	});
}

// submit_delegation_root_revocation_operation()
#[test]
fn empty_revoke_root_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);

	let mut operation = generate_base_delegation_root_revocation_details(root_id);
	operation.max_children = 2u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Delegation::revoke_root(
			get_origin(revoker.clone()),
			operation.root_id,
			operation.max_children
		));
	});

	let stored_delegation_root = ext
		.execute_with(|| Delegation::roots(&operation.root_id).expect("Delegation root should be present on chain."));
	assert!(stored_delegation_root.revoked);
}

#[test]
fn list_hierarchy_revoke_root_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let delegate_keypair = get_bob_ed25519();
	let delegate = get_ed25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);
	let (parent_delegation_id, parent_delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, revoker.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate),
	);
	delegation_node.parent = Some(parent_delegation_id);

	let mut operation = generate_base_delegation_root_revocation_details(root_id);
	operation.max_children = 2u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(parent_delegation_id, parent_delegation_node),
			(delegation_id, delegation_node),
		])
		.with_children(vec![
			// Root -> Parent -> Delegation
			(root_id, vec![parent_delegation_id]),
			(parent_delegation_id, vec![delegation_id]),
		])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Delegation::revoke_root(
			get_origin(revoker.clone()),
			operation.root_id,
			operation.max_children
		));
	});

	let stored_delegation_root = ext
		.execute_with(|| Delegation::roots(&operation.root_id).expect("Delegation root should be present on chain."));
	assert!(stored_delegation_root.revoked);

	let stored_parent_delegation = ext.execute_with(|| {
		Delegation::delegations(&parent_delegation_id).expect("Parent delegation should be present on chain.")
	});
	assert!(stored_parent_delegation.revoked);

	let stored_delegation =
		ext.execute_with(|| Delegation::delegations(&delegation_id).expect("Delegation should be present on chain."));
	assert!(stored_delegation.revoked);
}

#[test]
fn tree_hierarchy_revoke_root_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let delegate_keypair = get_bob_ed25519();
	let delegate = get_ed25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);
	let (delegation_id_1, delegation_node_1) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, revoker.clone()),
	);
	let (delegation_id_2, delegation_node_2) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate),
	);

	let mut operation = generate_base_delegation_root_revocation_details(root_id);
	operation.max_children = 2u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			// Root -> Delegation 1 && Delegation 2
			(root_id, vec![delegation_id_1, delegation_id_2]),
		])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Delegation::revoke_root(
			get_origin(revoker.clone()),
			operation.root_id,
			operation.max_children
		));
	});

	let stored_delegation_root = ext
		.execute_with(|| Delegation::roots(&operation.root_id).expect("Delegation root should be present on chain."));
	assert!(stored_delegation_root.revoked);

	let stored_delegation_1 = ext
		.execute_with(|| Delegation::delegations(&delegation_id_1).expect("Delegation 1 should be present on chain."));
	assert!(stored_delegation_1.revoked);

	let stored_delegation_2 = ext
		.execute_with(|| Delegation::delegations(&delegation_id_2).expect("Delegation 2 should be present on chain."));
	assert!(stored_delegation_2.revoked);
}

#[test]
fn greater_max_revocations_revoke_root_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let delegate_keypair = get_alice_ed25519();
	let delegate = get_ed25519_account(delegate_keypair.public());
	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);
	let (delegation_id, delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate),
	);

	let mut operation = generate_base_delegation_root_revocation_details(root_id);
	operation.max_children = MaxRevocations::get();

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![
			// Root -> Delegation
			(root_id, vec![delegation_id]),
		])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_ok!(Delegation::revoke_root(
			get_origin(revoker.clone()),
			operation.root_id,
			operation.max_children
		));
	});

	let stored_delegation_root = ext
		.execute_with(|| Delegation::roots(&operation.root_id).expect("Delegation root should be present on chain."));
	assert!(stored_delegation_root.revoked);

	let stored_delegation =
		ext.execute_with(|| Delegation::delegations(&delegation_id).expect("Delegation should be present on chain."));
	assert!(stored_delegation.revoked);
}

#[test]
fn root_not_found_revoke_root_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());

	let root_id = get_delegation_root_id(true);

	let operation = generate_base_delegation_root_revocation_details(root_id);

	let mut ext = ExtBuilder::default().build(None);

	ext.execute_with(|| {
		assert_noop!(
			Delegation::revoke_root(get_origin(revoker.clone()), operation.root_id, operation.max_children),
			delegation::Error::<Test>::RootNotFound
		);
	});
}

#[test]
fn different_root_creator_revoke_root_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let alternative_revoker_keypair = get_charlie_ed25519();
	let alternative_revoker = get_ed25519_account(alternative_revoker_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(alternative_revoker),
	);

	let operation = generate_base_delegation_root_revocation_details(root_id);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Delegation::revoke_root(get_origin(revoker.clone()), operation.root_id, operation.max_children),
			delegation::Error::<Test>::UnauthorizedRevocation
		);
	});
}

#[test]
fn too_small_max_revocations_revoke_root_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let delegate_keypair = get_alice_ed25519();
	let delegate = get_ed25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);
	let (delegation_id, delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate),
	);

	let mut operation = generate_base_delegation_root_revocation_details(root_id);
	operation.max_children = 0u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![
			// Root -> Delegation
			(root_id, vec![delegation_id]),
		])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Delegation::revoke_root(get_origin(revoker.clone()), operation.root_id, operation.max_children),
			delegation::Error::<Test>::ExceededRevocationBounds
		);
	});
}

#[test]
fn exact_children_max_revocations_revoke_root_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let delegate_keypair = get_alice_ed25519();
	let delegate = get_ed25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);
	let (delegation_id_1, delegation_node_1) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, delegate.clone()),
	);
	let (delegation_id_2, mut delegation_node_2) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate.clone()),
	);
	delegation_node_2.parent = Some(delegation_id_1);
	let (delegation_id_3, mut delegation_node_3) = (
		get_delegation_root_id(false),
		generate_base_delegation_node(root_id, delegate),
	);
	delegation_node_3.parent = Some(delegation_id_1);

	let mut operation = generate_base_delegation_root_revocation_details(root_id);
	operation.max_children = 2u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
			(delegation_id_3, delegation_node_3),
		])
		.with_children(vec![
			// Root -> Delegation 1 -> Delegation 2 && Delegation 3
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2, delegation_id_3]),
		])
		.build(Some(ext));

	ext.execute_with(|| {
		// assert_err and not asser_noop becase the storage is indeed changed, even tho
		// partially
		assert_err!(
			Delegation::revoke_root(get_origin(revoker.clone()), operation.root_id, operation.max_children),
			delegation::Error::<Test>::ExceededRevocationBounds
		);
	});

	let stored_delegation_root = ext
		.execute_with(|| Delegation::roots(&operation.root_id).expect("Delegation root should be present on chain."));
	assert!(!stored_delegation_root.revoked);

	let stored_delegation_1 = ext
		.execute_with(|| Delegation::delegations(&delegation_id_1).expect("Delegation 1 should be present on chain."));
	assert!(!stored_delegation_1.revoked);

	// Only this leaf should have been revoked as it is the first child of
	// delegation_1
	let stored_delegation_2 = ext
		.execute_with(|| Delegation::delegations(&delegation_id_2).expect("Delegation 2 should be present on chain."));
	assert!(stored_delegation_2.revoked);

	let stored_delegation_3 = ext
		.execute_with(|| Delegation::delegations(&delegation_id_3).expect("Delegation 3 should be present on chain."));
	assert!(!stored_delegation_3.revoked);
}

// submit_delegation_revocation_operation()

#[test]
fn direct_owner_revoke_delegation_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let delegate_keypair = get_alice_ed25519();
	let delegate = get_ed25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);
	let (parent_delegation_id, parent_delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, revoker.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate),
	);
	delegation_node.parent = Some(parent_delegation_id);

	let mut operation = generate_base_delegation_revocation_details(parent_delegation_id);
	operation.max_revocations = 2u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	// Root -> Parent -> Child
	let mut ext = ExtBuilder::default()
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

	ext.execute_with(|| {
		assert_ok!(Delegation::revoke_delegation(
			get_origin(revoker.clone()),
			operation.delegation_id,
			operation.max_parent_checks,
			operation.max_revocations
		));
	});

	let stored_parent_delegation = ext.execute_with(|| {
		Delegation::delegations(&parent_delegation_id).expect("Parent delegation should be present on chain.")
	});
	assert!(stored_parent_delegation.revoked);

	let stored_child_delegation = ext.execute_with(|| {
		Delegation::delegations(&delegation_id).expect("Child delegation should be present on chain.")
	});
	assert!(stored_child_delegation.revoked);
}

#[test]
fn parent_owner_revoke_delegation_successful() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let delegate_keypair = get_alice_ed25519();
	let delegate = get_ed25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);
	let (parent_delegation_id, parent_delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, revoker.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate),
	);
	delegation_node.parent = Some(parent_delegation_id);

	let mut operation = generate_base_delegation_revocation_details(delegation_id);
	operation.max_parent_checks = 1u32;
	operation.max_revocations = 1u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
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

	ext.execute_with(|| {
		assert_ok!(Delegation::revoke_delegation(
			get_origin(revoker.clone()),
			operation.delegation_id,
			operation.max_parent_checks,
			operation.max_revocations
		));
	});

	let stored_parent_delegation = ext.execute_with(|| {
		Delegation::delegations(&parent_delegation_id).expect("Parent delegation should be present on chain.")
	});
	assert!(!stored_parent_delegation.revoked);

	let stored_child_delegation = ext.execute_with(|| {
		Delegation::delegations(&delegation_id).expect("Child delegation should be present on chain.")
	});
	assert!(stored_child_delegation.revoked);
}

#[test]
fn delegation_not_found_revoke_delegation_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);
	let delegation_id = get_delegation_id(true);

	let operation = generate_base_delegation_revocation_details(delegation_id);

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Delegation::revoke_delegation(
				get_origin(revoker.clone()),
				operation.delegation_id,
				operation.max_parent_checks,
				operation.max_revocations
			),
			delegation::Error::<Test>::DelegationNotFound
		);
	});
}

#[test]
fn not_delegating_revoke_delegation_error() {
	let owner_keypair = get_alice_ed25519();
	let owner = get_ed25519_account(owner_keypair.public());
	let revoker_keypair = get_bob_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(owner.clone()),
	);
	let (delegation_id, delegation_node) = (get_delegation_id(false), generate_base_delegation_node(root_id, owner));

	let mut operation = generate_base_delegation_revocation_details(delegation_id);
	operation.max_parent_checks = MaxParentChecks::get();

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![(delegation_id, delegation_node)])
		.with_children(vec![(root_id, vec![delegation_id])])
		.build(Some(ext));

	ext.execute_with(|| {
		assert_noop!(
			Delegation::revoke_delegation(
				get_origin(revoker.clone()),
				operation.delegation_id,
				operation.max_parent_checks,
				operation.max_revocations
			),
			delegation::Error::<Test>::UnauthorizedRevocation
		);
	});
}

#[test]
fn parent_too_far_revoke_delegation_error() {
	let owner_keypair = get_alice_ed25519();
	let owner = get_ed25519_account(owner_keypair.public());
	let intermediate_keypair = get_charlie_ed25519();
	let intermediate = get_ed25519_account(intermediate_keypair.public());
	let delegate_keypair = get_bob_ed25519();
	let delegate = get_ed25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(owner.clone()),
	);
	let (parent_delegation_id, parent_delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, intermediate.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate),
	);
	delegation_node.parent = Some(parent_delegation_id);

	let mut operation = generate_base_delegation_revocation_details(delegation_id);
	operation.max_parent_checks = 0u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, owner)])
		.build(None);
	let mut ext = ExtBuilder::default()
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

	ext.execute_with(|| {
		assert_noop!(
			Delegation::revoke_delegation(
				get_origin(intermediate.clone()),
				operation.delegation_id,
				operation.max_parent_checks,
				operation.max_revocations
			),
			delegation::Error::<Test>::MaxSearchDepthReached
		);
	});
}

#[test]
fn too_many_revocations_revoke_delegation_error() {
	let revoker_keypair = get_alice_ed25519();
	let revoker = get_ed25519_account(revoker_keypair.public());
	let delegate_keypair = get_bob_ed25519();
	let delegate = get_ed25519_account(delegate_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(revoker.clone()),
	);
	let (parent_delegation_id, parent_delegation_node) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, revoker.clone()),
	);
	let (delegation_id, mut delegation_node) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, delegate),
	);
	delegation_node.parent = Some(parent_delegation_id);

	let mut operation = generate_base_delegation_revocation_details(delegation_id);
	operation.max_parent_checks = 1u32;

	let ext = mtype_mock::ExtBuilder::default()
		.with_mtypes(vec![(root_node.mtype_hash, revoker.clone())])
		.build(None);
	let mut ext = ExtBuilder::default()
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

	ext.execute_with(|| {
		assert_noop!(
			Delegation::revoke_delegation(
				get_origin(revoker.clone()),
				operation.delegation_id,
				operation.max_parent_checks,
				operation.max_revocations
			),
			delegation::Error::<Test>::ExceededRevocationBounds
		);
	});
}

// Internal function: is_delegating()

#[test]
fn is_delegating_direct_not_revoked() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());
	let user_2_keypair = get_bob_ed25519();
	let user_2 = get_ed25519_account(user_2_keypair.public());
	let user_3_keypair = get_charlie_ed25519();
	let user_3 = get_ed25519_account(user_3_keypair.public());

	let (root_id, root_node) = (get_delegation_root_id(true), generate_base_delegation_root(user_1));
	let (delegation_id_1, delegation_node_1) =
		(get_delegation_id(true), generate_base_delegation_node(root_id, user_2));
	let (delegation_id_2, mut delegation_node_2) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, user_3.clone()),
	);
	delegation_node_2.parent = Some(delegation_id_1);

	let max_parent_checks = 0u32;

	// Root -> Delegation 1 -> Delegation 2
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2]),
		])
		.build(None);

	let is_delegating = ext.execute_with(|| Delegation::is_delegating(&user_3, &delegation_id_2, max_parent_checks));
	assert_eq!(is_delegating, Ok(true));
}

#[test]
fn is_delegating_direct_not_revoked_max_parent_checks_value() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());
	let user_2_keypair = get_bob_ed25519();
	let user_2 = get_ed25519_account(user_2_keypair.public());
	let user_3_keypair = get_charlie_ed25519();
	let user_3 = get_ed25519_account(user_3_keypair.public());

	let (root_id, root_node) = (get_delegation_root_id(true), generate_base_delegation_root(user_1));
	let (delegation_id_1, delegation_node_1) =
		(get_delegation_id(true), generate_base_delegation_node(root_id, user_2));
	let (delegation_id_2, mut delegation_node_2) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, user_3.clone()),
	);
	delegation_node_2.parent = Some(delegation_id_1);

	let max_parent_checks = u32::MAX;

	// Root -> Delegation 1 -> Delegation 2
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2]),
		])
		.build(None);

	let is_delegating = ext.execute_with(|| Delegation::is_delegating(&user_3, &delegation_id_2, max_parent_checks));
	assert_eq!(is_delegating, Ok(true));
}

#[test]
fn is_delegating_direct_revoked() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());
	let user_2_keypair = get_bob_ed25519();
	let user_2 = get_ed25519_account(user_2_keypair.public());
	let user_3_keypair = get_charlie_ed25519();
	let user_3 = get_ed25519_account(user_3_keypair.public());

	let (root_id, root_node) = (get_delegation_root_id(true), generate_base_delegation_root(user_1));
	let (delegation_id_1, delegation_node_1) =
		(get_delegation_id(true), generate_base_delegation_node(root_id, user_2));
	let (delegation_id_2, mut delegation_node_2) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, user_3.clone()),
	);
	delegation_node_2.parent = Some(delegation_id_1);
	delegation_node_2.revoked = true;

	let max_parent_checks = 0u32;

	// Root -> Delegation 1 -> Delegation 2
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2]),
		])
		.build(None);

	let is_delegating = ext.execute_with(|| Delegation::is_delegating(&user_3, &delegation_id_2, max_parent_checks));
	assert_eq!(is_delegating, Ok(false));
}

#[test]
fn is_delegating_direct_revoked_max_parent_checks_value() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());
	let user_2_keypair = get_bob_ed25519();
	let user_2 = get_ed25519_account(user_2_keypair.public());
	let user_3_keypair = get_charlie_ed25519();
	let user_3 = get_ed25519_account(user_3_keypair.public());

	let (root_id, root_node) = (get_delegation_root_id(true), generate_base_delegation_root(user_1));
	let (delegation_id_1, delegation_node_1) =
		(get_delegation_id(true), generate_base_delegation_node(root_id, user_2));
	let (delegation_id_2, mut delegation_node_2) = (
		get_delegation_id(false),
		generate_base_delegation_node(root_id, user_3.clone()),
	);
	delegation_node_2.parent = Some(delegation_id_1);
	delegation_node_2.revoked = true;

	let max_parent_checks = u32::MAX;

	// Root -> Delegation 1 -> Delegation 2
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2]),
		])
		.build(None);

	let is_delegating = ext.execute_with(|| Delegation::is_delegating(&user_3, &delegation_id_2, max_parent_checks));
	assert_eq!(is_delegating, Ok(false));
}

#[test]
fn is_delegating_max_parent_not_revoked() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());
	let user_2_keypair = get_bob_ed25519();
	let user_2 = get_ed25519_account(user_2_keypair.public());
	let user_3_keypair = get_charlie_ed25519();
	let user_3 = get_ed25519_account(user_3_keypair.public());

	let (root_id, root_node) = (get_delegation_root_id(true), generate_base_delegation_root(user_1));
	let (delegation_id_1, delegation_node_1) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, user_2.clone()),
	);
	let (delegation_id_2, mut delegation_node_2) =
		(get_delegation_id(false), generate_base_delegation_node(root_id, user_3));
	delegation_node_2.parent = Some(delegation_id_1);

	let max_parent_checks = 1u32;

	// Root -> Delegation 1 -> Delegation 2
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2]),
		])
		.build(None);

	let is_delegating = ext.execute_with(|| Delegation::is_delegating(&user_2, &delegation_id_2, max_parent_checks));
	assert_eq!(is_delegating, Ok(true));
}

#[test]
fn is_delegating_max_parent_revoked() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());
	let user_2_keypair = get_bob_ed25519();
	let user_2 = get_ed25519_account(user_2_keypair.public());
	let user_3_keypair = get_charlie_ed25519();
	let user_3 = get_ed25519_account(user_3_keypair.public());

	let (root_id, root_node) = (get_delegation_root_id(true), generate_base_delegation_root(user_1));
	let (delegation_id_1, mut delegation_node_1) = (
		get_delegation_id(true),
		generate_base_delegation_node(root_id, user_2.clone()),
	);
	delegation_node_1.revoked = true;
	let (delegation_id_2, mut delegation_node_2) =
		(get_delegation_id(false), generate_base_delegation_node(root_id, user_3));
	delegation_node_2.parent = Some(delegation_id_1);

	let max_parent_checks = 2u32;

	// Root -> Delegation 1 -> Delegation 2
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2]),
		])
		.build(None);

	let is_delegating = ext.execute_with(|| Delegation::is_delegating(&user_2, &delegation_id_2, max_parent_checks));
	assert_eq!(is_delegating, Ok(false));
}

#[test]
fn is_delegating_root_owner_not_revoked() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());
	let user_2_keypair = get_bob_ed25519();
	let user_2 = get_ed25519_account(user_2_keypair.public());
	let user_3_keypair = get_charlie_ed25519();
	let user_3 = get_ed25519_account(user_3_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(user_1.clone()),
	);
	let (delegation_id_1, delegation_node_1) =
		(get_delegation_id(true), generate_base_delegation_node(root_id, user_2));
	let (delegation_id_2, mut delegation_node_2) =
		(get_delegation_id(false), generate_base_delegation_node(root_id, user_3));
	delegation_node_2.parent = Some(delegation_id_1);

	let max_parent_checks = 2u32;

	// Root -> Delegation 1 -> Delegation 2
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2]),
		])
		.build(None);

	let is_delegating = ext.execute_with(|| Delegation::is_delegating(&user_1, &delegation_id_2, max_parent_checks));
	assert_eq!(is_delegating, Ok(true));
}

#[test]
fn is_delegating_root_owner_revoked() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());
	let user_2_keypair = get_bob_ed25519();
	let user_2 = get_ed25519_account(user_2_keypair.public());
	let user_3_keypair = get_charlie_ed25519();
	let user_3 = get_ed25519_account(user_3_keypair.public());

	let (root_id, mut root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(user_1.clone()),
	);
	root_node.revoked = true;
	let (delegation_id_1, mut delegation_node_1) =
		(get_delegation_id(true), generate_base_delegation_node(root_id, user_2));
	delegation_node_1.revoked = true;
	let (delegation_id_2, mut delegation_node_2) =
		(get_delegation_id(false), generate_base_delegation_node(root_id, user_3));
	delegation_node_2.parent = Some(delegation_id_1);

	let max_parent_checks = u32::MAX;

	// Root -> Delegation 1 -> Delegation 2
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2]),
		])
		.build(None);

	let is_delegating = ext.execute_with(|| Delegation::is_delegating(&user_1, &delegation_id_2, max_parent_checks));
	assert_eq!(is_delegating, Ok(false));
}

#[test]
fn is_delegating_delegation_not_found() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());

	let (root_id, root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(user_1.clone()),
	);
	let delegation_id = get_delegation_id(true);

	let max_parent_checks = 2u32;

	// Root -> Delegation 1
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Delegation::is_delegating(&user_1, &delegation_id, max_parent_checks),
			delegation::Error::<Test>::DelegationNotFound
		);
	});
}

#[test]
fn is_delegating_root_after_max_limit() {
	let user_1_keypair = get_alice_ed25519();
	let user_1 = get_ed25519_account(user_1_keypair.public());
	let user_2_keypair = get_bob_ed25519();
	let user_2 = get_ed25519_account(user_2_keypair.public());
	let user_3_keypair = get_charlie_ed25519();
	let user_3 = get_ed25519_account(user_3_keypair.public());

	let (root_id, mut root_node) = (
		get_delegation_root_id(true),
		generate_base_delegation_root(user_1.clone()),
	);
	root_node.revoked = true;
	let (delegation_id_1, mut delegation_node_1) =
		(get_delegation_id(true), generate_base_delegation_node(root_id, user_2));
	delegation_node_1.revoked = true;
	let (delegation_id_2, mut delegation_node_2) =
		(get_delegation_id(false), generate_base_delegation_node(root_id, user_3));
	delegation_node_2.parent = Some(delegation_id_1);

	// 1 less than needed
	let max_parent_checks = 1u32;

	// Root -> Delegation 1 -> Delegation 2
	let mut ext = ExtBuilder::default()
		.with_root_delegations(vec![(root_id, root_node)])
		.with_delegations(vec![
			(delegation_id_1, delegation_node_1),
			(delegation_id_2, delegation_node_2),
		])
		.with_children(vec![
			(root_id, vec![delegation_id_1]),
			(delegation_id_1, vec![delegation_id_2]),
		])
		.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Delegation::is_delegating(&user_1, &delegation_id_2, max_parent_checks),
			delegation::Error::<Test>::MaxSearchDepthReached
		);
	});
}
