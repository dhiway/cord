// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Cumulus.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

use crate::chain_spec::{Extensions, GenericChainSpec};
use sc_service::ChainType;

pub fn asset_hub_loom_local_testnet_config() -> GenericChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 42.into());
	properties.insert("tokenSymbol".into(), "UNT".into());
	properties.insert("tokenDecimals".into(), 12.into());

	GenericChainSpec::builder(
		cord_weave_asset_hub_runtime::WASM_BINARY
			.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "loom-local".into(), para_id: 1000 },
	)
	.with_name("Loom Asset Hub Local")
	.with_id("loom-asset-hub-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(
		cord_weave_asset_hub_runtime::genesis_config_presets::asset_hub_loom_local_testnet_genesis(
			1000.into(),
		),
	)
	.with_properties(properties)
	.build()
}

pub fn asset_hub_loom_development_config() -> GenericChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 42.into());
	properties.insert("tokenSymbol".into(), "UNT".into());
	properties.insert("tokenDecimals".into(), 12.into());

	GenericChainSpec::builder(
		cord_weave_asset_hub_runtime::WASM_BINARY
			.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "loom-local".into(), para_id: 1000 },
	)
	.with_name("Loom Asset Hub Development")
	.with_id("loom-asset-hub-dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(
		cord_weave_asset_hub_runtime::genesis_config_presets::asset_hub_loom_development_genesis(
			1000.into(),
		),
	)
	.with_properties(properties)
	.build()
}
