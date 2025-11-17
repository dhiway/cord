// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Service and node wiring for the Origin Hub parachain.

use std::{sync::Arc, time::Duration};

use cumulus_client_bootnodes::{start_bootnode_tasks, StartBootnodeTasksParams};
use cumulus_client_cli::CollatorOptions;
use cumulus_client_collator::service::CollatorService;
use cumulus_client_consensus_aura::collators::lookahead::{self as aura, Params as AuraParams};
use cumulus_client_consensus_common::ParachainBlockImport as TParachainBlockImport;
use cumulus_client_service::{
	build_network, build_relay_chain_interface, prepare_node_config, start_relay_chain_tasks,
	BuildNetworkParams, CollatorSybilResistance, DARecoveryProfile, ParachainHostFunctions,
	StartRelayChainTasksParams,
};
use cumulus_primitives_core::{
	relay_chain::{CollatorPair, ValidationCode},
	ParaId,
};
use cumulus_relay_chain_interface::{OverseerHandle, RelayChainInterface};
use origin_hub_system_runtime_constants::origin::consensus::RELAY_CHAIN_SLOT_DURATION_MILLIS;
use frame_benchmarking_cli::SUBSTRATE_REFERENCE_HARDWARE;
use origin_hub_system_runtime::RuntimeApi;
use parachains_common::{Block, Hash};
use prometheus_endpoint::Registry;
use sc_client_api::Backend;
use sc_consensus::ImportQueue;
use sc_executor::{HeapAllocStrategy, WasmExecutor, DEFAULT_HEAP_ALLOC_STRATEGY};
use sc_network::{NetworkBackend, NetworkBlock, NetworkWorker};
use sc_service::{Configuration, PartialComponents, TFullBackend, TFullClient, TaskManager};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_keystore::KeystorePtr;

type ParachainExecutor = WasmExecutor<ParachainHostFunctions>;
type ParachainClient = TFullClient<Block, RuntimeApi, ParachainExecutor>;
type ParachainBackend = TFullBackend<Block>;
type ParachainBlockImport = TParachainBlockImport<Block, Arc<ParachainClient>, ParachainBackend>;

/// Assembly of PartialComponents (enough to run chain ops subcommands).
pub type Service = PartialComponents<
	ParachainClient,
	ParachainBackend,
	(),
	sc_consensus::DefaultImportQueue<Block>,
	sc_transaction_pool::TransactionPoolHandle<Block, ParachainClient>,
	(ParachainBlockImport, Option<Telemetry>, Option<TelemetryWorkerHandle>),
>;

/// Start building the service for chain-ops or full node.
pub fn new_partial(config: &Configuration) -> Result<Service, sc_service::Error> {
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let heap_pages = config
		.executor
		.default_heap_pages
		.map_or(DEFAULT_HEAP_ALLOC_STRATEGY, |h| HeapAllocStrategy::Static { extra_pages: h as _ });

	let executor = ParachainExecutor::builder()
		.with_execution_method(config.executor.wasm_method)
		.with_onchain_heap_alloc_strategy(heap_pages)
		.with_offchain_heap_alloc_strategy(heap_pages)
		.with_max_runtime_instances(config.executor.max_runtime_instances)
		.with_runtime_cache_size(config.executor.runtime_cache_size)
		.build();

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts_record_import::<Block, RuntimeApi, _>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
			true,
		)?;
	let client = Arc::new(client);

	let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let transaction_pool = Arc::from(
		sc_transaction_pool::Builder::new(
			task_manager.spawn_essential_handle(),
			client.clone(),
			config.role.is_authority().into(),
		)
		.with_options(config.transaction_pool.clone())
		.with_prometheus(config.prometheus_registry())
		.build(),
	);

	let block_import = ParachainBlockImport::new(client.clone(), backend.clone());

	let import_queue = build_import_queue(
		client.clone(),
		block_import.clone(),
		config,
		telemetry.as_ref().map(|telemetry| telemetry.handle()),
		&task_manager,
	);

	Ok(PartialComponents {
		backend,
		client,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		select_chain: (),
		other: (block_import, telemetry, telemetry_worker_handle),
	})
}

