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

//! CORD chain configurations.
#![allow(missing_docs)]

pub mod bootstrap;

pub use cord_primitives::{AccountId, Balance, NodeId, Signature};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::ChainSpecExtension;
use sc_consensus_grandpa::AuthorityId as GrandpaId;
pub use sc_service::{ChainType, Properties};
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
type AccountPublic = <Signature as Verify>::Signer;
use sc_telemetry::TelemetryEndpoints;

#[cfg(feature = "braid-native")]
pub use cord_braid_runtime_constants::currency::UNITS as BRAID_UNITS;
#[cfg(feature = "loom-native")]
pub use cord_loom_runtime_constants::currency::UNITS as LOOM_UNITS;
#[cfg(feature = "weave-native")]
pub use cord_weave_runtime_constants::currency::WAY;

#[cfg(any(feature = "braid-native", feature = "loom-native", feature = "weave-native"))]
const CORD_TELEMETRY_URL: &str = "wss://telemetry.cord.network/submit/";

#[cfg(feature = "braid-native")]
use cord_braid_runtime::SessionKeys as BraidSessionKeys;
#[cfg(feature = "loom-native")]
use cord_loom_runtime::SessionKeys as LoomSessionKeys;
#[cfg(feature = "weave-native")]
use cord_loom_runtime::SessionKeys as WeaveSessionKeys;

#[cfg(any(feature = "braid-native", feature = "loom-native"))]
use sp_std::collections::btree_map::BTreeMap;

const DEFAULT_PROTOCOL_ID: &str = "cord";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<cord_primitives::Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<cord_primitives::Block>,
	/// The light sync state extension used by the sync-state rpc.
	pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

// Generic chain spec, in case when we don't have the native runtime.
pub type GenericChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the braid runtime.
#[cfg(feature = "braid-native")]
pub type BraidChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the braid runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "braid-native"))]
pub type BraidChainSpec = GenericChainSpec;

/// The `ChainSpec` parameterized for the loom runtime.
#[cfg(feature = "loom-native")]
pub type LoomChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for loom runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "loom-native"))]
pub type LoomChainSpec = GenericChainSpec;

/// The `ChainSpec` parameterized for the weave runtime.
#[cfg(feature = "weave-native")]
pub type WeaveChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the weave runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "weave-native"))]
pub type WeaveChainSpec = GenericChainSpec;

// pub fn weave_config() -> Result<WeaveChainSpec, String> {
// 	WeaveChainSpec::from_json_bytes(&include_bytes!("../chain-specs/weave.json")[..])
// }

#[cfg(feature = "braid-native")]
fn braid_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> BraidSessionKeys {
	BraidSessionKeys { babe, grandpa, im_online, authority_discovery }
}

#[cfg(feature = "loom-native")]
fn loom_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> LoomSessionKeys {
	LoomSessionKeys { babe, grandpa, im_online, authority_discovery }
}

#[cfg(feature = "weave-native")]
fn weave_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> WeaveSessionKeys {
	WeaveSessionKeys { babe, grandpa, im_online, authority_discovery }
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to set properties
pub fn get_properties(symbol: &str, decimals: u32, ss58format: u32) -> Properties {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), symbol.into());
	properties.insert("tokenDecimals".into(), decimals.into());
	properties.insert("ss58Format".into(), ss58format.into());

	properties
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate controller and session key from seed
pub fn get_authority_keys_from_seed(
	seed: &str,
) -> (AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId) {
	let keys = get_authority_keys(seed);
	(keys.0, keys.1, keys.2, keys.3, keys.4)
}

/// Helper function to generate  controller and session key from seed
pub fn get_authority_keys(
	seed: &str,
) -> (AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId) {
	(
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<ImOnlineId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
	)
}

fn member_accounts() -> Vec<AccountId> {
	vec![
		(get_account_id_from_seed::<sr25519::Public>("Alice")),
		(get_account_id_from_seed::<sr25519::Public>("Bob")),
		(get_account_id_from_seed::<sr25519::Public>("Charlie")),
	]
}

