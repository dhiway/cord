use crate::{Config, CordAccountOf, Error, IdentifierOf, SchemaIdentifierOf, SpaceIdentifierOf};
use bitflags::bitflags;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::DispatchResult, storage::bounded_btree_set::BoundedBTreeSet, RuntimeDebug,
};
use scale_info::TypeInfo;

bitflags! {
	/// Bitflags for permissions.
	///
	/// Permission bits can be combined to express multiple permissions.
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct Permissions: u32 {
		/// Permission to write attestations on chain.
		const ATTEST = 0b0000_0001;
		/// Permission to write delegations on chain.
		const DELEGATE = 0b0000_0010;
	}
}

impl Permissions {
	/// Encode permission bitflags into u8 array.
	pub fn as_u8(self) -> [u8; 4] {
		let x: u32 = self.bits;
		let b1: u8 = ((x >> 24) & 0xff) as u8;
		let b2: u8 = ((x >> 16) & 0xff) as u8;
		let b3: u8 = ((x >> 8) & 0xff) as u8;
		let b4: u8 = (x & 0xff) as u8;
		[b4, b3, b2, b1]
	}
}

impl Default for Permissions {
	fn default() -> Self {
		Permissions::ATTEST
	}
}

/// A node in a delegation hierarchy.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct HierarchyNode<T: Config> {
	/// The identifier of the delegation hierarchy the node is part of.
	pub root: IdentifierOf,
	/// The identifier of the parent. For all but root nodes this is not None.
	pub parent: Option<IdentifierOf>,
	/// The set of identifier's of all the children nodes.
	pub children: BoundedBTreeSet<IdentifierOf, T::MaxChildren>,
	/// The additional information attached to the delegation node.
	pub details: HierarchyDetails<T>,
}

impl<T: Config> HierarchyNode<T> {
	/// Creates a new delegation root node with the given identifier and
	/// delegation details.
	pub fn new_root_node(id: IdentifierOf, details: HierarchyDetails<T>) -> Self {
		Self {
			root: id,
			parent: None,
			children: BoundedBTreeSet::<IdentifierOf, T::MaxChildren>::new(),
			details,
		}
	}

	/// Creates a new delegation node under the given hierarchy identifier, with
	/// the given parent and delegation details.
	pub fn new_node(
		root: IdentifierOf,
		parent: IdentifierOf,
		details: HierarchyDetails<T>,
	) -> Self {
		Self {
			root,
			parent: Some(parent),
			children: BoundedBTreeSet::<IdentifierOf, T::MaxChildren>::new(),
			details,
		}
	}

	/// Adds a node by its identifier to the current node's children.
	pub fn try_add_child(&mut self, child_id: IdentifierOf) -> DispatchResult {
		self.children
			.try_insert(child_id)
			.map_err(|_| Error::<T>::MaxChildrenExceeded)?;
		Ok(())
	}
}

/// Delegation information attached to delegation nodes.
#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct HierarchyDetails<T: Config> {
	/// The owner of the delegation (and its node).
	pub controller: CordAccountOf<T>,
	/// Status indicating whether the delegation has been revoked (true) or not
	/// (false).
	pub revoked: bool,
	/// The set of permissions associated with the delegation.
	pub permissions: Permissions,
}

impl<T: Config> HierarchyDetails<T> {
	/// Creates new delegation details including the given owner.
	pub fn default_with_controller(controller: CordAccountOf<T>) -> Self {
		Self { controller, permissions: Permissions::all(), revoked: false }
	}
}

/// The details associated with a delegation hierarchy.
#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, Ord, PartialOrd, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]

pub struct HierarchyElements {
	pub space: SpaceIdentifierOf,
	pub schema: SchemaIdentifierOf,
}

/// Transaction proposer details.
#[derive(
	Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct TxProposer<CordAccountOf, HashOf, SignatureOf> {
	/// The proposer of the transaction
	pub account: CordAccountOf,
	/// Transaition hash.
	pub digest: HashOf,
	/// Transaction Signature
	pub signature: SignatureOf,
}
