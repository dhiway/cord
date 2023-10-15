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
use cord_runtime::{
	AuthorityMembershipConfig, BabeConfig, CouncilMembershipConfig, IndicesConfig,
	NetworkMembershipConfig, NodeAuthorizationConfig, SessionConfig, SessionKeys, SudoConfig,
	SystemConfig, TechnicalMembershipConfig,
};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde::Deserialize;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::crypto::UncheckedInto;

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
fn cord_custom_config_genesis(
	wasm_binary: &[u8],
	config: ChainParams,
) -> cord_runtime::RuntimeGenesisConfig {
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
		wasm_binary,
		initial_network_members,
		initial_well_known_nodes,
		initial_authorities,
		initial_council_members,
		initial_tech_committee_members,
		initial_sudo_key,
	)
}

pub fn cord_custom_config(config: ChainParams) -> Result<CordChainSpec, String> {
	let wasm_binary = cord_runtime::WASM_BINARY.ok_or("CORD development wasm not available")?;
	let properties = get_properties("WAY", 12, 29);
	let chain_name = String::from(config.chain_name());
	let chain_type = config.chain_type();
	Ok(CordChainSpec::from_genesis(
		&chain_name,
		"crdcc",
		chain_type,
		move || cord_custom_config_genesis(wasm_binary, config.clone()),
		vec![],
		Some(
			TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
				.expect("CORD Staging telemetry url is valid; qed"),
		),
		Some(DEFAULT_PROTOCOL_ID),
		None,
		Some(properties),
		Default::default(),
	))
}

fn cord_custom_genesis(
	wasm_binary: &[u8],
	network_members: Vec<AccountId>,
	well_known_nodes: Vec<(NodeId, AccountId)>,
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)>,
	council_members: Vec<AccountId>,
	tech_committee_members: Vec<AccountId>,
	sudo_key: AccountId,
) -> RuntimeGenesisConfig {
	RuntimeGenesisConfig {
		system: SystemConfig { code: wasm_binary.to_vec(), ..Default::default() },
		balances: Default::default(),
		indices: IndicesConfig { indices: vec![] },
		node_authorization: NodeAuthorizationConfig {
			nodes: well_known_nodes.iter().map(|x| (x.0.clone(), x.1.clone())).collect(),
		},
		network_membership: NetworkMembershipConfig {
			members: network_members.iter().cloned().map(|member| (member, false)).collect(),
		},
		authority_membership: AuthorityMembershipConfig {
			initial_authorities: initial_authorities
				.iter()
				.map(|x| x.0.clone())
				.collect::<Vec<_>>(),
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						session_keys(x.1.clone(), x.2.clone(), x.3.clone(), x.4.clone()),
					)
				})
				.collect::<Vec<_>>(),
		},
		babe: BabeConfig {
			epoch_config: Some(cord_runtime::BABE_GENESIS_EPOCH_CONFIG),
			..Default::default()
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		council: Default::default(),
		council_membership: CouncilMembershipConfig {
			members: council_members
				.to_vec()
				.try_into()
				.unwrap_or_else(|e| panic!("Failed to add council memebers: {:?}", e)),
			phantom: Default::default(),
		},
		technical_committee: Default::default(),
		technical_membership: TechnicalMembershipConfig {
			members: tech_committee_members
				.to_vec()
				.try_into()
				.unwrap_or_else(|e| panic!("Failed to add committee members: {:?}", e)),
			phantom: Default::default(),
		},
		authority_discovery: Default::default(),
		sudo: SudoConfig { key: Some(sudo_key) },
	}
}
