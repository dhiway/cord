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

#[cfg(feature = "runtime-benchmarks")]
use crate::service::Block;
use crate::{
	chain_spec,
	chain_spec::GenericChainSpec,
	cli::{Cli, RelayChainCli, Subcommand},
	common::NodeExtraArgs,
	fake_runtime_api::aura::RuntimeApi as AuraRuntimeApi,
	service::{new_aura_node_spec, DynNodeSpec, ShellNode},
};
#[cfg(feature = "runtime-benchmarks")]
use cumulus_client_service::storage_proof_size::HostFunctions as ReclaimHostFunctions;
use cumulus_primitives_core::ParaId;
use frame_benchmarking_cli::{BenchmarkCmd, SUBSTRATE_REFERENCE_HARDWARE};
use log::info;
use parachains_common::AuraId;
use sc_cli::{
	ChainSpec, CliConfiguration, DefaultConfigurationValues, ImportParams, KeystoreParams,
	NetworkParams, Result, SharedParams, SubstrateCli,
};
use sc_service::config::{BasePath, PrometheusConfig};
use sp_runtime::traits::AccountIdConversion;
#[cfg(feature = "runtime-benchmarks")]
use sp_runtime::traits::HashingFor;
use std::{net::SocketAddr, path::PathBuf};

/// The choice of consensus for the parachain omni-node.
#[derive(PartialEq, Eq, Debug, Default)]
pub enum Consensus {
	/// Aura consensus.
	#[default]
	Aura,
	/// Use the relay chain consensus.
	// TODO: atm this is just a demonstration, not really reach-able. We can add it to the CLI,
	// env, or the chain spec. Or, just don't, and when we properly refactor this mess we will
	// re-introduce it.
	#[allow(unused)]
	Relay,
}

/// Helper enum that is used for better distinction of different parachain/runtime configuration
/// (it is based/calculated on ChainSpec's ID attribute)
#[derive(Debug, PartialEq)]
enum Runtime {
	Omni(Consensus),
	AssetHub,
	Coretime,
}

trait RuntimeResolver {
	fn runtime(&self) -> Result<Runtime>;
}

impl RuntimeResolver for dyn ChainSpec {
	fn runtime(&self) -> Result<Runtime> {
		Ok(runtime(self.id()))
	}
}

/// Implementation, that can resolve [`Runtime`] from any json configuration file
impl RuntimeResolver for PathBuf {
	fn runtime(&self) -> Result<Runtime> {
		#[derive(Debug, serde::Deserialize)]
		struct EmptyChainSpecWithId {
			id: String,
		}

		let file = std::fs::File::open(self)?;
		let reader = std::io::BufReader::new(file);
		let chain_spec: EmptyChainSpecWithId =
			serde_json::from_reader(reader).map_err(|e| sc_cli::Error::Application(Box::new(e)))?;

		Ok(runtime(&chain_spec.id))
	}
}

fn runtime(id: &str) -> Runtime {
	let id = id.replace('_', "-");

	if id.starts_with("loom-asset-hub") | id.starts_with("asset-hub-loom") {
		Runtime::AssetHub
	} else if id.starts_with("loom-asset-hub") | id.starts_with("asset-hub-loom") {
		Runtime::Coretime
	} else {
		log::warn!(
			"No specific runtime was recognized for ChainSpec's id: '{}', \
			so Runtime::Omni(Consensus::Aura) will be used",
			id
		);
		Runtime::Omni(Consensus::Aura)
	}
}

fn load_spec(id: &str) -> std::result::Result<Box<dyn ChainSpec>, String> {
	Ok(match id {
		// -- Asset Hub
		"loom-asset-hub-dev" =>
			Box::new(chain_spec::asset_hub::asset_hub_loom_development_config()) as Box<_>,
		"loom-asset-hub-local" =>
			Box::new(chain_spec::asset_hub::asset_hub_loom_local_testnet_config()),
		// -- Coretime
		"loom-coretime-dev" =>
			Box::new(chain_spec::coretime::coretime_loom_development_config()) as Box<_>,
		"loom-coretime-local" =>
			Box::new(chain_spec::coretime::coretime_loom_local_testnet_config()),
		// -- Loading a specific spec from disk
		path => Box::new(GenericChainSpec::from_json_file(path.into())?),
	})
}

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Dhiway CORD Weave".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		format!(
			"The command-line arguments provided first will be passed to the parachain node, \n\
			and the arguments provided after -- will be passed to the relay chain node. \n\
			\n\
			Example: \n\
			\n\
			{} [parachain-args] -- [relay-chain-args]",
			Self::executable_name()
		)
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/dhiway/cord/issues/new".into()
	}

	fn copyright_start_year() -> i32 {
		2019
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn ChainSpec>, String> {
		load_spec(id)
	}
}

