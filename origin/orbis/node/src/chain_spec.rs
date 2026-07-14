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

//! Chain specification helpers for the Orbis Commons system chain.

use cumulus_primitives_core::ParaId;
use origin_commons_runtime::genesis_config_presets::{
	orbis_development_genesis, orbis_local_testnet_genesis, orbis_production_genesis,
};
use origin_runtime_constants::system_parachain::ORBIS_ID;
use polkadot_omni_node_lib::chain_spec::{GenericChainSpec, LoadSpec};
use sc_chain_spec::{ChainSpec as _, ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::crypto::UncheckedFrom;
use std::{collections::BTreeSet, fs, path::Path};

#[path = "../../../common/launch_authorization.rs"]
mod launch_authorization;

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
		origin_commons_runtime::WASM_BINARY.expect("Orbis Commons WASM binary was not built"),
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

/// Orbis Commons development network.
pub fn orbis_development() -> ChainSpec {
	orbis_spec(
		"Orbis Commons Development",
		"orbis-dev",
		ChainType::Development,
		"origin-dev",
		orbis_development_genesis(ParaId::from(ORBIS_ID)),
	)
}

/// Orbis Commons local network.
pub fn orbis_local() -> ChainSpec {
	orbis_spec(
		"Orbis Commons Local",
		"orbis-local",
		ChainType::Local,
		"origin-local",
		orbis_local_testnet_genesis(ParaId::from(ORBIS_ID)),
	)
}

/// Reviewed operator input used to construct a live Orbis chain spec.
///
/// Account and Aura identifiers are exact 32-byte `0x` hex values. The format intentionally does
/// not accept development seed phrases or infer feeless accounts from endowments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionGenesisInput {
	/// Origin relay chain-spec identifier used by the live network.
	pub relay_chain: String,
	/// Network identifier stored by the Orbis Token pallet.
	pub token_network_id: u16,
	/// Governed root account; this should be a reviewed multisig/HSM-controlled account.
	pub root_key: String,
	/// Fixed permissioned collator accounts and their Aura session keys.
	pub collators: Vec<ProductionCollator>,
	/// Explicitly endowed accounts.
	pub endowed_accounts: Vec<String>,
	/// Explicitly feeless accounts. No implicit grant is made to endowed accounts.
	#[serde(default)]
	pub feeless_accounts: Vec<String>,
}

/// One production collator identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionCollator {
	/// Collator account as exact 32-byte `0x` hex.
	pub account_id: String,
	/// Aura session key as exact 32-byte `0x` hex.
	pub aura_id: String,
}

fn decode_hex32(value: &str, field: &str) -> Result<[u8; 32], String> {
	let raw = value.strip_prefix("0x").ok_or_else(|| format!("{field} must be 0x-prefixed"))?;
	let bytes = hex::decode(raw).map_err(|_| format!("{field} must be lowercase hexadecimal"))?;
	if raw.bytes().any(|byte| byte.is_ascii_uppercase()) || bytes.len() != 32 {
		return Err(format!("{field} must be exactly 32 lowercase-hex bytes"));
	}
	bytes.try_into().map_err(|_| format!("{field} must contain 32 bytes"))
}

fn account(value: &str, field: &str) -> Result<parachains_common::AccountId, String> {
	Ok(parachains_common::AccountId::new(decode_hex32(value, field)?))
}

