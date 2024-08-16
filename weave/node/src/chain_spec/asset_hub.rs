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

use crate::chain_spec::{
	get_account_id_from_seed, get_collator_keys_from_seed, AssetHubLoomChainSpec, Extensions,
	GenericChainSpec, SAFE_XCM_VERSION,
};
use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use parachains_common::{AccountId, AuraId, Balance as AssetHubBalance};
use sc_chain_spec::ChainSpec;
use sc_service::ChainType;
use sp_core::{crypto::UncheckedInto, sr25519};

const ASSET_HUB_LOOM_ED: AssetHubBalance = cord_weave_asset_hub_runtime::ExistentialDeposit::get();

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn cord_weave_asset_hub_session_keys(
	keys: AuraId,
) -> cord_weave_asset_hub_runtime::SessionKeys {
	cord_weave_asset_hub_runtime::SessionKeys { aura: keys }
}

// pub fn asset_hub_loom_local_testnet_config() -> Result<Box<dyn ChainSpec>, String> {
// 	let mut properties = sc_chain_spec::Properties::new();
// 	properties.insert("ss58Format".into(), 42.into());
// 	properties.insert("tokenSymbol".into(), "UNT".into());
// 	properties.insert("tokenDecimals".into(), 12.into());

// 	Ok(Box::new(
// 		GenericChainSpec::builder(
// 			cord_weave_asset_hub_runtime::WASM_BINARY.expect("AssetHubLoom wasm not available!"),
// 			Extensions { relay_chain: "loom-local".into(), para_id: 1000 },
// 		)
// 		.with_name("Loom Asset Hub Local")
// 		.with_id("loom-asset-hub-local")
// 		.with_chain_type(ChainType::Local)
// 		.with_genesis_config_patch(cord_weave_asset_hub_runtime::genesis_config_presets::asset_hub_loom_local_testnet_genesis(1000.into()))
// 		.with_properties(properties)
// 		.build(),
// 	))
// }

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

// pub fn asset_hub_loom_development_config() -> Result<Box<dyn ChainSpec>, String> {
// 	let mut properties = sc_chain_spec::Properties::new();
// 	properties.insert("ss58Format".into(), 42.into());
// 	properties.insert("tokenSymbol".into(), "UNT".into());
// 	properties.insert("tokenDecimals".into(), 12.into());

// 	Ok(Box::new(
// 		AssetHubLoomChainSpec::builder(
// 			cord_weave_asset_hub_runtime::WASM_BINARY.expect("AssetHubLoom wasm not available!"),
// 			Extensions { relay_chain: "loom-local".into(), para_id: 1000 },
// 		)
// 		.with_name("Loom Asset Hub Development")
// 		.with_id("loom-asset-hub-dev")
// 		.with_chain_type(ChainType::Local)
// 		.with_genesis_config_patch(cord_weave_asset_hub_runtime::genesis_config_presets::asset_hub_loom_development_genesis(1000.into()))
// 		.with_properties(properties)
// 		.build(),
// 	))
// }

// pub fn asset_hub_loom_development_config() -> GenericChainSpec {
// 	let mut properties = sc_chain_spec::Properties::new();
// 	properties.insert("ss58Format".into(), 42.into());
// 	properties.insert("tokenSymbol".into(), "UNT".into());
// 	properties.insert("tokenDecimals".into(), 12.into());

// 	GenericChainSpec::builder(
// 		cord_weave_asset_hub_runtime::WASM_BINARY
// 			.expect("WASM binary was not built, please build it!"),
// 		Extensions { relay_chain: "loom-local".into(), para_id: 1000 },
// 	)
// 	.with_name("Weave Asset Hub Development")
// 	.with_id("weave-asset-hub-dev")
// 	.with_chain_type(ChainType::Local)
// 	.with_genesis_config_patch(
// 		cord_weave_asset_hub_runtime::genesis_config_presets::asset_hub_loom_development_genesis(
// 			1000.into(),
// 		),
// 	)
// 	.with_properties(properties)
// 	.build()
// }

