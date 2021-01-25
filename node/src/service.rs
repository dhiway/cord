/*
 * This file is part of the CORD
 * Copyright (C) 2020  Dhiway
 *
 */

//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.
use sc_finality_grandpa::{self, FinalityProofProvider as GrandpaFinalityProofProvider};

#[cfg(feature = "full-node")]
use {
	std::convert::TryInto,
	std::time::Duration,
	tracing::info,
	sc_authority_discovery::Service as AuthorityDiscoveryService,
	sp_keystore::SyncCryptoStorePtr,
	sp_trie::PrefixedMemoryDB,
	sc_client_api::ExecutorProvider,
};

use sp_core::traits::SpawnNamed;
use std::sync::Arc;
use sp_blockchain::HeaderBackend;
use sc_executor::native_executor_instance;
use sc_service::RpcHandlers;
use prometheus_endpoint::Registry;
use sc_telemetry::TelemetryConnectionNotifier;
use sp_consensus_aura::sr25519::{AuthorityPair as AuraPair};
use frame_rpc_system::AccountNonceApi;

// pub use sc_client_api::{ExecutorProvider, RemoteBackend};
pub use sc_client_api::{Backend, ExecutionStrategy, CallExecutor, ExecutorProvider, RemoteBackend};
use sc_client_api::{Backend as BackendT, BlockchainEvents, KeyIterator};

pub use sc_consensus::LongestChain;
// pub use sc_executor::NativeExecutionDispatch;
pub use sc_service::{
	Role, PruningMode, TransactionPoolOptions, Error as ServiceError, RuntimeGenesis,
	TFullClient, TLightClient, TFullBackend, TLightBackend, TFullCallExecutor, TLightCallExecutor,
	Configuration, ChainSpec, TaskManager,
};
pub use sc_service::config::{DatabaseConfig, PrometheusConfig};
pub use sp_api::{ApiRef, Core as CoreApi, ConstructRuntimeApi, ProvideRuntimeApi, StateBackend, CallApiAt};
pub use sp_runtime::traits::{DigestFor, HashFor, NumberFor, Block as BlockT, self as runtime_traits, BlakeTwo256};
use primitives::{Block, AccountId, Nonce, Balance, Header, BlockNumber, Hash};


// use sp_api::{ProvideRuntimeApi, CallApiAt, NumberFor};
// use sp_blockchain::HeaderBackend;
// use sp_runtime::{
// 	Justification, generic::{BlockId, SignedBlock}, traits::{Block as BlockT, BlakeTwo256},
// };
// use sc_client_api::{Backend as BackendT, BlockchainEvents, KeyIterator};
// use sp_storage::{StorageData, StorageKey, ChildInfo, PrefixedStorageKey};

// use sc_service::{error::Error as ServiceError, Configuration, RpcHandlers, Role, TaskManager};

// use std::time::Duration;
// use primitives::Block;
use cord_node_runtime::{self, RuntimeApi};
use sp_inherents::InherentDataProviders;
use sc_network::{Event, NetworkService};
// use sp_runtime::traits::Block as BlockT;
// pub use sc_executor::NativeExecutor;
// use jsonrpc_core::IoHandler;