impl SubstrateCli for RelayChainCli {
	fn impl_name() -> String {
		Cli::impl_name()
	}

	fn impl_version() -> String {
		Cli::impl_version()
	}

	fn description() -> String {
		Cli::description()
	}

	fn author() -> String {
		Cli::author()
	}

	fn support_url() -> String {
		Cli::support_url()
	}

	fn copyright_start_year() -> i32 {
		Cli::copyright_start_year()
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn ChainSpec>, String> {
		cord_loom_node_cli::Cli::from_iter([RelayChainCli::executable_name()].iter()).load_spec(id)
	}
}

fn new_node_spec(
	config: &sc_service::Configuration,
	extra_args: NodeExtraArgs,
) -> std::result::Result<Box<dyn DynNodeSpec>, sc_cli::Error> {
	Ok(match config.chain_spec.runtime()? {
		Runtime::AssetHub | Runtime::Coretime =>
			new_aura_node_spec::<AuraRuntimeApi, AuraId>(extra_args),
		Runtime::Omni(consensus) => match consensus {
			Consensus::Aura => new_aura_node_spec::<AuraRuntimeApi, AuraId>(extra_args),
			Consensus::Relay => Box::new(ShellNode),
		},
	})
}

/// Parse command line arguments into service configuration.
pub fn run() -> Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let node = new_node_spec(&config, cli.node_extra_args())?;
				node.prepare_check_block_cmd(config, cmd)
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let node = new_node_spec(&config, cli.node_extra_args())?;
				node.prepare_export_blocks_cmd(config, cmd)
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let node = new_node_spec(&config, cli.node_extra_args())?;
				node.prepare_export_state_cmd(config, cmd)
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let node = new_node_spec(&config, cli.node_extra_args())?;
				node.prepare_import_blocks_cmd(config, cmd)
			})
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let node = new_node_spec(&config, cli.node_extra_args())?;
				node.prepare_revert_cmd(config, cmd)
			})
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let cord_loom_node_cli =
				RelayChainCli::new(runner.config(), cli.relay_chain_args.iter());

			runner.sync_run(|config| {
				let cord_loom_config = SubstrateCli::create_configuration(
					&cord_loom_node_cli,
					&cord_loom_node_cli,
					config.tokio_handle.clone(),
				)
				.map_err(|err| format!("Relay chain argument error: {}", err))?;

				cmd.run(config, cord_loom_config)
			})
		},
		Some(Subcommand::ExportGenesisHead(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| {
				let node = new_node_spec(&config, cli.node_extra_args())?;
				node.run_export_genesis_head_cmd(config, cmd)
			})
		},
		Some(Subcommand::ExportGenesisWasm(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|_config| {
				let spec = cli.load_spec(&cmd.shared_params.chain.clone().unwrap_or_default())?;
				cmd.run(&*spec)
			})
		},
		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			// Switch on the concrete benchmark sub-command-
			match cmd {
				#[cfg(feature = "runtime-benchmarks")]
				BenchmarkCmd::Pallet(cmd) => runner.sync_run(|config| {
					cmd.run_with_spec::<HashingFor<Block>, ReclaimHostFunctions>(Some(
						config.chain_spec,
					))
				}),
				BenchmarkCmd::Block(cmd) => runner.sync_run(|config| {
					let node = new_node_spec(&config, cli.node_extra_args())?;
					node.run_benchmark_block_cmd(config, cmd)
				}),
				#[cfg(feature = "runtime-benchmarks")]
				BenchmarkCmd::Storage(cmd) => runner.sync_run(|config| {
					let node = new_node_spec(&config, cli.node_extra_args())?;
					node.run_benchmark_storage_cmd(config, cmd)
				}),
				BenchmarkCmd::Machine(cmd) =>
					runner.sync_run(|config| cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone())),
				#[allow(unreachable_patterns)]
				_ => Err("Benchmarking sub-command unsupported or compilation feature missing. \
					Make sure to compile with --features=runtime-benchmarks \
					to enable all supported benchmarks."
					.into()),
			}
		},
		Some(Subcommand::Key(cmd)) => Ok(cmd.run(&cli)?),
		None => {
			let runner = cli.create_runner(&cli.run.normalize())?;
			let cord_loom_node_cli =
				RelayChainCli::new(runner.config(), cli.relay_chain_args.iter());
			let collator_options = cli.run.collator_options();

			runner.run_node_until_exit(|config| async move {
				let hwbench = (!cli.no_hardware_benchmarks)
					.then_some(config.database.path().map(|database_path| {
						let _ = std::fs::create_dir_all(database_path);
						sc_sysinfo::gather_hwbench(Some(database_path))
					}))
					.flatten();

				let para_id = chain_spec::Extensions::try_get(&*config.chain_spec)
					.map(|e| e.para_id)
					.ok_or("Could not find parachain extension in chain-spec.")?;

				let id = ParaId::from(para_id);

				let parachain_account =
					AccountIdConversion::<polkadot_primitives::AccountId>::into_account_truncating(
						&id,
					);

				let tokio_handle = config.tokio_handle.clone();
				let cord_loom_config = SubstrateCli::create_configuration(
					&cord_loom_node_cli,
					&cord_loom_node_cli,
					tokio_handle,
				)
				.map_err(|err| format!("Relay chain argument error: {}", err))?;

				info!("🪪 Parachain id: {:?}", id);
				info!("🧾 Parachain Account: {}", parachain_account);
				info!("✍️ Is collating: {}", if config.role.is_authority() { "yes" } else { "no" });

				start_node(
					config,
					cord_loom_config,
					collator_options,
					id,
					cli.node_extra_args(),
					hwbench,
				)
				.await
			})
		},
	}
}