/// Build the import queue for the parachain runtime.
fn build_import_queue(
	client: Arc<ParachainClient>,
	block_import: ParachainBlockImport,
	config: &Configuration,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
) -> sc_consensus::DefaultImportQueue<Block> {
	cumulus_client_consensus_aura::equivocation_import_queue::fully_verifying_import_queue::<
		sp_consensus_aura::sr25519::AuthorityPair,
		_,
		_,
		_,
		_,
	>(
		client,
		block_import,
		move |_, _| async move {
			let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
			Ok(timestamp)
		},
		&task_manager.spawn_essential_handle(),
		config.prometheus_registry(),
		telemetry,
	)
}

#[allow(clippy::too_many_arguments)]
fn start_consensus(
	client: Arc<ParachainClient>,
	backend: Arc<ParachainBackend>,
	block_import: ParachainBlockImport,
	prometheus_registry: Option<&Registry>,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
	relay_chain_interface: Arc<dyn RelayChainInterface>,
	transaction_pool: Arc<sc_transaction_pool::TransactionPoolHandle<Block, ParachainClient>>,
	keystore: KeystorePtr,
	relay_chain_slot_duration: Duration,
	para_id: ParaId,
	collator_key: CollatorPair,
	overseer_handle: OverseerHandle,
	announce_block: Arc<dyn Fn(Hash, Option<Vec<u8>>) + Send + Sync>,
) -> Result<(), sc_service::Error> {
	let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
		task_manager.spawn_handle(),
		client.clone(),
		transaction_pool,
		prometheus_registry,
		telemetry.clone(),
	);

	let proposer = proposer_factory;

	let collator_service = CollatorService::new(
		client.clone(),
		Arc::new(task_manager.spawn_handle()),
		announce_block,
		client.clone(),
	);

	let params = AuraParams {
		create_inherent_data_providers: move |_, ()| async move { Ok(()) },
		block_import,
		para_client: client.clone(),
		para_backend: backend,
		relay_client: relay_chain_interface,
		code_hash_provider: move |block_hash| {
			client.code_at(block_hash).ok().map(|c| ValidationCode::from(c).hash())
		},
		keystore,
		collator_key,
		para_id,
		overseer_handle,
		relay_chain_slot_duration,
		proposer,
		collator_service,
		authoring_duration: Duration::from_millis(2000),
		reinitialize: false,
		max_pov_percentage: None,
	};
	let fut = aura::run::<Block, sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _, _, _, _>(
		params,
	);
	task_manager.spawn_essential_handle().spawn("aura", None, fut);

	Ok(())
}

