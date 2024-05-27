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

pub use cord_primitives::{AccountId, Balance, NodeId, Signature};
pub use cord_runtime::RuntimeGenesisConfig;
use cord_runtime::SessionKeys;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde::Deserialize;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::crypto::UncheckedInto;
use sp_std::collections::btree_map::BTreeMap;

pub use cord_runtime_constants::{currency::*, time::*};

use crate::chain_spec::{get_properties, Extensions, CORD_TELEMETRY_URL, DEFAULT_PROTOCOL_ID};

#[derive(Debug, Deserialize, Clone)]
pub struct ChainParams {
	pub chain_name: String,
	pub chain_type: ChainType,
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
}

/// Specialized `ChainSpec`.
pub type CordChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig, Extensions>;

fn session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys { babe, grandpa, im_online, authority_discovery }
}

/// Custom config.
fn cord_custom_config_genesis(config: ChainParams) -> serde_json::Value {
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

	let initial_council_members: Vec<AccountId> =
		config.council_members.iter().map(array_bytes::hex_n_into_unchecked).collect();

	let initial_tech_committee_members: Vec<AccountId> = config
		.tech_committee_members
		.iter()
		.map(array_bytes::hex_n_into_unchecked)
		.collect();

	let initial_sudo_key: AccountId = array_bytes::hex_n_into_unchecked(&config.sudo_key);
	cord_custom_genesis(
		initial_network_members,
		initial_well_known_nodes,
		initial_authorities,
		initial_council_members,
		initial_tech_committee_members,
		initial_sudo_key,
	)
}

pub fn cord_custom_config(config: ChainParams) -> Result<CordChainSpec, String> {
	let properties = get_properties("WAY", 12, 29);
	let chain_name = String::from(config.chain_name());
	let chain_type = config.chain_type();
	Ok(CordChainSpec::builder(
		cord_runtime::WASM_BINARY.ok_or("Cord development wasm not available")?,
		Default::default(),
	)
	.with_name(&chain_name)
	.with_id("crdcc")
	.with_chain_type(chain_type)
	.with_genesis_config_patch(cord_custom_config_genesis(config.clone()))
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

fn cord_custom_genesis(
	network_members: Vec<AccountId>,
	well_known_nodes: Vec<(NodeId, AccountId)>,
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)>,
	council_members: Vec<AccountId>,
	tech_committee_members: Vec<AccountId>,
	sudo_key: AccountId,
) -> serde_json::Value {
	serde_json::json!( {
		"nodeAuthorization":  {
			"nodes": well_known_nodes.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>(),
		},
		"networkMembership":  {
			"members": network_members.iter().map(|member| (member, false)).collect::<BTreeMap<_, _>>(),
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
						session_keys(
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
			"epochConfig": Some(cord_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		"councilMembership":  {
			"members": council_members.to_vec(),
		},
		"technicalMembership":  {
			"members": tech_committee_members.to_vec(),
		},
		"sudo": { "key": Some(sudo_key) },
	})
}
