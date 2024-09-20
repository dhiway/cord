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

//! CORD CLI library.

#![allow(missing_docs)]
use crate::command::gen_key::KeySubcommand;

use clap::Parser;

use crate::chain_setup::BootstrapChainCmd;

#[allow(missing_docs)]
#[derive(Debug, Parser)]
pub enum Subcommand {
	/// The custom inspect subcommmand for decoding blocks and extrinsics.
	#[command(
		name = "inspect",
		about = "Decode given block or extrinsic using current native runtime."
	)]
	Inspect(cord_braid_node_inspect::cli::InspectCmd),

	/// Bootstrap a custom configuration
	BootstrapChain(BootstrapChainCmd),

	/// Key management cli utilities
	#[command(subcommand)]
	Key(KeySubcommand),

	/// Build a chain specification.
	BuildSpec(sc_cli::BuildSpecCmd),

	// Verify a signature for a message, provided on STDIN, with a given
	/// (public or secret) key.
	Verify(sc_cli::VerifyCmd),

	/// Generate a seed that provides a vanity address.
	Vanity(sc_cli::VanityCmd),

	/// Sign a message, with a given (secret) key.
	Sign(sc_cli::SignCmd),

	/// Validate blocks.
	CheckBlock(sc_cli::CheckBlockCmd),

	/// Export blocks.
	ExportBlocks(sc_cli::ExportBlocksCmd),

	/// Export the state of a given block into a chain spec.
	ExportState(sc_cli::ExportStateCmd),

	/// Import blocks.
	ImportBlocks(sc_cli::ImportBlocksCmd),

	/// Remove the whole chain.
	PurgeChain(sc_cli::PurgeChainCmd),

	/// Revert the chain to a previous state.
	Revert(sc_cli::RevertCmd),

	/// Sub-commands concerned with benchmarking.
	/// The pallet benchmarking moved to the `pallet` sub-command.
	#[command(subcommand)]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),

	/// Db meta columns information.
	ChainInfo(sc_cli::ChainInfoCmd),

	/// Start Braid Base development node
	Base {
		#[clap(
			long = "dev",
			required = true,
			help = "Run the Braid Base node in development mode"
		)]
		dev: bool,
	},

	/// Start Braid Plus development node
	Plus {
		#[clap(
			long = "dev",
			required = true,
			help = "Run the Braid Plus node in development mode"
		)]
		dev: bool,
	},
}

#[allow(missing_docs)]
#[derive(Debug, Parser)]
pub struct Cli {
	/// Possible subcommand with parameters.
	#[command(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub run: sc_cli::RunCmd,

	#[arg(long)]
	pub no_hardware_benchmarks: bool,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub storage_monitor: sc_storage_monitor::StorageMonitorParams,
}
