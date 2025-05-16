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

//! CORD Origin container node.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

mod chain_spec;

use polkadot_omni_node_lib::{run, CliConfig as CliConfigT, RunConfig};

/// The current node version, which takes the basic SemVer form `<major>.<minor>.<patch>`.
/// In general, minor should be bumped on every release while major or patch releases are
/// relatively rare.
///
/// The associated worker binaries should use the same version as the node that spawns them.
pub const NODE_VERSION: &'static str = "0.9.7";

struct CliConfig;

impl CliConfigT for CliConfig {
	fn impl_name() -> String {
		"Dhiway Cord Origin".into()
	}

	fn impl_version() -> String {
		let commit_hash = env!("SUBSTRATE_CLI_COMMIT_HASH");
		format!("{}-{commit_hash}", NODE_VERSION)
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/dhiway/cord/issues/new".into()
	}

	fn copyright_start_year() -> u16 {
		2019
	}
}

fn main() -> color_eyre::eyre::Result<()> {
	color_eyre::install()?;

	let config = RunConfig::new(
		Box::new(chain_spec::RuntimeResolver),
		Box::new(chain_spec::ChainSpecLoader),
	);

	Ok(run::<CliConfig>(config)?)
}