fn reviewed_spec(
	input: ProductionGenesisInput,
	name: &str,
	id: &str,
	chain_type: ChainType,
	relay_chain_id: &str,
) -> Result<ChainSpec, String> {
	if input.relay_chain != "origin" {
		return Err("production Orbis must target the live Origin chain id `origin`".into());
	}
	if input.token_network_id != ORBIS_ID as u16 {
		return Err(format!(
			"production Orbis token_network_id must equal parachain id {ORBIS_ID}"
		));
	}
	if input.collators.len() < 2 {
		return Err("production Orbis requires at least two fixed collators".into());
	}

	let root_key = account(&input.root_key, "root_key")?;
	let endowed_accounts = input
		.endowed_accounts
		.iter()
		.enumerate()
		.map(|(index, value)| account(value, &format!("endowed_accounts[{index}]")))
		.collect::<Result<Vec<_>, _>>()?;
	let feeless_accounts = input
		.feeless_accounts
		.iter()
		.enumerate()
		.map(|(index, value)| account(value, &format!("feeless_accounts[{index}]")))
		.collect::<Result<Vec<_>, _>>()?;
	let invulnerables = input
		.collators
		.iter()
		.enumerate()
		.map(|(index, item)| {
			Ok((
				account(&item.account_id, &format!("collators[{index}].account_id"))?,
				parachains_common::AuraId::unchecked_from(decode_hex32(
					&item.aura_id,
					&format!("collators[{index}].aura_id"),
				)?),
			))
		})
		.collect::<Result<Vec<_>, String>>()?;

	let unique_endowed = endowed_accounts.iter().cloned().collect::<BTreeSet<_>>();
	let unique_feeless = feeless_accounts.iter().cloned().collect::<BTreeSet<_>>();
	let unique_collators = invulnerables
		.iter()
		.map(|(account, _)| account.clone())
		.collect::<BTreeSet<_>>();
	let unique_aura = input.collators.iter().map(|item| &item.aura_id).collect::<BTreeSet<_>>();
	if unique_endowed.len() != endowed_accounts.len()
		|| unique_feeless.len() != feeless_accounts.len()
		|| unique_collators.len() != invulnerables.len()
		|| unique_aura.len() != invulnerables.len()
	{
		return Err("production identities must be unique within each role".into());
	}
	if !unique_endowed.contains(&root_key)
		|| invulnerables.iter().any(|(account, _)| !unique_endowed.contains(account))
		|| feeless_accounts.iter().any(|account| !unique_endowed.contains(account))
	{
		return Err("root, collator, and feeless accounts must be explicitly endowed".into());
	}
	if unique_collators.contains(&root_key) {
		return Err("production root and collator authority accounts must be separated".into());
	}
	let development_accounts = sp_keyring::Sr25519Keyring::well_known()
		.map(parachains_common::AccountId::from)
		.collect::<BTreeSet<_>>();
	if development_accounts.contains(&root_key)
		|| invulnerables.iter().any(|(account, _)| development_accounts.contains(account))
		|| endowed_accounts.iter().any(|account| development_accounts.contains(account))
	{
		return Err("well-known development accounts are forbidden in production genesis".into());
	}

	Ok(orbis_spec(
		name,
		id,
		chain_type,
		relay_chain_id,
		orbis_production_genesis(
			invulnerables,
			endowed_accounts,
			feeless_accounts,
			ParaId::from(ORBIS_ID),
			input.token_network_id.into(),
			root_key,
		),
	))
}

fn candidate_spec(input: ProductionGenesisInput) -> Result<ChainSpec, String> {
	reviewed_spec(input, "Orbis Candidate", "orbis-candidate", ChainType::Local, "origin-candidate")
}

fn production_spec(
	input: ProductionGenesisInput,
	exact_input_bytes: &[u8],
) -> Result<ChainSpec, String> {
	launch_authorization::authorize_production(
		launch_authorization::LaunchChain::Orbis,
		exact_input_bytes,
		include_bytes!("chain_spec.rs"),
	)?;
	reviewed_spec(input, "Orbis", "orbis", ChainType::Live, "origin")
}

fn candidate_spec_from_file(path: &Path) -> Result<ChainSpec, String> {
	let bytes = fs::read(path).map_err(|error| {
		format!("failed to read candidate genesis input {}: {error}", path.display())
	})?;
	let input: ProductionGenesisInput = serde_json::from_slice(&bytes)
		.map_err(|error| format!("invalid candidate genesis input {}: {error}", path.display()))?;
	candidate_spec(input)
}

fn production_spec_from_file(path: &Path) -> Result<ChainSpec, String> {
	let bytes = fs::read(path).map_err(|error| {
		format!("failed to read production genesis input {}: {error}", path.display())
	})?;
	let input: ProductionGenesisInput = serde_json::from_slice(&bytes)
		.map_err(|error| format!("invalid production genesis input {}: {error}", path.display()))?;
	production_spec(input, &bytes)
}

#[derive(Debug)]
pub(crate) struct ChainSpecLoader;

impl LoadSpec for ChainSpecLoader {
	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			// -- Orbis
			"orbis-dev" => Box::new(orbis_development()),
			"orbis-local" => Box::new(orbis_local()),
				"orbis" => return Err(
					"the live Orbis spec is never inferred; use orbis-candidate:<input.json> for deterministic evidence or orbis-production:<input.json> after launch approval".into(),
				),
				value if value.starts_with("orbis-candidate:") => {
					let path = value.trim_start_matches("orbis-candidate:");
					if path.is_empty() {
						return Err("orbis-candidate requires an input JSON path".into());
					}
					Box::new(candidate_spec_from_file(Path::new(path))?)
				},
			value if value.starts_with("orbis-production:") => {
				let path = value.trim_start_matches("orbis-production:");
				if path.is_empty() {
					return Err("orbis-production requires a reviewed input JSON path".into());
				}
				Box::new(production_spec_from_file(Path::new(path))?)
			},
			// -- Fallback (generic chainspec)
			"" => {
				log::warn!(
					"No ChainSpec.id specified, defaulting to the Orbis development chain spec"
				);
				Box::new(orbis_development())
			},

