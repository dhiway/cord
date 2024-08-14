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

//! Loom chain configurations.

use polkadot_primitives::{AccountId, AccountPublic, AssignmentId, ValidatorId};
use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_beefy::ecdsa_crypto::AuthorityId as BeefyId;

#[cfg(feature = "loom-native")]
use cord_loom_runtime as loom;
use sc_chain_spec::{ChainSpecExtension, ChainType, Properties};
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::IdentifyAccount;

#[cfg(feature = "loom-native")]
const LOOM_TELEMETRY_URL: &str = "wss://telemetry.dway.io/submit/";
const DEFAULT_PROTOCOL_ID: &str = "c0rd";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<polkadot_primitives::Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<polkadot_primitives::Block>,
	/// The light sync state.
	///
	/// This value will be set by the `sync-state rpc` implementation.
	pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

// TODO - enable this for production
// Generic chain spec, in case when we don't have the native runtime.
pub type GenericChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the rococo runtime.
#[cfg(feature = "loom-native")]
pub type LoomChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the rococo runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "loom-native"))]
pub type LoomChainSpec = GenericChainSpec;

// TODO - enable this for production
// pub fn cord_releay_config() -> Result<GenericChainSpec, String> {
// 	GenericChainSpec::from_json_bytes(&include_bytes!("../chain-specs/loom.json")[..])
// }

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to set properties
pub fn get_properties(symbol: &str, decimals: u32, ss58format: u32) -> Properties {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), symbol.into());
	properties.insert("tokenDecimals".into(), decimals.into());
	properties.insert("ss58Format".into(), ss58format.into());

	properties
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(
	seed: &str,
) -> (
	AccountId,
	AccountId,
	BabeId,
	GrandpaId,
	ValidatorId,
	AssignmentId,
	AuthorityDiscoveryId,
	BeefyId,
) {
	let keys = get_authority_keys_from_seed_no_beefy(seed);
	(keys.0, keys.1, keys.2, keys.3, keys.4, keys.5, keys.6, get_from_seed::<BeefyId>(seed))
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed_no_beefy(
	seed: &str,
) -> (AccountId, AccountId, BabeId, GrandpaId, ValidatorId, AssignmentId, AuthorityDiscoveryId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<ValidatorId>(seed),
		get_from_seed::<AssignmentId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
	)
}

/// Loom development config (single validator Alice)
#[cfg(feature = "loom-native")]
pub fn loom_development_config() -> Result<LoomChainSpec, String> {
	let properties = get_properties("ORU", 12, 29);
	Ok(LoomChainSpec::builder(
		loom::WASM_BINARY.ok_or("CORD Loom development wasm not available")?,
		Default::default(),
	)
	.with_name("Development")
	.with_id("loom_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_preset_name("development")
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(LOOM_TELEMETRY_URL.to_string(), 0)])
			.expect("Loom Development telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

/// Loom local testnet config (multivalidator Alice + Bob)
#[cfg(feature = "loom-native")]
pub fn loom_local_testnet_config() -> Result<LoomChainSpec, String> {
	let properties = get_properties("ORU", 12, 29);
	Ok(LoomChainSpec::builder(
		loom::WASM_BINARY.ok_or("CORD Loom development wasm not available")?,
		Default::default(),
	)
	.with_name("Loom Local Testnet")
	.with_id("loom_local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_preset_name("local_testnet")
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(LOOM_TELEMETRY_URL.to_string(), 0)])
			.expect("Loom Development telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}
