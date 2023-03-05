// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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

use crate::{
	chain_spec,
	cli::{Cli, Subcommand},
	service as cord_service,
};
use cord_client::benchmarking::{benchmark_inherent_data, RemarkBuilder, TransferKeepAliveBuilder};

use cord_primitives::Block;
use cord_runtime::{ExistentialDeposit, RuntimeApi};
use cord_service::IdentifyVariant;
use frame_benchmarking_cli::*;
use sc_cli::{ChainSpec, ExecutionStrategy, RuntimeVersion, SubstrateCli};
use sc_service::config::BasePath;
use sc_service::PartialComponents;
use sp_keyring::Sr25519Keyring;

use std::sync::Arc;

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Dhiway CORD".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
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

	// fn executable_name() -> String {
	// 	"cord".into()
	// }

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		let spec = match id {
			"" => {
				return Err(
					"Please specify which chain you want to run, e.g. --dev or --chain=local"
						.into(),
				)
			},
			// "cord" => Box::new(chain_spec::cord_config()),
			"cord-dev" | "dev" => Box::new(chain_spec::cord_development_config()?),
			"cord-local" | "local" => Box::new(chain_spec::cord_local_testnet_config()?),
			"cord-staging" | "staging" => Box::new(chain_spec::cord_staging_config()?),
			path => {
				Box::new(chain_spec::CordChainSpec::from_json_file(std::path::PathBuf::from(path))?)
			},
		};
		Ok(spec)
	}

	fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		&cord_runtime::VERSION
	}
}

/// Unwraps a [`service::Client`] into the concrete runtime client.
#[allow(unused)]
macro_rules! unwrap_client {
	(
        $client:ident,
        $code:expr
    ) => {
		match $client.as_ref() {
			cord_service::Client::Cord($client) => $code,
			#[allow(unreachable_patterns)]
			_ => Err("invalid chain spec".into()),
		}
	};
}

