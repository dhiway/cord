// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_core::{offchain::KeyTypeId, sr25519};
use sp_io::crypto::sr25519_generate;
use sp_std::num::NonZeroU32;

const SEED: u32 = 0;
const MAX_REVOCATIONS: u32 = 5;
const ONE_CHILD_PER_LEVEL: Option<NonZeroU32> = NonZeroU32::new(1);

struct DelegationTriplet<T: Config> {
	public: sr25519::Public,
	acc: T::AccountId,
	delegation_id: T::DelegationNodeId,
}

/// generats a delegation id from a given number
fn generate_delegation_id<T: Config>(number: u32) -> T::DelegationNodeId
where
	T::DelegationNodeId: From<T::Hash>,
{
	let hash: T::Hash = T::Hashing::hash(&number.to_ne_bytes());
	hash.into()
}

/// sets parent to `None` if it is the root
fn parent_id_check<T: Config>(
	root_id: T::DelegationNodeId,
	parent_id: T::DelegationNodeId,
) -> Option<T::DelegationNodeId> {
	if parent_id == root_id {
		None
	} else {
		Some(parent_id)
	}
}

/// add mtype to storage and root delegation
fn add_root_delegation<T: Config>(number: u32) -> Result<(DelegationTriplet<T>, T::Hash), DispatchError>
where
	T::AccountId: From<sr25519::Public>,
	T::DelegationNodeId: From<T::Hash>,
{
	let root_public = sr25519_generate(KeyTypeId(*b"aura"), None);
	let root_acc: T::AccountId = root_public.into();
	let mtype_hash = <T::Hash as Default>::default();
	let root_id = generate_delegation_id::<T>(number);

	pallet_mtype::Module::<T>::anchor(RawOrigin::Signed(root_acc.clone()).into(), mtype_hash)?;
	Module::<T>::create_root(RawOrigin::Signed(root_acc.clone()).into(), root_id, mtype_hash)?;

	Ok((
		DelegationTriplet::<T> {
			public: root_public,
			acc: root_acc,
			delegation_id: root_id,
		},
		mtype_hash,
	))
}

/// recursively adds children delegations to a parent delegation for each level
/// until reaching leaf level
fn add_children<T: Config>(
	root_id: T::DelegationNodeId,
	parent_id: T::DelegationNodeId,
	parent_acc_public: sr25519::Public,
	parent_acc_id: T::AccountId,
	permissions: Permissions,
	level: u32,
	children_per_level: NonZeroU32,
) -> Result<(sr25519::Public, T::AccountId, T::DelegationNodeId), DispatchError>
where
	T::AccountId: From<sr25519::Public>,
	T::Signature: From<sr25519::Signature>,
	T::DelegationNodeId: From<T::Hash>,
{
	if level == 0 {
		return Ok((parent_acc_public, parent_acc_id, parent_id));
	};

	let mut first_leaf = None;
	for c in 0..children_per_level.get() {
		// setup delegation account and id
		let delegation_acc_public = sr25519_generate(KeyTypeId(*b"aura"), None);
		let delegation_acc_id: T::AccountId = delegation_acc_public.into();
		let delegation_id = generate_delegation_id::<T>(level * children_per_level.get() + c);

		// only set parent if not root
		let parent = parent_id_check::<T>(root_id, parent_id);

		// delegate signs delegation to parent
		let hash: Vec<u8> = Module::<T>::calculate_hash(delegation_id, root_id, parent, permissions).encode();
		let sig: T::Signature = sp_io::crypto::sr25519_sign(KeyTypeId(*b"aura"), &delegation_acc_public, hash.as_ref())
			.ok_or("Error while building signature of delegation.")?
			.into();

		// add delegation from delegate to parent
		let _ = Module::<T>::add_delegation(
			RawOrigin::Signed(parent_acc_id.clone()).into(),
			delegation_id,
			root_id,
			parent,
			delegation_acc_id.clone(),
			permissions,
			sig,
		)?;

		// only return first leaf
		first_leaf = first_leaf.or(Some((delegation_acc_public, delegation_acc_id, delegation_id)));
	}

	let (leaf_acc_public, leaf_acc_id, leaf_id) =
		first_leaf.expect("Should not be None due to restricting children_per_level to NonZeroU32");

	// go to next level until we reach level 0
	add_children::<T>(
		root_id,
		leaf_id,
		leaf_acc_public,
		leaf_acc_id,
		permissions,
		level - 1,
		children_per_level,
	)
}

// setup delegations for an arbitrary depth and children per level
// 1. create mtype and root delegation
// 2. create and append children delegations to prior child for each level
pub fn setup_delegations<T: Config>(
	levels: u32,
	children_per_level: NonZeroU32,
	permissions: Permissions,
) -> Result<
	(
		sr25519::Public,
		T::DelegationNodeId,
		sr25519::Public,
		T::DelegationNodeId,
	),
	DispatchError,
