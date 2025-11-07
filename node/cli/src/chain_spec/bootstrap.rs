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

//! CORD custom chain configurations.

use cord_orb_runtime::SessionKeys as OrbSessionKeys;
pub use cord_primitives::{AccountId, Balance, NodeId, Signature};
use hex::decode;
use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::crypto::UncheckedInto;
use sp_runtime::AccountId32;

use crate::chain_spec::{get_properties, Extensions, CORD_TELEMETRY_URL, DEFAULT_PROTOCOL_ID};

pub use cord_orb_runtime_constants::currency::UNITS;

/// Specialized `ChainSpec`.
pub type CordChainSpec = sc_service::GenericChainSpec<Extensions>;

const ENDOWMENT: u128 = 500_000_000_000 * UNITS;
const CUSTOM_SPEC_ID: &str = "cord-orb-custom";

#[derive(Debug, Clone)]
pub struct ChainParams {
	pub chain_name: String,
	pub chain_type: ChainType,
	pub authorities: Vec<AuthorityKeys>,
	pub sudo_key: String,
	pub network_id: u32,
}

impl ChainParams {
	pub fn chain_type(&self) -> ChainType {
		self.chain_type.clone()
	}

	pub fn chain_name(&self) -> &str {
		&self.chain_name
	}
}

#[derive(Debug, Clone)]
pub struct AuthorityKeys {
	pub stash: String,
	pub babe: String,
	pub grandpa: String,
	pub authority_discovery: String,
}

pub fn cord_custom_config(config: &ChainParams) -> Result<CordChainSpec, String> {
	let wasm_binary =
		cord_orb_runtime::WASM_BINARY.ok_or_else(|| "Orb wasm not available".to_owned())?;
	let properties: sc_service::Properties = get_properties("UNITS", 12, config.network_id);
	let genesis_patch = cord_custom_config_genesis(config)?;

	let spec = CordChainSpec::builder(wasm_binary, Default::default())
		.with_name(config.chain_name())
		.with_id(CUSTOM_SPEC_ID)
		.with_chain_type(config.chain_type())
		.with_genesis_config_patch(genesis_patch)
		.with_telemetry_endpoints(
			TelemetryEndpoints::new(vec![(CORD_TELEMETRY_URL.to_string(), 0)])
				.map_err(|e| e.to_string())?,
		)
		.with_protocol_id(DEFAULT_PROTOCOL_ID)
		.with_properties(properties)
		.build();

	Ok(spec)
}

fn cord_custom_config_genesis(config: &ChainParams) -> Result<serde_json::Value, String> {
	let initial_authorities = parse_authorities(&config.authorities)?;
	let root_key =
		parse_account_id(&config.sudo_key).map_err(|e| format!("Invalid sudo_key: {e}"))?;

	let authority_accounts: Vec<_> =
		initial_authorities.iter().map(|(account, _, _, _)| account.clone()).collect();
	let balances: Vec<_> =
		authority_accounts.iter().cloned().map(|account| (account, ENDOWMENT)).collect();
	let session_keys: Vec<_> = initial_authorities
		.iter()
		.map(|(account, babe, grandpa, authority_discovery)| {
			(
				account.clone(),
				account.clone(),
				cord_custom_session_keys(
					babe.clone(),
					grandpa.clone(),
					authority_discovery.clone(),
				),
			)
		})
		.collect();

	Ok(serde_json::json!({
		"balances": {
			"balances": balances,
		},
		"token": { "protocolId": DEFAULT_PROTOCOL_ID.to_string(), "networkId": config.network_id as u16 },
		"authorityManager":  {
			"initialAuthorities": authority_accounts,
		},
		"session":  {
			"keys": session_keys,
		},
		"babe":  {
			"epochConfig": Some(cord_orb_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		"sudo": { "key": Some(root_key) },
	}))
}

fn cord_custom_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	authority_discovery: AuthorityDiscoveryId,
) -> OrbSessionKeys {
	OrbSessionKeys { babe, grandpa, authority_discovery }
}

fn parse_authorities(
	authorities: &[AuthorityKeys],
) -> Result<Vec<(AccountId, BabeId, GrandpaId, AuthorityDiscoveryId)>, String> {
	authorities
		.iter()
		.enumerate()
		.map(|(idx, authority)| {
			let stash = parse_account_id(&authority.stash)
				.map_err(|e| format!("Authority {idx} stash key error: {e}"))?;
			let babe = parse_babe_id(&authority.babe)
				.map_err(|e| format!("Authority {idx} BABE key error: {e}"))?;
			let grandpa = parse_grandpa_id(&authority.grandpa)
				.map_err(|e| format!("Authority {idx} GRANDPA key error: {e}"))?;
			let authority_discovery = parse_authority_discovery_id(&authority.authority_discovery)
				.map_err(|e| format!("Authority {idx} authority discovery key error: {e}"))?;

			Ok((stash, babe, grandpa, authority_discovery))
		})
		.collect()
}

fn parse_account_id(value: &str) -> Result<AccountId, String> {
	let raw = decode_hex_to_array::<32>(value, "stash account key")?;
	Ok(AccountId32::new(raw))
}

fn parse_babe_id(value: &str) -> Result<BabeId, String> {
	let raw = decode_hex_to_array::<32>(value, "BABE key")?;
	Ok(raw.unchecked_into())
}

fn parse_grandpa_id(value: &str) -> Result<GrandpaId, String> {
	let raw = decode_hex_to_array::<32>(value, "GRANDPA key")?;
	Ok(raw.unchecked_into())
}

fn parse_authority_discovery_id(value: &str) -> Result<AuthorityDiscoveryId, String> {
	let raw = decode_hex_to_array::<32>(value, "authority discovery key")?;
	Ok(raw.unchecked_into())
}

fn decode_hex_to_array<const N: usize>(value: &str, context: &str) -> Result<[u8; N], String> {
	let trimmed = value.trim();
	if trimmed.is_empty() {
		return Err(format!("{context} must not be empty"));
	}

	let without_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
	let decoded =
		decode(without_prefix).map_err(|e| format!("Invalid hex provided for {context}: {e}"))?;

	if decoded.len() != N {
		return Err(format!("{context} must be {N} bytes but received {} bytes.", decoded.len()));
	}

	let mut array = [0u8; N];
	array.copy_from_slice(&decoded);
	Ok(array)
}
