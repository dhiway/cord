use std::{sync::Arc, time::Duration};

use codec::Encode;
use cumulus_client_bootnodes::{start_bootnode_tasks, StartBootnodeTasksParams};
use cumulus_client_cli::CollatorOptions;
use cumulus_client_collator::service::CollatorService;
use cumulus_client_consensus_aura::collators::basic::{self as aura, Params as AuraParams};
use cumulus_client_consensus_common::ParachainBlockImport as TParachainBlockImport;
use cumulus_client_service::{
	build_network, build_relay_chain_interface, prepare_node_config, start_relay_chain_tasks,
	BuildNetworkParams, CollatorSybilResistance, DARecoveryProfile, ParachainHostFunctions,
	StartRelayChainTasksParams,
};
use cumulus_primitives_core::{relay_chain::CollatorPair, GetParachainInfo, ParaId};
use cumulus_relay_chain_interface::{OverseerHandle, RelayChainInterface};
use futures::StreamExt;
use origin_commons_runtime::{Block, RuntimeApi};
use prometheus_endpoint::Registry;
use sc_client_api::BlockchainEvents;
use sc_consensus::ImportQueue;
use sc_executor::{HeapAllocStrategy, WasmExecutor, DEFAULT_HEAP_ALLOC_STRATEGY};
use sc_network::{NetworkBackend, NetworkBlock, PeerId};
use sc_service::{Configuration, PartialComponents, TFullBackend, TFullClient, TaskManager};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sp_api::ProvideRuntimeApi;
use sp_core::H256 as Hash;
use sp_keystore::KeystorePtr;
use sp_runtime::traits::Header as _;

use super::{
	proposer::CampaignEnvironment,
	provider::{self, CampaignLatch},
	receipt, Campaign,
};

type Executor = WasmExecutor<ParachainHostFunctions>;
pub(super) type Client = TFullClient<Block, RuntimeApi, Executor>;
type Backend = TFullBackend<Block>;
type BlockImport = TParachainBlockImport<Block, Arc<Client>, Backend>;
pub(super) type Pool = sc_transaction_pool::TransactionPoolHandle<Block, Client>;

type Service = PartialComponents<
	Client,
	Backend,
	(),
	sc_consensus::DefaultImportQueue<Block>,
	Pool,
	(BlockImport, Option<Telemetry>, Option<TelemetryWorkerHandle>),
>;