#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node(
	config: sc_service::Configuration,
	cord_loom_config: sc_service::Configuration,
	collator_options: cumulus_client_cli::CollatorOptions,
	id: ParaId,
	extra_args: NodeExtraArgs,
	hwbench: Option<sc_sysinfo::HwBench>,
) -> Result<sc_service::TaskManager> {
	let node_spec = new_node_spec(&config, extra_args)?;
	node_spec
		.start_node(config, cord_loom_config, collator_options, id, hwbench)
		.await
		.map_err(Into::into)
}

impl DefaultConfigurationValues for RelayChainCli {
	fn p2p_listen_port() -> u16 {
		30334
	}

	fn rpc_listen_port() -> u16 {
		9945
	}

	fn prometheus_listen_port() -> u16 {
		9616
	}
}

impl CliConfiguration<Self> for RelayChainCli {
	fn shared_params(&self) -> &SharedParams {
		self.base.base.shared_params()
	}

	fn import_params(&self) -> Option<&ImportParams> {
		self.base.base.import_params()
	}

	fn network_params(&self) -> Option<&NetworkParams> {
		self.base.base.network_params()
	}

	fn keystore_params(&self) -> Option<&KeystoreParams> {
		self.base.base.keystore_params()
	}

	fn base_path(&self) -> Result<Option<BasePath>> {
		Ok(self
			.shared_params()
			.base_path()?
			.or_else(|| self.base_path.clone().map(Into::into)))
	}

	fn rpc_addr(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
		self.base.base.rpc_addr(default_listen_port)
	}

