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

use polkadot_omni_node_lib::chain_spec::{Extensions, GenericChainSpec};
use sc_service::ChainType;

pub fn entity_origin_staging_development_config() -> GenericChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 29.into());
	properties.insert("tokenSymbol".into(), "ORU".into());
	properties.insert("tokenDecimals".into(), 10.into());

	GenericChainSpec::builder(
		cord_origin_entity_staging_runtime::WASM_BINARY
			.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "origin-dev".into(), para_id: 1004 },
	)
	.with_name("Origin Entity Development")
	.with_id("origin_entity_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
	.with_properties(properties)
	.build()
}

pub fn entity_origin_staging_local_config() -> GenericChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 29.into());
	properties.insert("tokenSymbol".into(), "ORU".into());
	properties.insert("tokenDecimals".into(), 10.into());
	GenericChainSpec::builder(
		cord_origin_entity_staging_runtime::WASM_BINARY
			.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "origin-dev".into(), para_id: 1004 },
	)
	.with_name("Origin Entity Local")
	.with_id("origin-entity-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
	.with_properties(properties)
	.build()
}

pub fn origin_entity_genesis_config() -> GenericChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 29.into());
	properties.insert("tokenSymbol".into(), "ORU".into());
	properties.insert("tokenDecimals".into(), 10.into());
	let para_id = 1005;
	GenericChainSpec::builder(
		cord_origin_entity_staging_runtime::WASM_BINARY
			.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "origin".into(), para_id },
	)
	.with_name("Origin Entity")
	.with_id("origin-entity")
	.with_chain_type(ChainType::Live)
	.with_genesis_config_preset_name("genesis")
	.with_properties(properties)
	.build()
}