fn new_partial(config: &Configuration) -> Result<Service, sc_service::Error> {
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|endpoints| !endpoints.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let heap_pages =
		config.executor.default_heap_pages.map_or(DEFAULT_HEAP_ALLOC_STRATEGY, |pages| {
			HeapAllocStrategy::Static { extra_pages: pages as _ }
		});
	let executor = Executor::builder()
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
			Default::default(),
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
	let block_import = BlockImport::new(client.clone(), backend.clone());
	let import_queue =
		cumulus_client_consensus_aura::equivocation_import_queue::fully_verifying_import_queue::<
			sp_consensus_aura::sr25519::AuthorityPair,
			_,
			_,
			_,
			_,
		>(
			client.clone(),
			block_import.clone(),
			move |_, _| async move { Ok(sp_timestamp::InherentDataProvider::from_system_time()) },
			&task_manager.spawn_essential_handle(),
			config.prometheus_registry(),
			telemetry.as_ref().map(|telemetry| telemetry.handle()),
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

#[allow(clippy::too_many_arguments)]
fn start_consensus(
	client: Arc<Client>,
	block_import: BlockImport,
	prometheus: Option<&Registry>,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
	relay: Arc<dyn RelayChainInterface>,
	transaction_pool: Arc<Pool>,
	keystore: KeystorePtr,
	relay_slot: Duration,
	para_id: ParaId,
	collator_key: CollatorPair,
	collator_peer_id: PeerId,
	overseer_handle: OverseerHandle,
	announce_block: Arc<dyn Fn(Hash, Option<Vec<u8>>) + Send + Sync>,
	campaign: Campaign,
	latch: Arc<CampaignLatch>,
) -> Result<(), sc_service::Error> {
	let base_proposer = sc_basic_authorship::ProposerFactory::new(
		task_manager.spawn_handle(),
		client.clone(),
		transaction_pool,
		prometheus,
		telemetry.clone(),
	);
	let proposer =
		CampaignEnvironment::new(base_proposer, client.clone(), campaign.clone(), latch.clone());
	let collator_service = CollatorService::new(
		client.clone(),
		Arc::new(task_manager.spawn_handle()),
		announce_block,
		client.clone(),
	);
	let provider_client = client.clone();
	let provider_campaign = campaign.clone();
	let params = AuraParams {
		create_inherent_data_providers: move |parent, ()| {
			let client = provider_client.clone();
			let campaign = provider_campaign.clone();
			let latch = latch.clone();
			async move { provider::create(client, parent, campaign, latch).await }
		},
		block_import,
		para_client: client.clone(),
		relay_client: relay,
		keystore,
		collator_key,
		collator_peer_id,
		para_id,
		overseer_handle,
		relay_chain_slot_duration: relay_slot,
		proposer,
		collator_service,
		authoring_duration: Duration::from_millis(2000),
		collation_request_receiver: None,
	};
	let future =
		aura::run::<Block, sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _, _>(params);
	task_manager
		.spawn_essential_handle()
		.spawn("orbis-proof-campaign-aura", None, future);
	Ok(())
}

/// Start the feature-gated campaign collator.  The caller has already enforced the exact chain
/// id, explicit disposable acknowledgement, authority role, and positive target.
pub async fn start(
	parachain_config: Configuration,
	relay_config: Configuration,
	collator_options: CollatorOptions,
	campaign: Campaign,
) -> sc_service::error::Result<(TaskManager, Arc<Client>)> {
	let parachain_config = prepare_node_config(parachain_config);
	let params = new_partial(&parachain_config)?;
	let actual_genesis_hash = params.client.chain_info().genesis_hash;
	if actual_genesis_hash != campaign.expected_genesis_hash {
		receipt(
			&campaign,
			"genesis-mismatch",
			serde_json::json!({
				"actual_genesis_hash": format!("{actual_genesis_hash:?}"),
				"service_started": false,
			}),
		);
		return Err(sc_service::Error::Application(Box::new(std::io::Error::new(
			std::io::ErrorKind::InvalidInput,
			format!(
				"proof campaign genesis mismatch: expected {:?}, got {actual_genesis_hash:?}",
				campaign.expected_genesis_hash
			),
		))));
	}
	receipt(
		&campaign,
		"genesis-verified",
		serde_json::json!({ "actual_genesis_hash": format!("{actual_genesis_hash:?}") }),
	);
	let (block_import, mut telemetry, telemetry_worker_handle) = params.other;
	let prometheus = parachain_config.prometheus_registry().cloned();
	let net_config = sc_network::config::FullNetworkConfiguration::<
		_,
		_,
		sc_network::NetworkWorker<Block, Hash>,
	>::new(&parachain_config.network, prometheus.clone());
	let client = params.client.clone();
	let backend = params.backend.clone();
	let mut task_manager = params.task_manager;
	let relay_fork_id = relay_config.chain_spec.fork_id().map(ToString::to_string);
	let para_fork_id = parachain_config.chain_spec.fork_id().map(ToString::to_string);
	let advertise_non_global_ips = parachain_config.network.allow_non_globals_in_dht;
	let public_addresses = parachain_config.network.public_addresses.clone();

	let (relay, collator_key, relay_network, paranode_rx) = build_relay_chain_interface(
		relay_config,
		&parachain_config,
		telemetry_worker_handle,
		&mut task_manager,
		collator_options.clone(),
		None,
	)
	.await
	.map_err(|error| sc_service::Error::Application(Box::new(error)))?;
	let transaction_pool = params.transaction_pool.clone();
	let import_queue_service = params.import_queue.service();
	let best_hash = client.chain_info().best_hash;
	let para_id = client
		.runtime_api()
		.parachain_id(best_hash)
		.map_err(|_| "failed to retrieve Orbis parachain id from runtime")?;
	let (network, system_rpc_tx, tx_handler_controller, sync_service) =
		build_network(BuildNetworkParams {
			parachain_config: &parachain_config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			para_id,
			spawn_handle: task_manager.spawn_handle(),
			spawn_essential_handle: task_manager.spawn_essential_handle(),
			relay_chain_interface: relay.clone(),
			import_queue: params.import_queue,
			sybil_resistance_level: CollatorSybilResistance::Resistant,
			metrics: sc_network::NetworkWorker::<Block, Hash>::register_notification_metrics(
				parachain_config.prometheus_config.as_ref().map(|config| &config.registry),
			),
		})
		.await?;
	let collator_peer_id = relay_network.local_peer_id();
	let rpc_builder = Box::new(|_| -> std::result::Result<_, sc_service::Error> {
		Ok(jsonrpsee::RpcModule::new(()))
	});
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
		tracing_execute_block: None,
	})?;
	let announce_block = {
		let sync_service = sync_service.clone();
		Arc::new(move |hash, data| sync_service.announce_block(hash, data))
	};
	let relay_slot = Duration::from_secs(6);
	let overseer_handle = relay
		.overseer_handle()
		.map_err(|error| sc_service::Error::Application(Box::new(error)))?;
	start_relay_chain_tasks(StartRelayChainTasksParams {
		client: client.clone(),
		announce_block: announce_block.clone(),
		para_id,
		relay_chain_interface: relay.clone(),
		task_manager: &mut task_manager,
		da_recovery_profile: DARecoveryProfile::Collator,
		import_queue: import_queue_service,
		relay_chain_slot_duration: relay_slot,
		recovery_handle: Box::new(overseer_handle.clone()),
		sync_service: sync_service.clone(),
		prometheus_registry: prometheus.as_ref(),
	})?;
	start_bootnode_tasks(StartBootnodeTasksParams {
		embedded_dht_bootnode: collator_options.embedded_dht_bootnode,
		dht_bootnode_discovery: collator_options.dht_bootnode_discovery,
		para_id,
		task_manager: &mut task_manager,
		relay_chain_interface: relay.clone(),
		relay_chain_fork_id: relay_fork_id,
		relay_chain_network: relay_network,
		request_receiver: paranode_rx,
		parachain_network: network,
		advertise_non_global_ips,
		parachain_genesis_hash: client.chain_info().genesis_hash.encode(),
		parachain_fork_id: para_fork_id,
		parachain_public_addresses: public_addresses,
	});
	let latch = Arc::new(CampaignLatch::default());
	spawn_recovery_monitor(&task_manager, client.clone(), campaign.clone(), latch.clone());
	start_consensus(
		client.clone(),
		block_import,
		prometheus.as_ref(),
		telemetry.as_ref().map(|telemetry| telemetry.handle()),
		&task_manager,
		relay,
		transaction_pool,
		params.keystore_container.keystore(),
		relay_slot,
		para_id,
		collator_key.expect("authority role guarantees a collator key"),
		collator_peer_id,
		overseer_handle,
		announce_block,
		campaign,
		latch,
	)?;
	Ok((task_manager, client))
}

