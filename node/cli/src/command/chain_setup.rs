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

use std::{fs, io::Write, path::PathBuf};

use sc_cli::{
	clap::{self, Args},
	Error,
};
use serde::Deserialize;

use crate::chain_spec::{
	bootstrap::{cord_custom_config, ChainParams},
	ChainType,
};

#[derive(Debug, Deserialize)]
pub struct ChainConfigParams {
	pub chain_name: String,
	pub chain_type: String,
	pub runtime_type: String,
	pub authorities: Vec<Vec<String>>,
	pub sudo_key: Option<String>,
	pub network_id: u32,
}

#[derive(Debug, Args)]
pub struct BootstrapChainCmd {
	#[arg(long = "raw")]
	raw: bool,

	#[arg(long, short = 'c')]
	config: PathBuf,
}

impl BootstrapChainCmd {
	pub fn run(&self) -> Result<(), Error> {
		let toml_config = fs::read_to_string(&self.config)?;
		let config: ChainConfigParams =
			toml::from_str(&toml_config).map_err(|e| sc_cli::Error::Application(Box::new(e)))?;

		if config.authorities.is_empty() {
			eprintln!("Error: authorities cannot be empty");
			std::process::exit(1);
		}

		// Validate that each authority has exactly 4 keys
		for (i, auth) in config.authorities.iter().enumerate() {
			if auth.len() != 4 {
				eprintln!(
					"Error: Authority {} has invalid length: expected 4 keys, got {}",
					i,
					auth.len()
				);
				std::process::exit(1);
			}
		}

		let chain_name = if config.chain_name.len() <= 64 {
			config.chain_name.clone()
		} else {
			return Err("Chain name should not be more than 64 characters".into());
		};

		let chain_type: Result<ChainType, String> = match config.chain_type.as_str() {
			"dev" => Ok(ChainType::Development),
			"local" => Ok(ChainType::Local),
			"live" => Ok(ChainType::Live),
			other => Err(format!(
				"Invalid chain_type: {}. Possible values are 'dev', 'local', 'live'",
				other
			)),
		};

		let chain_type = chain_type?;

		let runtime_type =
			if ["braid", "loom", "weave"].contains(&config.runtime_type.to_lowercase().as_str()) {
				config.runtime_type.to_lowercase()
			} else {
				return Err(format!(
					"Invalid runtime_type: {}. Supported types are 'braid', 'loom', 'weave'.",
					config.runtime_type
				)
				.into());
			};

		let initial_authorities: Vec<Vec<String>> = config
			.authorities
			.iter()
			.map(|auth| vec![auth[1].clone(), auth[2].clone(), auth[3].clone()])
			.collect();

		let initial_sudo_key: String = config.sudo_key.unwrap_or_else(|| {
			config
				.authorities
				.get(0)
				.map(|auth| auth[0].clone())
				.expect("No authorities provided; cannot set sudo_key")
		});

		/* TODO: Make ProtocolId modular so we can support for Origin based custom chain
		 * deployments. Currently we have the default protocol_id as 'c0rd' which is standalone
		 * mode & it requires the network-id to be in range of [100, 1999)
		 */
		let network_id: u32 = config.network_id;

		let chain_params = ChainParams {
			chain_name,
			chain_type,
			runtime_type,
			authorities: initial_authorities,
			sudo_key: initial_sudo_key,
			network_id,
		};

		let chain_spec = match cord_custom_config(chain_params) {
			Ok(spec) => spec,
			Err(e) => panic!("Chain spec creation failed: {}", e),
		};

		let json = sc_service::chain_ops::build_spec(&chain_spec, self.raw)?;
		if std::io::stdout().write_all(json.as_bytes()).is_err() {
			let _ = std::io::stderr().write_all(b"Error writing to stdout\n");
		}

		Ok(())
	}
}