/// Parse command line arguments into service configuration.
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	// let old_base = BasePath::from_project("", "", "cord-node");
	// let new_base = BasePath::from_project("", "", &Cli::executable_name());
	// if old_base.path().exists() && !new_base.path().exists() {
	// 	_ = std::fs::rename(old_base.path(), new_base.path());
	// }

	// // Force setting `Wasm` as default execution strategy.
	// cli.run
	// 	.base
	// 	.import_params
	// 	.execution_strategies
	// 	.execution
	// 	.get_or_insert(ExecutionStrategy::Wasm);

	match &cli.subcommand {
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| async move {
				cord_service::new_full(config, cli).map_err(sc_cli::Error::Service)
			})
		},
		Some(Subcommand::Inspect(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| {
				cmd.run::<Block, RuntimeApi, cord_client::CordExecutorDispatch>(config)
			})
		},
		Some(Subcommand::Key(cmd)) => cmd.run(&cli),
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let (client, _, import_queue, task_manager) = cord_service::new_chain_ops(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let (client, _, _, task_manager) = cord_service::new_chain_ops(&config)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let (client, _, _, task_manager) = cord_service::new_chain_ops(&config)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let (client, _, import_queue, task_manager) = cord_service::new_chain_ops(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.database))
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let (client, backend, _, task_manager) = cord_service::new_chain_ops(&config)?;
				let aux_revert = Box::new(|client, backend, blocks| {
					cord_service::revert_backend(client, backend, blocks, config)
						.map_err(|err| sc_cli::Error::Application(err.into()))
				});
				Ok((cmd.run(client, backend, Some(aux_revert)), task_manager))
			})
		},
		#[cfg(feature = "runtime-benchmarks")]
		Some(Subcommand::Benchmark(cmd)) => {
			use crate::{inherent_benchmark_data, RemarkBuilder, TransferKeepAliveBuilder};
			use frame_benchmarking_cli::{
				BenchmarkCmd, ExtrinsicFactory, SUBSTRATE_REFERENCE_HARDWARE,
			};
			use sp_keyring::Sr25519Keyring;

			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| {
				// This switch needs to be in the client, since the client decides
				// which sub-commands it wants to support.
				match cmd {
					BenchmarkCmd::Pallet(cmd) => {
						if !cfg!(feature = "runtime-benchmarks") {
							return Err(
								"Runtime benchmarking wasn't enabled when building the node. \
                            You can enable it with `--features runtime-benchmarks`."
									.into(),
							);
						}
						match &config.chain_spec {
							spec if spec.is_cord() => cmd
								.run::<cord_service::cord_runtime::Block, cord_service::CordExecutorDispatch>(
									config,
								),
							_ => Err("invalid chain spec".into()),
						}
					},
					BenchmarkCmd::Block(cmd) => {
						let (client, _, _, _) = cord_service::new_chain_ops(&config)?;

						unwrap_client!(client, cmd.run(client.clone()))
					},
					#[cfg(not(feature = "runtime-benchmarks"))]
					BenchmarkCmd::Storage(_) => Err(
						"Storage benchmarking can be enabled with `--features runtime-benchmarks`."
							.into(),
					),
					#[cfg(feature = "runtime-benchmarks")]
					BenchmarkCmd::Storage(cmd) => {
						let (client, backend, _, _) = cord_service::new_chain_ops(&config)?;
						let db = backend.expose_db();
						let storage = backend.expose_storage();

						unwrap_client!(client, cmd.run(config, client.clone(), db, storage))
					},
					BenchmarkCmd::Overhead(cmd) => {
						let inherent_data = inherent_benchmark_data().map_err(|e| {
							sc_cli::Error::from(format!("generating inherent data: {e:?}"))
						})?;

						let (client, _, _, _) = cord_service::new_chain_ops(&config)?;
						let ext_builder = RemarkBuilder::new(client.clone());

						unwrap_client!(
							client,
							cmd.run(
								config,
								client.clone(),
								inherent_data,
								Vec::new(),
								&ext_builder
							)
						)
					},
					BenchmarkCmd::Extrinsic(cmd) => {
						let inherent_data = inherent_benchmark_data().map_err(|e| {
							sc_cli::Error::from(format!("generating inherent data: {e:?}"))
						})?;
						let (client, _, _, _) = cord_service::new_chain_ops(&config)?;
						// Register the *Remark* and *TKA* builders.
						let ext_factory = ExtrinsicFactory(vec![
							Box::new(RemarkBuilder::new(client.clone())),
							Box::new(TransferKeepAliveBuilder::new(
								client.clone(),
								Sr25519Keyring::Alice.to_account_id(),
							)),
						]);

						unwrap_client!(
							client,
							cmd.run(client.clone(), inherent_data, Vec::new(), &ext_factory)
						)
					},
					BenchmarkCmd::Machine(cmd) => {
						cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone())
					},
				}
			})
		},
		#[cfg(feature = "try-runtime")]
		Some(Subcommand::TryRuntime(cmd)) => {
			use sc_executor::{sp_wasm_interface::ExtendedHostFunctions, NativeExecutionDispatch};
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			let registry = &runner.config().prometheus_config.as_ref().map(|cfg| &cfg.registry);
			let task_manager =
				sc_service::TaskManager::new(runner.config().tokio_handle.clone(), *registry)
					.map_err(|e| sc_cli::Error::Service(sc_service::Error::Prometheus(e)))?;

			match chain_spec {
				spec if spec.is_cord() => runner.async_run(|_| {
					Ok((
                        cmd.run::<cord_service::cord_runtime::Block, ExtendedHostFunctions<
						sp_io::SubstrateHostFunctions,
						<cord_service::COrdExecutorDispatch as NativeExecutionDispatch>::ExtendHostFunctions,
					>>(
                        ),
                        task_manager,
                    ))
				}),
				_ => panic!("No runtime is enabled"),
			}
		},
		#[cfg(not(feature = "try-runtime"))]
		Some(Subcommand::TryRuntime) => Err("TryRuntime wasn't enabled when building the node. \
                You can enable it with `--features try-runtime`."
			.into()),
		Some(Subcommand::ChainInfo(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run::<Block>(&config))
		},
		_ => todo!(),
	}
}
