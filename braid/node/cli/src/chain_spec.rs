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

#[cfg(feature = "braid-base-native")]
pub use cord_braid_base_runtime_constants::currency::UNITS as BASE_UNITS;
#[cfg(feature = "braid-plus-native")]
pub use cord_braid_plus_runtime_constants::currency::UNITS as PLUS_UNITS;

#[cfg(any(feature = "braid-base-native", feature = "braid-plus-native"))]
const CORD_TELEMETRY_URL: &str = "wss://telemetry.cord.network/submit/";

#[cfg(feature = "braid-base-native")]
use cord_braid_base_runtime::SessionKeys as BraidBaseSessionKeys;
#[cfg(feature = "braid-plus-native")]
use cord_braid_plus_runtime::SessionKeys as BraidPlusSessionKeys;

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

/// The `ChainSpec` parameterized for the braid base runtime.
#[cfg(feature = "braid-base-native")]
pub type BraidBaseChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the braid base runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "braid-base-native"))]
pub type BraidBaseChainSpec = GenericChainSpec;

/// The `ChainSpec` parameterized for the braid plus runtime.
#[cfg(feature = "braid-plus-native")]
pub type BraidPlusChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for braid plus runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "braid-plus-native"))]
pub type BraidPlusChainSpec = GenericChainSpec;

#[cfg(feature = "braid-base-native")]
fn braid_base_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> BraidBaseSessionKeys {
	BraidBaseSessionKeys { babe, grandpa, im_online, authority_discovery }
}

#[cfg(feature = "braid-plus-native")]
fn braid_plus_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> BraidPlusSessionKeys {
	BraidPlusSessionKeys { babe, grandpa, im_online, authority_discovery }
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

#[cfg(feature = "braid-base-native")]
fn braid_base_development_config_genesis() -> serde_json::Value {
	braid_base_local_genesis(
		vec![get_authority_keys_from_seed("Alice")],
		vec![(
			b"12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2".to_vec(),
			get_account_id_from_seed::<sr25519::Public>("Alice"),
		)],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
	)
}

#[cfg(feature = "braid-base-native")]
fn braid_base_local_config_genesis() -> serde_json::Value {
	braid_base_local_genesis(
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

#[cfg(feature = "braid-base-native")]
pub fn braid_base_development_config() -> Result<BraidBaseChainSpec, String> {
	let properties = get_properties("UNITS", 12, 3893);
	Ok(BraidBaseChainSpec::builder(
		cord_braid_base_runtime::WASM_BINARY.ok_or("Braid Base development wasm not available")?,
		Default::default(),
	)
	.with_name("Braid Base Development")
	.with_id("braid-base-dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(braid_base_development_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "braid-base-native")]
pub fn braid_base_local_config() -> Result<BraidBaseChainSpec, String> {
	let properties = get_properties("UNITS", 12, 3893);
	Ok(BraidBaseChainSpec::builder(
		cord_braid_base_runtime::WASM_BINARY.ok_or("Braid Base wasm not available")?,
		Default::default(),
	)
	.with_name("Braid Base Local Testnet")
	.with_id("braid-base-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(braid_base_local_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "braid-base-native")]
fn braid_base_local_genesis(
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)>,
	initial_well_known_nodes: Vec<(NodeId, AccountId)>,
	root_key: AccountId,
) -> serde_json::Value {
	const ENDOWMENT: Balance = 50_000_000 * BASE_UNITS;

	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"networkParameters": {"permissioned": true},
		"nodeAuthorization":  {
			"nodes": initial_well_known_nodes.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>(),
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

#[cfg(feature = "braid-plus-native")]
fn braid_plus_development_config_genesis() -> serde_json::Value {
	braid_plus_local_genesis(
		vec![get_authority_keys_from_seed("Alice")],
		vec![(
			b"12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2".to_vec(),
			get_account_id_from_seed::<sr25519::Public>("Alice"),
		)],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
	)
}

#[cfg(feature = "braid-plus-native")]
fn braid_plus_local_config_genesis() -> serde_json::Value {
	braid_plus_local_genesis(
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

#[cfg(feature = "braid-plus-native")]
pub fn braid_plus_development_config() -> Result<BraidPlusChainSpec, String> {
	let properties = get_properties("UNITS", 12, 4926);
	Ok(BraidPlusChainSpec::builder(
		cord_braid_plus_runtime::WASM_BINARY.ok_or("Braid Plus development wasm not available")?,
		Default::default(),
	)
	.with_name("Braid Plus Development")
	.with_id("braid-plus-dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(braid_plus_development_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "braid-plus-native")]
pub fn braid_plus_local_config() -> Result<BraidPlusChainSpec, String> {
	let properties = get_properties("UNITS", 12, 4926);
	Ok(BraidPlusChainSpec::builder(
		cord_braid_plus_runtime::WASM_BINARY.ok_or("Braid Plus wasm not available")?,
		Default::default(),
	)
	.with_name("Braid Plus Local Testnet")
	.with_id("braid-plus-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(braid_plus_local_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "braid-plus-native")]
fn braid_plus_local_genesis(
	initial_authorities: Vec<(AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId)>,
	initial_well_known_nodes: Vec<(NodeId, AccountId)>,
	root_key: AccountId,
) -> serde_json::Value {
	const ENDOWMENT: Balance = 50_000_000 * PLUS_UNITS;

	serde_json::json!( {
		"balances": {
			"balances": initial_authorities.iter().map(|k| (k.0.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"networkParameters": {"permissioned": true},
		"nodeAuthorization":  {
			"nodes": initial_well_known_nodes.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>(),
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
