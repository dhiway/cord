// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Delegation: Handles delegations on chain,
//! creating and revoking root nodes of delegation hierarchies,
//! adding and revoking delegation nodes based on root nodes.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod delegation_hierarchy;
pub mod weights;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

/// Test module for delegations
#[cfg(test)]
mod tests;

pub use delegation_hierarchy::*;
pub use pallet::*;

use frame_support::{ensure, pallet_prelude::Weight, traits::Get};
use sp_runtime::{traits::Hash, DispatchError};
use sp_std::vec::Vec;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a delegation node identifier.
	pub type DelegationNodeIdOf<T> = <T as Config>::DelegationNodeId;

	/// Type of a delegator or a delegate.
	pub type DelegatorIdOf<T> = <T as Config>::DelegationEntityId;

	/// The type of a MTYPE hash.
	pub type MtypeHashOf<T> = pallet_mtype::MtypeHashOf<T>;

	/// Type of a signature verification operation over the delegation details.
	pub type DelegationSignatureVerificationOf<T> = <T as Config>::DelegationSignatureVerification;

	/// Type of the signature that the delegate generates over the delegation
	/// information.
	pub type DelegateSignatureTypeOf<T> = <DelegationSignatureVerificationOf<T> as VerifyDelegateSignature>::Signature;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_mtype::Config {
		type DelegationSignatureVerification: VerifyDelegateSignature<
			DelegateId = Self::DelegationEntityId,
			Payload = Vec<u8>,
			Signature = Vec<u8>,
		>;
		type DelegationEntityId: Parameter;
		type DelegationNodeId: Parameter + Copy + AsRef<[u8]>;
		type EnsureOrigin: EnsureOrigin<Success = DelegatorIdOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		#[pallet::constant]
		type MaxSignatureByteLength: Get<u16>;
		#[pallet::constant]
		type MaxRevocations: Get<u32>;
		#[pallet::constant]
		type MaxParentChecks: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Delegation root nodes stored on chain.
	///
	/// It maps from a root node ID to the full root node.
	#[pallet::storage]
	#[pallet::getter(fn roots)]
	pub type Roots<T> = StorageMap<_, Blake2_128Concat, DelegationNodeIdOf<T>, DelegationRoot<T>>;

	/// Delegation nodes stored on chain.
	///
	/// It maps from a node ID to the full delegation node.
	#[pallet::storage]
	#[pallet::getter(fn delegations)]
	pub type Delegations<T> = StorageMap<_, Blake2_128Concat, DelegationNodeIdOf<T>, DelegationNode<T>>;

	/// Children delegation nodes.
	///
	/// It maps from a delegation node ID, including the root node, to the list
	/// of children nodes, sorted by time of creation.
	#[pallet::storage]
	#[pallet::getter(fn children)]
	pub type Children<T> = StorageMap<_, Blake2_128Concat, DelegationNodeIdOf<T>, Vec<DelegationNodeIdOf<T>>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new root has been created.
		/// \[creator ID, registry root ID, MTYPE hash\]
		DelegationRegistryAnchored(DelegatorIdOf<T>, DelegationNodeIdOf<T>, MtypeHashOf<T>),
		/// A Registry root has been revoked.
		/// \[revoker ID, registry root ID\]
		DelegationRegistryRevoked(DelegatorIdOf<T>, DelegationNodeIdOf<T>),
		/// A new delegation has been added to the registry.
		/// \[creator ID, registry root ID, delegation node ID, parent node ID,
		/// delegate ID, permissions\]
		DelegationAdded(
			DelegatorIdOf<T>,
			DelegationNodeIdOf<T>,
			DelegationNodeIdOf<T>,
			Option<DelegationNodeIdOf<T>>,
			DelegatorIdOf<T>,
			Permissions,
		),
		/// A delegation has been revoked.
		/// \[revoker ID, delegation node ID\]
		DelegationRevoked(DelegatorIdOf<T>, DelegationNodeIdOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a delegation node with the same ID stored on chain.
		DelegationAlreadyExists,
		/// The delegate's signature for the delegation creation operation is
		/// invalid.
		InvalidDelegateSignature,
		/// No delegation with the given ID stored on chain.
		DelegationNotFound,
		/// No delegate with the given ID stored on chain.
		DelegateNotFound,
		/// There is already a root node with the same ID stored on chain.
		RootAlreadyExists,
		/// No root delegation with the given ID stored on chain.
		RootNotFound,
		/// Max number of nodes checked without verifying the given condition.
		MaxSearchDepthReached,
		/// Max number of nodes checked without verifying the given condition.
		NotOwnerOfParentDelegation,
		/// The delegation creator is not allowed to write the delegation
		/// because he is not the owner of the delegation root node.
		NotOwnerOfRootDelegation,
		/// No parent delegation with the given ID stored on chain.
		ParentDelegationNotFound,
		/// The delegation revoker is not allowed to revoke the delegation.
		UnauthorizedRevocation,
		/// The delegation creator is not allowed to create the delegation.
		UnauthorizedDelegation,
		/// Max number of delegation nodes revocation has been reached for the
		/// operation.
		ExceededRevocationBounds,
		/// The max number of revocation exceeds the limit for the pallet.
		MaxRevocationsTooLarge,
		/// The max number of parent checks exceeds the limit for the pallet.
		MaxParentChecksTooLarge,
		/// An error that is not supposed to take place, yet it happened.
		InternalError,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new delegation root.
		///
		/// The new root will allow a new trust hierarchy to be created by
		/// adding children delegations to the root.
		///
		/// * origin: the identifier of the delegation creator
		/// * root_id: the ID of the root node. It has to be unique
		/// * mtype_hash: the MTYPE hash that delegates can use for marks
		#[pallet::weight(0)]
		pub fn create_root(
			origin: OriginFor<T>,
			root_id: DelegationNodeIdOf<T>,
			mtype_hash: MtypeHashOf<T>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(!<Roots<T>>::contains_key(&root_id), Error::<T>::RootAlreadyExists);

			ensure!(
				<pallet_mtype::Mtypes<T>>::contains_key(&mtype_hash),
				<pallet_mtype::Error<T>>::MTypeNotFound
			);

			log::debug!("insert Delegation Root");
			<Roots<T>>::insert(&root_id, DelegationRoot::new(mtype_hash, creator.clone()));

			Self::deposit_event(Event::DelegationRegistryAnchored(creator, root_id, mtype_hash));

			Ok(())
		}

		/// Create a new delegation node.
		///
		/// The new delegation node represents a new trust hierarchy that
		/// considers the new node as its root. The owner of this node has full
		/// control over any of its direct and indirect descendants.
		///
		/// * origin: the identifier of the delegation creator
		/// * delegation_id: the ID of the new delegation node. It has to be
		///   unique
		/// * root_id: the ID of the delegation hierarchy root to add this
		///   delegation to
		/// * parent_id: \[OPTIONAL\] The ID of the parent node to verify that
		///   the creator is allowed to create a new delegation. If None, the
		///   verification is performed against the provided root node
		/// * delegate: the identifier of the delegate
		/// * permissions: the permission flags for the operations the delegate
		///   is allowed to perform
		/// * delegate_signature: the delegate's signature over the new
		///   delegation ID, root ID, parent ID, and permission flags
		#[pallet::weight(0)]
		pub fn add_delegation(
			origin: OriginFor<T>,
			delegation_id: DelegationNodeIdOf<T>,
			root_id: DelegationNodeIdOf<T>,
			parent_id: Option<DelegationNodeIdOf<T>>,
			delegate: DelegatorIdOf<T>,
			permissions: Permissions,
			delegate_signature: DelegateSignatureTypeOf<T>,
		) -> DispatchResult {
			let delegator = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			// Calculate the hash root
			let hash_root = Self::calculate_hash(&delegation_id, &root_id, &parent_id, &permissions);

			// Verify that the hash root signature is correct.
			DelegationSignatureVerificationOf::<T>::verify(&delegate, &hash_root.encode(), &delegate_signature)
				.map_err(|err| match err {
					SignatureVerificationError::SignerInformationNotPresent => Error::<T>::DelegateNotFound,
					SignatureVerificationError::SignatureInvalid => Error::<T>::InvalidDelegateSignature,
				})?;

			ensure!(
				!<Delegations<T>>::contains_key(&delegation_id),
				Error::<T>::DelegationAlreadyExists
			);

			let root = <Roots<T>>::get(&root_id).ok_or(Error::<T>::RootNotFound)?;

			// Computes the delegation parent. Either the given parent (if allowed) or the
			// root node.
			let parent = if let Some(parent_id) = parent_id {
				let parent_node = <Delegations<T>>::get(&parent_id).ok_or(Error::<T>::ParentDelegationNotFound)?;

				// Check if the parent's delegate is the creator of this delegation node...
				ensure!(parent_node.owner == delegator, Error::<T>::NotOwnerOfParentDelegation);
				// ... and has permission to delegate
				ensure!(
					(parent_node.permissions & Permissions::DELEGATE) == Permissions::DELEGATE,
					Error::<T>::UnauthorizedDelegation
				);

				log::debug!("insert Delegation with parent");
				<Delegations<T>>::insert(
					&delegation_id,
					DelegationNode::<T>::new_node_child(root_id, parent_id, delegate.clone(), permissions),
				);

				// Return parent_id as the result of this if branch
				parent_id
			} else {
				// Check if the creator of this delegation node is the creator of the root node
				// (as no parent is given)
				ensure!(root.owner == delegator, Error::<T>::NotOwnerOfRootDelegation);

				log::debug!("insert Delegation without parent");
				<Delegations<T>>::insert(
					&delegation_id,
					DelegationNode::<T>::new_root_child(root_id, delegate.clone(), permissions),
				);

				// Return node_id as the result of this if branch
				root_id
			};

			// Regardless of the node returned as parent, add the new node as a child of
			// that node
			Self::add_child(delegation_id, parent);

			Self::deposit_event(Event::DelegationAdded(
				delegator,
				delegation_id,
				root_id,
				parent_id,
				delegate,
				permissions,
			));

			Ok(())
		}

		/// Revoke a delegation root.
		///
		/// Revoking a delegation root results in the whole trust hierarchy
		/// being revoked. Nevertheless, revocation starts from the leave nodes
		/// upwards, so if the operation ends prematurely because it runs out of
		/// gas, the delegation state would be consisent as no child would
		/// "survive" its parent. As a consequence, if the root node is revoked,
		/// the whole trust hierarchy is to be considered revoked.
		///
		/// * origin: the identifier of the revoker
		/// * root_id: the ID of the delegation root to revoke
		/// * max_children: the maximum number of nodes descending from the root
		///   to revoke as a consequence of the root revocation
		#[pallet::weight(0)]
		pub fn revoke_root(
			origin: OriginFor<T>,
			root_id: DelegationNodeIdOf<T>,
			max_children: u32,
		) -> DispatchResultWithPostInfo {
			let invoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mut root = <Roots<T>>::get(&root_id).ok_or(Error::<T>::RootNotFound)?;

			ensure!(root.owner == invoker, Error::<T>::UnauthorizedRevocation);

			ensure!(
				max_children <= T::MaxRevocations::get(),
				Error::<T>::MaxRevocationsTooLarge
			);

			let consumed_weight: Weight = if !root.revoked {
				// Recursively revoke all children
				let (_, post_weight) = Self::revoke_children(&root_id, &invoker, max_children)?;

				// If we didn't return an ExceededRevocationBounds error, we can revoke the root
				// too.
				root.revoked = true;
				<Roots<T>>::insert(&root_id, root);

				post_weight.saturating_add(T::DbWeight::get().writes(1))
			} else {
				0
			};

			Self::deposit_event(Event::DelegationRegistryRevoked(invoker, root_id));

			Ok(Some(consumed_weight.saturating_add(T::DbWeight::get().reads(1))).into())
		}

		/// Revoke a delegation node and all its children.
		///
		/// Revoking a delegation node results in the trust hierarchy starting
		/// from the given node being revoked. Nevertheless, revocation starts
		/// from the leave nodes upwards, so if the operation ends prematurely
		/// because it runs out of gas, the delegation state would be consisent
		/// as no child would "survive" its parent. As a consequence, if the
		/// given node is revoked, the trust hierarchy with the node as root is
		/// to be considered revoked.
		///
		/// * origin: the identifier of the revoker
		/// * delegation_id: the ID of the delegation root to revoke
		/// * max_parent_checks: in case the revoker is not the owner of the
		///   specified node, the number of parent nodes to check to verify that
		///   the revoker is authorised to perform the revokation. The
		///   evaluation terminates when a valid node is reached, when the whole
		///   hierarchy including the root node has been checked, or when the
		///   max number of parents is reached
		/// * max_revocations: the maximum number of nodes descending from this
		///   one to revoke as a consequence of this node revocation
		#[pallet::weight(0)]
		pub fn revoke_delegation(
			origin: OriginFor<T>,
			delegation_id: DelegationNodeIdOf<T>,
			max_parent_checks: u32,
			max_revocations: u32,
		) -> DispatchResultWithPostInfo {
			let invoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				<Delegations<T>>::contains_key(&delegation_id),
				Error::<T>::DelegationNotFound
			);

			ensure!(
				max_parent_checks <= T::MaxParentChecks::get(),
				Error::<T>::MaxParentChecksTooLarge
			);

			let (authorized, parent_checks) = Self::is_delegating(&invoker, &delegation_id, max_parent_checks)?;
			ensure!(authorized, Error::<T>::UnauthorizedRevocation);
			ensure!(
				max_revocations <= T::MaxRevocations::get(),
				Error::<T>::MaxRevocationsTooLarge
			);
			// Revoke the delegation and recursively all of its children
			let (revocation_checks, _) = Self::revoke(&delegation_id, &invoker, max_revocations)?;

			// Add worst case reads from `is_delegating`
			Ok(Some(
				<T as Config>::WeightInfo::revoke_delegation_root_child(revocation_checks, parent_checks).max(
					<T as Config>::WeightInfo::revoke_delegation_leaf(revocation_checks, parent_checks),
				),
			)
			.into())
		}
	}
}

