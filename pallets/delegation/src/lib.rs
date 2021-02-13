// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Delegation: Handles delegations on chain,
//! creating and revoking root nodes of delegation hierarchies,
//! adding and revoking delegation nodes based on root nodes.
#![cfg_attr(not(feature = "std"), no_std)]

/// Test module for delegations
#[cfg(test)]
mod tests;

#[macro_use]
extern crate bitflags;

use codec::{Decode, Encode};
use core::default::Default;
use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
	Parameter, StorageMap,
};
use frame_system::{self, ensure_signed};
use sp_runtime::{
	codec::Codec,
	traits::{CheckEqual, Hash, IdentifyAccount, MaybeDisplay, Member, SimpleBitOps, Verify},
	verify_encoded_lazy, DispatchError,
};
use sp_std::{
	prelude::{Clone, Eq, PartialEq, Vec},
};

bitflags! {
	/// Bitflags for permissions
	#[derive(Encode, Decode)]
	pub struct Permissions: u32 {
		/// Bit flag for mark permission
		const ANCHOR = 0b0000_0001;
		/// Bit flag for delegation permission
		const DELEGATE = 0b0000_0010;
	}
}

/// Implementation for permissions
impl Permissions {
	/// Encode permission bitflags into u8 array
	fn as_u8(self) -> [u8; 4] {
		let x: u32 = self.bits;
		let b1: u8 = ((x >> 24) & 0xff) as u8;
		let b2: u8 = ((x >> 16) & 0xff) as u8;
		let b3: u8 = ((x >> 8) & 0xff) as u8;
		let b4: u8 = (x & 0xff) as u8;
		[b4, b3, b2, b1]
	}
}

/// Implement Default trait for permissions
impl Default for Permissions {
	/// Default permissions to anchor
	fn default() -> Self {
		Permissions::ANCHOR
	}
}

/// The delegation trait
pub trait Trait: mtype::Trait + frame_system::Config {
	/// Delegation specific event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// Signature of a delegation
	type Signature: Verify<Signer = Self::Signer> + Member + Codec + Default;

	/// Signer of a delegation
	// type Signer: From<Self::AccountId> + IdentifyAccount<AccountId = Self::AccountId>> + Member + Codec;
	type Signer: IdentifyAccount<AccountId = Self::AccountId> + Member + Codec;

	/// Delegation node id type
	type DelegationNodeId: Parameter
		+ Member
		+ Codec
		+ MaybeDisplay
		+ SimpleBitOps
		+ Default
		+ Copy
		+ CheckEqual
		+ sp_std::hash::Hash
		+ AsRef<[u8]>
		+ AsMut<[u8]>;
}

decl_event!(
	/// Events for delegations
	pub enum Event<T> where <T as frame_system::Config>::Hash, <T as frame_system::Config>::AccountId,
			<T as Trait>::DelegationNodeId {
		/// A new root has been created
		RootCreated(AccountId, DelegationNodeId, Hash),
		/// A root has been revoked
		RootRevoked(AccountId, DelegationNodeId),
		/// A new delegation has been created
		DelegationCreated(AccountId, DelegationNodeId, DelegationNodeId, Option<DelegationNodeId>,
				AccountId, Permissions),
		/// A delegation has been revoked
		DelegationRevoked(AccountId, DelegationNodeId),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		AlreadyExists,
		BadSignature,
		DelegationNotFound,
		RootAlreadyExists,
		RootNotFound,
		MaxSearchDepthReached,
		NotOwnerOfParent,
		NotOwnerOfRoot,
		ParentNotFound,
		UnauthorizedRevocation,
		UnauthorizedDelegation,
	}
}

