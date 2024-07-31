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

#![allow(missing_docs)]

pub mod chain_setup;
pub mod gen_key;

use crate::{
	benchmarking::{inherent_benchmark_data, RemarkBuilder, TransferKeepAliveBuilder},
	chain_spec,
	cli::{Cli, Subcommand},
	service::{self as cord_service, IdentifyVariant, RuntimeApi},
};

use cord_primitives::Block;
use cord_runtime_common::Ss58AddressFormatPrefix;
use cord_service::{new_partial, FullClient};
use frame_benchmarking_cli::{BenchmarkCmd, ExtrinsicFactory, SUBSTRATE_REFERENCE_HARDWARE};
use sc_cli::{Result, SubstrateCli};
use sc_service::PartialComponents;
use sp_core::crypto::Ss58AddressFormat;
use sp_keyring::Sr25519Keyring;
use std::sync::Arc;

fn get_exec_name() -> Option<String> {
	std::env::current_exe()
		.ok()
		.and_then(|pb| pb.file_name().map(|s| s.to_os_string()))
		.and_then(|s| s.into_string().ok())
}

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

	fn executable_name() -> String {
		"cord".into()
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		let incoming_id = id;

		let id = if id == "" {
			let n = get_exec_name().unwrap_or_default();
			["braid", "loom", "weave"]
				.iter()
				.cloned()
				.find(|&chain| n.starts_with(chain))
				.unwrap_or("loom")
		} else {
			match id {
				"dev-node-braid" | "dev-node-loom" | "dev-node-weave" => "dev",
				_ => id,
			}
		};
		Ok(match id {
			#[cfg(feature = "braid-native")]
			"braid" | "braid-local" => Box::new(chain_spec::braid_local_config()?),
			#[cfg(feature = "loom-native")]
			"loom" | "loom-local" => Box::new(chain_spec::loom_local_config()?),
			#[cfg(feature = "weave-native")]
			"weave" | "weave-local" => Box::new(chain_spec::weave_local_config()?),
			"dev" => match incoming_id {
				"dev-node-braid" => Box::new(chain_spec::braid_development_config()?),
				"dev-node-loom" => Box::new(chain_spec::loom_development_config()?),
				"dev-node-weave" => Box::new(chain_spec::weave_development_config()?),
				_ => Box::new(chain_spec::loom_development_config()?),
			},
			#[cfg(not(feature = "braid-native"))]
			name if name.starts_with("braid-") && !name.ends_with(".json") =>
				Err(format!("`{}` only supported with `braid-native` feature enabled.", name))?,
			#[cfg(not(feature = "loom-native"))]
			name if name.starts_with("loom-") && !name.ends_with(".json") =>
				Err(format!("`{}` only supported with `loom-native` feature enabled.", name))?,
			// "weave" => Box::new(chain_spec::weave_config()?),
			path => {
				let path = std::path::PathBuf::from(path);

				let chain_spec =
					Box::new(cord_service::GenericChainSpec::from_json_file(path.clone())?)
						as Box<dyn cord_service::ChainSpec>;

				// When the file name starts with the name of one of the known
				// chains, we use the chain spec for the specific chain.
				if chain_spec.is_braid() {
					Box::new(cord_service::BraidChainSpec::from_json_file(path)?)
				} else if chain_spec.is_loom() {
					Box::new(cord_service::LoomChainSpec::from_json_file(path)?)
				} else if chain_spec.is_weave() {
					Box::new(cord_service::WeaveChainSpec::from_json_file(path)?)
				} else {
					chain_spec
				}
			},
		})
	}
}

fn set_default_ss58_version(spec: &Box<dyn cord_service::ChainSpec>) {
	let ss58_version = if spec.is_weave() {
		Ss58AddressFormatPrefix::Weave.into()
	} else if spec.is_loom() {
		Ss58AddressFormatPrefix::Loom.into()
	} else if spec.is_braid() {
		Ss58AddressFormatPrefix::Braid.into()
	} else {
		spec.properties()
			.get("ss58Format")
			.and_then(|v| v.as_u64())
			.map(|v| Ss58AddressFormat::custom(v as u16))
			.unwrap_or_else(|| Ss58AddressFormatPrefix::Default.into())
	};

	sp_core::crypto::set_default_ss58_version(ss58_version);
}