impl<T: Config> Pallet<T> {
	// Calculate the hash of all values of a delegation transaction
	fn calculate_hash(
		delegation_id: &DelegationNodeIdOf<T>,
		root_id: &DelegationNodeIdOf<T>,
		parent_id: &Option<DelegationNodeIdOf<T>>,
		permissions: &Permissions,
	) -> T::Hash {
		// Add all values to an u8 vector
		let mut hashed_values: Vec<u8> = delegation_id.as_ref().to_vec();
		hashed_values.extend_from_slice(root_id.as_ref());
		if let Some(parent) = parent_id {
			hashed_values.extend_from_slice(parent.as_ref())
		}
		hashed_values.extend_from_slice(permissions.as_u8().as_ref());
		// Hash the resulting vector
		T::Hashing::hash(&hashed_values)
	}

	/// Check if an identity is the owner of the given delegation node or any
	/// node up the hierarchy, and if the delegation has not been yet revoked.
	///
	/// It checks whether the conditions are required for the given node,
	/// otherwise it goes up up to `max_parent_checks` nodes, including the root
	/// node, to check whether the given identity is a valid delegator of the
	/// given delegation.
	pub fn is_delegating(
		identity: &DelegatorIdOf<T>,
		delegation: &DelegationNodeIdOf<T>,
		max_parent_checks: u32,
	) -> Result<(bool, u32), DispatchError> {
		let delegation_node = <Delegations<T>>::get(delegation).ok_or(Error::<T>::DelegationNotFound)?;

		// Check if the given account is the owner of the delegation and that the
		// delegation has not been removed
		if &delegation_node.owner == identity {
			Ok((!delegation_node.revoked, 0u32))
		} else {
			// Counter is decreased regardless of whether we are checking the parent node
			// next of the root node, as the root node is as a matter of fact the top node's
			// parent.
			let remaining_lookups = max_parent_checks
				.checked_sub(1)
				.ok_or(Error::<T>::MaxSearchDepthReached)?;

			if let Some(parent) = delegation_node.parent {
				// Recursively check upwards in hierarchy
				Self::is_delegating(identity, &parent, remaining_lookups)
			} else {
				// Return whether the given account is the owner of the root and the root has
				// not been revoked
				let root = <Roots<T>>::get(delegation_node.root_id).ok_or(Error::<T>::RootNotFound)?;
				Ok((
					(&root.owner == identity) && !root.revoked,
					// safe because remaining lookups is at most max_parent_checks
					max_parent_checks - remaining_lookups,
				))
			}
		}
	}

