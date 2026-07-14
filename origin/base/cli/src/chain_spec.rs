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

use sc_chain_spec::{ChainSpec as _, ChainSpecExtension, ChainType};
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[path = "../../../common/launch_authorization.rs"]
mod launch_authorization;

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

/// Load a non-live JSON spec. Live JSON must go through the production authorization scheme.
pub fn origin_spec_from_json_file(path: PathBuf) -> Result<OriginChainSpec, String> {
	let spec = OriginChainSpec::from_json_file(path.clone())?;
	if spec.chain_type() == ChainType::Live {
		return Err(format!(
			"refusing Live Origin chain spec from bare path {}; use origin-production:<input.json> so launch authorization cannot be bypassed",
			path.display()
		));
	}
	Ok(spec)
}

/// Returns the properties for the [`OriginChainSpec`].
pub fn origin_chain_spec_properties() -> serde_json::map::Map<String, serde_json::Value> {
	serde_json::json!({
		"ss58Format": "29",
		"tokenSymbol":"ORGN",
		"tokenDecimals": 10,
	})
	.as_object()
	.expect("Map given; qed")
	.clone()
}

/// Origin Relay development config (single validator )
pub fn origin_development_config() -> Result<OriginChainSpec, String> {
	Ok(OriginChainSpec::builder(
		origin_foundation_runtime::WASM_BINARY.ok_or("Origin Foundation WASM not available")?,
		Default::default(),
	)
	.with_name("Origin Foundation Development")
	.with_id("origin_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(
		origin_foundation_runtime::genesis_config_presets::origin_development_config_genesis(),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(origin_chain_spec_properties())
	.build())
}

/// Origin Relay local/staging config (multi-validator )
pub fn origin_local_config() -> Result<OriginChainSpec, String> {
	Ok(OriginChainSpec::builder(
		origin_foundation_runtime::WASM_BINARY.ok_or("Origin Foundation WASM not available")?,
		Default::default(),
	)
	.with_name("Origin Foundation Local")
	.with_id("origin_local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(
		origin_foundation_runtime::genesis_config_presets::origin_staging_config_genesis(),
	)
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(ORIGIN_TELEMETRY_URL.to_string(), 0)])
			.expect("Origin telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(origin_chain_spec_properties())
	.build())
}

/// Reviewed public launch material for a live Origin relay chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginProductionGenesisInput {
	/// Governed root account, encoded as an exact 32-byte `0x` hex value.
	pub root_key: String,
	/// Fixed initial validator accounts and public session keys.
	pub validators: Vec<OriginProductionValidator>,
	/// Explicitly endowed accounts; all root and validator accounts must be included.
	pub endowed_accounts: Vec<String>,
}

/// Public account and session keys for one production validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginProductionValidator {
	/// Validator account, encoded as exact 32-byte `0x` hex.
	pub account_id: String,
	/// BABE public key, encoded as exact 32-byte `0x` hex.
	pub babe: String,
	/// GRANDPA public key, encoded as exact 32-byte `0x` hex.
	pub grandpa: String,
	/// Parachain validator public key, encoded as exact 32-byte `0x` hex.
	pub para_validator: String,
	/// Parachain assignment public key, encoded as exact 32-byte `0x` hex.
	pub para_assignment: String,
	/// Authority-discovery public key, encoded as exact 32-byte `0x` hex.
	pub authority_discovery: String,
	/// Compressed BEEFY ECDSA public key, encoded as exact 33-byte `0x` hex.
	pub beefy: String,
}

fn decode_hex<const N: usize>(value: &str, field: &str) -> Result<[u8; N], String> {
	let raw = value.strip_prefix("0x").ok_or_else(|| format!("{field} must be 0x-prefixed"))?;
	let bytes = hex::decode(raw).map_err(|_| format!("{field} must be lowercase hexadecimal"))?;
	if raw.bytes().any(|byte| byte.is_ascii_uppercase()) || bytes.len() != N {
		return Err(format!("{field} must be exactly {N} lowercase-hex bytes"));
	}
	bytes.try_into().map_err(|_| format!("{field} must contain {N} bytes"))
}

