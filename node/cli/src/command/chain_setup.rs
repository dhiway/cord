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
	pub well_known_nodes: Vec<Vec<String>>,
	pub network_members: Vec<Vec<String>>,
	pub council_members: Option<Vec<String>>,
	pub tech_committee_members: Option<Vec<String>>,
	pub sudo_key: Option<String>,
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

		if config.authorities.is_empty() || config.well_known_nodes.is_empty() {
			eprintln!("Error: authorities and well_known_nodes cannot be empty");
			std::process::exit(1);
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

		let initial_members: Vec<String> =
			config.network_members.iter().map(|net| net[1].clone()).collect();

		let initial_well_known_nodes: Vec<Vec<String>> = config
			.well_known_nodes
			.iter()
			.map(|node| vec![node[1].clone(), node[2].clone()])
			.collect();

		let initial_authorities: Vec<Vec<String>> = config
			.authorities
			.iter()
			.map(|auth| vec![auth[1].clone(), auth[2].clone()])
			.collect();

		let initial_council_members: Vec<String> =
			if let Some(council_members) = &config.council_members {
				council_members.to_vec()
			} else {
				config.authorities.iter().map(|auth| auth[1].clone()).collect()
			};

		let initial_tech_committee_members: Vec<String> =
			if let Some(committee_members) = &config.tech_committee_members {
				committee_members.to_vec()
			} else {
				config.authorities.iter().map(|auth| auth[1].clone()).collect()
			};

		let initial_sudo_key: String = config.sudo_key.unwrap_or_else(|| {
			config
				.authorities
				.get(0)
				.map(|auth| auth[1].clone())
				.expect("No authorities provided; cannot set sudo_key")
		});

		let chain_params = ChainParams {
			chain_name,
			chain_type,
			runtime_type,
			authorities: initial_authorities,
			well_known_nodes: initial_well_known_nodes,
			network_members: initial_members,
			council_members: initial_council_members,
			tech_committee_members: initial_tech_committee_members,
			sudo_key: initial_sudo_key,
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