// // (
// // 	// initial collators.
// // 	vec![(
// // 		get_account_id_from_seed::<sr25519::Public>("Alice"),
// // 		get_collator_keys_from_seed::<AuraId>("Alice"),
// // 	)],
// // 	vec![
// // 		get_account_id_from_seed::<sr25519::Public>("Alice"),
// // 		get_account_id_from_seed::<sr25519::Public>("Bob"),
// // 		get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
// // 		get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
// // 	],
// // 	cord_weave_system_parachains_constants::loom::currency::UNITS * 1_000_000,
// // 	get_account_id_from_seed::<sr25519::Public>("Alice"),
// // 	1000.into(),
// // ),
// // )

// pub fn cord_weave_asset_hub_local_config() -> GenericChainSpec {
// 	let mut properties = sc_chain_spec::Properties::new();
// 	properties.insert("ss58Format".into(), 42.into());
// 	properties.insert("tokenSymbol".into(), "UNT".into());
// 	properties.insert("tokenDecimals".into(), 12.into());

// 	GenericChainSpec::builder(
// 		cord_weave_asset_hub_runtime::WASM_BINARY
// 			.expect("WASM binary was not built, please build it!"),
// 		Extensions { relay_chain: "loom-local".into(), para_id: 1000 },
// 	)
// 	.with_name("Weave Asset Hub Local")
// 	.with_id("weave-asset-hub-local")
// 	.with_chain_type(ChainType::Local)
// 	.with_genesis_config_patch(cord_weave_asset_hub_genesis(
// 		// initial collators.
// 		vec![
// 			(
// 				get_account_id_from_seed::<sr25519::Public>("Alice"),
// 				get_collator_keys_from_seed::<AuraId>("Alice"),
// 			),
// 			(
// 				get_account_id_from_seed::<sr25519::Public>("Bob"),
// 				get_collator_keys_from_seed::<AuraId>("Bob"),
// 			),
// 		],
// 		vec![
// 			get_account_id_from_seed::<sr25519::Public>("Alice"),
// 			get_account_id_from_seed::<sr25519::Public>("Bob"),
// 			get_account_id_from_seed::<sr25519::Public>("Charlie"),
// 			get_account_id_from_seed::<sr25519::Public>("Dave"),
// 			get_account_id_from_seed::<sr25519::Public>("Eve"),
// 			get_account_id_from_seed::<sr25519::Public>("Ferdie"),
// 			get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
// 			get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
// 			get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
// 			get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
// 			get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
// 			get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
// 		],
// 		cord_weave_system_parachains_constants::loom::currency::UNITS * 1_000_000,
// 		get_account_id_from_seed::<sr25519::Public>("Alice"),
// 		1000.into(),
// 	))
// 	.with_properties(properties)
// 	.build()
// }

// fn cord_weave_asset_hub_genesis(
// 	invulnerables: Vec<(AccountId, AuraId)>,
// 	endowed_accounts: Vec<AccountId>,
// 	endowment: AssetHubBalance,
// 	root: AccountId,
// 	id: ParaId,
// ) -> serde_json::Value {
// 	serde_json::json!({
// 		"balances": {
// 			"balances": endowed_accounts
// 				.iter()
// 				.cloned()
// 				.map(|k| (k, endowment))
// 				.collect::<Vec<_>>(),
// 		},
// 		"parachainInfo": {
// 			"parachainId": id,
// 		},
// 		"collatorSelection": {
// 			"invulnerables": invulnerables.iter().cloned().map(|(acc, _)| acc).collect::<Vec<_>>(),
// 			"candidacyBond": ASSET_HUB_LOOM_ED * 16,
// 		},
// 		"session": {
// 			"keys": invulnerables
// 				.into_iter()
// 				.map(|(acc, aura)| {
// 					(
// 						acc.clone(),                          // account id
// 						acc,                                  // validator id
// 						cord_weave_asset_hub_session_keys(aura), // session keys
// 					)
// 				})
// 				.collect::<Vec<_>>(),
// 		},
// 		"polkadotXcm": {
// 			"safeXcmVersion": Some(SAFE_XCM_VERSION),
// 		},
// 		"sudo": { "key": Some(root) }
// 	})
// }