			// -- Loading a specific spec from disk
				path => {
					let spec = GenericChainSpec::from_json_file(path.into())?;
					if spec.chain_type() == ChainType::Live {
						return Err(format!(
							"refusing Live Orbis chain spec from bare path {path}; use orbis-production:<input.json> so launch authorization cannot be bypassed"
						));
					}
					Box::new(spec)
				},
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

	fn hex_account(byte: u8) -> String {
		format!("0x{}", hex::encode([byte; 32]))
	}

	fn production_input() -> ProductionGenesisInput {
		ProductionGenesisInput {
			relay_chain: "origin".into(),
			token_network_id: ORBIS_ID as u16,
			root_key: hex_account(0x41),
			collators: vec![
				ProductionCollator { account_id: hex_account(0x42), aura_id: hex_account(0x52) },
				ProductionCollator { account_id: hex_account(0x43), aura_id: hex_account(0x53) },
			],
			endowed_accounts: vec![hex_account(0x41), hex_account(0x42), hex_account(0x43)],
			feeless_accounts: vec![],
		}
	}

	#[test]
	fn live_alias_never_falls_back_to_local_development_genesis() {
		let error = ChainSpecLoader.load_spec("orbis").unwrap_err();
		assert!(error.contains("never inferred"));
	}

	#[test]
	fn candidate_builder_is_non_live_and_requires_explicit_unique_non_development_authorities() {
		let spec = candidate_spec(production_input()).expect("reviewed explicit input is accepted");
		assert_eq!(spec.id(), "orbis-candidate");
		assert_eq!(spec.chain_type(), ChainType::Local);
		assert_eq!(spec.extensions().relay_chain, "origin-candidate");
		assert_eq!(spec.extensions().para_id, ORBIS_ID);

		let mut duplicate = production_input();
		duplicate.collators[1].aura_id = duplicate.collators[0].aura_id.clone();
		assert!(candidate_spec(duplicate).unwrap_err().contains("unique"));

		let mut development = production_input();
		development.root_key = format!(
			"0x{}",
			hex::encode(
				<origin_commons_runtime::AccountId>::from(sp_keyring::Sr25519Keyring::Alice).as_ref()
			)
		);
		development.endowed_accounts[0] = development.root_key.clone();
		assert!(candidate_spec(development).unwrap_err().contains("development accounts"));

		let mut wrong_relay = production_input();
		wrong_relay.relay_chain = "another-live-relay".into();
		assert!(candidate_spec(wrong_relay).unwrap_err().contains("live Origin"));

		let mut wrong_network = production_input();
		wrong_network.token_network_id = 29;
		assert!(candidate_spec(wrong_network).unwrap_err().contains("parachain id"));

		let mut combined_authority = production_input();
		combined_authority.root_key = combined_authority.collators[0].account_id.clone();
		combined_authority.endowed_accounts.remove(0);
		assert!(candidate_spec(combined_authority).unwrap_err().contains("separated"));
	}

	#[test]
	fn candidate_chain_spec_storage_is_deterministic() {
		let first = candidate_spec(production_input())
			.expect("candidate input is valid")
			.build_storage()
			.expect("candidate genesis builds");
		let second = candidate_spec(production_input())
			.expect("candidate input is valid")
			.build_storage()
			.expect("candidate genesis builds");

		assert_eq!(first.top, second.top);
		assert_eq!(first.children_default, second.children_default);
	}

	#[test]
	fn unsigned_pending_candidate_cannot_build_a_live_spec() {
		let bytes = serde_json::to_vec(&production_input()).unwrap();
		let input = serde_json::from_slice(&bytes).unwrap();
		let error = production_spec(input, &bytes).unwrap_err();
		assert!(error.contains("activation_state is not production-approved"));
	}

	#[test]
	fn bare_live_json_cannot_bypass_production_authorization() {
		let candidate = candidate_spec(production_input()).unwrap();
		let mut json: serde_json::Value =
			serde_json::from_str(&candidate.as_json(false).unwrap()).unwrap();
		json["chainType"] = serde_json::json!("Live");
		let path =
			std::env::temp_dir().join(format!("orbis-live-bypass-{}.json", std::process::id()));
		std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
		let error = ChainSpecLoader.load_spec(path.to_str().unwrap()).unwrap_err();
		let _ = std::fs::remove_file(path);
		assert!(error.contains("refusing Live Orbis chain spec"));
	}
}