decl_module! {
	/// The delegation runtime module
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Deposit events
		fn deposit_event() = default;
		
		// Initializing errors
		// this includes information about your errors in the node's metadata.
		// it is needed only if you are using errors in your pallet
		type Error = Error<T>;
		
		/// Creates a delegation hierarchy root on chain
		///, where, origin is the signed sender account,
		/// root_id is the unique identifier of the root node,
		/// and, mtype_hash - hash of the MTYPE the hierarchy is created for
		#[weight = 10_000_000]
		pub fn create_root(origin, root_id: T::DelegationNodeId, mtype_hash: T::Hash) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// check if a root with the given id already exists
			ensure!(!<Root<T>>::contains_key(root_id), Error::<T>::RootAlreadyExists);

			// check if MTYPE exists
			ensure!(<mtype::MTYPEs<T>>::contains_key(mtype_hash), mtype::Error::<T>::NotFound);

			// add root node to storage
			debug::print!("insert Delegation Root");
			<Root<T>>::insert(root_id, DelegationRoot::new(mtype_hash, sender.clone()));
			// deposit event that the root node has been created
			Self::deposit_event(RawEvent::RootCreated(sender, root_id, mtype_hash));
			Ok(())
		}

		/// Adds a delegation node on chain, where
		/// origin - the origin of the transaction
		/// delegation_id - unique identifier of the delegation node to be added
		/// root_id - id of the hierarchy root node
		/// parent_id - optional identifier of a parent node this delegation node is created under
		/// delegate - the delegate account
		/// permission - the permissions delegated
		/// delegate_signature - the signature of the delegate to ensure it's done under his permission
		#[weight = 10_000_000]
		pub fn add_delegation(
			origin,
			delegation_id: T::DelegationNodeId,
			root_id: T::DelegationNodeId,
			parent_id: Option<T::DelegationNodeId>,
			delegate: T::AccountId,
			permissions: Permissions,
			delegate_signature: T::Signature
		) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;

			// check if a delegation node with the given identifier already exists
			ensure!(!<Delegations<T>>::contains_key(delegation_id), Error::<T>::AlreadyExists);
			
			// calculate the hash root and check if the signature matches
			let hash_root = Self::calculate_hash(delegation_id, root_id, parent_id, permissions);
			ensure!(verify_encoded_lazy(&delegate_signature, &&hash_root, &delegate), Error::<T>::BadSignature);
			
			// check if root exists
			let root = <Root<T>>::get(root_id).ok_or(Error::<T>::RootNotFound)?;

			// check if this delegation has a parent
			if let Some(parent_id) = parent_id {
				// check if the parent exists
				let parent_node = <Delegations<T>>::get(parent_id).ok_or(Error::<T>::ParentNotFound)?;

				// check if the parent's delegate is the sender of this transaction and has permission to delegate
				ensure!(parent_node.owner.eq(&sender), Error::<T>::NotOwnerOfParent);

				// check if the parent has permission to delegate
				ensure!((parent_node.permissions & Permissions::DELEGATE) == Permissions::DELEGATE, Error::<T>::UnauthorizedDelegation);
			
				// insert delegation
				debug::print!("insert Delegation with parent");
				<Delegations<T>>::insert(delegation_id, DelegationNode::<T>::new_child(
					root_id,
					parent_id,
					delegate.clone(),
					permissions,
				));
				// add child to tree structure
				Self::add_child(delegation_id, parent_id);
			} else {
				// check if the sender of this transaction is the creator of the root node (as no parent is given)
				ensure!(root.owner.eq(&sender), Error::<T>::NotOwnerOfRoot);

				// insert delegation
				debug::print!("insert Delegation without parent");
				<Delegations<T>>::insert(delegation_id, DelegationNode::<T>::new_root(root_id, delegate.clone(), permissions));
				// add child to tree structure
				Self::add_child(delegation_id, root_id);
			}
			// deposit event that the delegation node has been added
			Self::deposit_event(RawEvent::DelegationCreated(sender, delegation_id,
					root_id, parent_id, delegate, permissions));
			Ok(())
		}

		/// Revoke the root and therefore a complete hierarchy, where
		/// origin - the origin of the transaction
		/// root_id - id of the hierarchy root node
		#[weight = 10_000_000]
		pub fn revoke_root(origin, root_id: T::DelegationNodeId) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// check if root node exists
			let mut root = <Root<T>>::get(root_id).ok_or(Error::<T>::RootNotFound)?;

			// check if root node has been created by the sender of this transaction
			ensure!(root.owner.eq(&sender), Error::<T>::UnauthorizedRevocation);

			if !root.revoked {
				// store revoked root node
				root.revoked = true;
				<Root<T>>::insert(root_id, root);
				// recursively revoke all children
				Self::revoke_children(&root_id, &sender)?;
			}
			// deposit event that the root node has been revoked
			Self::deposit_event(RawEvent::RootRevoked(sender, root_id));
			Ok(())
		}

		/// Revoke a delegation node and all its children, where
		/// origin - the origin of the transaction
		/// delegation_id - id of the delegation node
		#[weight = 10_000_000]
		pub fn revoke_delegation(origin, delegation_id: T::DelegationNodeId, max_depth: u64) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = ensure_signed(origin)?;
			// check if delegation node exists
			ensure!(<Delegations<T>>::contains_key(delegation_id), Error::<T>::DelegationNotFound);

			// check if the sender of this transaction is permitted by being the
			// owner of the delegation or of one of its parents
			ensure!(Self::is_delegating(&sender, &delegation_id, max_depth)?, Error::<T>::UnauthorizedRevocation);

			// revoke the delegation and recursively all of its children
			Self::revoke(&delegation_id, &sender)
		}
	}
}

/// Implementation of further module constants and functions for delegations
impl<T: Trait> Module<T> {
	/// Calculates the hash of all values of a delegation transaction
	pub fn calculate_hash(
		delegation_id: T::DelegationNodeId,
		root_id: T::DelegationNodeId,
		parent_id: Option<T::DelegationNodeId>,
		permissions: Permissions,
	) -> T::Hash {
		// add all values to an u8 vector
		let mut hashed_values: Vec<u8> = delegation_id.as_ref().to_vec();
		hashed_values.extend_from_slice(root_id.as_ref());
		if let Some(parent) = parent_id {
			hashed_values.extend_from_slice(parent.as_ref())
		}
		hashed_values.extend_from_slice(permissions.as_u8().as_ref());
		// hash vector
		T::Hashing::hash(&hashed_values)
	}