	// Revoke a delegation and all of its children recursively.
	fn revoke(
		delegation: &DelegationNodeIdOf<T>,
		sender: &DelegatorIdOf<T>,
		max_revocations: u32,
	) -> Result<(u32, Weight), DispatchError> {
		let mut revocations: u32 = 0;
		let mut consumed_weight: Weight = 0;
		// Retrieve delegation node from storage
		let mut delegation_node = <Delegations<T>>::get(*delegation).ok_or(Error::<T>::DelegationNotFound)?;
		consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));

		// Check if already revoked
		if !delegation_node.revoked {
			// First revoke all children recursively
			let remaining_revocations = max_revocations
				.checked_sub(1)
				.ok_or(Error::<T>::ExceededRevocationBounds)?;
			Self::revoke_children(delegation, sender, remaining_revocations).map(|(r, w)| {
				revocations = revocations.saturating_add(r);
				consumed_weight = consumed_weight.saturating_add(w);
			})?;

			// If we run out of revocation gas, we only revoke children. The tree will be
			// changed but is still valid.
			ensure!(revocations < max_revocations, Error::<T>::ExceededRevocationBounds);

			// Set revoked flag and store delegation node
			delegation_node.revoked = true;
			<Delegations<T>>::insert(*delegation, delegation_node);
			consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().writes(1));
			// Deposit event that the delegation has been revoked
			Self::deposit_event(Event::DelegationRevoked(sender.clone(), *delegation));
			revocations = revocations.saturating_add(1);
		}
		Ok((revocations, consumed_weight))
	}

	/// Revokes all children of a delegation.
	/// Returns the number of revoked delegations and the consumed weight.
	fn revoke_children(
		delegation: &DelegationNodeIdOf<T>,
		sender: &DelegatorIdOf<T>,
		max_revocations: u32,
	) -> Result<(u32, Weight), DispatchError> {
		let mut revocations: u32 = 0;
		let mut consumed_weight: Weight = 0;
		// Check if there's a child vector in the storage
		if let Some(children) = <Children<T>>::get(delegation) {
			consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));

			// Iterate child vector and revoke all nodes
			for child in children {
				let remaining_revocations = max_revocations
					.checked_sub(revocations)
					.ok_or(Error::<T>::ExceededRevocationBounds)?;

				// Check whether we ran out of gas
				ensure!(remaining_revocations > 0, Error::<T>::ExceededRevocationBounds);

				Self::revoke(&child, sender, remaining_revocations).map(|(r, w)| {
					revocations = revocations.saturating_add(r);
					consumed_weight = consumed_weight.saturating_add(w);
				})?;
			}
		}
		Ok((revocations, consumed_weight.saturating_add(T::DbWeight::get().reads(1))))
	}

	// Add a child node into the delegation hierarchy
	fn add_child(child: DelegationNodeIdOf<T>, parent: DelegationNodeIdOf<T>) {
		// Get the children vector or initialize an empty one if none
		let mut children = <Children<T>>::get(parent).unwrap_or_default();
		children.push(child);
		<Children<T>>::insert(parent, children);
	}
}