/// Parse command line arguments into service configuration.
pub fn run() -> Result<()> {
	let mut cli: Cli = Cli::from_args();

	match &cli.subcommand {
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| async move {
				let chain_spec = config.chain_spec.cloned_box();
				set_default_ss58_version(&chain_spec);
				cord_service::new_full(config, cli).map_err(sc_cli::Error::Service)
			})
		},
		Some(Subcommand::Braid { dev: _ }) => {
			cli.run.shared_params.dev = true;
			cli.run.shared_params.chain = Some("dev-node-braid".into());
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| async move {
				let chain_spec = config.chain_spec.cloned_box();
				set_default_ss58_version(&chain_spec);
				cord_service::new_full(config, cli).map_err(sc_cli::Error::Service)
			})
		},
		Some(Subcommand::Loom { dev: _ }) => {
			cli.run.shared_params.dev = true;
			cli.run.shared_params.chain = Some("dev-node-loom".into());
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| async move {
				let chain_spec = config.chain_spec.cloned_box();
				set_default_ss58_version(&chain_spec);
				cord_service::new_full(config, cli).map_err(sc_cli::Error::Service)
			})
		},
		Some(Subcommand::Weave { dev: _ }) => {
			cli.run.shared_params.dev = true;
			cli.run.shared_params.chain = Some("dev-node-weave".into());
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| async move {
				let chain_spec = config.chain_spec.cloned_box();
				set_default_ss58_version(&chain_spec);
				cord_service::new_full(config, cli).map_err(sc_cli::Error::Service)
			})
		},
		Some(Subcommand::Inspect(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| cmd.run::<Block, RuntimeApi>(config))
		},
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::Key(cmd)) => cmd.run(&cli),
		Some(Subcommand::BootstrapChain(cmd)) => cmd.run(),
		Some(Subcommand::Sign(cmd)) => cmd.run(),
		Some(Subcommand::Verify(cmd)) => cmd.run(),
		Some(Subcommand::Vanity(cmd)) => cmd.run(),
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } = new_partial(&config)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } = new_partial(&config)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					new_partial(&config)?;
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
				let PartialComponents { client, task_manager, backend, .. } = new_partial(&config)?;
				let aux_revert = Box::new(|client: Arc<FullClient>, backend, blocks| {
					sc_consensus_babe::revert(client.clone(), backend, blocks)?;
					sc_consensus_grandpa::revert(client, blocks)?;
					Ok(())
				});
				Ok((cmd.run(client, backend, Some(aux_revert)), task_manager))
			})
		},
		Some(Subcommand::Benchmark(cmd)) => {
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
							)
						}
						cmd.run_with_spec::<sp_runtime::traits::HashingFor<cord_service::Block>, ()>(Some(config.chain_spec))
					},
					BenchmarkCmd::Block(cmd) => {
						// ensure that we keep the task manager alive
						let partial = new_partial(&config)?;
						cmd.run(partial.client)
					},
					#[cfg(not(feature = "runtime-benchmarks"))]
					BenchmarkCmd::Storage(_) => Err(
						"Storage benchmarking can be enabled with `--features runtime-benchmarks`."
							.into(),
					),
					#[cfg(feature = "runtime-benchmarks")]
					BenchmarkCmd::Storage(cmd) => {
						// ensure that we keep the task manager alive
						let partial = new_partial(&config)?;
						let db = partial.backend.expose_db();
						let storage = partial.backend.expose_storage();

						cmd.run(config, partial.client, db, storage)
					},
					BenchmarkCmd::Overhead(cmd) => {
						// ensure that we keep the task manager alive
						let partial = new_partial(&config)?;
						let ext_builder = RemarkBuilder::new(partial.client.clone(),config.chain_spec.identify_chain());

						cmd.run(
							config,
							partial.client,
							inherent_benchmark_data()?,
							Vec::new(),
							&ext_builder,
						)
					},
					BenchmarkCmd::Extrinsic(cmd) => {
						// ensure that we keep the task manager alive
						let partial = cord_service::new_partial(&config)?;
						// Register the *Remark* and *TKA* builders.
						let ext_factory = ExtrinsicFactory(vec![
							Box::new(RemarkBuilder::new(partial.client.clone(),config.chain_spec.identify_chain())),
							Box::new(TransferKeepAliveBuilder::new(
								partial.client.clone(),
								Sr25519Keyring::Alice.to_account_id(),
								config.chain_spec.identify_chain(),
							)),
						]);

						cmd.run(
							partial.client,
							inherent_benchmark_data()?,
							Vec::new(),
							&ext_factory,
						)
					},
					BenchmarkCmd::Machine(cmd) => {
						cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone())
					},
				}
			})
		},
		Some(Subcommand::ChainInfo(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run::<Block>(&config))
		},
	}
}