	/// Check if an account is the owner of the delegation or any delegation up the hierarchy (including the root)
	pub fn is_delegating(
		account: &T::AccountId,
		delegation: &T::DelegationNodeId,
		max_depth: u64,
	) -> Result<bool, DispatchError> {
		// check for recursion anchor
		ensure!(max_depth > 0, Error::<T>::MaxSearchDepthReached);

		// check if delegation exists
		let delegation_node =
			<Delegations<T>>::get(delegation).ok_or(Error::<T>::DelegationNotFound)?;

		// check if the account is the owner of the delegation
		if delegation_node.owner.eq(account) {
			Ok(true)
		} else if let Some(parent) = delegation_node.parent {
			// recurse upwards in hierarchy
			Self::is_delegating(account, &parent, max_depth - 1)
		} else {
			// return whether the given account is the owner of the root
			let root = <Root<T>>::get(delegation_node.root_id).ok_or(Error::<T>::RootNotFound)?;
			Ok(root.owner.eq(account))
		}
	}

	/// Revoke a delegation an all of its children
	fn revoke(delegation: &T::DelegationNodeId, sender: &T::AccountId) -> DispatchResult {
		// retrieve delegation node from storage
		let mut delegation_node =
			<Delegations<T>>::get(*delegation).ok_or(Error::<T>::DelegationNotFound)?;

		// check if already revoked
		if !delegation_node.revoked {
			// set revoked flag and store delegation node
			delegation_node.revoked = true;
			<Delegations<T>>::insert(*delegation, delegation_node);
			// deposit event that the delegation has been revoked
			Self::deposit_event(RawEvent::DelegationRevoked(sender.clone(), *delegation));
			// revoke all children recursively
			Self::revoke_children(delegation, sender)?;
		}
		Ok(())
	}

	/// Revoke all children of a delegation
	fn revoke_children(delegation: &T::DelegationNodeId, sender: &T::AccountId) -> DispatchResult {
		// check if there's a child vector in the storage
		if <Children<T>>::contains_key(delegation) {
			// iterate child vector and revoke all nodes
			let children = <Children<T>>::get(delegation);
			for child in children {
				Self::revoke(&child, sender)?;
			}
		}
		Ok(())
	}

	/// Add a child node into the delegation hierarchy
	fn add_child(child: T::DelegationNodeId, parent: T::DelegationNodeId) {
		// get the children vector
		let mut children = <Children<T>>::get(parent);
		// add child element
		children.push(child);
		// store vector with new child
		<Children<T>>::insert(parent, children);
	}
}

#[derive(Encode, Decode)]
pub struct DelegationNode<T: Trait> {
	pub root_id: T::DelegationNodeId,
	pub parent: Option<T::DelegationNodeId>,
	pub owner: T::AccountId,
	pub permissions: Permissions,
	pub revoked: bool,
}

impl<T: Trait> DelegationNode<T> {
	pub fn new_root(
		root_id: T::DelegationNodeId,
		owner: T::AccountId,
		permissions: Permissions,
	) -> Self {
		DelegationNode {
			root_id,
			owner,
			permissions,
			revoked: false,
			parent: None,
		}
	}

	/// new_child creates a new child node for the delegation tree.
	///
	/// root_id - the root of the delegation tree
	/// parent - the parent in the tree
	/// owner - the owner of the new child root. He will receive the delegated permissions
	/// permissions - the permissions that are delegated
	pub fn new_child(
		root_id: T::DelegationNodeId,
		parent: T::DelegationNodeId,
		owner: T::AccountId,
		permissions: Permissions,
	) -> Self {
		DelegationNode {
			root_id,
			parent: Some(parent),
			owner,
			permissions,
			revoked: false,
		}
	}
}

#[derive(Encode, Decode)]
pub struct DelegationRoot<T: Trait> {
	pub mtype_hash: T::Hash,
	pub owner: T::AccountId,
	pub revoked: bool,
}

impl<T: Trait> DelegationRoot<T> {
	fn new(mtype_hash: T::Hash, owner: T::AccountId) -> Self {
		DelegationRoot {
			mtype_hash,
			owner,
			revoked: false,
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Delegation {
		// Root: root-id => DelegationRoot?
		pub Root get(fn root):map hasher(opaque_blake2_256) T::DelegationNodeId => Option<DelegationRoot<T>>;

		// Root: delegation-id => Delegation?
		pub Delegations get(fn delegation):
			map hasher(opaque_blake2_256) T::DelegationNodeId
			=> Option<DelegationNode<T>>;

		// Children: root-or-delegation-id => [delegation-id]
		pub Children get(fn children):map hasher(opaque_blake2_256) T::DelegationNodeId => Vec<T::DelegationNodeId>;
	}
}