// Our native executor instance.
native_executor_instance!(
	pub Executor,
	cord_node_runtime::api::dispatch,
    cord_node_runtime::native_version,
    frame_benchmarking::benchmarking::HostFunctions,
);

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error(transparent)]
	Io(#[from] std::io::Error),

	#[error(transparent)]
	AddrFormatInvalid(#[from] std::net::AddrParseError),

	#[error(transparent)]
	Sub(#[from] ServiceError),

	#[error(transparent)]
	Blockchain(#[from] sp_blockchain::Error),

	#[error(transparent)]
	Consensus(#[from] sp_consensus::Error),

	#[error(transparent)]
	Prometheus(#[from] prometheus_endpoint::PrometheusError),

	#[cfg(feature = "full-node")]
	#[error(transparent)]
	Availability(#[from] AvailabilityError),
}


// If we're using prometheus, use a registry with a prefix of `cord`.
fn set_prometheus_registry(config: &mut Configuration) -> Result<(), Error> {
	if let Some(PrometheusConfig { registry, .. }) = config.prometheus_config.as_mut() {
		*registry = Registry::new_custom(Some("cord".into()), None)?;
	}

	Ok(())
}

/// A set of APIs runtimes must implement.
pub trait RuntimeApiCollection:
	sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
	+ sp_api::ApiExt<Block, Error = sp_blockchain::Error>
	// + sp_consensus_aura::AuraApi<Block, AuthorityId>
	+ sc_finality_grandpa::GrandpaApi<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
	+ sp_api::Metadata<Block>
	+ sp_offchain::OffchainWorkerApi<Block>
	+ sp_session::SessionKeys<Block>
	+ sp_authority_discovery::AuthorityDiscoveryApi<Block>
where
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
{}

impl<Api> RuntimeApiCollection for Api
where
	Api: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::ApiExt<Block, Error = sp_blockchain::Error>
        // + sp_consensus_aura::AuraApi<Block, AuraPair>
		+ sc_finality_grandpa::GrandpaApi<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_authority_discovery::AuthorityDiscoveryApi<Block>,
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
{}

// /// Trait that abstracts over all available client implementations.
// pub trait AbstractClient<Block, Backend>:
// 	BlockchainEvents<Block> + Sized + Send + Sync
// 	+ ProvideRuntimeApi<Block>
// 	+ HeaderBackend<Block>
// 	+ CallApiAt<
// 		Block,
// 		Error = sp_blockchain::Error,
// 		StateBackend = Backend::State
// 	>
// 	where
// 		Block: BlockT,
// 		Backend: BackendT<Block>,
// 		Backend::State: sp_api::StateBackend<BlakeTwo256>,
// 		Self::Api: RuntimeApiCollection<StateBackend = Backend::State>,
// {}

// impl<Block, Backend, Client> AbstractClient<Block, Backend> for Client
// 	where
// 		Block: BlockT,
// 		Backend: BackendT<Block>,
// 		Backend::State: sp_api::StateBackend<BlakeTwo256>,
// 		Client: BlockchainEvents<Block> + ProvideRuntimeApi<Block> + HeaderBackend<Block>
// 			+ Sized + Send + Sync
// 			+ CallApiAt<
// 				Block,
// 				Error = sp_blockchain::Error,
// 				StateBackend = Backend::State
// 			>,
// 		Client::Api: RuntimeApiCollection<StateBackend = Backend::State>,
// {}


pub type FullClient = sc_service::TFullClient<Block, RuntimeApi, Executor>;
pub type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type FullGrandpaBlockImport =
	sc_finality_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;
type LightBackend = sc_service::TLightBackendWithHash<Block, sp_runtime::traits::BlakeTwo256>;
type LightClient = sc_service::TLightClient<Block, RuntimeApi, Executor>;

pub fn new_partial(config: &mut Configuration) -> Result<sc_service::PartialComponents<
	FullClient, FullBackend, FullSelectChain,
	sp_consensus::DefaultImportQueue<Block, FullClient>,
	sc_transaction_pool::FullPool<Block, FullClient>,
	(
		impl Fn(
			crate::rpc::DenyUnsafe,
			crate::rpc::SubscriptionTaskExecutor
		) -> crate::rpc::RpcExtension,
		(
			sc_consensus_aura::AuraBlockImport<
				Block,
				FullClient,
				FullGrandpaBlockImport,
				AuraPair
			>,
			sc_finality_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
		),
		sc_finality_grandpa::SharedVoterState,
		Option<sc_telemetry::TelemetrySpan>,
	)
>, Error> 
	// where
	// 	RuntimeApi: ConstructRuntimeApi<Block, FullClient> + Send + Sync + 'static,
	// 	RuntimeApi::RuntimeApi:
	// 	RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	// 	Executor: NativeExecutionDispatch + 'static,
{
	set_prometheus_registry(config)?;

	let inherent_data_providers = sp_inherents::InherentDataProviders::new();

	let (client, backend, keystore_container, task_manager, telemetry_span) =
		sc_service::new_full_parts::<Block, RuntimeApi, Executor>(&config)?;
	let client = Arc::new(client);

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
	);

	let (grandpa_block_import, grandpa_link) = sc_finality_grandpa::block_import(
		client.clone(), 
		&(client.clone() as Arc<_>), 
		select_chain.clone(),
	)?;

	let justification_import = grandpa_block_import.clone();

	let aura_block_import = 
		sc_consensus_aura::AuraBlockImport::<_, _, _, AuraPair>::new(
			grandpa_block_import.clone(), 
			client.clone(),
	);

	let import_queue = sc_consensus_aura::import_queue::<_, _, _, AuraPair, _, _>(
		sc_consensus_aura::slot_duration(&*client)?,
		aura_block_import.clone(),
		Some(Box::new(justification_import)),
		client.clone(),
		inherent_data_providers.clone(),
		&task_manager.spawn_handle(),
		config.prometheus_registry(),
		sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
	)?;

	let justification_stream = grandpa_link.justification_stream();
	let shared_authority_set = grandpa_link.shared_authority_set().clone();
	let shared_voter_state = sc_finality_grandpa::SharedVoterState::empty();
	let finality_proof_provider = GrandpaFinalityProofProvider::new_for_service(
		backend.clone(),
		client.clone(),
		Some(shared_authority_set.clone()),
	);

	let import_setup = (aura_block_import.clone(), grandpa_link);
	let rpc_setup = shared_voter_state.clone();

	let rpc_extensions_builder = {
		let (_, grandpa_link) = &import_setup;
		let client = client.clone();
		let keystore = keystore_container.sync_keystore();
		let transaction_pool = transaction_pool.clone();
		let select_chain = select_chain.clone();
		let chain_spec = config.chain_spec.cloned_box();

		move |deny_unsafe, subscription_executor| -> crate::rpc::RpcExtension {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				select_chain: select_chain.clone(),
				chain_spec: chain_spec.cloned_box(),
				deny_unsafe,
				grandpa: crate::rpc::GrandpaDeps {
					shared_voter_state: shared_voter_state.clone(),
					shared_authority_set: shared_authority_set.clone(),
					justification_stream: justification_stream.clone(),
					subscription_executor,
					finality_provider: finality_proof_provider.clone(),
				},
			};

			crate::rpc::create_full(deps)
		};
		// (rpc_extensions_builder, rpc_setup)
	};

	Ok(sc_service::PartialComponents {
		client, 
		backend, 
		task_manager, 
		keystore_container, 
		select_chain, 
		import_queue, 
		transaction_pool,
		inherent_data_providers,
		other: (rpc_extensions_builder, import_setup, rpc_setup, telemetry_span),
	})
}

// #[cfg(feature = "full-node")]
// pub struct NewFull<C> {
// 	pub task_manager: TaskManager,
// 	pub client: C,
// 	pub network: Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>,
// 	pub inherent_data_providers: InherentDataProviders,
// 	pub network_status_sinks: service::NetworkStatusSinks<Block>,
// 	pub transaction_pool: Arc<sc_transaction_pool::FullPool<Block, FullClient>>,
// 	pub rpc_handlers: RpcHandlers,
// 	pub backend: Arc<FullBackend>,
// }

// #[cfg(feature = "full-node")]
// impl<C> NewFull<C> {
// 	/// Convert the client type using the given `func`.
// 	pub fn with_client<NC>(self, func: impl FnOnce(C) -> NC) -> NewFull<NC> {
// 		NewFull {
// 			client: func(self.client),
// 			task_manager: self.task_manager,
// 			network: self.network,
// 			inherent_data_providers: self.inherent_data_providers,
// 			network_status_sinks: self.network_status_sinks,
// 			transaction_pool: self.transaction_pool,
// 			rpc_handlers: self.rpc_handlers,
// 			backend: self.backend,
// 		}
// 	}
// }

	
/// Builds a new service for a full client.
pub fn new_full_base (
	mut config: Configuration,
	with_startup_data: impl FnOnce(
		&sc_consensus_aura::AuraBlockImport<
			Block,
			FullClient,
			FullGrandpaBlockImport,
			AuraPair
		>,
		&sc_finality_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
	),
) -> Result<NewFullBase, Error>
	// where
	// 	RuntimeApi: ConstructRuntimeApi<Block, FullClient> + Send + Sync + 'static,
	// 	RuntimeApi::RuntimeApi:
	// 	RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	// 	Executor: NativeExecutionDispatch + 'static,
{

	let role = config.role.clone();
	let force_authoring = config.force_authoring;
	let disable_grandpa = config.disable_grandpa;
	let name = config.network.node_name.clone();
	
	let sc_service::PartialComponents {
		client, 
		backend, 
		mut task_manager, 
		keystore_container, 
		select_chain, 
		import_queue,
		transaction_pool,
		inherent_data_providers,
		other: (rpc_extensions_builder, import_setup, rpc_setup, telemetry_span),
	} = new_partial(&mut config)?;

	let prometheus_registry = config.prometheus_registry().cloned();

	let shared_voter_state = rpc_setup;
	
	let (grandpa_block_import, grandpa_link) = sc_finality_grandpa::block_import(
		client.clone(), 
		&(client.clone() as Arc<_>), 
		select_chain.clone(),
	)?;

	// config.network.extra_sets.push(sc_finality_grandpa::grandpa_peers_set_config());

	let shared_authority_set = grandpa_link.shared_authority_set().clone();

	let finality_proof_provider = GrandpaFinalityProofProvider::new_for_service(
		backend.clone(),
		client.clone(),
		Some(shared_authority_set.clone()),
	);
	
	let (network, network_status_sinks, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: None,
			block_announce_validator_builder: None,
			finality_proof_provider: Some(finality_proof_provider.clone()),
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config, backend.clone(), task_manager.spawn_handle(), client.clone(), network.clone(),
		);
	}

	// let availability_config = config.database.clone().try_into().map_err(Error::Availability)?;

	let (rpc_handlers, telemetry_connection_notifier) = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		config,
		backend: backend.clone(),
		client: client.clone(),
		keystore: keystore_container.sync_keystore(),
		network: network.clone(),
		rpc_extensions_builder: Box::new(rpc_extensions_builder),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		on_demand: None,
		remote_blockchain: None,
		network_status_sinks: network_status_sinks.clone(),
		system_rpc_tx,
		telemetry_span,
	})?;

	let (block_import, grandpa_link) = import_setup;
	
	// let spawner = task_manager.spawn_handle();
	// let leaves: Vec<_> = select_chain.clone()
	// 	.leaves()
	// 	.unwrap_or_else(|_| vec![])
	// 	.into_iter()
	// 	.filter_map(|hash| {
	// 		let number = client.number(hash).ok()??;
	// 		let parent_hash = client.header(&BlockId::Hash(hash)).ok()??.parent_hash;

	// 		Some(BlockInfo {
	// 			hash,
	// 			parent_hash,
	// 			number,
	// 		})
	// 	})
	// 	.collect();

	let authority_discovery_service = if role.is_authority() {
		use sc_network::Event;
		use futures::StreamExt;

		let authority_discovery_role = if role.is_authority() {
			sc_authority_discovery::Role::PublishAndDiscover(
				keystore_container.keystore(),
			)
		} else {
			// don't publish our addresses when we're only a collator
			sc_authority_discovery::Role::Discover
		};
		let dht_event_stream = network.event_stream("authority-discovery")
			.filter_map(|e| async move { match e {
				Event::Dht(e) => Some(e),
				_ => None,
			}});
		let (worker, service) = sc_authority_discovery::new_worker_and_service(
			client.clone(),
			network.clone(),
			Box::pin(dht_event_stream),
			authority_discovery_role,
			prometheus_registry.clone(),
		);

		task_manager.spawn_handle().spawn("authority-discovery-worker", worker.run());
		Some(service)
	} else {
		None
	};

	if role.is_authority() {
		let can_author_with =
			sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

		let proposer = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool,
			prometheus_registry.as_ref(),
		);

		let aura = sc_consensus_aura::start_aura::<_, _, _, _, _, AuraPair, _, _, _>(
			sc_consensus_aura::slot_duration(&*client)?,
			client.clone(),
			select_chain,
			block_import,
			proposer,
			network.clone(),
			inherent_data_providers.clone(),
			force_authoring,
			keystore_container.sync_keystore(),
			can_author_with,
		)?;

		// the AURA authoring task is considered essential, i.e. if it
		// fails we take down the service with it.
		task_manager.spawn_essential_handle().spawn_blocking("aura", aura);
	}

	// if the node isn't actively participating in consensus then it doesn't
	// need a keystore, regardless of which protocol we use below.
	let keystore_opt = if role.is_authority() {
		Some(keystore_container.sync_keystore())
	} else {
		None
	};

	let config = sc_finality_grandpa::Config {
		// FIXME #1578 make this available through chainspec
		gossip_duration: std::time::Duration::from_millis(1000),
		justification_period: 512,
		name: Some(name),
		observer_enabled: false,
		keystore: keystore_opt,
		is_authority: role.is_network_authority(),
	};

	let enable_grandpa = !disable_grandpa;
	if enable_grandpa {
		// start the full GRANDPA voter
		// NOTE: non-authorities could run the GRANDPA observer protocol, but at
		// this point the full voter should provide better guarantees of block
		// and vote data availability than the observer. The observer has not
		// been tested extensively yet and having most nodes in a network run it
		// could lead to finality stalls.
		let grandpa_config = sc_finality_grandpa::GrandpaParams {
			config,
			link: grandpa_link,
			network: network.clone(),
			// inherent_data_providers: inherent_data_providers.clone(),
			telemetry_on_connect: telemetry_connection_notifier.map(|x| x.on_connect_stream()),
			voting_rule: sc_finality_grandpa::VotingRulesBuilder::default().build(),
			prometheus_registry: prometheus_registry.clone(),
			shared_voter_state,
		};

		// the GRANDPA voter task is considered infallible, i.e.
		// if it fails we take down the service with it.
		task_manager.spawn_essential_handle().spawn_blocking(
			"grandpa-voter",
			sc_finality_grandpa::run_grandpa_voter(grandpa_config)?
		);
	}

	network_starter.start_network();

	Ok(NewFullBase {
		task_manager,
		client, 
		network,
		inherent_data_providers,
		network_status_sinks,
		transaction_pool,
		rpc_handlers,
		backend,
	})
}

