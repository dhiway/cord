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

pub use cord_orb_runtime::genesis_config_presets::{
	cord_orb_development_config_genesis, cord_orb_staging_config_genesis,
};

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
pub type GenericCordChainSpec = sc_service::GenericChainSpec<Extensions>;

// pub fn orb_config() -> Result<GenericCordChainSpec, String> {
// 	GenericCordChainSpec::from_json_bytes(&include_bytes!("../chain-specs/orb.json")[..])
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

pub fn orb_development_config() -> Result<GenericCordChainSpec, String> {
	let properties = get_properties("UNITS", 12, 3893);
	Ok(GenericCordChainSpec::builder(
		cord_orb_runtime::WASM_BINARY.ok_or("Orb development wasm not available")?,
		Default::default(),
	)
	.with_name("Orb Development")
	.with_id("orb-dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(cord_orb_development_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}

pub fn orb_staging_config() -> Result<GenericCordChainSpec, String> {
	let properties = get_properties("UNITS", 12, 3893);
	Ok(GenericCordChainSpec::builder(
		cord_orb_runtime::WASM_BINARY.ok_or("Orb wasm not available")?,
		Default::default(),
	)
	.with_name("Orb Local Testnet")
	.with_id("orb-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(cord_orb_staging_config_genesis())
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
			.expect("Cord telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties)
	.build())
}