enum RecoveryEvent {
	Imported(u32, Hash),
	Finalized(u32, Hash),
}

fn spawn_recovery_monitor(
	task_manager: &TaskManager,
	client: Arc<Client>,
	campaign: Campaign,
	latch: Arc<CampaignLatch>,
) {
	let imports = client.every_import_notification_stream().map(|notification| {
		RecoveryEvent::Imported(*notification.header.number(), notification.hash)
	});
	let finality = client.finality_notification_stream().map(|notification| {
		RecoveryEvent::Finalized(*notification.header.number(), notification.hash)
	});
	let mut events = futures::stream::select(imports, finality);
	task_manager.spawn_essential_handle().spawn(
		"orbis-proof-campaign-recovery-monitor",
		None,
		async move {
			let mut pending_finality = None;
			while let Some(event) = events.next().await {
				match event {
					RecoveryEvent::Imported(number, hash) if number == campaign.target => {
						latch.mark_recovery_imported().unwrap_or_else(|error| {
							panic!("fault/recovery import ordering violated: {error}")
						});
						receipt(
							&campaign,
							"recovery-imported",
							serde_json::json!({ "hash": format!("{hash:?}") }),
						);
						if let Some((number, hash)) = pending_finality.take() {
							mark_recovery_finalized(&campaign, &latch, number, hash);
							break;
						}
					},
					RecoveryEvent::Finalized(number, hash) if number >= campaign.target => {
						if latch.mark_recovery_finalized().is_ok() {
							receipt(
								&campaign,
								"recovery-finalized",
								serde_json::json!({
									"hash": format!("{hash:?}"),
									"finalized_number": number,
									"campaign_complete": true,
								}),
							);
							break;
						}
						pending_finality = Some((number, hash));
					},
					_ => {},
				}
			}
		},
	);
}

fn mark_recovery_finalized(campaign: &Campaign, latch: &CampaignLatch, number: u32, hash: Hash) {
	latch
		.mark_recovery_finalized()
		.unwrap_or_else(|error| panic!("fault/recovery finality ordering violated: {error}"));
	receipt(
		campaign,
		"recovery-finalized",
		serde_json::json!({
			"hash": format!("{hash:?}"),
			"finalized_number": number,
			"campaign_complete": true,
		}),
	);
}