fn production_account(value: &str, field: &str) -> Result<polkadot_primitives::AccountId, String> {
	Ok(polkadot_primitives::AccountId::new(decode_hex::<32>(value, field)?))
}

/// Construct a live Origin spec. Validation is deliberately fail-closed: production never derives
/// seed keys or silently reuses the local staging authorities.
fn origin_reviewed_config(
	input: OriginProductionGenesisInput,
	name: &str,
	id: &str,
	chain_type: ChainType,
) -> Result<OriginChainSpec, String> {
	if input.validators.len() < 4 {
		return Err("production Origin requires at least four validators".into());
	}

	let root_key = production_account(&input.root_key, "root_key")?;
	let endowed_accounts = input
		.endowed_accounts
		.iter()
		.enumerate()
		.map(|(index, value)| production_account(value, &format!("endowed_accounts[{index}]")))
		.collect::<Result<Vec<_>, _>>()?;
	let authorities = input
		.validators
		.iter()
		.enumerate()
		.map(|(index, validator)| {
			let beefy = decode_hex::<33>(&validator.beefy, &format!("validators[{index}].beefy"))?;
			if !matches!(beefy[0], 2 | 3) {
				return Err(format!(
					"validators[{index}].beefy must be a compressed ECDSA public key"
				));
			}
			Ok(origin_foundation_runtime::genesis_config_presets::OriginProductionAuthority {
				account_id: production_account(
					&validator.account_id,
					&format!("validators[{index}].account_id"),
				)?,
				babe: decode_hex::<32>(&validator.babe, &format!("validators[{index}].babe"))?,
				grandpa: decode_hex::<32>(
					&validator.grandpa,
					&format!("validators[{index}].grandpa"),
				)?,
				para_validator: decode_hex::<32>(
					&validator.para_validator,
					&format!("validators[{index}].para_validator"),
				)?,
				para_assignment: decode_hex::<32>(
					&validator.para_assignment,
					&format!("validators[{index}].para_assignment"),
				)?,
				authority_discovery: decode_hex::<32>(
					&validator.authority_discovery,
					&format!("validators[{index}].authority_discovery"),
				)?,
				beefy,
			})
		})
		.collect::<Result<Vec<_>, String>>()?;

	let endowed = endowed_accounts.iter().cloned().collect::<BTreeSet<_>>();
	let accounts = authorities
		.iter()
		.map(|authority| authority.account_id.clone())
		.collect::<BTreeSet<_>>();
	let session_keys = authorities
		.iter()
		.flat_map(|authority| {
			[
				authority.babe,
				authority.grandpa,
				authority.para_validator,
				authority.para_assignment,
				authority.authority_discovery,
			]
		})
		.collect::<BTreeSet<_>>();
	let beefy_keys = authorities.iter().map(|authority| authority.beefy).collect::<BTreeSet<_>>();
	if endowed.len() != endowed_accounts.len()
		|| accounts.len() != authorities.len()
		|| session_keys.len() != authorities.len() * 5
		|| beefy_keys.len() != authorities.len()
	{
		return Err("production accounts and session keys must be unique".into());
	}
	if !endowed.contains(&root_key) || accounts.iter().any(|account| !endowed.contains(account)) {
		return Err("root and validator accounts must be explicitly endowed".into());
	}
	if accounts.contains(&root_key) {
		return Err("production root and validator authority accounts must be separated".into());
	}
	let development_accounts = sp_keyring::Sr25519Keyring::well_known()
		.map(|key| polkadot_primitives::AccountId::from(key.public()))
		.collect::<BTreeSet<polkadot_primitives::AccountId>>();
	if development_accounts.contains(&root_key)
		|| accounts.iter().any(|account| development_accounts.contains(account))
	{
		return Err("well-known development accounts are forbidden in production genesis".into());
	}

	Ok(OriginChainSpec::builder(
		origin_foundation_runtime::WASM_BINARY.ok_or("Origin Foundation WASM not available")?,
		Default::default(),
	)
	.with_name(name)
	.with_id(id)
	.with_chain_type(chain_type)
	.with_genesis_config_patch(
		origin_foundation_runtime::genesis_config_presets::origin_production_config_genesis(
			authorities,
			root_key,
			endowed_accounts,
		),
	)
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(ORIGIN_TELEMETRY_URL.to_string(), 0)])
			.expect("Origin telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.with_properties(origin_chain_spec_properties())
	.build())
}

