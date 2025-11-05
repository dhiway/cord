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

use cord_loom_runtime::SessionKeys as LoomSessionKeys;
use cord_orb_runtime::SessionKeys as OrbSessionKeys;
pub use cord_primitives::{AccountId, Balance, NodeId, Signature};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde::Deserialize;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_beefy::ecdsa_crypto::AuthorityId as BeefyId;
use sp_core::crypto::UncheckedInto;

pub use cord_orb_runtime_constants::currency::UNITS;

use crate::chain_spec::{get_properties, Extensions, CORD_TELEMETRY_URL, DEFAULT_PROTOCOL_ID};

#[derive(Debug, Deserialize, Clone)]
pub struct ChainParams {
	pub chain_name: String,
	pub chain_type: ChainType,
	pub runtime_type: String,
	pub authorities: Vec<Vec<String>>,
	pub well_known_nodes: Vec<Vec<String>>,
	pub network_members: Vec<String>,
	pub council_members: Vec<String>,
	pub tech_committee_members: Vec<String>,
	pub sudo_key: String,
	pub network_id: i32,
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

fn orb_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	authority_discovery: AuthorityDiscoveryId,
) -> OrbSessionKeys {
	OrbSessionKeys { babe, grandpa, authority_discovery }
}

fn loom_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
	beefy: BeefyId,
) -> LoomSessionKeys {
	LoomSessionKeys { babe, grandpa, im_online, authority_discovery, beefy }
}

/* TODO: Refer from weave to update below */
fn cord_orb_custom_config_genesis(config: ChainParams) -> serde_json::Value {
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
	cord_braid_custom_genesis(initial_authorities, initial_sudo_key, config.network_id)
}

fn cord_loom_custom_config_genesis(config: ChainParams) -> serde_json::Value {
	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
		BeefyId,
	)> = config
		.authorities
		.iter()
		.map(|auth| {
			(
				array_bytes::hex_n_into_unchecked(&auth[0]),
				array_bytes::hex_n_into_unchecked(&auth[0]),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[1]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[2]).unchecked_into(),
			)
		})
		.collect();

	let initial_sudo_key: AccountId = array_bytes::hex_n_into_unchecked(&config.authorities[0][0]);

	cord_loom_custom_genesis(initial_authorities, initial_sudo_key, config.network_id)
}

pub fn cord_custom_config(config: ChainParams) -> Result<CordChainSpec, String> {
	let chain_name = String::from(config.chain_name());
	let chain_type = config.chain_type();
	let runtime_type = config.runtime_type.to_lowercase();

	/* 'id' must start with either `orb', or 'loom' for the config to run */
	if runtime_type == "orb" {
		let properties = get_properties("UNITS", 12, 29);
		Ok(CordChainSpec::builder(
			cord_orb_runtime::WASM_BINARY.ok_or("Orb wasm not available")?,
			Default::default(),
		)
		.with_name(&chain_name)
		.with_id("orb-cord-custom")
		.with_chain_type(chain_type)
		.with_genesis_config_patch(cord_orb_custom_config_genesis(config.clone()))
		.with_telemetry_endpoints(
			TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
				.expect("Cord telemetry url is valid; qed"),
		)
		.with_protocol_id(DEFAULT_PROTOCOL_ID)
		.with_properties(properties)
		.build())
	} else if runtime_type == "loom" {
		let properties = get_properties("UNITS", 12, 29);
		Ok(CordChainSpec::builder(
			cord_loom_runtime::WASM_BINARY.ok_or("Loom wasm not available")?,
			Default::default(),
		)
		.with_name(&chain_name)
		.with_id("loom-cord-custom")
		.with_chain_type(chain_type)
		.with_genesis_config_patch(cord_loom_custom_config_genesis(config.clone()))
		.with_telemetry_endpoints(
			TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
				.expect("Cord telemetry url is valid; qed"),
		)
		.with_protocol_id(DEFAULT_PROTOCOL_ID)
		.with_properties(properties)
		.build())
	} else {
		Err(format!("Invalid runtime_type: {}. Supported types are 'orb', & 'loom'.", runtime_type))
	}
}

fn cord_braid_custom_genesis(
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, AuthorityDiscoveryId)>,
	root_key: AccountId,
	network_id: i32,
) -> serde_json::Value {
	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"token": { "protocolId": "c0rd".to_string(), "networkId": network_id },
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
						orb_session_keys(
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

fn cord_loom_custom_genesis(
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
		BeefyId,
	)>,
	root_key: AccountId,
	network_id: i32,
) -> serde_json::Value {
	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		/* TODO: Make the protocolId modular as well, to support origin chains */
		"token": { "protocolId": "c0rd".to_string(), "networkId": network_id },
		"authorityMembership":  {
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
						loom_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		"babe":  {
			"epochConfig": Some(cord_loom_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		"councilMembership":  {
			"members": initial_authorities
				.iter()
				.map(|x| x.0.clone())
				.collect::<Vec<_>>(),
		},
		"technicalMembership":  {
			"members": initial_authorities
				.iter()
				.map(|x| x.0.clone())
				.collect::<Vec<_>>(),
		},
		"sudo": { "key": Some(root_key) },
	})
}
