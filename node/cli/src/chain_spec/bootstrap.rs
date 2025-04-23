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

pub use cord_braid_runtime::RuntimeGenesisConfig as BraidRuntimeGenesisConfig;
pub use cord_loom_runtime::RuntimeGenesisConfig as LoomRuntimeGenesisConfig;
pub use cord_weave_runtime::RuntimeGenesisConfig as WeaveRuntimeGenesisConfig;

use cord_braid_runtime::SessionKeys as BraidSessionKeys;
use cord_loom_runtime::SessionKeys as LoomSessionKeys;
use cord_weave_runtime::SessionKeys as WeaveSessionKeys;

pub use cord_primitives::{AccountId, Balance, NodeId, Signature};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_staking::{Forcing, StakerStatus};
use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde::Deserialize;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_beefy::ecdsa_crypto::AuthorityId as BeefyId;
use sp_core::crypto::UncheckedInto;
use sp_runtime::Perbill;
use sp_std::collections::btree_map::BTreeMap;

// pub use cord_braid_runtime_constants::currency::UNITS as BRAID_UNITS;
// pub use cord_loom_runtime_constants::currency::UNITS as LOOM_UNITS;
pub use cord_weave_runtime_constants::currency::UNITS;

use crate::chain_spec::{get_properties, Extensions, CORD_TELEMETRY_URL, DEFAULT_PROTOCOL_ID};

use array_bytes::hex2array;
use sp_core::{
	crypto::{AccountId32, Ss58Codec},
	ed25519, sr25519,
};

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

// pub const BRAID_ENDOWMENT: Balance = 10_000_000 * BRAID_UNITS;
// pub const LOOOM_ENDOWMENT: Balance = 10_000_000 * LOOM_UNITS;

const ENDOWMENT: u128 = 500_000_000_000 * UNITS;
const STASH: u128 = 100_000_000 * UNITS;

fn braid_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
	beefy: BeefyId,
) -> BraidSessionKeys {
	BraidSessionKeys { babe, grandpa, im_online, authority_discovery, beefy }
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

fn weave_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
	beefy: BeefyId,
) -> WeaveSessionKeys {
	WeaveSessionKeys { babe, grandpa, im_online, authority_discovery, beefy }
}

fn cord_braid_custom_config_genesis(config: ChainParams) -> serde_json::Value {
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

	let initial_authorities: Vec<(
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
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[1]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
			)
		})
		.collect();

	let initial_sudo_key: AccountId = array_bytes::hex_n_into_unchecked(&config.sudo_key);
	cord_braid_custom_genesis(
		initial_network_members,
		initial_well_known_nodes,
		initial_authorities,
		initial_sudo_key,
	)
}

fn cord_loom_custom_config_genesis(config: ChainParams) -> serde_json::Value {
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

	let initial_authorities: Vec<(
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
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[1]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
				array_bytes::hex2array_unchecked(&auth[0]).unchecked_into(),
			)
		})
		.collect();

	let initial_sudo_key: AccountId = array_bytes::hex_n_into_unchecked(&config.sudo_key);
	cord_loom_custom_genesis(
		initial_network_members,
		initial_well_known_nodes,
		initial_authorities,
		initial_sudo_key,
	)
}

