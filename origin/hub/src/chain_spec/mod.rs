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
pub mod system;

#[derive(Debug)]
pub(crate) struct ChainSpecLoader;

impl LoadSpec for ChainSpecLoader {
	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			// -- System
			"origin-system-dev" | "system-dev" => {
				Box::new(system::system_origin_staging_development_config())
			},
			"origin-system-na-dev" | "system-na-dev" => {
				Box::new(system::system_origin_staging_development_config_na())
			},
			"origin-system-local" | "system-local" => {
				Box::new(system::system_origin_staging_local_config())
			},
			"origin-system-na-local" | "system-na-local" => {
				Box::new(system::system_origin_staging_local_config_na())
			},
			"origin-system" | "system-genesis" | "system" => {
				Box::new(system::origin_system_genesis_config())
			},

			// -- Fallback (generic chainspec)
			"" => {
				log::warn!(
					"No ChainSpec.id specified, defaulting to the origin system development chain spec"
				);
				Box::new(system::system_origin_staging_development_config())
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
	System,
}

impl LegacyRuntime {
	fn from_id(id: &str) -> LegacyRuntime {
		let id = id.replace('_', "-");

		if id.starts_with("origin-system") || id.starts_with("system") {
			LegacyRuntime::System
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
		LegacyRuntime::System | LegacyRuntime::Omni => {
			Runtime::Omni(BlockNumber::U32, Consensus::Aura(AuraConsensusId::Sr25519))
		},
		})
	}
}
