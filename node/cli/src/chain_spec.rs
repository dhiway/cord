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

pub use cord_primitives::{AccountId, AccountPublic, Balance, NodeId, Signature};
use sc_chain_spec::ChainSpecExtension;
pub use sc_service::{ChainType, Properties};
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use sp_core::{Pair, Public};

#[cfg(feature = "braid-native")]
pub use cord_braid_runtime::genesis_config_presets::{
	cord_braid_development_config_genesis, cord_braid_local_testnet_genesis,
};

#[cfg(feature = "loom-native")]
pub use cord_loom_runtime::genesis_config_presets::{
	cord_loom_development_config_genesis, cord_loom_local_testnet_genesis,
};

#[cfg(feature = "weave-native")]
pub use cord_weave_runtime::genesis_config_presets::{
	cord_weave_development_config_genesis, cord_weave_local_testnet_genesis,
};

#[cfg(any(feature = "braid-native", feature = "loom-native", feature = "weave-native"))]
const CORD_TELEMETRY_URL: &str = "wss://telemetry.cord.network/submit/";

const DEFAULT_PROTOCOL_ID: &str = "c0rd";

// Node `ChainSpec` extensions.
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
	.with_genesis_config_patch(cord_braid_development_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "braid-native")]
pub fn braid_local_testnet_config() -> Result<BraidChainSpec, String> {
	let properties = get_properties("UNITS", 12, 3893);
	Ok(BraidChainSpec::builder(
		cord_braid_runtime::WASM_BINARY.ok_or("Braid wasm not available")?,
		Default::default(),
	)
	.with_name("Braid Local Testnet")
	.with_id("braid-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(cord_braid_local_testnet_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
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
	.with_genesis_config_patch(cord_loom_development_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "loom-native")]
pub fn loom_local_testnet_config() -> Result<LoomChainSpec, String> {
	let properties = get_properties("UNITS", 12, 4926);
	Ok(LoomChainSpec::builder(
		cord_loom_runtime::WASM_BINARY.ok_or("Loom wasm not available")?,
		Default::default(),
	)
	.with_name("Loom Local Testnet")
	.with_id("loom-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(cord_loom_local_testnet_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "weave-native")]
pub fn weave_development_config() -> Result<WeaveChainSpec, String> {
	let properties = get_properties("UNITS", 12, 29);
	Ok(WeaveChainSpec::builder(
		cord_weave_runtime::WASM_BINARY.ok_or("Weave development wasm not available")?,
		Default::default(),
	)
	.with_name("Weave Development")
	.with_id("weave-dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(cord_weave_development_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

#[cfg(feature = "weave-native")]
pub fn weave_local_testnet_config() -> Result<WeaveChainSpec, String> {
	let properties = get_properties("UNITS", 12, 29);
	Ok(WeaveChainSpec::builder(
		cord_weave_runtime::WASM_BINARY.ok_or("Loom wasm not available")?,
		Default::default(),
	)
	.with_name("Weave Local Testnet")
	.with_id("weave-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(cord_weave_local_testnet_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}