fn cord_weave_custom_config_genesis(config: ChainParams) -> serde_json::Value {
	let initial_authorities: Vec<(
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
		BeefyId,
	)> = config
		.authorities
		.iter()
		.enumerate()
		.filter_map(|(i, auth)| {
			if auth.len() != 6 {
				eprintln!(
					"Authority {} has invalid length: expected 6 keys, got {}",
					i,
					auth.len()
				);
				return None;
			}

			let account_id: AccountId = match AccountId32::from_ss58check(&auth[0]) {
				Ok(acc) => acc,
				Err(e) => {
					eprintln!("Failed to decode SS58 AccountId {}: {:?}", auth[0], e);
					return None;
				},
			};

			let babe_id: BabeId = match array_bytes::hex2array::<_, 32>(&auth[1]) {
				Ok(bytes) => match sp_core::sr25519::Public::try_from(&bytes[..]) {
					Ok(pubkey) => pubkey.into(),
					Err(_) => {
						eprintln!(
							"Invalid sr25519 public key for BabeId {}: not a valid key",
							auth[1]
						);
						return None;
					},
				},
				Err(e) => {
					eprintln!("Failed to decode BabeId {}: {:?}", auth[1], e);
					return None;
				},
			};

			let grandpa_id: GrandpaId = match array_bytes::hex2array::<_, 32>(&auth[2]) {
				Ok(bytes) => match sp_core::ed25519::Public::try_from(&bytes[..]) {
					Ok(pubkey) => pubkey.into(),
					Err(_) => {
						eprintln!(
							"Invalid ed25519 public key for GrandpaId {}: not a valid key",
							auth[2]
						);
						return None;
					},
				},
				Err(e) => {
					eprintln!("Failed to decode GrandpaId {}: {:?}", auth[2], e);
					return None;
				},
			};

			let im_online_id: ImOnlineId = match array_bytes::hex2array::<_, 32>(&auth[3]) {
				Ok(bytes) => match sp_core::sr25519::Public::try_from(&bytes[..]) {
					Ok(pubkey) => pubkey.into(),
					Err(_) => {
						eprintln!(
							"Invalid sr25519 public key for ImOnlineId {}: not a valid key",
							auth[3]
						);
						return None;
					},
				},
				Err(e) => {
					eprintln!("Failed to decode ImOnlineId {}: {:?}", auth[3], e);
					return None;
				},
			};

			let authority_discovery_id: AuthorityDiscoveryId = match array_bytes::hex2array::<_, 32>(
				&auth[4],
			) {
				Ok(bytes) => match sp_core::sr25519::Public::try_from(&bytes[..]) {
					Ok(pubkey) => pubkey.into(),
					Err(_) => {
						eprintln!("Invalid sr25519 public key for AuthorityDiscoveryId {}: not a valid key", auth[4]);
						return None;
					},
				},
				Err(e) => {
					eprintln!("Failed to decode AuthorityDiscoveryId {}: {:?}", auth[4], e);
					return None;
				},
			};

			let beefy_id: BeefyId = match array_bytes::hex2array::<_, 33>(&auth[5]) {
				Ok(bytes) => {
					if bytes[0] != 0x02 && bytes[0] != 0x03 {
						eprintln!(
							"Invalid BeefyId ECDSA key {}: must start with 0x02 or 0x03",
							auth[5]
						);
						return None;
					}
					match sp_consensus_beefy::ecdsa_crypto::Public::try_from(&bytes[..]) {
						Ok(pubkey) => pubkey.into(),
						Err(e) => {
							eprintln!("Invalid BeefyId ECDSA key format {}: {:?}", auth[5], e);
							return None;
						},
					}
				},
				Err(e) => {
					eprintln!("Failed to decode BeefyId {}: {:?}", auth[5], e);
					return None;
				},
			};

			Some((account_id, babe_id, grandpa_id, im_online_id, authority_discovery_id, beefy_id))
		})
		.collect();

	let initial_sudo_key: AccountId = match AccountId32::from_ss58check(&config.sudo_key) {
		Ok(acc) => acc,
		Err(e) => {
			eprintln!("Failed to decode sudo key {}: {:?}", config.sudo_key, e);
			panic!("Invalid sudo key");
		},
	};

	cord_weave_custom_genesis(initial_authorities, initial_sudo_key)
}

pub fn cord_custom_config(config: ChainParams) -> Result<CordChainSpec, String> {
	let chain_name = String::from(config.chain_name());
	let chain_type = config.chain_type();
	let runtime_type = config.runtime_type.to_lowercase();

	/* 'id' must start with either `braid', 'loom' or 'weave' for config to run */
	if runtime_type == "braid" {
		let properties = get_properties("UNITS", 12, 3893);
		Ok(CordChainSpec::builder(
			cord_braid_runtime::WASM_BINARY.ok_or("Braid wasm not available")?,
			Default::default(),
		)
		.with_name(&chain_name)
		.with_id("braid-cord-custom")
		.with_chain_type(chain_type)
		.with_genesis_config_patch(cord_braid_custom_config_genesis(config.clone()))
		.with_telemetry_endpoints(
			TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
				.expect("Cord telemetry url is valid; qed"),
		)
		.with_protocol_id(DEFAULT_PROTOCOL_ID)
		.with_properties(properties)
		.build())
	} else if runtime_type == "loom" {
		let properties = get_properties("UNITS", 12, 4926);
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
	} else if runtime_type == "weave" {
		let properties = get_properties("WAY", 12, 29);
		Ok(CordChainSpec::builder(
			cord_weave_runtime::WASM_BINARY.ok_or("Weave development wasm not available")?,
			Default::default(),
		)
		.with_name(&chain_name)
		.with_id("weave-cord-custom")
		.with_chain_type(chain_type)
		.with_genesis_config_patch(cord_weave_custom_config_genesis(config.clone()))
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

fn cord_braid_custom_genesis(
	initial_network_members: Vec<AccountId>,
	initial_well_known_nodes: Vec<(NodeId, AccountId)>,
	initial_authorities: Vec<(
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
		BeefyId,
	)>,
	root_key: AccountId,
) -> serde_json::Value {
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
						braid_session_keys(
							x.1.clone(),
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		"babe":  {
			"epochConfig": Some(cord_braid_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		"sudo": { "key": Some(root_key) },
	})
}

fn cord_loom_custom_genesis(
	initial_network_members: Vec<AccountId>,
	initial_well_known_nodes: Vec<(NodeId, AccountId)>,
	initial_authorities: Vec<(
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
		BeefyId,
	)>,
	root_key: AccountId,
) -> serde_json::Value {
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
						loom_session_keys(
							x.1.clone(),
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
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

fn cord_weave_custom_genesis(
	initial_authorities: Vec<(
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
		BeefyId,
	)>,
	root_key: AccountId,
) -> serde_json::Value {
	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"session":  {
			"keys": initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						weave_session_keys(
							x.1.clone(),
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		"staking": {
			"minimumValidatorCount": 1,
			"validatorCount": initial_authorities.len() as u32,
			"stakers": initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), STASH, StakerStatus::<AccountId>::Validator))
				.collect::<Vec<_>>(),
			"invulnerables": initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
			"forceEra": Forcing::NotForcing,
			"slashRewardFraction": Perbill::from_percent(10),
		},
		"babe":  {
			"epochConfig": Some(cord_weave_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		"sudo": { "key": Some(root_key) },
	})
}
