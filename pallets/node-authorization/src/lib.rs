// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Node authorization pallet
//!
//! This pallet manages a configurable set of nodes for a permissioned network.
//! Each node is dentified by a PeerId (i.e. `Vec<u8>`). It provides two ways to
//! authorize a node,
//!
//! - a set of well known nodes across different organizations in which the
//! connections are allowed.
//! - users can claim the ownership for each node, then manage the connections
//!   of
//! the node.
//!
//! A node must have an owner. The owner can additionally change the connections
//! for the node. Only one user is allowed to claim a specific node. To
//! eliminate false claim, the maintainer of the node should claim it before
//! even starting the node. This pallet uses offchain worker to set reserved
//! nodes, if the node is not an authority, make sure to enable offchain worker
//! with the right CLI flag. The node can be lagged with the latest block, in
//! this case you need to disable offchain worker and manually set reserved
//! nodes when starting it.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod types;
pub mod weights;

pub use crate::{pallet::*, types::*, weights::WeightInfo};
use base58::FromBase58;
use cord_primitives::NodeId;
use sp_core::OpaquePeerId as PeerId;
use sp_runtime::traits::StaticLookup;
use sp_std::{collections::btree_set::BTreeSet, iter::FromIterator, prelude::*};

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

	pub type NodeIdOf<T> = BoundedVec<u8, <T as Config>::MaxNodeIdLength>;

	/// Type for a schema entry
	pub type NodeInfoOf<T> = NodeInfo<NodeIdOf<T>, AccountIdOf<T>>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// The module configuration trait
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The maximum number of well known nodes that are allowed to set
		#[pallet::constant]
		type MaxWellKnownNodes: Get<u32>;

		/// The maximum length in bytes of PeerId
		#[pallet::constant]
		type MaxPeerIdLength: Get<u32>;

		/// The maximum length in bytes of PeerId
		#[pallet::constant]
		type MaxNodeIdLength: Get<u32>;

		/// The origin which can add a well known node.
		type NodeAuthorizationOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// The set of well known nodes. This is stored sorted (just by value).
	#[pallet::storage]
	#[pallet::getter(fn well_known_nodes)]
	pub type WellKnownNodes<T> = StorageValue<_, BTreeSet<PeerId>, ValueQuery>;

	/// A map that maintains the ownership of each node.
	#[pallet::storage]
	#[pallet::getter(fn owners)]
	pub type Owners<T: Config> = StorageMap<_, Blake2_128Concat, PeerId, NodeInfoOf<T>>;

	/// The additional adapative connections of each node.
	#[pallet::storage]
	#[pallet::getter(fn additional_connection)]
	pub type AdditionalConnections<T> =
		StorageMap<_, Blake2_128Concat, PeerId, BTreeSet<PeerId>, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub nodes: Vec<(PeerId, T::AccountId)>,
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize_nodes(&self.nodes);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The given well known node was added.
		NodeAdded { node_id: NodeId, who: T::AccountId },
		/// The given well known node was removed.
		NodeRemoved { node_id: NodeId },
		/// The given well known node was swapped; first item was removed,
		/// the latter was added.
		NodeSwapped { removed: NodeId, added: NodeId },
		/// The given well known nodes were reset.
		NodesReset { nodes: Vec<(PeerId, T::AccountId)> },
		/// The given node was claimed by a user.
		NodeClaimed { peer_id: PeerId, who: T::AccountId },
		/// The given claim was removed by its owner.
		ClaimRemoved { peer_id: PeerId, who: T::AccountId },
		/// The node was transferred to another account.
		NodeTransferred { node_id: NodeId, target: T::AccountId },
		/// The allowed connections were added to a node.
		ConnectionsAdded { node_id: NodeId, connection: NodeId },
		/// The allowed connections were removed from a node.
		ConnectionsRemoved { node_id: NodeId, connection: NodeId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The Node identifier is too long.
		NodeIdTooLong,
		/// The PeerId is too long.
		PeerIdTooLong,
		/// Too many well known nodes.
		TooManyNodes,
		/// The node is already joined in the list.
		AlreadyJoined,
		/// The node doesn't exist in the list.
		NotExist,
		/// The node is already claimed by a user.
		AlreadyClaimed,
		/// The node hasn't been claimed yet.
		NotClaimed,
		/// You are not the owner of the node.
		NotOwner,
		/// No permisson to perform specific operation.
		PermissionDenied,
		/// The Utf8 string is not proper.
		InvalidUtf8,
		/// The node identifier is not valid
		InvalidNodeIdentifier,
		/// The node is already connected.
		AlreadyConnected,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Set reserved node every block. It may not be enabled depends on the
		/// offchain worker settings when starting the node.
		fn offchain_worker(now: T::BlockNumber) {
			let network_state = sp_io::offchain::network_state();
			match network_state {
				Err(_) => log::error!(
					target: "runtime::node-authorization",
					"Error: failed to get network state of node at {:?}",
					now,
				),
				Ok(state) => {
					let encoded_peer = state.peer_id.0;
					match Decode::decode(&mut &encoded_peer[..]) {
						Err(_) => log::error!(
							target: "runtime::node-authorization",
							"Error: failed to decode PeerId at {:?}",
							now,
						),
						Ok(node) => sp_io::offchain::set_authorized_nodes(
							Self::get_authorized_nodes(&PeerId(node)),
							true,
						),
					}
				},
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a node to the set of well known nodes. If the node is already
		/// claimed, the owner will be updated and keep the existing additional
		/// connection unchanged.
		///
		/// May only be called from `T::AddOrigin`.
		///
		/// - `node`: identifier of the node.
		#[pallet::call_index(0)]
		#[pallet::weight((T::WeightInfo::add_well_known_node(), DispatchClass::Operational))]
		pub fn add_well_known_node(
			origin: OriginFor<T>,
			node_id: NodeId,
			owner: AccountIdLookupOf<T>,
		) -> DispatchResult {
			T::NodeAuthorizationOrigin::ensure_origin(origin)?;
			let owner = T::Lookup::lookup(owner)?;
			let node_id_bytes: BoundedVec<u8, T::MaxNodeIdLength> =
				BoundedVec::try_from(node_id.clone()).map_err(|_| Error::<T>::NodeIdTooLong)?;

			let node = Self::generate_peer_id(&node_id)?;
			ensure!(node.0.len() < T::MaxPeerIdLength::get() as usize, Error::<T>::PeerIdTooLong);

			let mut nodes = WellKnownNodes::<T>::get();
			ensure!(nodes.len() < T::MaxWellKnownNodes::get() as usize, Error::<T>::TooManyNodes);
			ensure!(!nodes.contains(&node), Error::<T>::AlreadyJoined);
			ensure!(!Owners::<T>::contains_key(&node), Error::<T>::AlreadyClaimed);

			nodes.insert(node.clone());

			WellKnownNodes::<T>::put(&nodes);
			<Owners<T>>::insert(&node, NodeInfoOf::<T> { id: node_id_bytes, owner: owner.clone() });

			Self::deposit_event(Event::NodeAdded { node_id, who: owner });
			Ok(())
		}

		/// Remove a node from the set of well known nodes. The ownership and
		/// additional connections of the node will also be removed.
		///
		/// May only be called from `T::RemoveOrigin`.
		///
		/// - `node`: identifier of the node.
		#[pallet::call_index(1)]
		#[pallet::weight((T::WeightInfo::remove_well_known_node(), DispatchClass::Operational))]
		pub fn remove_well_known_node(origin: OriginFor<T>, node_id: NodeId) -> DispatchResult {
			T::NodeAuthorizationOrigin::ensure_origin(origin)?;
			ensure!(node_id.len() < T::MaxNodeIdLength::get() as usize, Error::<T>::NodeIdTooLong);

			let node = Self::generate_peer_id(&node_id)?;
			ensure!(node.0.len() < T::MaxPeerIdLength::get() as usize, Error::<T>::PeerIdTooLong);

			let mut nodes = WellKnownNodes::<T>::get();
			ensure!(nodes.contains(&node), Error::<T>::NotExist);

			nodes.remove(&node);

			WellKnownNodes::<T>::put(&nodes);
			<Owners<T>>::remove(&node);
			AdditionalConnections::<T>::remove(&node);

			Self::deposit_event(Event::NodeRemoved { node_id });
			Ok(())
		}

		/// Swap a well known node to another. Both the ownership and additional
		/// connections stay untouched.
		///
		/// - `remove`: the node which will be moved out from the list.
		/// - `add`: the node which will be put in the list.
		#[pallet::call_index(2)]
		#[pallet::weight((T::WeightInfo::swap_well_known_node(), DispatchClass::Operational))]
		pub fn swap_well_known_node(
			origin: OriginFor<T>,
			remove_id: NodeId,
			add_id: NodeId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			ensure!(
				remove_id.len() < T::MaxNodeIdLength::get() as usize,
				Error::<T>::NodeIdTooLong
			);
			ensure!(add_id.len() < T::MaxNodeIdLength::get() as usize, Error::<T>::NodeIdTooLong);

			if remove_id == add_id {
				return Ok(())
			}

			let remove = Self::generate_peer_id(&remove_id)?;
			ensure!(remove.0.len() < T::MaxPeerIdLength::get() as usize, Error::<T>::PeerIdTooLong);

			let node_info = Owners::<T>::get(&remove).ok_or(Error::<T>::NotExist)?;
			ensure!(node_info.owner == sender, Error::<T>::NotOwner);

			let add = Self::generate_peer_id(&add_id)?;
			ensure!(add.0.len() < T::MaxPeerIdLength::get() as usize, Error::<T>::PeerIdTooLong);

			let mut nodes = WellKnownNodes::<T>::get();
			ensure!(nodes.contains(&remove), Error::<T>::NotExist);
			ensure!(!nodes.contains(&add), Error::<T>::AlreadyJoined);

			nodes.remove(&remove);
			nodes.insert(add.clone());

			WellKnownNodes::<T>::put(&nodes);
			Owners::<T>::swap(&remove, &add);
			AdditionalConnections::<T>::swap(&remove, &add);

			Self::deposit_event(Event::NodeSwapped { removed: remove_id, added: add_id });
			Ok(())
		}

		/// A node can be transferred to a new owner.
		///
		/// - `node`: identifier of the node.
		/// - `owner`: new owner of the node.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::transfer_node())]
		pub fn transfer_node(
			origin: OriginFor<T>,
			node_id: NodeId,
			owner: AccountIdLookupOf<T>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let owner = T::Lookup::lookup(owner)?;

			let node = Self::generate_peer_id(&node_id)?;

			ensure!(node.0.len() < T::MaxPeerIdLength::get() as usize, Error::<T>::PeerIdTooLong);
			let node_info = Owners::<T>::get(&node).ok_or(Error::<T>::NotClaimed)?;
			ensure!(node_info.owner == sender, Error::<T>::NotOwner);

			let node_id_bytes: BoundedVec<u8, T::MaxNodeIdLength> =
				BoundedVec::try_from(node_id.clone()).map_err(|_| Error::<T>::NodeIdTooLong)?;

			<Owners<T>>::insert(&node, NodeInfoOf::<T> { id: node_id_bytes, owner: owner.clone() });

			Self::deposit_event(Event::NodeTransferred { node_id, target: owner });
			Ok(())
		}

		/// Add additional connections to a given node.
		///
		/// - `node`: identifier of the node.
		/// - `connections`: additonal nodes from which the connections are
		///   allowed.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::add_connection())]
		pub fn add_connection(
			origin: OriginFor<T>,
			node_id: NodeId,
			connection_id: NodeId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let node = Self::generate_peer_id(&node_id)?;
			ensure!(node.0.len() < T::MaxPeerIdLength::get() as usize, Error::<T>::PeerIdTooLong);

			let node_info = Owners::<T>::get(&node).ok_or(Error::<T>::NotClaimed)?;
			ensure!(node_info.owner == sender, Error::<T>::NotOwner);

			let connection = Self::generate_peer_id(&connection_id)?;
			ensure!(
				connection.0.len() < T::MaxPeerIdLength::get() as usize,
				Error::<T>::PeerIdTooLong
			);

			let mut nodes = AdditionalConnections::<T>::get(&node);
			// Ensure the connection does not already exist
			ensure!(!nodes.contains(&connection), Error::<T>::AlreadyConnected);
			nodes.insert(connection);

			AdditionalConnections::<T>::insert(&node, nodes);

			Self::deposit_event(Event::ConnectionsAdded { node_id, connection: connection_id });
			Ok(())
		}

		/// Remove additional connections of a given node.
		///
		/// - `node`: identifier of the node.
		/// - `connections`: additonal nodes from which the connections are not
		///   allowed anymore.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::remove_connection())]
		pub fn remove_connection(
			origin: OriginFor<T>,
			node_id: NodeId,
			connection_id: NodeId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let node = Self::generate_peer_id(&node_id)?;
			ensure!(node.0.len() < T::MaxPeerIdLength::get() as usize, Error::<T>::PeerIdTooLong);
			let node_info = Owners::<T>::get(&node).ok_or(Error::<T>::NotClaimed)?;
			ensure!(node_info.owner == sender, Error::<T>::NotOwner);

			let connection = Self::generate_peer_id(&connection_id)?;
			ensure!(
				connection.0.len() < T::MaxPeerIdLength::get() as usize,
				Error::<T>::PeerIdTooLong
			);

			let mut nodes = AdditionalConnections::<T>::get(&node);
			ensure!(nodes.contains(&connection), Error::<T>::NotExist);
			nodes.remove(&connection);

			AdditionalConnections::<T>::insert(&node, nodes);

			Self::deposit_event(Event::ConnectionsRemoved { node_id, connection: connection_id });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn initialize_nodes(nodes: &[(PeerId, T::AccountId)]) {
		let peer_ids = nodes.iter().map(|item| item.0.clone()).collect::<BTreeSet<PeerId>>();

		WellKnownNodes::<T>::put(&peer_ids);

		for (node, who) in nodes.iter() {
			let base58_str = bs58::encode(node.0.clone()).into_string();
			if let Ok(node_id_bytes) = frame_support::BoundedVec::try_from(base58_str.into_bytes())
			{
				<Owners<T>>::insert(
					node,
					NodeInfoOf::<T> { id: node_id_bytes, owner: who.clone() },
				);
			} else {
				log::error!("Node ownership update failed!")
			}
		}
	}

	fn get_authorized_nodes(node: &PeerId) -> Vec<PeerId> {
		let mut nodes = AdditionalConnections::<T>::get(node);

		let mut well_known_nodes = WellKnownNodes::<T>::get();
		if well_known_nodes.contains(node) {
			well_known_nodes.remove(node);
			nodes.extend(well_known_nodes);
		}

		Vec::from_iter(nodes)
	}

	fn generate_peer_id(node_identity: &NodeId) -> Result<PeerId, Error<T>> {
		log::info!("Incoming Vec: {:?}", node_identity);
		let encoded = sp_std::str::from_utf8(node_identity).map_err(|_| Error::<T>::InvalidUtf8)?;
		log::info!("Incoming Vec String: {:?}", encoded);
		let decoded = bs58::decode(encoded)
			.into_vec()
			.map_err(|_| Error::<T>::InvalidNodeIdentifier)?;

		let node = PeerId(decoded);

		Ok(node)
	}
}