/// Start a node with the given parachain `Configuration` and relay chain `Configuration`.
#[sc_tracing::logging::prefix_logs_with("Parachain")]
pub async fn start_parachain_node(
	parachain_config: Configuration,
	relay_chain_config: Configuration,
	collator_options: CollatorOptions,
	para_id: ParaId,
	hwbench: Option<sc_sysinfo::HwBench>,
) -> sc_service::error::Result<(TaskManager, Arc<ParachainClient>)> {
	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial(&parachain_config)?;
	let (block_import, mut telemetry, telemetry_worker_handle) = params.other;

	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let net_config = sc_network::config::FullNetworkConfiguration::<
		_,
		_,
		sc_network::NetworkWorker<Block, Hash>,
	>::new(&parachain_config.network, prometheus_registry.clone());

	let client = params.client.clone();
	let backend = params.backend.clone();
	let mut task_manager = params.task_manager;

	let relay_chain_fork_id = relay_chain_config.chain_spec.fork_id().map(ToString::to_string);
	let parachain_fork_id = parachain_config.chain_spec.fork_id().map(ToString::to_string);
	let advertise_non_global_ips = parachain_config.network.allow_non_globals_in_dht;
	let parachain_public_addresses = parachain_config.network.public_addresses.clone();

	let (relay_chain_interface, collator_key, relay_chain_network, paranode_rx) =
		build_relay_chain_interface(
			relay_chain_config,
			&parachain_config,
			telemetry_worker_handle,
			&mut task_manager,
			collator_options.clone(),
			hwbench.clone(),
		)
		.await
		.map_err(|e| sc_service::Error::Application(Box::new(e) as Box<_>))?;

	let validator = parachain_config.role.is_authority();
	let transaction_pool = params.transaction_pool.clone();
	let import_queue_service = params.import_queue.service();

	let (network, system_rpc_tx, tx_handler_controller, sync_service) =
		build_network(BuildNetworkParams {
			parachain_config: &parachain_config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			para_id,
			spawn_handle: task_manager.spawn_handle(),
			relay_chain_interface: relay_chain_interface.clone(),
			import_queue: params.import_queue,
			sybil_resistance_level: CollatorSybilResistance::Resistant,
			metrics: NetworkWorker::<Block, Hash>::register_notification_metrics(
				parachain_config.prometheus_config.as_ref().map(|config| &config.registry),
			),
		})
		.await?;

	if parachain_config.offchain_worker.enabled {
		use futures::FutureExt;

		let offchain_workers =
			sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
				runtime_api_provider: client.clone(),
				keystore: Some(params.keystore_container.keystore()),
				offchain_db: backend.offchain_storage(),
				transaction_pool: Some(OffchainTransactionPoolFactory::new(
					transaction_pool.clone(),
				)),
				network_provider: Arc::new(network.clone()),
				is_validator: parachain_config.role.is_authority(),
				enable_http_requests: false,
				custom_extensions: move |_| vec![],
			})?;
		task_manager.spawn_handle().spawn(
			"offchain-workers-runner",
			"offchain-work",
			offchain_workers.run(client.clone(), task_manager.spawn_handle()).boxed(),
		);
	}

	let rpc_builder = {
		let client = client.clone();
		let transaction_pool = transaction_pool.clone();

		Box::new(move |_| {
			let deps = crate::rpc::FullDeps { client: client.clone(), pool: transaction_pool.clone() };

			crate::rpc::create_full(deps).map_err(Into::into)
		})
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_builder,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.keystore(),
		backend: backend.clone(),
		network: network.clone(),
		sync_service: sync_service.clone(),
		system_rpc_tx,
		tx_handler_controller,
		telemetry: telemetry.as_mut(),
	})?;

	if let Some(hwbench) = hwbench {
		sc_sysinfo::print_hwbench(&hwbench);
		match SUBSTRATE_REFERENCE_HARDWARE.check_hardware(&hwbench, false) {
			Err(err) if validator => {
				log::warn!(
					"⚠️  The hardware does not meet the minimal requirements {} for role 'Authority'.",
					err
				);
			},
			_ => {},
		}

		if let Some(ref mut telemetry) = telemetry {
			let telemetry_handle = telemetry.handle();
			task_manager.spawn_handle().spawn(
				"telemetry_hwbench",
				None,
				sc_sysinfo::initialize_hwbench_telemetry(telemetry_handle, hwbench),
			);
		}
	}

	let announce_block = {
		let sync_service = sync_service.clone();
		Arc::new(move |hash, data| sync_service.announce_block(hash, data))
	};

	let relay_chain_slot_duration = Duration::from_millis(
		RELAY_CHAIN_SLOT_DURATION_MILLIS as u64,
	);

	let overseer_handle = relay_chain_interface
		.overseer_handle()
		.map_err(|e| sc_service::Error::Application(Box::new(e)))?;

	start_relay_chain_tasks(StartRelayChainTasksParams {
		client: client.clone(),
		announce_block: announce_block.clone(),
		para_id,
		relay_chain_interface: relay_chain_interface.clone(),
		task_manager: &mut task_manager,
		da_recovery_profile: if validator {
			DARecoveryProfile::Collator
		} else {
			DARecoveryProfile::FullNode
		},
		import_queue: import_queue_service,
		relay_chain_slot_duration,
		recovery_handle: Box::new(overseer_handle.clone()),
		sync_service: sync_service.clone(),
		prometheus_registry: prometheus_registry.as_ref(),
	})?;

	start_bootnode_tasks(StartBootnodeTasksParams {
		embedded_dht_bootnode: collator_options.embedded_dht_bootnode,
		dht_bootnode_discovery: collator_options.dht_bootnode_discovery,
		para_id,
		task_manager: &mut task_manager,
		relay_chain_interface: relay_chain_interface.clone(),
		relay_chain_fork_id,
		relay_chain_network,
		request_receiver: paranode_rx,
		parachain_network: network.clone(),
		advertise_non_global_ips,
		parachain_genesis_hash: client.chain_info().genesis_hash,
		parachain_fork_id,
		parachain_public_addresses,
	});

	if validator {
		start_consensus(
			client.clone(),
			backend,
			block_import,
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|t| t.handle()),
			&task_manager,
			relay_chain_interface,
			transaction_pool,
			params.keystore_container.keystore(),
			relay_chain_slot_duration,
			para_id,
			collator_key.expect("Command line arguments do not allow this. qed"),
			overseer_handle,
			announce_block,
		)?;
	}

	Ok((task_manager, client))
}
