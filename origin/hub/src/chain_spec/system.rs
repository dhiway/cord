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

use origin_hub_system_runtime::genesis_config_presets::{
	system_origin_development_genesis, system_origin_local_testnet_genesis,
};
use origin_runtime_constants::system_parachain::{ORIGIN_HUB_IN_ID, ORIGIN_HUB_NA_ID};
use polkadot_omni_node_lib::chain_spec::{Extensions, GenericChainSpec};
use cumulus_primitives_core::ParaId;
use sc_service::ChainType;
const DEFAULT_PROTOCOL_ID: &str = "0rbit";

fn properties() -> sc_chain_spec::Properties {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 29.into());
	properties.insert("tokenSymbol".into(), "ORGN".into());
	properties.insert("tokenDecimals".into(), 10.into());
	properties
}

fn system_spec(
	name: &str,
	id: &str,
	chain_type: ChainType,
	relay_chain: &str,
	para_id: u32,
	genesis_patch: serde_json::Value,
) -> GenericChainSpec {

	GenericChainSpec::builder(
		origin_hub_system_runtime::WASM_BINARY
			.expect("WASM binary was not built, please build it!"),
		Extensions::new(relay_chain.into(), para_id),
	)
	.with_name(name)
	.with_id(id)
	.with_chain_type(chain_type)
	.with_genesis_config_patch(genesis_patch)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(properties())
	.build()
}

pub fn system_origin_staging_development_config() -> GenericChainSpec {
	system_spec(
		"Origin System Development",
		"origin-system-dev",
		ChainType::Development,
		"origin-dev",
		ORIGIN_HUB_IN_ID,
		system_origin_development_genesis(ParaId::from(ORIGIN_HUB_IN_ID)),
	)
}

pub fn system_origin_staging_development_config_na() -> GenericChainSpec {
	system_spec(
		"Origin System NA Development",
		"origin-system-na-dev",
		ChainType::Development,
		"origin-dev",
		ORIGIN_HUB_NA_ID,
		system_origin_development_genesis(ParaId::from(ORIGIN_HUB_NA_ID)),
	)
}

pub fn system_origin_staging_local_config() -> GenericChainSpec {
	system_spec(
		"Origin System Local",
		"origin-system-local",
		ChainType::Local,
		"origin-dev",
		ORIGIN_HUB_IN_ID,
		system_origin_local_testnet_genesis(ParaId::from(ORIGIN_HUB_IN_ID)),
	)
}

pub fn system_origin_staging_local_config_na() -> GenericChainSpec {
	system_spec(
		"Origin System NA Local",
		"origin-system-na-local",
		ChainType::Local,
		"origin-dev",
		ORIGIN_HUB_NA_ID,
		system_origin_local_testnet_genesis(ParaId::from(ORIGIN_HUB_NA_ID)),
	)
}

pub fn origin_system_genesis_config() -> GenericChainSpec {
	system_spec(
		"Origin System",
		"origin-system",
		ChainType::Live,
		"origin",
		ORIGIN_HUB_IN_ID,
		system_origin_local_testnet_genesis(ParaId::from(ORIGIN_HUB_IN_ID)),
	)
}
