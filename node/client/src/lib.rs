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

//! Cord Client
//!
//! Provides the [`AbstractClient`] trait that is a super trait that combines all the traits the client implements.
//! There is also the [`Client`] enum that combines all the different clients into one common structure.

use cord_primitives::{AccountId, Balance, Block, BlockNumber, DidIdentifier, Hash, Header, Index};
use sc_client_api::{
	AuxStore, Backend as BackendT, BlockchainEvents, KeysIter, PairsIter, UsageProvider,
};
use sc_executor::NativeElseWasmExecutor;
use sp_api::{CallApiAt, Encode, NumberFor, ProvideRuntimeApi};
use sp_blockchain::{HeaderBackend, HeaderMetadata};
use sp_consensus::BlockStatus;
use sp_runtime::{
	generic::SignedBlock,
	traits::{BlakeTwo256, Block as BlockT},
	Justifications,
};
use sp_storage::{ChildInfo, StorageData, StorageKey};
use std::sync::Arc;

pub mod benchmarking;

pub type FullBackend = sc_service::TFullBackend<Block>;

pub type FullClient<RuntimeApi, ExecutorDispatch> =
	sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;

/// The native executor instance for Cord.
#[cfg(feature = "cord")]
pub struct CordExecutorDispatch;

#[cfg(feature = "cord")]
impl sc_executor::NativeExecutionDispatch for CordExecutorDispatch {
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		cord_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		cord_runtime::native_version()
	}
}

/// A set of APIs that runtimes must implement.
pub trait RuntimeApiCollection:
	sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
	+ sp_api::ApiExt<Block>
	+ sp_consensus_babe::BabeApi<Block>
	+ sp_consensus_grandpa::GrandpaApi<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
	+ pallet_did_runtime_api::Did<Block, DidIdentifier, Hash, BlockNumber>
	+ sp_api::Metadata<Block>
	+ sp_offchain::OffchainWorkerApi<Block>
	+ sp_session::SessionKeys<Block>
	+ sp_authority_discovery::AuthorityDiscoveryApi<Block>
where
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
{
}

impl<Api> RuntimeApiCollection for Api
where
	Api: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::ApiExt<Block>
		+ sp_consensus_babe::BabeApi<Block>
		+ sp_consensus_grandpa::GrandpaApi<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ pallet_did_runtime_api::Did<Block, DidIdentifier, Hash, BlockNumber>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_authority_discovery::AuthorityDiscoveryApi<Block>,
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
{
}

/// Trait that abstracts over all available client implementations.
///
/// For a concrete type there exists [`Client`].
pub trait AbstractClient<Block, Backend>:
	BlockchainEvents<Block>
	+ Sized
	+ Send
	+ Sync
	+ ProvideRuntimeApi<Block>
	+ HeaderBackend<Block>
	+ CallApiAt<Block, StateBackend = Backend::State>
	+ AuxStore
	+ UsageProvider<Block>
	+ HeaderMetadata<Block, Error = sp_blockchain::Error>
where
	Block: BlockT,
	Backend: BackendT<Block>,
	Backend::State: sp_api::StateBackend<BlakeTwo256>,
	Self::Api: RuntimeApiCollection<StateBackend = Backend::State>,
{
}

impl<Block, Backend, Client> AbstractClient<Block, Backend> for Client
where
	Block: BlockT,
	Backend: BackendT<Block>,
	Backend::State: sp_api::StateBackend<BlakeTwo256>,
	Client: BlockchainEvents<Block>
		+ ProvideRuntimeApi<Block>
		+ HeaderBackend<Block>
		+ AuxStore
		+ UsageProvider<Block>
		+ Sized
		+ Send
		+ Sync
		+ CallApiAt<Block, StateBackend = Backend::State>
		+ HeaderMetadata<Block, Error = sp_blockchain::Error>,
	Client::Api: RuntimeApiCollection<StateBackend = Backend::State>,
{
}

/// Execute something with the client instance.
pub trait ExecuteWithClient {
	/// The return type when calling this instance.
	type Output;

	/// Execute whatever should be executed with the given client instance.
	fn execute_with_client<Client, Api, Backend>(self, client: Arc<Client>) -> Self::Output
	where
		<Api as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
		Backend: sc_client_api::Backend<Block> + 'static,
		Backend::State: sp_api::StateBackend<BlakeTwo256>,
		Api: crate::RuntimeApiCollection<StateBackend = Backend::State>,
		Client: AbstractClient<Block, Backend, Api = Api> + 'static;
}

/// A handle to Cord client instance.
pub trait ClientHandle {
	/// Execute the given something with the client.
	fn execute_with<T: ExecuteWithClient>(&self, t: T) -> T::Output;
}

/// Unwraps a [`Client`] into the concrete client type and
/// provides the concrete runtime as `runtime`.
macro_rules! with_client {
	{
		// The client instance that should be unwrapped.
		$self:expr,
		// The name that the unwrapped client will have.
		$client:ident,
		// NOTE: Using an expression here is fine since blocks are also expressions.
		$code:expr
	} => {
		match $self {
				#[cfg(feature = "cord")]

			Client::Cord($client) => {
				#[allow(unused_imports)]
				use cord_runtime as runtime;

				$code
			},
		}
	}
}
// Make the macro available only within this crate.
pub(crate) use with_client;

