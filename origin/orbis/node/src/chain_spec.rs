// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Chain specification helpers for the Orbis system chain.

use cumulus_primitives_core::ParaId;
use origin_orbis_runtime::genesis_config_presets::{
	orbis_development_genesis, orbis_local_testnet_genesis,
};
use origin_runtime_constants::system_parachain::ORBIS_ID;
use polkadot_omni_node_lib::chain_spec::{GenericChainSpec, LoadSpec};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};

/// Specialized `ChainSpec` for the Orbis system chain.
pub type ChainSpec = sc_service::GenericChainSpec<Extensions>;

/// Chain spec extensions required by Cumulus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
pub struct Extensions {
	/// Relay chain identifier.
	#[serde(alias = "relayChain", alias = "RelayChain")]
	pub relay_chain: String,
	/// Parachain identifier.
	#[serde(alias = "paraId", alias = "ParaId")]
	pub para_id: u32,
}

const ORBIS_PROTOCOL_ID: &str = "orbis";

fn properties() -> sc_chain_spec::Properties {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 29.into());
	properties.insert("tokenSymbol".into(), "ORGN".into());
	properties.insert("tokenDecimals".into(), 10.into());
	properties
}

fn orbis_spec(
	name: &str,
	id: &str,
	chain_type: ChainType,
	relay_chain: &str,
	genesis_patch: serde_json::Value,
) -> ChainSpec {
	ChainSpec::builder(
		origin_orbis_runtime::WASM_BINARY.expect("Orbis WASM binary was not built"),
		Extensions { relay_chain: relay_chain.into(), para_id: ORBIS_ID },
	)
	.with_name(name)
	.with_id(id)
	.with_chain_type(chain_type)
	.with_genesis_config_patch(genesis_patch)
	.with_protocol_id(ORBIS_PROTOCOL_ID)
	.with_properties(properties())
	.build()
}

/// Orbis development network.
pub fn orbis_development() -> ChainSpec {
	orbis_spec(
		"Orbis Development",
		"orbis-dev",
		ChainType::Development,
		"origin-dev",
		orbis_development_genesis(ParaId::from(ORBIS_ID)),
	)
}

/// Orbis local network.
pub fn orbis_local() -> ChainSpec {
	orbis_spec(
		"Orbis Local",
		"orbis-local",
		ChainType::Local,
		"origin-local",
		orbis_local_testnet_genesis(ParaId::from(ORBIS_ID)),
	)
}

#[derive(Debug)]
pub(crate) struct ChainSpecLoader;

impl LoadSpec for ChainSpecLoader {
	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			// -- Orbis
			"orbis-dev" => Box::new(orbis_development()),
			"orbis-local" | "orbis" => Box::new(orbis_local()),
			// -- Fallback (generic chainspec)
			"" => {
				log::warn!(
					"No ChainSpec.id specified, defaulting to the Orbis development chain spec"
				);
				Box::new(orbis_development())
			},

			// -- Loading a specific spec from disk
			path => Box::new(GenericChainSpec::from_json_file(path.into())?),
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn orbis_development_spec_targets_origin_and_para_1006() {
		let spec = orbis_development();

		assert_eq!(spec.id(), "orbis-dev");
		assert_eq!(spec.protocol_id(), Some(ORBIS_PROTOCOL_ID));
		assert_eq!(spec.extensions().relay_chain, "origin-dev");
		assert_eq!(spec.extensions().para_id, ORBIS_ID);
	}

	#[test]
	fn orbis_local_spec_targets_origin_and_para_1006() {
		let spec = orbis_local();

		assert_eq!(spec.id(), "orbis-local");
		assert_eq!(spec.extensions().relay_chain, "origin-local");
		assert_eq!(spec.extensions().para_id, ORBIS_ID);
	}
}