#[cfg(feature = "braid-native")]
fn braid_development_config_genesis() -> serde_json::Value {
	braid_local_genesis(
		vec![get_authority_keys_from_seed("Alice")],
		vec![(
			b"12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2".to_vec(),
			get_account_id_from_seed::<sr25519::Public>("Alice"),
		)],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
	)
}

#[cfg(feature = "braid-native")]
fn braid_local_config_genesis() -> serde_json::Value {
	braid_local_genesis(
		vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
			get_authority_keys_from_seed("Charlie"),
		],
		vec![
			(
				b"12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Alice"),
			),
			(
				b"12D3KooWQYV9dGMFoRzNStwpXztXaBUjtPqi6aU76ZgUriHhKust".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Bob"),
			),
			(
				b"12D3KooWJvyP3VJYymTqG7eH4PM5rN4T2agk5cdNCfNymAqwqcvZ".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Charlie"),
			),
			(
				b"12D3KooWPHWFrfaJzxPnqnAYAoRUyAHHKqACmEycGTVmeVhQYuZN".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Dave"),
			),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
	)
}

#[cfg(feature = "braid-native")]
pub fn braid_development_config() -> Result<BraidChainSpec, String> {
	let properties = get_properties("UNITS", 12, 3893);
	Ok(BraidChainSpec::builder(
		cord_braid_runtime::WASM_BINARY.ok_or("Braid development wasm not available")?,
		Default::default(),
	)
	.with_name("Braid Development")
	.with_id("braid-dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(braid_development_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "braid-native")]
pub fn braid_local_config() -> Result<BraidChainSpec, String> {
	let properties = get_properties("UNITS", 12, 3893);
	Ok(BraidChainSpec::builder(
		cord_braid_runtime::WASM_BINARY.ok_or("Braid wasm not available")?,
		Default::default(),
	)
	.with_name("Braid Local Testnet")
	.with_id("braid-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(braid_local_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "braid-native")]
fn braid_local_genesis(
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)>,
	initial_well_known_nodes: Vec<(NodeId, AccountId)>,
	root_key: AccountId,
) -> serde_json::Value {
	const ENDOWMENT: Balance = 10_000_000 * BRAID_UNITS;

	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"networkParameters": {"permissioned": true},
		"nodeAuthorization":  {
			"nodes": initial_well_known_nodes.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>(),
		},
		"networkMembership":  {
			"members": member_accounts().into_iter().map(|member| (member, false)).collect::<BTreeMap<_, _>>(),
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
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		"babe":  {
			"epochConfig": Some(cord_loom_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		"sudo": { "key": Some(root_key) },
	})
}

#[cfg(feature = "loom-native")]
fn loom_development_config_genesis() -> serde_json::Value {
	loom_local_genesis(
		vec![get_authority_keys_from_seed("Alice")],
		vec![(
			b"12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2".to_vec(),
			get_account_id_from_seed::<sr25519::Public>("Alice"),
		)],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
	)
}

#[cfg(feature = "loom-native")]
fn loom_local_config_genesis() -> serde_json::Value {
	loom_local_genesis(
		vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
			get_authority_keys_from_seed("Charlie"),
		],
		vec![
			(
				b"12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Alice"),
			),
			(
				b"12D3KooWQYV9dGMFoRzNStwpXztXaBUjtPqi6aU76ZgUriHhKust".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Bob"),
			),
			(
				b"12D3KooWJvyP3VJYymTqG7eH4PM5rN4T2agk5cdNCfNymAqwqcvZ".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Charlie"),
			),
			(
				b"12D3KooWPHWFrfaJzxPnqnAYAoRUyAHHKqACmEycGTVmeVhQYuZN".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Dave"),
			),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
	)
}

#[cfg(feature = "loom-native")]
pub fn loom_development_config() -> Result<LoomChainSpec, String> {
	let properties = get_properties("UNITS", 12, 4926);
	Ok(LoomChainSpec::builder(
		cord_loom_runtime::WASM_BINARY.ok_or("Loom development wasm not available")?,
		Default::default(),
	)
	.with_name("Loom Development")
	.with_id("loom-dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(loom_development_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "loom-native")]
pub fn loom_local_config() -> Result<LoomChainSpec, String> {
	let properties = get_properties("UNITS", 12, 4926);
	Ok(LoomChainSpec::builder(
		cord_loom_runtime::WASM_BINARY.ok_or("Loom wasm not available")?,
		Default::default(),
	)
	.with_name("Loom Local Testnet")
	.with_id("loom-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(loom_local_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "loom-native")]
fn loom_local_genesis(
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)>,
	initial_well_known_nodes: Vec<(NodeId, AccountId)>,
	root_key: AccountId,
) -> serde_json::Value {
	const ENDOWMENT: Balance = 10_000_000 * LOOM_UNITS;

	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"networkParameters": {"permissioned": true},
		"nodeAuthorization":  {
			"nodes": initial_well_known_nodes.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>(),
		},
		"networkMembership":  {
			"members": member_accounts().into_iter().map(|member| (member, false)).collect::<BTreeMap<_, _>>(),
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

#[cfg(feature = "weave-native")]
fn weave_development_config_genesis() -> serde_json::Value {
	weave_local_genesis(
		vec![get_authority_keys_from_seed("Alice")],
		vec![(
			b"12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2".to_vec(),
			get_account_id_from_seed::<sr25519::Public>("Alice"),
		)],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
	)
}

#[cfg(feature = "weave-native")]
fn weave_local_config_genesis() -> serde_json::Value {
	weave_local_genesis(
		vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
			get_authority_keys_from_seed("Charlie"),
		],
		vec![
			(
				b"12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Alice"),
			),
			(
				b"12D3KooWQYV9dGMFoRzNStwpXztXaBUjtPqi6aU76ZgUriHhKust".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Bob"),
			),
			(
				b"12D3KooWJvyP3VJYymTqG7eH4PM5rN4T2agk5cdNCfNymAqwqcvZ".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Charlie"),
			),
			(
				b"12D3KooWPHWFrfaJzxPnqnAYAoRUyAHHKqACmEycGTVmeVhQYuZN".to_vec(),
				get_account_id_from_seed::<sr25519::Public>("Dave"),
			),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
	)
}

#[cfg(feature = "weave-native")]
pub fn weave_development_config() -> Result<WeaveChainSpec, String> {
	let properties = get_properties("WAY", 12, 29);
	Ok(WeaveChainSpec::builder(
		cord_weave_runtime::WASM_BINARY.ok_or("Weave development wasm not available")?,
		Default::default(),
	)
	.with_name("Weave Development")
	.with_id("weave-dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(weave_development_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "weave-native")]
pub fn weave_local_config() -> Result<WeaveChainSpec, String> {
	let properties = get_properties("WAY", 12, 29);
	Ok(WeaveChainSpec::builder(
		cord_weave_runtime::WASM_BINARY.ok_or("Weave wasm not available")?,
		Default::default(),
	)
	.with_name("Weave Local Testnet")
	.with_id("weave-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(weave_local_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "weave-native")]
fn weave_local_genesis(
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)>,
	initial_well_known_nodes: Vec<(NodeId, AccountId)>,
	root_key: AccountId,
) -> serde_json::Value {
	const ENDOWMENT: Balance = 10_000_000 * WAY;

	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"networkParameters": {"permissioned": false},
		"nodeAuthorization":  {
			"nodes": initial_well_known_nodes.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>(),
		},
		"networkMembership":  {
			"members": member_accounts().into_iter().map(|member| (member, false)).collect::<BTreeMap<_, _>>(),
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
						weave_session_keys(
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
			"epochConfig": Some(cord_loom_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		"sudo": { "key": Some(root_key) },
	})
}
