#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod default_weights;
pub mod delegation_hierarchy;

pub use crate::{default_weights::WeightInfo, delegation_hierarchy::*, pallet::*};

use codec::Encode;
use frame_support::{dispatch::DispatchResult, ensure, pallet_prelude::Weight, traits::Get};
use sp_runtime::DispatchError;

#[frame_support::pallet]
pub mod pallet {

	use super::*;
	pub use cord_primitives::{ss58identifier, IdentifierOf, StatusOf, HIERARCHY_PREFIX};
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, ExistenceRequirement, OnUnbalanced, WithdrawReasons},
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_runtime::traits::{IdentifyAccount, Saturating, Verify};

	/// Hash of the space.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a cord signature.
	pub type SignatureOf<T> = <T as Config>::Signature;
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	pub(crate) type BalanceOf<T> = <<T as Config>::Currency as Currency<CordAccountOf<T>>>::Balance;
	type NegativeImbalanceOf<T> =
		<<T as Config>::Currency as Currency<CordAccountOf<T>>>::NegativeImbalance;

	pub type TxProposerOf<T> = TxProposer<CordAccountOf<T>, HashOf<T>, SignatureOf<T>>;

	pub(crate) type SpaceIdentifierOf = pallet_space::IdentifierOf;
	pub(crate) type SchemaIdentifierOf = pallet_schema::IdentifierOf;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config + pallet_space::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = CordAccountOf<Self>,
		>;
		type Currency: Currency<CordAccountOf<Self>>;
		type HierarchyFee: Get<BalanceOf<Self>>;
		type NodeFee: Get<BalanceOf<Self>>;
		type FeeCollector: OnUnbalanced<NegativeImbalanceOf<Self>>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer>
			+ Parameter
			+ MaxEncodedLen
			+ TypeInfo;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		type WeightInfo: WeightInfo;
		/// Maximum number of revocations.
		#[pallet::constant]
		type MaxRevocations: Get<u32>;
		/// Maximum number of removals. Should be same as MaxRevocations
		#[pallet::constant]
		type MaxRemovals: Get<u32>;
		/// Maximum number of upwards traversals of the delegation tree from a
		/// node to the root and thus the depth of the delegation tree.
		#[pallet::constant]
		type MaxParentChecks: Get<u32>;
		/// Maximum number of all children for a delegation node. For a binary
		/// tree, this should be twice the maximum depth of the tree, i.e.
		/// `2 ^ MaxParentChecks`.
		#[pallet::constant]
		type MaxChildren: Get<u32> + Clone;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Hierarchy nodes stored on chain.
	/// It maps from a node Identifier to the details.
	#[pallet::storage]
	#[pallet::getter(fn hierarchies)]
	pub type Hierarchies<T> = StorageMap<_, Blake2_128Concat, IdentifierOf, HierarchyNode<T>>;

	/// Hierarchies node elements stored on chain.
	///
	/// It maps for a (root) node identifier to the element details.
	#[pallet::storage]
	#[pallet::getter(fn hierarchy_nodes)]
	pub type HierarchyNodes<T> = StorageMap<_, Blake2_128Concat, IdentifierOf, HierarchyElements>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new hierarchy has been created.
		/// \[controller, root, space, schema\]
		HierarchyCreated {
			controller: CordAccountOf<T>,
			identifier: IdentifierOf,
			space: SpaceIdentifierOf,
			schema: SchemaIdentifierOf,
		},
		/// A hierarchy has been revoked.
		/// \[revoker, root\]
		HierarchyRevoked { revoker: CordAccountOf<T>, identifier: IdentifierOf },
		/// A hierarchy has been removed from the storage on chain.
		/// \[remover, root\]
		HierarchyRemoved { remover: CordAccountOf<T>, identifier: IdentifierOf },
		/// A new delegation node has been created.
		/// \[proposer, root, node, parent, delegate, permissions\]
		DelegationNodeCreated {
			creator: CordAccountOf<T>,
			root: IdentifierOf,
			parent: IdentifierOf,
			node: IdentifierOf,
			delegate: CordAccountOf<T>,
			permissions: Permissions,
		},
		/// A delegation node has been revoked.
		/// \[revoker, delegation node identifier\]
		DelegationNodeRevoked { revoker: CordAccountOf<T>, identifier: IdentifierOf },
		/// A delegation node has been removed.
		/// \[remover, delegation node identifier\]
		DelegationNodeRemoved { remover: CordAccountOf<T>, identifier: IdentifierOf },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a delegation node with the same identifier stored
		/// on chain.
		DelegationNodeAlreadyExists,
		/// The signature for the transaction is invalid.
		InvalidSignature,
		/// No delegation node with the given identifier stored on chain.
		DelegationNodeNotFound,
		/// No delegate with the given identity stored on chain.
		DelegateNotFound,
		/// There is already a hierarchy with the same identifier stored on
		/// chain.
		HierarchyAlreadyExists,
		/// No hierarchy with the given identifier stored on chain.
		HierarchyNotFound,
		/// Max number of delegation nodes checked without verifying the given
		/// condition.
		MaxSearchDepthReached,
		/// The delegation creator is not allowed to write the delegation
		/// because they are not the owner of the delegation parent node.
		NotOwnerOfParentDelegation,
		/// The delegation creator is not allowed to write the delegation
		/// because they are not the owner of the delegation root node.
		NotOwnerOfDelegationHierarchy,
		/// No parent delegation with the given ID stored on chain.
		ParentDelegationNotFound,
		/// The parent delegation has previously been revoked.
		ParentDelegationRevoked,
		/// The delegation revoker is not allowed to revoke the delegation.
		UnauthorizedRevocation,
		/// The call origin is not authorized to remove the delegation.
		UnauthorizedRemoval,
		/// The delegation creator is not allowed to create the delegation.
		UnauthorizedDelegation,
		/// The operation wasn't allowed because of insufficient rights.
		AccessDenied,
		/// Max number of revocations for delegation nodes has been reached for
		/// the operation.
		ExceededRevocationBounds,
		/// Max number of removals for delegation nodes has been reached for the
		/// operation.
		ExceededRemovalBounds,
		/// The max number of revocation exceeds the limit for the pallet.
		MaxRevocationsTooLarge,
		/// The max number of removals exceeds the limit for the pallet.
		MaxRemovalsTooLarge,
		/// The max number of parent checks exceeds the limit for the pallet.
		MaxParentChecksTooLarge,
		/// An error that is not supposed to take place, yet it happened.
		InternalError,
		/// The max number of all children has been reached for the
		/// corresponding delegation node.
		MaxChildrenExceeded,
		/// Length of the identifier exceeds the limit.
		InvalidIdentifierLength,
		/// Transaction author not able to pay fees.
		UnableToPayFees,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new hierarchy root associated with a given space and schema
		/// identifiers.
		#[pallet::weight(<T as Config>::WeightInfo::create_hierarchy())]
		pub fn create_hierarchy(
			origin: OriginFor<T>,
			creator: TxProposerOf<T>,
			space: SpaceIdentifierOf,
			schema: SchemaIdentifierOf,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				creator.signature.verify(&(&creator.digest).encode()[..], &creator.account),
				Error::<T>::InvalidSignature
			);

			// Check the free balance
			let balance =
				<<T as pallet::Config>::Currency as Currency<CordAccountOf<T>>>::free_balance(
					&author,
				);
			<<T as pallet::Config>::Currency as Currency<CordAccountOf<T>>>::ensure_can_withdraw(
				&author,
				T::HierarchyFee::get(),
				WithdrawReasons::FEE,
				balance.saturating_sub(T::HierarchyFee::get()),
			)?;

			ensure!(
				<pallet_schema::Schemas<T>>::contains_key(&schema),
				<pallet_schema::Error<T>>::SchemaNotFound
			);

			ensure!(
				<pallet_space::Spaces<T>>::contains_key(&space),
				<pallet_space::Error<T>>::SpaceNotFound
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(creator.digest).encode()[..], HIERARCHY_PREFIX)
					.into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Hierarchies<T>>::contains_key(&identifier),
				Error::<T>::HierarchyAlreadyExists
			);

			Self::create_and_store_new_hierarchy(
				identifier.clone(),
				HierarchyElements { space: space.clone(), schema: schema.clone() },
				creator.account.clone(),
				author,
			)?;

			Self::deposit_event(Event::HierarchyCreated {
				controller: creator.account,
				identifier,
				space,
				schema,
			});

			Ok(())
		}

		/// Create a new delegation node.
		#[pallet::weight(<T as Config>::WeightInfo::add_delegation())]
		pub fn add_delegation(
			origin: OriginFor<T>,
			creator: TxProposerOf<T>,
			parent: IdentifierOf,
			delegate: CordAccountOf<T>,
			permissions: Permissions,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let parent_node =
				<Hierarchies<T>>::get(&parent).ok_or(Error::<T>::ParentDelegationNotFound)?;
			let hierarchy_root = parent_node.root.clone();

			ensure!(
				creator.signature.verify(&(&creator.digest).encode()[..], &creator.account),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(creator.digest).encode()[..], HIERARCHY_PREFIX)
					.into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Hierarchies<T>>::contains_key(&identifier),
				Error::<T>::HierarchyAlreadyExists
			);
			// Check if the parent's delegate is the creator of this delegation node...
			ensure!(
				parent_node.details.controller == creator.account,
				Error::<T>::NotOwnerOfParentDelegation
			);
			// ... and that the node has not been revoked...
			ensure!(!parent_node.details.revoked, Error::<T>::ParentDelegationRevoked);
			// ... and that has permission to delegate
			ensure!(
				(parent_node.details.permissions & Permissions::DELEGATE) == Permissions::DELEGATE,
				Error::<T>::UnauthorizedDelegation
			);

			Self::store_delegation_under_parent(
				identifier.clone(),
				HierarchyNode::new_node(
					hierarchy_root.clone(),
					parent.clone(),
					HierarchyDetails { controller: delegate.clone(), permissions, revoked: false },
				),
				parent.clone(),
				parent_node.clone(),
				author,
			)?;

			Self::deposit_event(Event::DelegationNodeCreated {
				creator: creator.account,
				root: hierarchy_root,
				parent,
				node: identifier,
				delegate,
				permissions,
			});

			Ok(())
		}

		/// Revoke a delegation node (potentially a root node) and all its
		/// children.
		#[pallet::weight(<T as Config>::WeightInfo::revoke_delegation_root_child(*max_revocations, *max_parent_checks)
				.max(<T as Config>::WeightInfo::revoke_delegation_leaf(*max_revocations, *max_parent_checks)))]
		pub fn revoke_delegation(
			origin: OriginFor<T>,
			revoker: TxProposerOf<T>,
			delegation: IdentifierOf,
			max_parent_checks: u32,
			max_revocations: u32,
		) -> DispatchResultWithPostInfo {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				revoker.signature.verify(&(&revoker.digest).encode()[..], &revoker.account),
				Error::<T>::InvalidSignature
			);

			ensure!(
				<Hierarchies<T>>::contains_key(&delegation),
				Error::<T>::DelegationNodeNotFound
			);

			ensure!(
				max_parent_checks <= T::MaxParentChecks::get(),
				Error::<T>::MaxParentChecksTooLarge
			);

			ensure!(
				max_revocations <= T::MaxRevocations::get(),
				Error::<T>::MaxRevocationsTooLarge
			);

			let (authorized, parent_checks) =
				Self::is_delegating(&revoker.account, &delegation, max_parent_checks)?;
			ensure!(authorized, Error::<T>::UnauthorizedRevocation);

			// Revoke the delegation and recursively all of its children (add 1 to
			// max_revocations to account for the node itself)
			let (revocation_checks, _) =
				Self::revoke(&delegation, &revoker.account, max_revocations.saturating_add(1))?;

			// If the revoked node is a root node, emit also a HierarchyRevoked event.
			if HierarchyNodes::<T>::contains_key(&delegation) {
				Self::deposit_event(Event::HierarchyRevoked {
					revoker: revoker.account,
					identifier: delegation,
				});
			}

			Ok(Some(
				<T as Config>::WeightInfo::revoke_delegation_root_child(
					revocation_checks,
					parent_checks,
				)
				.max(<T as Config>::WeightInfo::revoke_delegation_leaf(
					revocation_checks,
					parent_checks,
				)),
			)
			.into())
		}

		/// Remove a delegation node (potentially a root node) and all its
		/// children.
		#[pallet::weight(<T as Config>::WeightInfo::remove_delegation(*max_removals))]
		pub fn remove_delegation(
			origin: OriginFor<T>,
			remover: TxProposerOf<T>,
			delegation: IdentifierOf,
			max_removals: u32,
		) -> DispatchResultWithPostInfo {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				remover.signature.verify(&(&remover.digest).encode()[..], &remover.account),
				Error::<T>::InvalidSignature
			);

			let hierarchy =
				Hierarchies::<T>::get(&delegation).ok_or(Error::<T>::DelegationNodeNotFound)?;

			// Node can only be removed by owner of the node, not the parent or another
			// ancestor
			ensure!(
				hierarchy.details.controller == remover.account,
				Error::<T>::UnauthorizedRemoval
			);

			ensure!(max_removals <= T::MaxRemovals::get(), Error::<T>::MaxRemovalsTooLarge);

			// Remove the delegation and recursively all of its children (add 1 to
			// max_removals to account for the node itself)
			let (removal_checks, _) = Self::remove(&delegation, max_removals.saturating_add(1))?;

			// If the removed node is a root node, emit also a HierarchyRemoved event.
			if HierarchyNodes::<T>::take(&delegation).is_some() {
				Self::deposit_event(Event::HierarchyRemoved {
					remover: remover.account,
					identifier: delegation,
				});
			}

			Ok(Some(<T as Config>::WeightInfo::remove_delegation(removal_checks)).into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Creates a new root node with the given details
		pub(crate) fn create_and_store_new_hierarchy(
			identifier: IdentifierOf,
			elements: HierarchyElements,
			controller: CordAccountOf<T>,
			author: CordAccountOf<T>,
		) -> DispatchResult {
			let imbalance =
				<<T as pallet::Config>::Currency as Currency<CordAccountOf<T>>>::withdraw(
					&author,
					T::HierarchyFee::get(),
					WithdrawReasons::FEE,
					ExistenceRequirement::AllowDeath,
				)
				.map_err(|_| Error::<T>::UnableToPayFees)?;
			<T as pallet::Config>::FeeCollector::on_unbalanced(imbalance);

			let root_node = HierarchyNode::new_root_node(
				identifier.clone(),
				HierarchyDetails::default_with_controller(controller),
			);

			<Hierarchies<T>>::insert(identifier.clone(), root_node);
			<HierarchyNodes<T>>::insert(identifier, elements);

			Ok(())
		}

		// Adds the given node to the storage and updates the parent node to include the
		// given node as child.
		pub(crate) fn store_delegation_under_parent(
			delegation: IdentifierOf,
			delegation_node: HierarchyNode<T>,
			parent: IdentifierOf,
			mut parent_node: HierarchyNode<T>,
			author: CordAccountOf<T>,
		) -> DispatchResult {
			let imbalance =
				<<T as pallet::Config>::Currency as Currency<CordAccountOf<T>>>::withdraw(
					&author,
					T::NodeFee::get(),
					WithdrawReasons::FEE,
					ExistenceRequirement::AllowDeath,
				)
				.map_err(|_| Error::<T>::UnableToPayFees)?;
			<T as pallet::Config>::FeeCollector::on_unbalanced(imbalance);

			// Add the new node as a child of that node
			parent_node.try_add_child(delegation.clone())?;

			<Hierarchies<T>>::insert(delegation, delegation_node);
			<Hierarchies<T>>::insert(parent, parent_node);

			Ok(())
		}

		/// Check if an identity is the owner of the given delegation node or
		/// any node up the hierarchy, and if the delegation has not been yet
		/// revoked.
		pub fn is_delegating(
			identity: &CordAccountOf<T>,
			delegation: &IdentifierOf,
			max_parent_checks: u32,
		) -> Result<(bool, u32), DispatchError> {
			let delegation_node =
				<Hierarchies<T>>::get(delegation).ok_or(Error::<T>::DelegationNodeNotFound)?;

			// Check if the given account is the owner of the delegation and that the
			// delegation has not been revoked
			if &delegation_node.details.controller == identity {
				Ok((!delegation_node.details.revoked, 0u32))
			} else if let Some(parent) = delegation_node.parent {
				// Only decrease (and perhaps fail) remaining_lookups if there are more parents
				// to visit
				let remaining_lookups =
					max_parent_checks.checked_sub(1).ok_or(Error::<T>::MaxSearchDepthReached)?;

				// Recursively check upwards in hierarchy
				Self::is_delegating(identity, &parent, remaining_lookups)
			} else {
				// Return false and return max_parent_checks as no other check is performed
				Ok((false, max_parent_checks))
			}
		}

		/// Revokes all children of a delegation.
		fn revoke_children(
			delegation: &IdentifierOf,
			sender: &CordAccountOf<T>,
			max_revocations: u32,
		) -> Result<(u32, Weight), DispatchError> {
			let mut revocations: u32 = 0;
			let mut consumed_weight: Weight = Weight::zero();
			if let Some(delegation_node) = <Hierarchies<T>>::get(delegation) {
				// Iterate children and revoke all nodes
				for child in delegation_node.children.iter() {
					let remaining_revocations = max_revocations
						.checked_sub(revocations)
						.ok_or(Error::<T>::ExceededRevocationBounds)?;

					// Check whether we ran out of gas
					ensure!(remaining_revocations > 0, Error::<T>::ExceededRevocationBounds);

					Self::revoke(child, sender, remaining_revocations).map(|(r, w)| {
						revocations = revocations.saturating_add(r);
						consumed_weight = consumed_weight.saturating_add(w);
					})?;
				}
			}
			Ok((revocations, consumed_weight.saturating_add(T::DbWeight::get().reads(1))))
		}

		/// Revoke a delegation and all of its children recursively.
		fn revoke(
			delegation: &IdentifierOf,
			sender: &CordAccountOf<T>,
			max_revocations: u32,
		) -> Result<(u32, Weight), DispatchError> {
			let mut revocations: u32 = 0;
			let mut consumed_weight: Weight = Weight::zero();
			let delegate = delegation.clone();
			// Retrieve delegation node from storage
			let mut delegation_node =
				<Hierarchies<T>>::get(delegation).ok_or(Error::<T>::DelegationNodeNotFound)?;
			consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));

			// Check if already revoked
			if !delegation_node.details.revoked {
				// First revoke all children recursively
				let remaining_revocations =
					max_revocations.checked_sub(1).ok_or(Error::<T>::ExceededRevocationBounds)?;
				Self::revoke_children(delegation, sender, remaining_revocations).map(
					|(r, w)| {
						revocations = revocations.saturating_add(r);
						consumed_weight = consumed_weight.saturating_add(w);
					},
				)?;

				// If we run out of revocation gas, we only revoke children. The tree will be
				// changed but is still valid.
				ensure!(revocations < max_revocations, Error::<T>::ExceededRevocationBounds);

				// Set revoked flag and store delegation node
				delegation_node.details.revoked = true;
				<Hierarchies<T>>::insert(delegation.clone(), delegation_node);
				consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().writes(1));
				// Deposit event that the delegation has been revoked
				Self::deposit_event(Event::HierarchyRevoked {
					revoker: sender.clone(),
					identifier: delegate,
				});
				revocations = revocations.saturating_add(1);
			}
			Ok((revocations, consumed_weight))
		}

		/// Removes all children of a delegation.
		fn remove_children(
			delegation: &IdentifierOf,
			max_removals: u32,
		) -> Result<(u32, Weight), DispatchError> {
			let mut removals: u32 = 0;
			let mut consumed_weight: Weight = Weight::zero();

			// Can't clear storage until we have reached a leaf
			if let Some(mut delegation_node) = Hierarchies::<T>::get(delegation) {
				// Iterate and remove all children
				for child in delegation_node.clone().children.iter() {
					let remaining_removals = max_removals
						.checked_sub(removals)
						.ok_or(Error::<T>::ExceededRemovalBounds)?;

					// Check whether we ran out of gas
					ensure!(remaining_removals > 0, Error::<T>::ExceededRemovalBounds);

					Self::remove(child, remaining_removals).map(|(r, w)| {
						removals = removals.saturating_add(r);
						consumed_weight = consumed_weight.saturating_add(w);
					})?;

					// Remove child from set and update parent node in case of pre-emptive stops due
					// to insufficient removal gas
					delegation_node.children.remove(child);
					Hierarchies::<T>::insert(delegation, delegation_node.clone());
				}
			}
			Ok((removals, consumed_weight.saturating_add(T::DbWeight::get().reads(1))))
		}

		/// Remove a delegation and all of its children recursively.
		fn remove(
			delegation: &IdentifierOf,
			max_removals: u32,
		) -> Result<(u32, Weight), DispatchError> {
			let mut removals: u32 = 0;
			let mut consumed_weight: Weight = Weight::zero();
			let delegate = delegation.clone();

			// Retrieve delegation node from storage
			// Storage removal has to be postponed until children have been removed

			let delegation_node =
				Hierarchies::<T>::get(delegation).ok_or(Error::<T>::DelegationNodeNotFound)?;
			consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));

			// First remove all children recursively
			let remaining_removals =
				max_removals.checked_sub(1).ok_or(Error::<T>::ExceededRemovalBounds)?;
			Self::remove_children(delegation, remaining_removals).map(|(r, w)| {
				removals = removals.saturating_add(r);
				consumed_weight = consumed_weight.saturating_add(w);
			})?;

			// If we run out of removal gas, we only remove children. The tree will be
			// changed but is still valid.
			ensure!(removals < max_removals, Error::<T>::ExceededRemovalBounds);

			// We can clear storage now that all children have been removed
			Hierarchies::<T>::remove(delegation.clone());

			consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads_writes(1, 2));

			// Deposit event that the delegation has been removed
			Self::deposit_event(Event::HierarchyRemoved {
				remover: delegation_node.details.controller,
				identifier: delegate,
			});
			removals = removals.saturating_add(1);
			Ok((removals, consumed_weight))
		}
	}
}