>
where
	T::AccountId: From<sr25519::Public>,
	T::Signature: From<sr25519::Signature>,
	T::DelegationNodeId: From<T::Hash>,
{
	let (
		DelegationTriplet::<T> {
			public: root_public,
			acc: root_acc,
			delegation_id: root_id,
		},
		_,
	) = add_root_delegation::<T>(0)?;

	// iterate levels and start with parent == root
	let (leaf_acc_public, _, leaf_id) = add_children::<T>(
		root_id,
		root_id,
		root_public,
		root_acc,
		permissions,
		levels,
		children_per_level,
	)?;
	Ok((root_public, root_id, leaf_acc_public, leaf_id))
}

benchmarks! {
	where_clause { where T: core::fmt::Debug, T::Signature: From<sr25519::Signature>, T::AccountId: From<sr25519::Public>, 	T::DelegationNodeId: From<T::Hash> }

	create_root {
		let caller: T::AccountId = account("caller", 0, SEED);
		let mtype_hash = <T::Hash as Default>::default();
		let delegation = generate_delegation_id::<T>(0);
		pallet_mtype::Module::<T>::anchor(RawOrigin::Signed(caller.clone()).into(), mtype_hash)?;
	}: _(RawOrigin::Signed(caller), delegation, mtype_hash)
	verify {
		assert!(Root::<T>::contains_key(delegation));
	}

	revoke_root {
		let r in 1 .. MAX_REVOCATIONS;
		let (root_acc, root_id, leaf_acc, leaf_id) = setup_delegations::<T>(r, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::DELEGATE)?;
		let root_acc_id: T::AccountId = root_acc.into();
	}: _(RawOrigin::Signed(root_acc_id.clone()), root_id, r)
	verify {
		assert!(Root::<T>::contains_key(root_id));
		let root_delegation = Root::<T>::get(root_id).ok_or("Missing root delegation")?;
		assert_eq!(root_delegation.owner, root_acc_id);
		assert_eq!(root_delegation.revoked, true);

		assert!(Delegations::<T>::contains_key(leaf_id));
		let leaf_delegation = Delegations::<T>::get(leaf_id).ok_or("Missing leaf delegation")?;
		assert_eq!(leaf_delegation.root_id, root_id);
		assert_eq!(leaf_delegation.owner, leaf_acc.into());
		assert_eq!(leaf_delegation.revoked, true);
	}

	add_delegation {
		// do setup
		let (_, root_id, leaf_acc, leaf_id) = setup_delegations::<T>(1, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::DELEGATE)?;

		// add one more delegation
		let delegate_acc_public = sr25519_generate(
			KeyTypeId(*b"aura"),
			None
		);
		let delegation_id = generate_delegation_id::<T>(u32::MAX);
		let parent_id = parent_id_check::<T>(root_id, leaf_id);

		let perm: Permissions = Permissions::ANCHOR | Permissions::DELEGATE;
		let hash_root = Module::<T>::calculate_hash(delegation_id, root_id, parent_id, perm);
		let sig: T::Signature = sp_io::crypto::sr25519_sign(KeyTypeId(*b"aura"), &delegate_acc_public, hash_root.as_ref()).ok_or("Error while building signature of delegation.")?.into();

		let delegate_acc_id: T::AccountId = delegate_acc_public.into();
		let leaf_acc_id: T::AccountId = leaf_acc.into();
	}: _(RawOrigin::Signed(leaf_acc_id), delegation_id, root_id, parent_id, delegate_acc_id, perm, sig)
	verify {
		assert!(Delegations::<T>::contains_key(delegation_id));
	}

	// worst case #1: revoke a child of the root delegation
	// because all of its children have to be revoked
	// complexitiy: O(h * c) with h = height of the delegation tree, c = max number of children in a level
	revoke_delegation_root_child {
		let r in 1 .. MAX_REVOCATIONS;
		let (_, root_id, leaf_acc, leaf_id) = setup_delegations::<T>(r, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::DELEGATE)?;
		let children: Vec<T::DelegationNodeId> = Children::<T>::get(root_id);
		let child_id: T::DelegationNodeId = *children.get(0).ok_or("Root should have children")?;
		let child_delegation = Delegations::<T>::get(child_id).ok_or("Child of root should have delegation id")?;
	}: revoke_delegation(RawOrigin::Signed(child_delegation.owner.clone()), child_id, r, r)
	verify {
		assert!(Delegations::<T>::contains_key(child_id));
		let DelegationNode::<T> { revoked, .. } = Delegations::<T>::get(leaf_id).ok_or("Child of root should have delegation id")?;
		assert_eq!(revoked, true);

		assert!(Delegations::<T>::contains_key(leaf_id));
		let leaf_delegation = Delegations::<T>::get(leaf_id).ok_or("Missing leaf delegation")?;
		assert_eq!(leaf_delegation.root_id, root_id);
		assert_eq!(leaf_delegation.owner, leaf_acc.into());
		assert_eq!(leaf_delegation.revoked, true);
	}
	// TODO: Might want to add variant iterating over children instead of depth at some later point

	// worst case #2: revoke leaf node as root
	// because `is_delegating` has to traverse up to the root
	// complexitiy: O(h) with h = height of the delegation tree
	revoke_delegation_leaf {
		let r in 1 .. MAX_REVOCATIONS;
		let (root_acc, _, _, leaf_id) = setup_delegations::<T>(r, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::DELEGATE)?;
	}: revoke_delegation(RawOrigin::Signed(root_acc.clone().into()), leaf_id, r, r)
	verify {
		assert!(Delegations::<T>::contains_key(leaf_id));
		let DelegationNode::<T> { revoked, .. } = Delegations::<T>::get(leaf_id).ok_or("Child of root should have delegation id")?;
		assert_eq!(revoked, true);
	}
	// TODO: Might want to add variant iterating over children instead of depth at some later point
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::{ExtBuilder, Test};
	use ctype::CTYPEs;
	use frame_support::{assert_ok, StorageMap};
	use sp_std::num::NonZeroU32;

	#[test]
	fn test_benchmark_utils_generate_id() {
		ExtBuilder::build_with_keystore().execute_with(|| {
			assert_eq!(generate_delegation_id::<Test>(1), generate_delegation_id::<Test>(1));
			assert_ne!(generate_delegation_id::<Test>(1), generate_delegation_id::<Test>(2));
			let root = generate_delegation_id::<Test>(1);
			let parent = generate_delegation_id::<Test>(2);
			assert_eq!(parent_id_check::<Test>(root, root), None);
			assert_eq!(parent_id_check::<Test>(root, parent), Some(parent));
		});
	}
	#[test]
	fn test_benchmark_utils_manual_setup() {
		ExtBuilder::build_with_keystore().execute_with(|| {
			let (
				DelegationTriplet::<Test> {
					public: root_acc_public,
					acc: root_acc_id,
					delegation_id: root_id,
				},
				mtype_hash,
			) = add_root_delegation::<Test>(0).expect("failed to add root delegation");
			assert_eq!(root_id, generate_delegation_id::<Test>(0));
			assert!(Root::<Test>::contains_key(root_id));
			assert!(MTYPEs::<Test>::contains_key(mtype_hash));

			// add "parent" as child delegation of root
			let (parent_acc_public, parent_acc_id, parent_id) = add_children::<Test>(
				root_id,
				root_id,
				root_acc_public,
				root_acc_id,
				Permissions::DELEGATE,
				1,
				NonZeroU32::new(1).expect(">0"),
			)
			.expect("failed to add children to root delegation");
			assert_eq!(
				Delegations::<Test>::get(parent_id),
				Some(DelegationNode::<Test> {
					root_id,
					parent: None,
					owner: parent_acc_id.clone(),
					permissions: Permissions::DELEGATE,
					revoked: false
				})
			);

			// add "leaf" as child delegation of "parent"
			let (_, leaf_acc_id, leaf_id) = add_children::<Test>(
				root_id,
				parent_id,
				parent_acc_public,
				parent_acc_id,
				Permissions::DELEGATE,
				1,
				NonZeroU32::new(2).expect(">0"),
			)
			.expect("failed to add children to child of root delegation");
			assert_eq!(
				Delegations::<Test>::get(leaf_id),
				Some(DelegationNode::<Test> {
					root_id,
					parent: Some(parent_id),
					owner: leaf_acc_id,
					permissions: Permissions::DELEGATE,
					revoked: false
				})
			);
		});
	}
	#[test]
	fn test_benchmark_utils_auto_setup() {
		ExtBuilder::build_with_keystore().execute_with(|| {
			let (_, root_id, _, leaf_id) =
				setup_delegations::<Test>(2, NonZeroU32::new(2).expect(">0"), Permissions::DELEGATE)
					.expect("failed to run delegation setup");
			assert!(Root::<Test>::contains_key(root_id));
			assert!(Delegations::<Test>::contains_key(leaf_id));
		});
	}

	#[test]
	fn test_benchmarks() {
		ExtBuilder::build_with_keystore().execute_with(|| {
			assert_ok!(test_benchmark_create_root::<Test>());
			assert_ok!(test_benchmark_revoke_root::<Test>());
			assert_ok!(test_benchmark_add_delegation::<Test>());
			assert_ok!(test_benchmark_revoke_delegation_root_child::<Test>());
			assert_ok!(test_benchmark_revoke_delegation_leaf::<Test>());
		});
	}
}