pub struct NewFullBase {
	pub task_manager: TaskManager,
	pub client: Arc<FullClient>,
	pub network: Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>,
	pub inherent_data_providers: InherentDataProviders,
	pub network_status_sinks: sc_service::NetworkStatusSinks<Block>,
	pub transaction_pool: Arc<sc_transaction_pool::FullPool<Block, FullClient>>,
	pub rpc_handlers: RpcHandlers,
	pub backend: Arc<FullBackend>,
}

// pub struct NewFullBase {
// 	pub task_manager: TaskManager,
// 	pub client: Arc<FullClient>,
// 	pub network: Arc<NetworkService<Block, <Block as BlockT>::Hash>>,
// 	pub inherent_data_providers: InherentDataProviders,
// 	pub network_status_sinks: sc_service::NetworkStatusSinks<Block>,
// 	pub transaction_pool: Arc<sc_transaction_pool::FullPool<Block, FullClient>>,
// }

/// Builds a new service for a full client.
pub fn new_full(config: Configuration)
-> Result<TaskManager, Error> {
	new_full_base(config, |_, _| ()).map(|NewFullBase { task_manager, .. }| {
		task_manager
	})
}


fn new_light_base<Runtime, Dispatch>(mut config: Configuration) -> Result<(
	TaskManager, RpcHandlers, Arc<LightClient>,
	Arc<NetworkService<Block, <Block as BlockT>::Hash>>,
	Arc<sc_transaction_pool::LightPool<Block, LightClient, sc_network::config::OnDemand<Block>>>,
	// TaskManager,
	// RpcHandlers,
	Option<TelemetryConnectionNotifier>,
), Error>
	// where
	// 	Runtime: 'static + Send + Sync + ConstructRuntimeApi<Block, LightClient<Runtime, Dispatch>>,
	// 	<Runtime as ConstructRuntimeApi<Block, LightClient<Runtime, Dispatch>>>::RuntimeApi:
	// 	RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<LightBackend, Block>>,
	// 	Dispatch: NativeExecutionDispatch + 'static,
		// Runtime::RuntimeApi:RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<LightBackend, Block>>,
		// Runtime: 'static + Send + Sync + ConstructRuntimeApi<Block, LightClient<Runtime, Dispatch>>,
		// Dispatch: NativeExecutionDispatch + 'static,
{	
	set_prometheus_registry(&mut config)?;
	// use sc_client_api::backend::RemoteBackend;

	let (client, backend, keystore_container, mut task_manager, on_demand, telemetry_span) =
		sc_service::new_light_parts::<Block, RuntimeApi, Executor>(&config)?;

	// let select_chain = sc_consensus::LongestChain::new(backend.clone());
	
	let transaction_pool = Arc::new(sc_transaction_pool::BasicPool::new_light(
		config.transaction_pool.clone(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
		on_demand.clone(),
	));

	let (grandpa_block_import, _) = sc_finality_grandpa::block_import(
		client.clone(),
		&(client.clone() as Arc<_>),
		Arc::new(on_demand.checker().clone()),
		// select_chain.clone(),
	)?;

	let justification_import = grandpa_block_import.clone();

	let aura_block_import = 
		sc_consensus_aura::AuraBlockImport::<_, _, _, AuraPair>::new(
			grandpa_block_import.clone(), 
			client.clone(),
	);

	// let finality_proof_import = grandpa_block_import.clone();
	// let finality_proof_request_builder =
	// 	finality_proof_import.create_finality_proof_request_builder();

	let import_queue = sc_consensus_aura::import_queue::<_, _, _, AuraPair, _, _>(
		sc_consensus_aura::slot_duration(&*client)?,
		grandpa_block_import,
		Some(Box::new(justification_import)),
		client.clone(),
		InherentDataProviders::new(),
		&task_manager.spawn_handle(),
		config.prometheus_registry(),
		sp_consensus::NeverCanAuthor,
	)?;
	
	// config.network.extra_sets.push(sc_finality_grandpa::grandpa_peers_set_config());

	// let shared_authority_set = grandpa_link.shared_authority_set().clone();

	// let finality_proof_provider = GrandpaFinalityProofProvider::new_for_service(
	// 	backend.clone(),
	// 	client.clone(),
	// 	Some(shared_authority_set.clone()),
	// );
	// // grandpa_block_import,
	// let finality_proof_provider =
	// 	GrandpaFinalityProofProvider::new_for_service(backend.clone(), client.clone());

	let (network, network_status_sinks, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: Some(on_demand.clone()),
			block_announce_validator_builder: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config, 
			backend.clone(), 
			task_manager.spawn_handle(), 
			client.clone(), 
			network.clone(),
		);
	}

	let light_deps = crate::rpc::LightDeps {
		remote_blockchain: backend.remote_blockchain(),
		fetcher: on_demand.clone(),
		client: client.clone(),
		pool: transaction_pool.clone(),
	};

	let rpc_extensions = crate::rpc::create_light(light_deps);

	let (rpc_handlers, telemetry_connection_notifier) = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
			on_demand: Some(on_demand),
			remote_blockchain: Some(backend.remote_blockchain()),
			rpc_extensions_builder: Box::new(sc_service::NoopRpcExtensionBuilder(rpc_extensions)),
			task_manager: &mut task_manager,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			config, 
			keystore: keystore_container.sync_keystore(),
			backend, 
			network: network.clone(),
			network_status_sinks, 
			system_rpc_tx,
			telemetry_span,
		})?;

	 network_starter.start_network();

	 Ok((task_manager, rpc_handlers, client, network, transaction_pool, telemetry_connection_notifier))
}

/// Builds a new service for a light client.
pub fn new_light(config: Configuration) -> Result<TaskManager, Error> {
	new_light_base(config).map(|(task_manager, _, _, _, _)| {
		task_manager
	})
}