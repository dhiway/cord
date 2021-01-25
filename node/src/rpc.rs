/*
 * This file is part of the CORD
 * Copyright (C) 2020-21  Dhiway
 *
 */

//! A collection of node-specific RPC methods.
//!
//! Since `substrate` core functionality makes no assumptions
//! about the modules used inside the runtime, so do
//! RPC methods defined in `substrate-rpc` crate.
//! It means that `client/rpc` can't have any methods that
//! need some strong assumptions about the particular runtime.
//!
//! The RPCs available in this crate however can make some assumptions
//! about how the runtime is constructed and what `SRML` modules
//! are part of it. Therefore all node-runtime-specific RPCs can
//! be placed here or imported from corresponding `SRML` RPC definitions.

#![warn(missing_docs)]

use std::{sync::Arc};
use primitives::{AccountId, Balance, Block, BlockNumber, Nonce, Hash, Index};
use sc_finality_grandpa::{
	SharedVoterState, SharedAuthoritySet, FinalityProofProvider, GrandpaJustificationStream
};
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderMetadata, HeaderBackend};
use sp_consensus::SelectChain;
use sp_keystore::SyncCryptoStorePtr;
use sp_transaction_pool::TransactionPool;
use sc_client_api::AuxStore;
use sc_client_api::light::{Fetcher, RemoteBlockchain};
use sc_sync_state_rpc::{SyncStateRpcApi, SyncStateRpcHandler};
pub use sc_rpc::{DenyUnsafe, SubscriptionTaskExecutor};

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;

/// Light client extra dependencies.
pub struct LightDeps<C, F, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Remote access to the blockchain (async).
	pub remote_blockchain: Arc<dyn RemoteBlockchain<Block>>,
	/// Fetcher instance.
	pub fetcher: Arc<F>,
}

/// Extra dependencies for GRANDPA
pub struct GrandpaDeps<B> {
	/// Voting round info.
	pub shared_voter_state: SharedVoterState,
	/// Authority set info.
	pub shared_authority_set: SharedAuthoritySet<Hash, BlockNumber>,
	/// Receives notifications about justification events from Grandpa.
	pub justification_stream: GrandpaJustificationStream<Block>,
	/// Executor to drive the subscription manager in the Grandpa RPC handler.
	pub subscription_executor: SubscriptionTaskExecutor,
	/// Finality proof provider.
	pub finality_provider: Arc<FinalityProofProvider<B, Block>>,
}

/// Full client dependencies.
pub struct FullDeps<C, P, SC, B> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// The SelectChain Strategy
	pub select_chain: SC,
	/// A copy of the chain spec.
	pub chain_spec: Box<dyn sc_chain_spec::ChainSpec>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
	/// GRANDPA specific dependencies.
	pub grandpa: GrandpaDeps<B>,
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, SC, B>(deps: FullDeps<C, P, SC, B>) -> RpcExtension where
	C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + AuxStore +
		HeaderMetadata<Block, Error=BlockChainError> + Send + Sync + 'static,
	C::Api: frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: BlockBuilder<Block>,
	P: TransactionPool + Sync + Send + 'static,
	SC: SelectChain<Block> +'static,
	B: sc_client_api::Backend<Block> + Send + Sync + 'static,
	B::State: sc_client_api::backend::StateBackend<sp_runtime::traits::HashFor<Block>>,
{
	use frame_rpc_system::{FullSystem, SystemApi};
	use sc_finality_grandpa_rpc::{GrandpaApi, GrandpaRpcHandler};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};

	let mut io = jsonrpc_core::IoHandler::default();
	let FullDeps {
		client,
		pool,
		select_chain,
		chain_spec,
		deny_unsafe,
		grandpa,
	} = deps;
	let GrandpaDeps {
		shared_voter_state,
		shared_authority_set,
		justification_stream,
		subscription_executor,
		finality_provider,
	} = grandpa;

	io.extend_with(
		SystemApi::to_delegate(FullSystem::new(client.clone(), pool, deny_unsafe))
	);
	io.extend_with(
		TransactionPaymentApi::to_delegate(TransactionPayment::new(client.clone()))
	);
	io.extend_with(
		GrandpaApi::to_delegate(GrandpaRpcHandler::new(
				shared_authority_set.clone(),
				shared_voter_state,
				justification_stream,
				subscription_executor,
				finality_provider,
			)
		)
	);
	// io.extend_with(
	// 	SyncStateRpcApi::to_delegate(SyncStateRpcHandler::new(
	// 		chain_spec,
	// 		client,
	// 		shared_authority_set,
	// 		deny_unsafe,
	// 	))
	// );
	io
}

/// Instantiate all Light RPC extensions.
pub fn create_light<C, P, F>(deps: LightDeps<C, F, P>) -> RpcExtension 
	where
		C: ProvideRuntimeApi<Block>,		
		C: HeaderBackend<Block>,
		C: Send + Sync + 'static,
		C::Api: frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
		C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,	
		P: TransactionPool + Sync + Send + 'static,
		F: Fetcher<Block> + 'static,
{
	use frame_rpc_system::{LightSystem, SystemApi};

	let LightDeps {
		client,
		pool,
		remote_blockchain,
		fetcher
	} = deps;
	let mut io = jsonrpc_core::IoHandler::default();
	io.extend_with(
		SystemApi::<Hash, AccountId, Nonce>::to_delegate(LightSystem::new(client, remote_blockchain, fetcher, pool))
	);
	io
}