// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

use codec::Encode;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::dispatch::DispatchErrorWithPostInfo;
use frame_system::RawOrigin;
use sp_core::{offchain::KeyTypeId, sr25519};
use sp_io::crypto::sr25519_generate;
use sp_runtime::MultiSignature;
use sp_std::{num::NonZeroU32, vec::Vec};

use crate::*;

const SEED: u32 = 0;
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
fn add_root_delegation<T: Config>(number: u32) -> Result<(DelegationTriplet<T>, T::Hash), DispatchErrorWithPostInfo>
where
	T::AccountId: From<sr25519::Public>,
	T::DelegationNodeId: From<T::Hash>,
{
	let root_public = sr25519_generate(KeyTypeId(*b"aura"), None);
	let root_acc: T::AccountId = root_public.into();
	let mtype_hash = <T::Hash as Default>::default();
	let root_id = generate_delegation_id::<T>(number);

	pallet_mtype::Pallet::<T>::anchor(RawOrigin::Signed(root_acc.clone()).into(), mtype_hash)?;
	Pallet::<T>::create_root(RawOrigin::Signed(root_acc.clone()).into(), root_id, mtype_hash)?;

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
) -> Result<(sr25519::Public, T::AccountId, T::DelegationNodeId), DispatchErrorWithPostInfo>
where
	T::AccountId: From<sr25519::Public> + Into<T::DelegationEntityId>,
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
		let hash: Vec<u8> = Pallet::<T>::calculate_hash(&delegation_id, &root_id, &parent, &permissions).encode();
		let sig = sp_io::crypto::sr25519_sign(KeyTypeId(*b"aura"), &delegation_acc_public, hash.as_ref())
			.ok_or("Error while building signature of delegation.")?;

		// add delegation from delegate to parent
		let _ = Pallet::<T>::add_delegation(
			RawOrigin::Signed(parent_acc_id.clone()).into(),
			delegation_id,
			root_id,
			parent,
			delegation_acc_id.clone().into(),
			permissions,
			MultiSignature::from(sig).encode(),
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
	DispatchErrorWithPostInfo,
>
where
	T::AccountId: From<sr25519::Public> + Into<T::DelegationEntityId>,
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
	where_clause { where T: core::fmt::Debug, T::AccountId: From<sr25519::Public> + Into<T::DelegationEntityId>, T::DelegationNodeId: From<T::Hash>, <T as frame_system::Config>::Origin: From<RawOrigin<<T as pallet::Config>::DelegationEntityId>> }

	create_root {
		let caller: T::AccountId = account("caller", 0, SEED);
		let mtype_hash = <T::Hash as Default>::default();
		let delegation = generate_delegation_id::<T>(0);
		pallet_mtype::Pallet::<T>::anchor(RawOrigin::Signed(caller.clone()).into(), mtype)?;
	}: _(RawOrigin::Signed(caller), delegation, mtype)
	verify {
		assert!(Roots::<T>::contains_key(delegation));
	}

	revoke_root {
		let r in 1 .. T::MaxRevocations::get();
		let (root_acc, root_id, leaf_acc, leaf_id) = setup_delegations::<T>(r, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::DELEGATE)?;
		let root_acc_id: T::AccountId = root_acc.into();
	}: _(RawOrigin::Signed(root_acc_id.clone()), root_id, r)
	verify {
		assert!(Roots::<T>::contains_key(root_id));
		let root_delegation = Roots::<T>::get(root_id).ok_or("Missing root delegation")?;
		assert_eq!(root_delegation.owner, root_acc_id.into());
		assert!(root_delegation.revoked);

		assert!(Delegations::<T>::contains_key(leaf_id));
		let leaf_delegation = Delegations::<T>::get(leaf_id).ok_or("Missing leaf delegation")?;
		assert_eq!(leaf_delegation.root_id, root_id);
		assert_eq!(leaf_delegation.owner, T::AccountId::from(leaf_acc).into());
		assert!(leaf_delegation.revoked);
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
		let hash_root = Pallet::<T>::calculate_hash(&delegation_id, &root_id, &parent_id, &perm);
		let sig = sp_io::crypto::sr25519_sign(KeyTypeId(*b"aura"), &delegate_acc_public, hash_root.as_ref()).ok_or("Error while building signature of delegation.")?;

		let delegate_acc_id: T::AccountId = delegate_acc_public.into();
		let leaf_acc_id: T::AccountId = leaf_acc.into();
	}: _(RawOrigin::Signed(leaf_acc_id), delegation_id, root_id, parent_id, delegate_acc_id.into(), perm, MultiSignature::from(sig).encode())
	verify {
		assert!(Delegations::<T>::contains_key(delegation_id));
	}

	// worst case #1: revoke a child of the root delegation
	// because all of its children have to be revoked
	// complexitiy: O(h * c) with h = height of the delegation tree, c = max number of children in a level
	revoke_delegation_root_child {
		let r in 1 .. T::MaxRevocations::get();
		let c in 1 .. T::MaxParentChecks::get();
		let (_, root_id, leaf_acc, leaf_id) = setup_delegations::<T>(r, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::DELEGATE)?;
		let children: Vec<T::DelegationNodeId> = Children::<T>::get(root_id).unwrap_or_default();
		let child_id: T::DelegationNodeId = *children.get(0).ok_or("Root should have children")?;
		let child_delegation = Delegations::<T>::get(child_id).ok_or("Child of root should have delegation id")?;
	}: revoke_delegation(RawOrigin::Signed(child_delegation.owner.clone()), child_id, c, r)
	verify {
		assert!(Delegations::<T>::contains_key(child_id));
		let DelegationNode::<T> { revoked, .. } = Delegations::<T>::get(leaf_id).ok_or("Child of root should have delegation id")?;
		assert!(revoked);

		assert!(Delegations::<T>::contains_key(leaf_id));
		let leaf_delegation = Delegations::<T>::get(leaf_id).ok_or("Missing leaf delegation")?;
		assert_eq!(leaf_delegation.root_id, root_id);
		assert_eq!(leaf_delegation.owner, T::AccountId::from(leaf_acc).into());
		assert!(leaf_delegation.revoked);
	}
	// TODO: Might want to add variant iterating over children instead of depth at some later point

	// worst case #2: revoke leaf node as root
	// because `is_delegating` has to traverse up to the root
	// complexitiy: O(h) with h = height of the delegation tree
	revoke_delegation_leaf {
		let r in 1 .. T::MaxRevocations::get();
		let c in 1 .. T::MaxParentChecks::get();
		let (root_acc, _, _, leaf_id) = setup_delegations::<T>(c, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::DELEGATE)?;
	}: revoke_delegation(RawOrigin::Signed(T::AccountId::from(root_acc).into()), leaf_id, c, r)
	verify {
		assert!(Delegations::<T>::contains_key(leaf_id));
		let DelegationNode::<T> { revoked, .. } = Delegations::<T>::get(leaf_id).ok_or("Child of root should have delegation id")?;
		assert!(revoked);
	}
	// TODO: Might want to add variant iterating over children instead of depth at some later point
}

impl_benchmark_test_suite! {
	Pallet,
	crate::mock::ExtBuilder::default().build_with_keystore(None),
	crate::mock::Test
}
