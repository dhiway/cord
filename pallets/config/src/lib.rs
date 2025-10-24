// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! # Network Config Information

#![warn(unused_extern_crates)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::{str, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use cord_primitives::{Id as NetworkId, NetworkInfoProvider};
use cord_uri::{EntryTypeOf, EventStamp, Identifier, Ss58Identifier};
use fluent_uri::Uri;
use frame_support::{
	pallet_prelude::*,
	traits::{Get, StorageVersion},
	BoundedVec,
};
use frame_system::pallet_prelude::{BlockNumberFor, *};
use sp_runtime::traits::{Hash, Zero};

pub use pallet::*;

/// Identifier
pub type IdentifierOf = Ss58Identifier;
pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
pub type HashOf<T> = <T as frame_system::Config>::Hash;
pub(crate) type NetworkName = BoundedVec<u8, ConstU32<64>>;
pub(crate) type DataNodeId = BoundedVec<u8, ConstU32<60>>;
pub(crate) type NetworkEndpoints = BoundedVec<BoundedVec<u8, ConstU32<256>>, ConstU32<50>>;
pub(crate) type NetworkWebsite = Option<BoundedVec<u8, ConstU32<256>>>;
pub(crate) type NetworkToken = BoundedVec<u8, ConstU32<142>>;

#[frame_support::pallet]
pub mod pallet {

