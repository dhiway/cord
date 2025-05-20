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

use polkadot_omni_node_lib::{
	chain_spec::{GenericChainSpec, LoadSpec},
	runtime::{
		AuraConsensusId, BlockNumber, Consensus, Runtime, RuntimeResolver as RuntimeResolverT,
	},
};
use sc_chain_spec::ChainSpec;
pub mod coretime;
pub mod entity;

#[derive(Debug)]
pub(crate) struct ChainSpecLoader;

impl LoadSpec for ChainSpecLoader {
	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			// // -- Coretime
			// "origin-coretime" | "coretime" => Box::new(GenericChainSpec::from_json_bytes(
			// 	&include_bytes!("../../chain-specs/tbd.json")[..],
			// )?),
			"origin-coretime-dev" | "coretime-dev" =>
				Box::new(coretime::coretime_origin_staging_development_config()),
			"origin-coretime-local" | "coretime-local" =>
				Box::new(coretime::coretime_origin_staging_local_config()),
			"origin-coretime-genesis" | "coretime-genesis" =>
				Box::new(coretime::coretime_origin_genesis_config()),

			// // -- Entity
			// "origin-entity" | "entity" => Box::new(GenericChainSpec::from_json_bytes(
			// 	&include_bytes!("../../chain-specs/tbd.json")[..],
			// )?),
			"origin-entity-dev" | "entity-dev" =>
				Box::new(entity::entity_origin_staging_development_config()),
			"origin-entity-local" | "entity-local" =>
				Box::new(entity::entity_origin_staging_local_config()),
			"origin-entity-genesis" | "entity-genesis" =>
				Box::new(entity::origin_entity_genesis_config()),

			// -- Fallback (generic chainspec)
			"" => {
				log::warn!("No ChainSpec.id specified, so using default one, based on origin entity runtime");
				Box::new(entity::entity_origin_staging_development_config())
			},

			// -- Loading a specific spec from disk
			path => Box::new(GenericChainSpec::from_json_file(path.into())?),
		})
	}
}

/// Helper enum that is used for better distinction of different parachain/runtime configuration
/// (it is based/calculated on ChainSpec's ID attribute)
#[derive(Debug, PartialEq)]
enum LegacyRuntime {
	Omni,
	Asset,
	Coretime,
	Entity,
}

impl LegacyRuntime {
	fn from_id(id: &str) -> LegacyRuntime {
		let id = id.replace('_', "-");

		if id.starts_with("origin-asset") || id.starts_with("asset-hub") {
			LegacyRuntime::Asset
		} else if id.starts_with("origin-coretime") || id.starts_with("coretime") {
			LegacyRuntime::Coretime
		} else if id.starts_with("origin-entity") || id.starts_with("entity") {
			LegacyRuntime::Entity
		} else {
			log::warn!(
				"No specific runtime was recognized for ChainSpec's id: '{}', \
				so Runtime::Omni(Consensus::Aura) will be used",
				id
			);
			LegacyRuntime::Omni
		}
	}
}

#[derive(Debug)]
pub(crate) struct RuntimeResolver;

impl RuntimeResolverT for RuntimeResolver {
	fn runtime(&self, chain_spec: &dyn ChainSpec) -> sc_cli::Result<Runtime> {
		let legacy_runtime = LegacyRuntime::from_id(chain_spec.id());
		Ok(match legacy_runtime {
			LegacyRuntime::Asset |
			LegacyRuntime::Coretime |
			LegacyRuntime::Entity |
			LegacyRuntime::Omni =>
				Runtime::Omni(BlockNumber::U32, Consensus::Aura(AuraConsensusId::Sr25519)),
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup, ChainType, Extension};
	use serde::{Deserialize, Serialize};
	use sp_keyring::Sr25519Keyring;

	#[derive(
		Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension, Default,
	)]
	#[serde(deny_unknown_fields)]
	pub struct Extensions1 {
		pub attribute1: String,
		pub attribute2: u32,
	}

	#[derive(
		Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension, Default,
	)]
	#[serde(deny_unknown_fields)]
	pub struct Extensions2 {
		pub attribute_x: String,
		pub attribute_y: String,
		pub attribute_z: u32,
	}

	pub type DummyChainSpec<E> = sc_service::GenericChainSpec<E>;

	pub fn create_default_with_extensions<E: Extension>(
		id: &str,
		extension: E,
	) -> DummyChainSpec<E> {
		DummyChainSpec::builder(
			rococo_parachain_runtime::WASM_BINARY
				.expect("WASM binary was not built, please build it!"),
			extension,
		)
		.with_name("Dummy local testnet")
		.with_id(id)
		.with_chain_type(ChainType::Local)
		.with_genesis_config_patch(crate::chain_spec::rococo_parachain::testnet_genesis(
			Sr25519Keyring::Alice.to_account_id(),
			vec![Sr25519Keyring::Alice.public().into(), Sr25519Keyring::Bob.public().into()],
			vec![Sr25519Keyring::Bob.to_account_id()],
			1000.into(),
		))
		.build()
	}

	#[test]
	fn test_legacy_runtime_for_different_chain_specs() {
		let chain_spec =
			create_default_with_extensions("penpal-rococo-1000", Extensions2::default());
		assert_eq!(LegacyRuntime::Penpal, LegacyRuntime::from_id(chain_spec.id()));

		let chain_spec = crate::chain_spec::rococo_parachain::rococo_parachain_local_config();
		assert_eq!(LegacyRuntime::Omni, LegacyRuntime::from_id(chain_spec.id()));
	}
}
