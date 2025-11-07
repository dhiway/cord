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

// Shared CLI subcommand definitions.

use std::path::PathBuf;

use sc_cli::{
	clap::{self, Args},
	KeySubcommand as ScKeySubcommand, KeystoreParams, SharedParams,
};

#[derive(Debug, Args)]
pub struct BootstrapChainCmd {
	#[arg(long = "raw")]
	pub raw: bool,

	#[arg(long, short = 'c')]
	pub config: PathBuf,
}

#[derive(Debug, clap::Subcommand)]
pub enum KeySubcommand {
	/// Generate session keys and store them in the keystore.
	GenerateSessionKeys(GenSessionKeysCmd),

	#[allow(missing_docs)]
	#[clap(flatten)]
	Key(ScKeySubcommand),
}

#[derive(Debug, clap::Args)]
pub struct GenSessionKeysCmd {
	/// The secret key URI.
	/// If the value is a file, the file content is used as URI.
	/// If not given, you will be prompted for the URI.
	#[clap(long)]
	pub suri: Option<String>,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub shared_params: SharedParams,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub keystore_params: KeystoreParams,
}
