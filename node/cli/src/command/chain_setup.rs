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

use std::{fs, io::Write};

use sc_cli::Error;
use serde::Deserialize;

use crate::{
	chain_spec::{
		bootstrap::{cord_custom_config, AuthorityKeys, ChainParams},
		ChainType,
	},
	subcommands::BootstrapChainCmd,
};

const MAX_CHAIN_NAME_LEN: usize = 64;
const NETWORK_ID_MIN: u32 = 100;
const NETWORK_ID_MAX: u32 = 1_999;

#[derive(Debug, Deserialize)]
pub struct ChainConfigParams {
	pub chain_name: String,
	pub chain_type: String,
	pub authorities: Vec<Vec<String>>,
	pub sudo_key: Option<String>,
	pub network_id: u32,
}

impl BootstrapChainCmd {
	pub fn run(&self) -> Result<(), Error> {
		let toml_config = fs::read_to_string(&self.config)?;
		let config: ChainConfigParams =
			toml::from_str(&toml_config).map_err(|e| sc_cli::Error::Application(Box::new(e)))?;

		if config.authorities.is_empty() {
			return Err("Authorities cannot be empty".into());
		}

		for (index, auth) in config.authorities.iter().enumerate() {
			if auth.len() != 4 {
				return Err(format!(
					"Authority {} has invalid length: expected 4 keys, got {}",
					index,
					auth.len()
				)
				.into());
			}
		}

		let trimmed_name = config.chain_name.trim();
		if trimmed_name.is_empty() {
			return Err("Chain name must not be empty".into());
		}

		let chain_name = if trimmed_name.len() <= MAX_CHAIN_NAME_LEN {
			trimmed_name.to_string()
		} else {
			return Err(format!(
				"Chain name should not be more than {} characters",
				MAX_CHAIN_NAME_LEN
			)
			.into());
		};

		let chain_type_input = config.chain_type.trim();
		let chain_type = match chain_type_input.to_lowercase().as_str() {
			"dev" => ChainType::Development,
			"local" => ChainType::Local,
			"live" => ChainType::Live,
			_ => {
				return Err(format!(
					"Invalid chain_type: {}. Possible values are 'dev', 'local', 'live'",
					chain_type_input
				)
				.into())
			},
		};

		let authorities = config
			.authorities
			.iter()
			.map(|auth| AuthorityKeys {
				stash: auth[0].trim().to_string(),
				babe: auth[1].trim().to_string(),
				grandpa: auth[2].trim().to_string(),
				authority_discovery: auth[3].trim().to_string(),
			})
			.collect::<Vec<_>>();

		let sudo_key = config
			.sudo_key
			.as_ref()
			.map(|key| key.trim().to_string())
			.filter(|key| !key.is_empty())
			.unwrap_or_else(|| {
				authorities
					.get(0)
					.expect("authorities list validated as non-empty; qed")
					.stash
					.clone()
			});

		if !(NETWORK_ID_MIN..=NETWORK_ID_MAX).contains(&config.network_id) {
			return Err(format!(
				"network_id must be between {} and {} (inclusive)",
				NETWORK_ID_MIN, NETWORK_ID_MAX
			)
			.into());
		}

		let chain_params = ChainParams {
			chain_name,
			chain_type,
			authorities,
			sudo_key,
			network_id: config.network_id,
		};

		let chain_spec = cord_custom_config(&chain_params).map_err(Error::from)?;

		let json = sc_service::chain_ops::build_spec(&chain_spec, self.raw)?;
		if std::io::stdout().write_all(json.as_bytes()).is_err() {
			let _ = std::io::stderr().write_all(b"Error writing to stdout\n");
		}

		Ok(())
	}
}