	use super::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + cord_uri::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type NetworkConfigOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		#[pallet::constant]
		type DefaultNetworkId: Get<u32>;
	}

	#[derive(
		Encode,
		Decode,
		DecodeWithMemTracking,
		Clone,
		PartialEq,
		Eq,
		RuntimeDebug,
		TypeInfo,
		Default,
		MaxEncodedLen,
	)]
	pub struct NetworkInfo<NetworkName, NetworkEndpoints, NetworkWebSite, NetworkToken, Account> {
		pub name: NetworkName,
		pub endpoints: NetworkEndpoints,
		pub website: NetworkWebSite,
		pub token: NetworkToken,
		pub owner: Account,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The identifier activity update failed
		ActivityUpdateFailed,
		/// The network information was not found.
		NetworkInfoNotFound,
		/// The provided input is invalid or malformed.
		InvalidInput,
		/// The provided entry input is invalid or malformed.
		InvalidEntryTypeInput,
		/// The provided URI is invalid or cannot be parsed.
		InvalidUri,
		/// The identifier length is invalid or exceeds the maximum allowed.
		InvalidIdentifierLength,
		/// The network configuration has already been added.
		NetworkConfigAlreadyAdded,
		/// The storage configuration has already been added.
		StorageConfigAlreadyAdded,
		/// The storage configuration not found.
		StorageConfigNotFound,
		/// The provided token is invalid.
		InvalidToken,
		/// The cord genesis head is invalid or corrupted.
		InvalidCordGenesisHead,
		/// The network genesis head is invalid or corrupted.
		InvalidNetworkGenesisHead,
		/// The provided account ID is invalid or not recognized.
		InvalidAccountId,
		/// The checksum of the token is invalid or does not match.
		InvalidChecksum,
		/// The prefix in the provided token is invalid.
		InvalidPrefix,
		/// The network ID is invalid or not recognized.
		InvalidNetworkId,
		/// The origin of the operation is not authorized or invalid.
		Badorigin,
	}

	#[pallet::storage]
	pub type NetworkConfigInfo<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		NetworkId,
		NetworkInfo<NetworkName, NetworkEndpoints, NetworkWebsite, NetworkToken, CordAccountOf<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub(super) type NetworkIdentifier<T: Config> = StorageValue<_, NetworkId, ValueQuery>;

	// Stores the network configuration type
	#[pallet::storage]
	pub type NetworkPermissioned<T> = StorageValue<_, bool, ValueQuery>;

	#[pallet::storage]
	pub type StorageNodeConfigInfo<T> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		(DataNodeId, CordAccountOf<T>, bool),
		OptionQuery,
	>;

	#[pallet::storage]
	pub type StorageNodes<T> =
		StorageMap<_, Blake2_128Concat, DataNodeId, (IdentifierOf, CordAccountOf<T>), OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Network Info added.
		NetworkInfo {
			network: NetworkId,
			manager: CordAccountOf<T>,
		},
		NetworkNameUpdate {
			network: NetworkId,
		},
		NetworkRpcUpdate {
			network: NetworkId,
		},
		NetworkWebUpdate {
			network: NetworkId,
		},
		NetworkRpcRemove {
			network: NetworkId,
		},
		StorageNodeAdded {
			identifier: IdentifierOf,
			node: DataNodeId,
		},
		StorageNodeUpdated {
			identifier: IdentifierOf,
			node: DataNodeId,
		},
		StorageNodeRemoved {
			identifier: IdentifierOf,
			node: DataNodeId,
		},
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		#[serde(skip)]
		pub _config: core::marker::PhantomData<T>,
		pub permissioned: bool,
		pub network_id: NetworkId,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				network_id: NetworkId::from(1000u32),
				permissioned: false,
				_config: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			NetworkIdentifier::<T>::put(self.network_id);
			NetworkPermissioned::<T>::put(&self.permissioned);
			cord_uri::Pallet::<T>::set_network_id(self.network_id);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight({200_000})]
		pub fn network_info(
			origin: OriginFor<T>,
			name: Vec<u8>,
			endpoints: Vec<Vec<u8>>,
			website: Option<Vec<u8>>,
			token: Vec<u8>,
		) -> DispatchResult {
			T::NetworkConfigOrigin::ensure_origin(origin)?;

			let network_id = NetworkIdentifier::<T>::get();
			ensure!(
				!NetworkConfigInfo::<T>::contains_key(&network_id),
				Error::<T>::NetworkConfigAlreadyAdded
			);

			let (t_network_id, _t_cord_genesis_hash, t_network_genesis_hash, t_account_id) =
				Self::resolve(&token)?;
			let genesis_hash = <frame_system::Pallet<T>>::block_hash(BlockNumberFor::<T>::zero());
			ensure!(t_network_genesis_hash == genesis_hash, Error::<T>::InvalidNetworkGenesisHead);
			ensure!(t_network_id == network_id, Error::<T>::InvalidNetworkId);

			let name_bounded = BoundedVec::<u8, ConstU32<64>>::try_from(name)
				.map_err(|_| Error::<T>::InvalidInput)?;
			let token_bounded = BoundedVec::<u8, ConstU32<142>>::try_from(token)
				.map_err(|_| Error::<T>::InvalidInput)?;

			let rpc_endpoints_vec: Vec<BoundedVec<u8, ConstU32<256>>> = endpoints
				.into_iter()
				.map(|uri| {
					let uri_bounded = BoundedVec::<u8, ConstU32<256>>::try_from(uri)
						.map_err(|_| Error::<T>::InvalidInput)?;
					if Self::validate_rpc_bounded_uri(&uri_bounded) {
						Ok(uri_bounded)
					} else {
						Err(Error::<T>::InvalidUri)
					}
				})
				.collect::<Result<Vec<BoundedVec<u8, ConstU32<256>>>, _>>()?;

			let rpc_endpoints = BoundedVec::<_, ConstU32<50>>::try_from(rpc_endpoints_vec)
				.map_err(|_| Error::<T>::InvalidInput)?;

			let website = match website {
				Some(uri) => {
					let bounded_home = BoundedVec::<u8, ConstU32<256>>::try_from(uri)
						.map_err(|_| Error::<T>::InvalidInput)?;
					if Self::validate_http_bounded_uri(&bounded_home) {
						Some(bounded_home)
					} else {
						return Err(Error::<T>::InvalidUri.into());
					}
				},
				None => None,
			};

			let network_info = NetworkInfo {
				name: name_bounded,
				endpoints: rpc_endpoints,
				website,
				token: token_bounded,
				owner: t_account_id.clone(),
			};

			NetworkConfigInfo::<T>::insert(network_id.clone(), network_info.clone());

			Self::deposit_event(Event::NetworkInfo { network: network_id, manager: t_account_id });

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight({200_000})]
		pub fn update_token(origin: OriginFor<T>, token: Vec<u8>) -> DispatchResult {
			T::NetworkConfigOrigin::ensure_origin(origin)?;

			let network_id = NetworkIdentifier::<T>::get();
			let mut network_info =
				NetworkConfigInfo::<T>::get(network_id).ok_or(Error::<T>::NetworkInfoNotFound)?;

			let (t_network_id, _t_cord_genesis_hash, t_network_genesis_hash, t_account_id) =
				Self::resolve(&token)?;
			// let pallet_name = <Self as PalletInfoAccess>::name();
			let genesis_hash = <frame_system::Pallet<T>>::block_hash(BlockNumberFor::<T>::zero());
			ensure!(t_account_id == network_info.owner, Error::<T>::InvalidNetworkGenesisHead);
			ensure!(t_network_genesis_hash == genesis_hash, Error::<T>::InvalidNetworkGenesisHead);
			ensure!(t_network_id == network_id, Error::<T>::InvalidNetworkId);

			let bounded_token = BoundedVec::<u8, ConstU32<142>>::try_from(token)
				.map_err(|_| Error::<T>::InvalidInput)?;
			network_info.token = bounded_token.clone();

			NetworkConfigInfo::<T>::insert(&network_id, network_info);

			Self::deposit_event(Event::NetworkNameUpdate { network: network_id.clone() });

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight({200_000})]
		pub fn update_name(origin: OriginFor<T>, name: Vec<u8>) -> DispatchResult {
			T::NetworkConfigOrigin::ensure_origin(origin)?;

			let network_id = NetworkIdentifier::<T>::get();
			let mut network_info =
				NetworkConfigInfo::<T>::get(network_id).ok_or(Error::<T>::NetworkInfoNotFound)?;

			let bounded_name = BoundedVec::<u8, ConstU32<64>>::try_from(name)
				.map_err(|_| Error::<T>::InvalidInput)?;
			network_info.name = bounded_name.clone();

			NetworkConfigInfo::<T>::insert(&network_id, network_info);

			Self::deposit_event(Event::NetworkNameUpdate { network: network_id.clone() });

			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight({200_000})]
		pub fn update_rpc_endpoints(
			origin: OriginFor<T>,
			endpoints: Vec<Vec<u8>>,
		) -> DispatchResult {
			T::NetworkConfigOrigin::ensure_origin(origin)?;

			let network_id = NetworkIdentifier::<T>::get();

			let mut network_info =
				NetworkConfigInfo::<T>::get(&network_id).ok_or(Error::<T>::NetworkInfoNotFound)?;

			let rpc_endpoints_vec: Vec<BoundedVec<u8, ConstU32<256>>> = endpoints
				.into_iter()
				.map(|uri| {
					let uri_bounded = BoundedVec::<u8, ConstU32<256>>::try_from(uri)
						.map_err(|_| Error::<T>::InvalidInput)?;
					if Self::validate_rpc_bounded_uri(&uri_bounded) {
						Ok(uri_bounded)
					} else {
						Err(Error::<T>::InvalidUri)
					}
				})
				.collect::<Result<Vec<BoundedVec<u8, ConstU32<256>>>, _>>()?;

			network_info.endpoints = BoundedVec::<_, ConstU32<50>>::try_from(rpc_endpoints_vec)
				.map_err(|_| Error::<T>::InvalidInput)?;

			NetworkConfigInfo::<T>::insert(&network_id, network_info);

			Self::deposit_event(Event::NetworkRpcUpdate { network: network_id.clone() });

			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight({200_000})]
		pub fn update_website(origin: OriginFor<T>, website: Option<Vec<u8>>) -> DispatchResult {
			T::NetworkConfigOrigin::ensure_origin(origin)?;

			let network_id = NetworkIdentifier::<T>::get();

			let mut network_info =
				NetworkConfigInfo::<T>::get(&network_id).ok_or(Error::<T>::NetworkInfoNotFound)?;

			network_info.website = match website {
				Some(uri) => {
					let bounded_web = BoundedVec::<u8, ConstU32<256>>::try_from(uri)
						.map_err(|_| Error::<T>::InvalidInput)?;
					if Self::validate_http_bounded_uri(&bounded_web) {
						Some(bounded_web.clone())
					} else {
						return Err(Error::<T>::InvalidUri.into());
					}
				},
				None => None,
			};

			NetworkConfigInfo::<T>::insert(&network_id, network_info);

			Self::deposit_event(Event::NetworkWebUpdate { network: network_id.clone() });

			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight({200_000})]
		pub fn remove_rpc_endpoint(origin: OriginFor<T>, endpoint: Vec<u8>) -> DispatchResult {
			T::NetworkConfigOrigin::ensure_origin(origin)?;

			let network_id = NetworkIdentifier::<T>::get();

			let mut network_info =
				NetworkConfigInfo::<T>::get(&network_id).ok_or(Error::<T>::NetworkInfoNotFound)?;

			ensure!(network_info.endpoints.len() > 1, Error::<T>::InvalidInput);

			let uri_bounded = BoundedVec::<u8, ConstU32<256>>::try_from(endpoint)
				.map_err(|_| Error::<T>::InvalidInput)?;

			network_info.endpoints.retain(|uri| uri != &uri_bounded);

			NetworkConfigInfo::<T>::insert(&network_id, network_info);

			Self::deposit_event(Event::NetworkRpcRemove { network: network_id.clone() });

			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight({200_000})]
		pub fn add_storage_node(
			origin: OriginFor<T>,
			node_id: Vec<u8>,
			author: CordAccountOf<T>,
		) -> DispatchResult {
			T::NetworkConfigOrigin::ensure_origin(origin)?;

			let bounded_node_id = BoundedVec::<u8, ConstU32<60>>::try_from(node_id)
				.map_err(|_| Error::<T>::InvalidInput)?;

			let pallet_name = <Self as PalletInfoAccess>::name();
			let digest = <T as frame_system::Config>::Hashing::hash(
				&[&bounded_node_id.encode()[..], &author.encode()[..]].concat()[..],
			);

			let identifier =
				<cord_uri::Pallet<T> as Identifier>::build(&(digest).encode()[..], pallet_name)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!StorageNodeConfigInfo::<T>::contains_key(&identifier),
				Error::<T>::StorageConfigAlreadyAdded
			);

			let entry: EntryTypeOf = b"StorageNodeAdded"
				.to_vec()
				.try_into()
				.map_err(|_| Error::<T>::InvalidEntryTypeInput)?;
			let stamp = EventStamp::current::<T>();

			<cord_uri::Pallet<T> as Identifier>::record_activity(&identifier, entry, stamp)
				.map_err(|_| Error::<T>::ActivityUpdateFailed)?;

			StorageNodeConfigInfo::<T>::insert(&identifier, (&bounded_node_id, &author, true));
			StorageNodes::<T>::insert(&bounded_node_id, (&identifier, &author));

			Self::deposit_event(Event::StorageNodeAdded {
				identifier,
				node: bounded_node_id.clone(),
			});

			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight({200_000})]
		pub fn update_storage_node_info(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			node_id: Option<Vec<u8>>,
			author: Option<CordAccountOf<T>>,
		) -> DispatchResult {
			T::NetworkConfigOrigin::ensure_origin(origin)?;

			ensure!(node_id.is_some() || author.is_some(), Error::<T>::InvalidInput);

			let (existing_node_id, existing_author, _active) =
				StorageNodeConfigInfo::<T>::get(&identifier)
					.ok_or(Error::<T>::StorageConfigNotFound)?;

			let mut updated_node_id = existing_node_id.clone();

			if let Some(ref new_node_id) = node_id {
				let bounded_node_id = BoundedVec::<u8, ConstU32<60>>::try_from(new_node_id.clone())
					.map_err(|_| Error::<T>::InvalidInput)?;
				updated_node_id = bounded_node_id;

				ensure!(
					!StorageNodes::<T>::contains_key(&updated_node_id),
					Error::<T>::StorageConfigAlreadyAdded
				);
			}

			let updated_author = author.unwrap_or(existing_author);

			let entry: EntryTypeOf = b"StorageNodeUpdated"
				.to_vec()
				.try_into()
				.map_err(|_| Error::<T>::InvalidEntryTypeInput)?;
			let stamp = EventStamp::current::<T>();

			<cord_uri::Pallet<T> as Identifier>::record_activity(&identifier, entry, stamp)
				.map_err(|_| Error::<T>::ActivityUpdateFailed)?;

			StorageNodeConfigInfo::<T>::insert(
				&identifier,
				(&updated_node_id, &updated_author, true),
			);
			StorageNodes::<T>::insert(&updated_node_id, (&identifier, &updated_author));

			if node_id.is_some() && existing_node_id != updated_node_id {
				StorageNodes::<T>::remove(&existing_node_id);
			}

			Self::deposit_event(Event::StorageNodeUpdated {
				identifier,
				node: updated_node_id.clone(),
			});

			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight({200_000})]
		pub fn remove_storage_node(origin: OriginFor<T>, node_id: Vec<u8>) -> DispatchResult {
			T::NetworkConfigOrigin::ensure_origin(origin)?;

			let bounded_node_id = BoundedVec::<u8, ConstU32<60>>::try_from(node_id)
				.map_err(|_| Error::<T>::InvalidInput)?;

			let (identifier, _author) = StorageNodes::<T>::get(&bounded_node_id)
				.ok_or(Error::<T>::StorageConfigNotFound)?;

			let (existing_node_id, existing_author, _active) =
				StorageNodeConfigInfo::<T>::get(&identifier)
					.ok_or(Error::<T>::StorageConfigNotFound)?;

			let entry: EntryTypeOf = b"StorageNodeRemoved"
				.to_vec()
				.try_into()
				.map_err(|_| Error::<T>::InvalidEntryTypeInput)?;
			let stamp = EventStamp::current::<T>();

			<cord_uri::Pallet<T> as Identifier>::record_activity(&identifier, entry, stamp)
				.map_err(|_| Error::<T>::ActivityUpdateFailed)?;

			StorageNodes::<T>::remove(&bounded_node_id);

			StorageNodeConfigInfo::<T>::insert(
				&identifier,
				(&existing_node_id, &existing_author, false),
			);

			Self::deposit_event(Event::StorageNodeRemoved {
				identifier,
				node: bounded_node_id.clone(),
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn validate_rpc_uri(uri: &str) -> bool {
			Uri::parse(uri)
				.map(|u| {
					let scheme = u.scheme().as_str();
					matches!(scheme, "ws" | "wss")
				})
				.unwrap_or(false)
		}

		pub(crate) fn validate_http_uri(uri: &str) -> bool {
			Uri::parse(uri)
				.map(|u| {
					let scheme = u.scheme().as_str();
					matches!(scheme, "http" | "https")
				})
				.unwrap_or(false)
		}

		pub(crate) fn validate_rpc_bounded_uri(uri: &BoundedVec<u8, ConstU32<256>>) -> bool {
			if let Ok(uri_str) = str::from_utf8(uri) {
				Self::validate_rpc_uri(uri_str)
			} else {
				false
			}
		}

		pub(crate) fn validate_http_bounded_uri(uri: &BoundedVec<u8, ConstU32<256>>) -> bool {
			if let Ok(uri_str) = str::from_utf8(uri) {
				Self::validate_http_uri(uri_str)
			} else {
				false
			}
		}

		pub fn resolve(
			token: &Vec<u8>,
		) -> Result<(NetworkId, HashOf<T>, HashOf<T>, CordAccountOf<T>), Error<T>> {
			// Decode the Base58-encoded token
			let decoded = bs58::decode(&token).into_vec().map_err(|_| Error::<T>::InvalidToken)?;

			ensure!(decoded.len() >= 2 && decoded.len() <= 142, Error::<T>::InvalidToken);

			let (ident, mut offset) = Self::compact_decode(&decoded)?;
			ensure!(ident == 8381 || ident == 2969, Error::<T>::InvalidToken);

			let cord_genesis_hash_end = offset + 32;
			ensure!(cord_genesis_hash_end <= decoded.len(), Error::<T>::InvalidCordGenesisHead);
			let cord_genesis_hash_bytes = &decoded[offset..cord_genesis_hash_end];
			let cord_genesis_hash = HashOf::<T>::decode(&mut &*cord_genesis_hash_bytes)
				.map_err(|_| Error::<T>::InvalidCordGenesisHead)?;
			offset = cord_genesis_hash_end;

			let nid_offset = offset + 4;
			let nid = u32::from_le_bytes(
				decoded[offset..nid_offset]
					.try_into()
					.map_err(|_| Error::<T>::InvalidNetworkId)?,
			);
			// let (nid, nid_offset) = Self::compact_decode(&decoded[offset..])?;
			offset = nid_offset;

			let network_genesis_hash_end = offset + 32;
			ensure!(
				network_genesis_hash_end <= decoded.len(),
				Error::<T>::InvalidNetworkGenesisHead
			);
			let network_genesis_hash_bytes = &decoded[offset..network_genesis_hash_end];
			let network_genesis_hash = HashOf::<T>::decode(&mut &*network_genesis_hash_bytes)
				.map_err(|_| Error::<T>::InvalidNetworkGenesisHead)?;
			offset = network_genesis_hash_end;

			// Extract `account_id` (32 bytes)
			let account_id_end = offset + 32;
			ensure!(account_id_end <= decoded.len(), Error::<T>::InvalidAccountId);
			let account_id_bytes = &decoded[offset..account_id_end];
			let account_id = CordAccountOf::<T>::decode(&mut &*account_id_bytes)
				.map_err(|_| Error::<T>::InvalidAccountId)?;

			// Verify checksum
			let checksum = &decoded[decoded.len() - 2..];
			let expected_checksum = &Self::checksum(&decoded[..decoded.len() - 2])[..2];
			ensure!(checksum == expected_checksum, Error::<T>::InvalidChecksum);

			Ok((NetworkId::from(nid), cord_genesis_hash, network_genesis_hash, account_id))
		}

		fn compact_decode(data: &[u8]) -> Result<(u16, usize), Error<T>> {
			match data[0] {
				0..=63 => Ok((data[0] as u16, 1)),
				64..=127 => {
					ensure!(data.len() >= 2, Error::<T>::InvalidPrefix);
					let lower = (data[0] << 2) | (data[1] >> 6);
					let upper = data[1] & 0b0011_1111;
					Ok(((lower as u16) | ((upper as u16) << 8), 2))
				},
				_ => Err(Error::<T>::InvalidPrefix),
			}
		}

		fn checksum(data: &[u8]) -> Vec<u8> {
			use blake2::{Blake2b512, Digest};
			const PREFIX: &[u8] = b"NIDV01";

			let mut hasher = Blake2b512::new();
			hasher.update(PREFIX);
			hasher.update(data);
			hasher.finalize().to_vec()
		}
	}
}

impl<T: Config> NetworkInfoProvider for Pallet<T> {
	fn get_network_id() -> NetworkId {
		NetworkIdentifier::<T>::get()
	}
}
impl<T: Config> Pallet<T> {
	/// check if the network is permissioned
	pub fn is_permissioned() -> bool {
		NetworkPermissioned::<T>::get()
	}
}

impl<T: Config> cord_primitives::IsPermissioned for Pallet<T> {
	fn is_permissioned() -> bool {
		Self::is_permissioned()
	}
}

pub trait StorageNodeInterface {
	type AccountId;
	type NodeId;
	type Identifier;

	/// Get the details of a storage node by its `node_id`.
	fn get_storage_node_details(
		node_id: Self::NodeId,
	) -> Option<(Self::Identifier, Self::AccountId, bool)>;

	/// Get the details of a storage node by its `identifier`.
	fn get_storage_node_details_by_identifier(
		identifier: Self::Identifier,
	) -> Option<(Self::NodeId, Self::AccountId, bool)>;

	/// Check if a storage node is active by its `node_id`.
	fn is_storage_node_active(node_id: Self::NodeId) -> bool;

	/// Check if a storage node is active by its `identifier`.
	fn is_storage_node_active_by_identifier(identifier: Self::Identifier) -> bool;
}

impl<T: Config> StorageNodeInterface for Pallet<T> {
	type AccountId = T::AccountId;
	type NodeId = BoundedVec<u8, ConstU32<60>>;
	type Identifier = IdentifierOf;

	/// Get the details of a storage node by its `node_id`.
	fn get_storage_node_details(
		node_id: Self::NodeId,
	) -> Option<(Self::Identifier, Self::AccountId, bool)> {
		if let Some((identifier, author)) = StorageNodes::<T>::get(&node_id) {
			if let Some((_, _, active)) = StorageNodeConfigInfo::<T>::get(&identifier) {
				return Some((identifier, author, active));
			}
		}
		None
	}
	/// Get the details of a storage node by its `identifier`.
	fn get_storage_node_details_by_identifier(
		identifier: Self::Identifier,
	) -> Option<(Self::NodeId, Self::AccountId, bool)> {
		StorageNodeConfigInfo::<T>::get(&identifier)
	}

	/// Check if a storage node is active by its `node_id`.
	fn is_storage_node_active(node_id: Self::NodeId) -> bool {
		if let Some((identifier, _)) = StorageNodes::<T>::get(&node_id) {
			if let Some((_, _, active)) = StorageNodeConfigInfo::<T>::get(&identifier) {
				return active;
			}
		}
		false
	}

	/// Check if a storage node is active by its `identifier`.
	fn is_storage_node_active_by_identifier(identifier: Self::Identifier) -> bool {
		if let Some((_, _, active)) = StorageNodeConfigInfo::<T>::get(&identifier) {
			return active;
		}
		false
	}
}