/// Construct the deterministic P5 candidate without making it a live network.
pub fn origin_candidate_config(
	input: OriginProductionGenesisInput,
) -> Result<OriginChainSpec, String> {
	origin_reviewed_config(input, "Origin Candidate", "origin-candidate", ChainType::Local)
}

/// Construct a live Origin spec only after the embedded launch ceremony verifies.
pub fn origin_production_config(
	input: OriginProductionGenesisInput,
	exact_input_bytes: &[u8],
) -> Result<OriginChainSpec, String> {
	launch_authorization::authorize_production(
		launch_authorization::LaunchChain::Origin,
		exact_input_bytes,
		include_bytes!("chain_spec.rs"),
	)?;
	origin_reviewed_config(input, "Origin", "origin", ChainType::Live)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn hex_value(byte: u8, length: usize) -> String {
		format!("0x{}", hex::encode(vec![byte; length]))
	}

	fn production_input() -> OriginProductionGenesisInput {
		let validators = (0..4)
			.map(|index| OriginProductionValidator {
				account_id: hex_value(0x41 + index, 32),
				babe: hex_value(0x51 + index, 32),
				grandpa: hex_value(0x61 + index, 32),
				para_validator: hex_value(0x71 + index, 32),
				para_assignment: hex_value(0x81 + index, 32),
				authority_discovery: hex_value(0x91 + index, 32),
				beefy: format!("0x02{}", hex::encode(vec![0xa1 + index; 32])),
			})
			.collect();
		OriginProductionGenesisInput {
			root_key: hex_value(0x40, 32),
			validators,
			endowed_accounts: (0x40..=0x44).map(|byte| hex_value(byte, 32)).collect(),
		}
	}

	#[test]
	fn candidate_builder_is_non_live_and_requires_separate_explicit_authorities() {
		let spec = origin_candidate_config(production_input()).expect("candidate input is valid");
		assert_eq!(spec.id(), "origin-candidate");
		assert_eq!(spec.chain_type(), ChainType::Local);

		let mut combined_authority = production_input();
		combined_authority.root_key = combined_authority.validators[0].account_id.clone();
		combined_authority.endowed_accounts.remove(0);
		assert!(origin_candidate_config(combined_authority).unwrap_err().contains("separated"));
	}

	#[test]
	fn candidate_chain_spec_storage_is_deterministic() {
		let first = origin_candidate_config(production_input())
			.expect("candidate input is valid")
			.build_storage()
			.expect("candidate genesis builds");
		let second = origin_candidate_config(production_input())
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
		let error = origin_production_config(input, &bytes).unwrap_err();
		assert!(error.contains("activation_state is not production-approved"));
	}

	#[test]
	fn bare_live_json_cannot_bypass_production_authorization() {
		let candidate = origin_candidate_config(production_input()).unwrap();
		let mut json: serde_json::Value =
			serde_json::from_str(&candidate.as_json(false).unwrap()).unwrap();
		json["chainType"] = serde_json::json!("Live");
		let path =
			std::env::temp_dir().join(format!("origin-live-bypass-{}.json", std::process::id()));
		std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
		let error = origin_spec_from_json_file(path.clone()).unwrap_err();
		let _ = std::fs::remove_file(path);
		assert!(error.contains("refusing Live Origin chain spec"));
	}
}
