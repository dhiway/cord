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

pub use cord_braid_base_runtime::RuntimeGenesisConfig as BraidBaseRuntimeGenesisConfig;
pub use cord_braid_plus_runtime::RuntimeGenesisConfig as BraidPlusRuntimeGenesisConfig;

use cord_braid_base_runtime::SessionKeys as BraidBaseSessionKeys;
use cord_braid_plus_runtime::SessionKeys as BraidPlusSessionKeys;

pub use cord_primitives::{AccountId, Balance, NodeId, Signature};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde::Deserialize;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::crypto::UncheckedInto;
use sp_std::collections::btree_map::BTreeMap;

pub use cord_braid_base_runtime_constants::currency::UNITS as BRAID_UNITS;
pub use cord_braid_plus_runtime_constants::currency::UNITS as LOOM_UNITS;

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
/// Todo: Fix individual chainspec
pub type CordChainSpec = sc_service::GenericChainSpec<Extensions>;
// pub type BraidChainSpec = sc_service::GenericChainSpec<BraidRuntimeGenesisConfig, Extensions>;
// pub type LoomChainSpec = sc_service::GenericChainSpec<LoomRuntimeGenesisConfig, Extensions>;
// pub type WeaveChainSpec = sc_service::GenericChainSpec<WeaveRuntimeGenesisConfig, Extensions>;

pub const BASE_ENDOWMENT: Balance = 10_000_000 * BRAID_UNITS;
pub const PLUS_ENDOWMENT: Balance = 10_000_000 * LOOM_UNITS;

fn braid_base_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> BraidBaseSessionKeys {
	BraidBaseSessionKeys { babe, grandpa, im_online, authority_discovery }
}

fn braid_plus_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> BraidPlusSessionKeys {
	BraidPlusSessionKeys { babe, grandpa, im_online, authority_discovery }
}

fn cord_braid_base_custom_config_genesis(config: ChainParams) -> serde_json::Value {
	let initial_network_members: Vec<AccountId> =
		config.network_members.iter().map(array_bytes::hex_n_into_unchecked).collect();

	let initial_well_known_nodes: Vec<(NodeId, AccountId)> = config
		.well_known_nodes
		.iter()
		.map(|node| {
			let node_id = node[0].as_bytes().to_vec();
			let account = array_bytes::hex_n_into_unchecked(&node[1]);
			(node_id, account)
		})
		.collect();

	let initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)> =
		config
			.authorities
			.iter()
			.map(|auth| {
				(
					array_bytes::hex_n_into_unchecked(&auth[0]),
					array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
					array_bytes::hex2array_unchecked(&auth[1]).unchecked_into(),
					array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
					array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				)
			})
			.collect();

	let initial_sudo_key: AccountId = array_bytes::hex_n_into_unchecked(&config.sudo_key);
	cord_braid_base_custom_genesis(
		initial_network_members,
		initial_well_known_nodes,
		initial_authorities,
		initial_sudo_key,
	)
}

fn cord_braid_plus_custom_config_genesis(config: ChainParams) -> serde_json::Value {
	let initial_network_members: Vec<AccountId> =
		config.network_members.iter().map(array_bytes::hex_n_into_unchecked).collect();

	let initial_well_known_nodes: Vec<(NodeId, AccountId)> = config
		.well_known_nodes
		.iter()
		.map(|node| {
			let node_id = node[0].as_bytes().to_vec();
			let account = array_bytes::hex_n_into_unchecked(&node[1]);
			(node_id, account)
		})
		.collect();

	let initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)> =
		config
			.authorities
			.iter()
			.map(|auth| {
				(
					array_bytes::hex_n_into_unchecked(&auth[0]),
					array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
					array_bytes::hex2array_unchecked(&auth[1]).unchecked_into(),
					array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
					array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				)
			})
			.collect();

	let initial_sudo_key: AccountId = array_bytes::hex_n_into_unchecked(&config.sudo_key);
	cord_braid_plus_custom_genesis(
		initial_network_members,
		initial_well_known_nodes,
		initial_authorities,
		initial_sudo_key,
	)
}

pub fn cord_custom_config(config: ChainParams) -> Result<CordChainSpec, String> {
	let chain_name = String::from(config.chain_name());
	let chain_type = config.chain_type();
	let runtime_type = config.runtime_type.to_lowercase();

	/* 'id' must start with either `braid', 'loom' or 'weave' for config to run */
	if runtime_type == "base" {
		let properties = get_properties("UNITS", 12, 3893);
		Ok(CordChainSpec::builder(
			cord_braid_base_runtime::WASM_BINARY.ok_or("Braid Base wasm not available")?,
			Default::default(),
		)
		.with_name(&chain_name)
		.with_id("braid-base-custom")
		.with_chain_type(chain_type)
		.with_genesis_config_patch(cord_braid_base_custom_config_genesis(config.clone()))
		.with_telemetry_endpoints(
			TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
				.expect("Cord telemetry url is valid; qed"),
		)
		.with_protocol_id(DEFAULT_PROTOCOL_ID)
		.with_properties(properties)
		.build())
	} else if runtime_type == "plus" {
		let properties = get_properties("UNITS", 12, 4926);
		Ok(CordChainSpec::builder(
			cord_braid_plus_runtime::WASM_BINARY.ok_or("Braid Plus wasm not available")?,
			Default::default(),
		)
		.with_name(&chain_name)
		.with_id("braid-plus-custom")
		.with_chain_type(chain_type)
		.with_genesis_config_patch(cord_braid_plus_custom_config_genesis(config.clone()))
		.with_telemetry_endpoints(
			TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
				.expect("Cord telemetry url is valid; qed"),
		)
		.with_protocol_id(DEFAULT_PROTOCOL_ID)
		.with_properties(properties)
		.build())
	} else {
		Err(format!(
			"Invalid runtime_type: {}. Supported types are 'braid', 'loom', & 'weave'.",
			runtime_type
		))
	}
}

fn cord_braid_base_custom_genesis(
	initial_network_members: Vec<AccountId>,
	initial_well_known_nodes: Vec<(NodeId, AccountId)>,
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)>,
	root_key: AccountId,
) -> serde_json::Value {
	const ENDOWMENT: Balance = BASE_ENDOWMENT;

	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"networkParameters": {"permissioned": true},
		"nodeAuthorization":  {
			"nodes": initial_well_known_nodes.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>(),
		},
		"networkMembership":  {
			"members": initial_network_members.iter().map(|member| (member, false)).collect::<BTreeMap<_, _>>(),
		},
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
						braid_base_session_keys(
							x.1.clone(),
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		"babe":  {
			"epochConfig": Some(cord_braid_base_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		"sudo": { "key": Some(root_key) },
	})
}

fn cord_braid_plus_custom_genesis(
	initial_network_members: Vec<AccountId>,
	initial_well_known_nodes: Vec<(NodeId, AccountId)>,
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)>,
	root_key: AccountId,
) -> serde_json::Value {
	const ENDOWMENT: Balance = PLUS_ENDOWMENT;

	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"networkParameters": {"permissioned": true},
		"nodeAuthorization":  {
			"nodes": initial_well_known_nodes.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>(),
		},
		"networkMembership":  {
			"members": initial_network_members.iter().map(|member| (member, false)).collect::<BTreeMap<_, _>>(),
		},
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
						braid_plus_session_keys(
							x.1.clone(),
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		"babe":  {
			"epochConfig": Some(cord_braid_plus_runtime::BABE_GENESIS_EPOCH_CONFIG),
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