/// A client instance of Cord.
///
/// See [`ExecuteWithClient`] for more information.
#[derive(Clone)]
pub enum Client {
	#[cfg(feature = "cord")]
	Cord(Arc<FullClient<cord_runtime::RuntimeApi, CordExecutorDispatch>>),
}

impl ClientHandle for Client {
	fn execute_with<T: ExecuteWithClient>(&self, t: T) -> T::Output {
		with_client! {
			self,
			client,
			{
				T::execute_with_client::<_, _, FullBackend>(t, client.clone())
			}
		}
	}
}

impl UsageProvider<Block> for Client {
	fn usage_info(&self) -> sc_client_api::ClientInfo<Block> {
		with_client! {
			self,
			client,
			{
				client.usage_info()
			}
		}
	}
}

impl sc_client_api::BlockBackend<Block> for Client {
	fn block_body(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<Vec<<Block as BlockT>::Extrinsic>>> {
		with_client! {
			self,
			client,
			{
				client.block_body(hash)
			}
		}
	}

	fn block(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<SignedBlock<Block>>> {
		with_client! {
			self,
			client,
			{
				client.block(hash)
			}
		}
	}

	fn block_status(&self, hash: <Block as BlockT>::Hash) -> sp_blockchain::Result<BlockStatus> {
		with_client! {
			self,
			client,
			{
				client.block_status(hash)
			}
		}
	}

	fn justifications(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<Justifications>> {
		with_client! {
			self,
			client,
			{
				client.justifications(hash)
			}
		}
	}

	fn block_hash(
		&self,
		number: NumberFor<Block>,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		with_client! {
			self,
			client,
			{
				client.block_hash(number)
			}
		}
	}

	fn indexed_transaction(
		&self,
		id: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<Vec<u8>>> {
		with_client! {
			self,
			client,
			{
				client.indexed_transaction(id)
			}
		}
	}

	fn block_indexed_body(
		&self,
		id: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<Vec<Vec<u8>>>> {
		with_client! {
			self,
			client,
			{
				client.block_indexed_body(id)
			}
		}
	}

	fn requires_full_sync(&self) -> bool {
		with_client! {
			self,
			client,
			{
				client.requires_full_sync()
			}
		}
	}
}

impl sc_client_api::StorageProvider<Block, crate::FullBackend> for Client {
	fn storage(
		&self,
		hash: <Block as BlockT>::Hash,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<StorageData>> {
		with_client! {
			self,
			client,
			{
				client.storage(hash, key)
			}
		}
	}

	fn storage_hash(
		&self,
		hash: <Block as BlockT>::Hash,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		with_client! {
			self,
			client,
			{
				client.storage_hash(hash, key)
			}
		}
	}

	fn storage_pairs(
		&self,
		hash: <Block as BlockT>::Hash,
		key_prefix: Option<&StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<
		PairsIter<<crate::FullBackend as sc_client_api::Backend<Block>>::State, Block>,
	> {
		with_client! {
			self,
			client,
			{
				client.storage_pairs(hash, key_prefix, start_key)
			}
		}
	}

	fn storage_keys(
		&self,
		hash: <Block as BlockT>::Hash,
		prefix: Option<&StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<
		KeysIter<<crate::FullBackend as sc_client_api::Backend<Block>>::State, Block>,
	> {
		with_client! {
			self,
			client,
			{
				client.storage_keys(hash, prefix, start_key)
			}
		}
	}

	fn child_storage(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<StorageData>> {
		with_client! {
			self,
			client,
			{
				client.child_storage(hash, child_info, key)
			}
		}
	}

	fn child_storage_keys(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: ChildInfo,
		prefix: Option<&StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<
		KeysIter<<crate::FullBackend as sc_client_api::Backend<Block>>::State, Block>,
	> {
		with_client! {
			self,
			client,
			{
				client.child_storage_keys(hash, child_info, prefix, start_key)
			}
		}
	}

	fn child_storage_hash(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		with_client! {
			self,
			client,
			{
				client.child_storage_hash(hash, child_info, key)
			}
		}
	}
}

impl sp_blockchain::HeaderBackend<Block> for Client {
	fn header(&self, hash: Hash) -> sp_blockchain::Result<Option<Header>> {
		with_client! {
			self,
			client,
			{
				client.header(hash)
			}
		}
	}

	fn info(&self) -> sp_blockchain::Info<Block> {
		with_client! {
			self,
			client,
			{
				client.info()
			}
		}
	}

	fn status(&self, hash: Hash) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
		with_client! {
			self,
			client,
			{
				client.status(hash)
			}
		}
	}

	fn number(&self, hash: Hash) -> sp_blockchain::Result<Option<BlockNumber>> {
		with_client! {
			self,
			client,
			{
				client.number(hash)
			}
		}
	}

	fn hash(&self, number: BlockNumber) -> sp_blockchain::Result<Option<Hash>> {
		with_client! {
			self,
			client,
			{
				client.hash(number)
			}
		}
	}
}
