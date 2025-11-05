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

//! CORD custom chain configurations.

use cord_orb_runtime::SessionKeys as OrbSessionKeys;
pub use cord_primitives::{AccountId, Balance, NodeId, Signature};
use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde::Deserialize;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::crypto::UncheckedInto;

pub use cord_orb_runtime_constants::currency::UNITS;

use crate::chain_spec::{get_properties, Extensions, CORD_TELEMETRY_URL, DEFAULT_PROTOCOL_ID};

#[derive(Debug, Deserialize, Clone)]
pub struct ChainParams {
	pub chain_name: String,
	pub chain_type: ChainType,
	pub runtime_type: String,
	pub authorities: Vec<Vec<String>>,
	pub sudo_key: String,
	pub network_id: u32,
}

impl ChainParams {
	pub fn chain_type(&self) -> ChainType {
		self.chain_type.clone()
	}

	pub fn chain_name(&self) -> &str {
		&self.chain_name
	}

	pub fn runtime_type(&self) -> &str {
		&self.runtime_type
	}
}

/// Specialized `ChainSpec`.
pub type CordChainSpec = sc_service::GenericChainSpec<Extensions>;

const ENDOWMENT: u128 = 500_000_000_000 * UNITS;

fn cord_custom_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	authority_discovery: AuthorityDiscoveryId,
) -> OrbSessionKeys {
	OrbSessionKeys { babe, grandpa, authority_discovery }
}

fn cord_custom_config_genesis(config: ChainParams) -> serde_json::Value {
	let initial_authorities: Vec<(AccountId, BabeId, GrandpaId, AuthorityDiscoveryId)> = config
		.authorities
		.iter()
		.map(|auth| {
			(
				array_bytes::hex_n_into_unchecked(&auth[0]),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[1]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
			)
		})
		.collect();

	let initial_sudo_key: AccountId = array_bytes::hex_n_into_unchecked(&config.sudo_key);
	cord_custom_chain_genesis(initial_authorities, initial_sudo_key, config.network_id)
}

pub fn cord_custom_config(config: ChainParams) -> Result<CordChainSpec, String> {
	let chain_name = config.chain_name().to_string();
	let chain_type = config.chain_type();
	let runtime_type = config.runtime_type.to_lowercase();
	let properties: sc_service::Properties = get_properties("UNITS", 12, config.network_id);

	match runtime_type.as_str() {
		"orb" | "cord" => {
			let spec = CordChainSpec::builder(
				cord_orb_runtime::WASM_BINARY.ok_or("Orb wasm not available")?,
				Default::default(),
			)
			.with_name(&chain_name)
			.with_id("cord-orb-custom")
			.with_chain_type(chain_type)
			.with_genesis_config_patch(cord_custom_config_genesis(config.clone()))
			.with_telemetry_endpoints(
				TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
					.map_err(|e| e.to_string())?,
			)
			.with_protocol_id(DEFAULT_PROTOCOL_ID)
			.with_properties(properties)
			.build();

			Ok(spec)
		},
		_ => Err(format!(
			"Invalid runtime_type: {}. Supported types are 'orb' & 'loom'.",
			runtime_type
		)),
	}
}

fn cord_custom_chain_genesis(
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, AuthorityDiscoveryId)>,
	root_key: AccountId,
	network_id: u32,
) -> serde_json::Value {
	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"token": { "protocolId": "c0rd".to_string(), "networkId": network_id as u16 },
		"authorityManager":  {
			"initialAuthorities": initial_authorities
				.iter()
				.map(|x| x.0.clone())
				.collect::<Vec<_>>(),
		},
		"session":  {
			"keys": initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						cord_custom_session_keys(
							x.1.clone(),
							x.2.clone(),
							x.3.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		"babe":  {
			"epochConfig": Some(cord_orb_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		"sudo": { "key": Some(root_key) },
	})
}