	fn prometheus_config(
		&self,
		default_listen_port: u16,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<PrometheusConfig>> {
		self.base.base.prometheus_config(default_listen_port, chain_spec)
	}

	fn init<F>(
		&self,
		_support_url: &String,
		_impl_version: &String,
		_logger_hook: F,
		_config: &sc_service::Configuration,
	) -> Result<()>
	where
		F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
	{
		unreachable!("LoomCli is never initialized; qed");
	}

	fn chain_id(&self, is_dev: bool) -> Result<String> {
		let chain_id = self.base.base.chain_id(is_dev)?;

		Ok(if chain_id.is_empty() { self.chain_id.clone().unwrap_or_default() } else { chain_id })
	}

	fn role(&self, is_dev: bool) -> Result<sc_service::Role> {
		self.base.base.role(is_dev)
	}

	fn transaction_pool(&self, is_dev: bool) -> Result<sc_service::config::TransactionPoolOptions> {
		self.base.base.transaction_pool(is_dev)
	}

	fn trie_cache_maximum_size(&self) -> Result<Option<usize>> {
		self.base.base.trie_cache_maximum_size()
	}

	fn rpc_methods(&self) -> Result<sc_service::config::RpcMethods> {
		self.base.base.rpc_methods()
	}

	fn rpc_max_connections(&self) -> Result<u32> {
		self.base.base.rpc_max_connections()
	}

	fn rpc_cors(&self, is_dev: bool) -> Result<Option<Vec<String>>> {
		self.base.base.rpc_cors(is_dev)
	}

	fn default_heap_pages(&self) -> Result<Option<u64>> {
		self.base.base.default_heap_pages()
	}

	fn force_authoring(&self) -> Result<bool> {
		self.base.base.force_authoring()
	}

	fn disable_grandpa(&self) -> Result<bool> {
		self.base.base.disable_grandpa()
	}

	fn max_runtime_instances(&self) -> Result<Option<usize>> {
		self.base.base.max_runtime_instances()
	}

	fn announce_block(&self) -> Result<bool> {
		self.base.base.announce_block()
	}

	fn telemetry_endpoints(
		&self,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<sc_telemetry::TelemetryEndpoints>> {
		self.base.base.telemetry_endpoints(chain_spec)
	}

	fn node_name(&self) -> Result<String> {
		self.base.base.node_name()
	}
}

#[cfg(test)]
mod tests {
	// use cord_weave_system_parachains_constants::genesis_presets::{
	// 	get_account_id_from_seed, get_from_seed,
	// };
	use sc_chain_spec::{ChainSpec, ChainSpecExtension, ChainSpecGroup, ChainType, Extension};
	use serde::{Deserialize, Serialize};
	// use sp_core::sr25519;
	use std::path::PathBuf;
	use tempfile::TempDir;

	#[derive(
		Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension, Default,
	)]
	#[serde(deny_unknown_fields)]
	pub struct Extensions1 {
		pub attribute1: String,
		pub attribute2: u32,
	}

	#[derive(
		Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension, Default,
	)]
	#[serde(deny_unknown_fields)]
	pub struct Extensions2 {
		pub attribute_x: String,
		pub attribute_y: String,
		pub attribute_z: u32,
	}

	#[allow(dead_code)]
	fn store_configuration(dir: &TempDir, spec: &dyn ChainSpec) -> PathBuf {
		let raw_output = true;
		let json = sc_service::chain_ops::build_spec(spec, raw_output)
			.expect("Failed to build json string");
		let mut cfg_file_path = dir.path().to_path_buf();
		cfg_file_path.push(spec.id());
		cfg_file_path.set_extension("json");
		std::fs::write(&cfg_file_path, json).expect("Failed to write to json file");
		cfg_file_path
	}

	#[allow(dead_code)]
	pub type DummyChainSpec<E> = sc_service::GenericChainSpec<E>;

	#[allow(dead_code)]
	pub fn create_default_with_extensions<E: Extension>(
		id: &str,
		extension: E,
	) -> DummyChainSpec<E> {
		DummyChainSpec::builder(
			cord_weave_asset_hub_runtime::WASM_BINARY
				.expect("WASM binary was not built, please build it!"),
			extension,
		)
		.with_name("Dummy local testnet")
		.with_id(id)
		.with_chain_type(ChainType::Local)
		.with_genesis_config_patch(
			cord_weave_asset_hub_runtime::genesis_config_presets::asset_hub_loom_development_genesis(
				1000.into(),
			),
		)
		.build()
	}
}
