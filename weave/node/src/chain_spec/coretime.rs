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

use crate::chain_spec::{Extensions, GenericChainSpec};
use sc_chain_spec::ChainType;

pub fn coretime_loom_local_testnet_config() -> GenericChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 42.into());
	properties.insert("tokenSymbol".into(), "UNT".into());
	properties.insert("tokenDecimals".into(), 12.into());

	GenericChainSpec::builder(
		cord_weave_coretime_runtime::WASM_BINARY
			.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "loom-local".into(), para_id: 1001 },
	)
	.with_name("Loom Coretime Local")
	.with_id("loom-coretime-local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(
		cord_weave_coretime_runtime::genesis_config_presets::coretime_loom_local_testnet_genesis(
			1001.into(),
		),
	)
	.with_properties(properties)
	.build()
}

pub fn coretime_loom_development_config() -> GenericChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 42.into());
	properties.insert("tokenSymbol".into(), "UNT".into());
	properties.insert("tokenDecimals".into(), 12.into());

	GenericChainSpec::builder(
		cord_weave_coretime_runtime::WASM_BINARY
			.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "loom-local".into(), para_id: 1001 },
	)
	.with_name("Loom Cortime Development")
	.with_id("loom-coretime-dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(
		cord_weave_coretime_runtime::genesis_config_presets::coretime_loom_development_genesis(
			1001.into(),
		),
	)
	.with_properties(properties)
	.build()
}
