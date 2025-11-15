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

use sc_chain_spec::{ChainSpecExtension, ChainType};
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};

const ORIGIN_TELEMETRY_URL: &str = "wss://telemetry.cord.network/submit/";
const DEFAULT_PROTOCOL_ID: &str = "0rigin";

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

/// Cord Origin chain spec, in case when we don't have the native runtime.
pub type OriginChainSpec = sc_service::GenericChainSpec<Extensions>;

// pub fn origin_config() -> Result<OriginChainSpec, String> {
// 	OriginChainSpec::from_json_bytes(&include_bytes!("../chain-specs/tbd.json")[..])
// }

/// Returns the properties for the [`OriginChainSpec`].
pub fn origin_chain_spec_properties() -> serde_json::map::Map<String, serde_json::Value> {
	serde_json::json!({
		"ss58Format": "29",
		"tokenSymbol":"UNIT",
		"tokenDecimals": 10,
	})
	.as_object()
	.expect("Map given; qed")
	.clone()
}

/// Origin development config (single validator Alice)
pub fn origin_development_config() -> Result<OriginChainSpec, String> {
	Ok(OriginChainSpec::builder(
		origin_staging_runtime::WASM_BINARY.ok_or("Origin wasm not available")?,
		Default::default(),
	)
	.with_name("Origin Development")
	.with_id("origin_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(
		origin_staging_runtime::genesis_config_presets::origin_development_config_genesis(),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(origin_chain_spec_properties())
	.build())
}

/// Origin local testnet config (multivalidator Alice + Bob)
pub fn origin_staging_config() -> Result<OriginChainSpec, String> {
	Ok(OriginChainSpec::builder(
		origin_staging_runtime::WASM_BINARY.ok_or("Origin wasm not available")?,
		Default::default(),
	)
	.with_name("Origin Staging")
	.with_id("origin_staging")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(
		origin_staging_runtime::genesis_config_presets::origin_staging_config_genesis(),
	)
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(ORIGIN_TELEMETRY_URL.to_string(), 0)])
			.expect("Origin Staging telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(origin_chain_spec_properties())
	.build())
}
