// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! CORD chain configurations.

pub use cord_primitives::{AccountId, Balance, Signature};
pub use cord_runtime::GenesisConfig;
use cord_runtime::{
	constants::currency::*, AuraConfig, AuthorityConfig, BalancesConfig, Block, CouncilConfig,
	DemocracyConfig, DhiCouncilConfig, GrandpaConfig, IndicesConfig, PhragmenElectionConfig,
	SessionConfig, SessionKeys, SudoConfig, SystemConfig, TechnicalCommitteeConfig,
};
use hex_literal::hex;
use sc_chain_spec::ChainSpecExtension;
use sc_service::{ChainType, Properties};
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::{IdentifyAccount, Verify};

type AccountPublic = <Signature as Verify>::Signer;

pub use cord_runtime::constants::time::*;

// Note this is the URL for the telemetry server
const STAGING_TELEMETRY_URL: &str = "wss://telemetry.dway.io/submit/";
const DEFAULT_PROTOCOL_ID: &str = "cord";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<Block>,
	/// The light sync state.
	/// This value will be set by the `sync-state rpc` implementation.
	pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

/// Specialized `ChainSpec`.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

fn session_keys(aura: AuraId, grandpa: GrandpaId) -> SessionKeys {
	SessionKeys { aura, grandpa }
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to set properties
pub fn get_properties(symbol: &str, decimals: u32, ss58format: u32) -> Properties {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), symbol.into());
	properties.insert("tokenDecimals".into(), decimals.into());
	properties.insert("ss58Format".into(), ss58format.into());

	properties
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, AuraId, GrandpaId) {
	let keys = get_authority_keys(seed);
	(keys.0, keys.1, keys.2, keys.3)
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys(seed: &str) -> (AccountId, AccountId, AuraId, GrandpaId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<AuraId>(seed),
		get_from_seed::<GrandpaId>(seed),
	)
}

fn testnet_accounts() -> Vec<AccountId> {
	vec![
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		get_account_id_from_seed::<sr25519::Public>("Bob"),
		get_account_id_from_seed::<sr25519::Public>("Charlie"),
		get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
		get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
		get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
	]
}

/// Development config.
fn cord_development_config_genesis(wasm_binary: &[u8]) -> cord_runtime::GenesisConfig {
	cord_development_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

fn cord_local_testnet_genesis(wasm_binary: &[u8]) -> cord_runtime::GenesisConfig {
	cord_development_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed("Alice"), get_authority_keys_from_seed("Bob")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

pub fn cord_development_config() -> Result<ChainSpec, String> {
	let wasm_binary = cord_runtime::WASM_BINARY.ok_or("CORD development wasm not available")?;
	let properties = get_properties("WAYT", 12, 29);
	Ok(ChainSpec::from_genesis(
		"Development",
		"cord_dev",
		ChainType::Development,
		move || cord_development_config_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		Some(properties),
		Default::default(),
	))
}

pub fn cord_local_testnet_config() -> Result<ChainSpec, String> {
	let wasm_binary = cord_runtime::WASM_BINARY.ok_or("CORD development wasm not available")?;
	let properties = get_properties("WAYT", 12, 29);
	Ok(ChainSpec::from_genesis(
		"Local Testnet",
		"cord_local",
		ChainType::Local,
		move || cord_local_testnet_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		Some(properties),
		Default::default(),
	))
}

fn cord_staging_config_genesis(wasm_binary: &[u8]) -> cord_runtime::GenesisConfig {
	let initial_authorities: Vec<(AccountId, AccountId, AuraId, GrandpaId)> = vec![
		(
			//3wF3nbuyb97oSkVBSgZe9dpYcFw5dypX8SPhBWrUcCpZxBWW
			hex!["6ab68082628ad0cfab68b1a00377170ff0dea4da06030cdd0c21a364ecbbc23b"].into(),
			//3yzE5N1DMjibaSesw1hAZ8wwvPJnxM3RzvQFanitVm4rkC8h
			hex!["e41d2833b0b2f629e52a1bc1ace3079c395673bab26a14626b52c132b1fb5f1c"].into(),
			//3xaQXFoMVNgQ2qMCXHazaEiQ4bzWfVX3TowLc1DHMB1sL4nx
			hex!["a5b6331fcff809f2b3419332678fd7b23a2a9320240ec36652337fe66a7337e0"]
				.unchecked_into(),
			//3y5rP4K3E9QoyPk8Ax47vrnoJPD89AEBThUJQHqHCA4uRHFL
			hex!["bc2b5d4a95a29479caf0c5a065274b63870f200dbc68b0dc85d2dfe5005f8f32"]
				.unchecked_into(),
		),
		(
			//3wLfSLg4AbbfZggDsZ2BScSjkF8XEC7gCtoHTDrUr28hSbMG
			hex!["6efebd6198dc606b9074d7b3cd205261f36e143701a393ee880d29ebab55e92d"].into(),
			//3uPAkKYpvJwYFzasFfEoj6K4hwRiKGbbX4qDsuXmngRcRDE8
			hex!["186f6e121c08e7d2951f086cec0d6cf90e5b964a321175914ab5cb938cb51006"].into(),
			//3yPbpB1VCL1mna4UFXqhcnepQuXJmoJFgfgedZXqteucf1W3
			hex!["c9b4beb11d90a463dbf7dfc9a20d00538333429e1f93874bf3937de98e49939f"]
				.unchecked_into(),
			//3u5KV5TCUjqhw4td1gAyEQpuKp9etNueNNEvh4EmPtvhxQ5w
			hex!["0ad26cb81f15cfeb79f57fb63ce12a87ba27182301ce5adcbbc11675507c3e09"]
				.unchecked_into(),
		),
		(
			//3tssweCjh9wU7A33RJ1WhTsmXkdUJwyhrE3h7AwHum7YXy5M
			hex!["0218be44e37405b283cd8e2ddf9fb73ec9bde2efc1b6567f2df55fc311bd4502"].into(),
			//3yDhdkwPaAp1fghGhPW5KwL6xKDCmvM7LGtvtiYvLHMrtBXp
			hex!["c227e25885b199a75429484278681c276062e6b0639c75aba6d7eba622ae773d"].into(),
			//3zJUM1FL1xjSVZhcJhhYEeiHLwrJucC5XAWZpyJQr9XyDmgR
			hex!["f2079c41fe0f05f17138e205da91e90958212daf50605d99699baf081daae49d"]
				.unchecked_into(),
			//3vpuUKGMrtP1qpabD63iNLBQm2DCo1LAdPoT9VMZsT6UXYg5
			hex!["584c9ed9c6628d311dbe64069227d3a4c25ff7bee43b18d5dc8b1bf8f69e8878"]
				.unchecked_into(),
		),
	];

	let endowed_accounts: Vec<AccountId> = vec![
		//3yzE5N1DMjibaSesw1hAZ8wwvPJnxM3RzvQFanitVm4rkC8h
		hex!["e41d2833b0b2f629e52a1bc1ace3079c395673bab26a14626b52c132b1fb5f1c"].into(),
		//3uPAkKYpvJwYFzasFfEoj6K4hwRiKGbbX4qDsuXmngRcRDE8
		hex!["186f6e121c08e7d2951f086cec0d6cf90e5b964a321175914ab5cb938cb51006"].into(),
		//3yDhdkwPaAp1fghGhPW5KwL6xKDCmvM7LGtvtiYvLHMrtBXp
		hex!["c227e25885b199a75429484278681c276062e6b0639c75aba6d7eba622ae773d"].into(),
	];
	let root_key: AccountId = endowed_accounts[0].clone();
	let num_endowed_accounts = endowed_accounts.len();
	const STASH: u128 = 100 * WAY;
	const ENDOWMENT: u128 = 1_110_101_200 * WAY;

	GenesisConfig {
		system: SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		indices: IndicesConfig { indices: vec![] },
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, ENDOWMENT))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
				.collect(),
		},
		authority: AuthorityConfig {
			authorities: initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), session_keys(x.2.clone(), x.3.clone())))
				.collect::<Vec<_>>(),
		},
		phragmen_election: PhragmenElectionConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.map(|member| (member, STASH))
				.collect(),
		},
		democracy: DemocracyConfig::default(),
		council: CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: TechnicalCommitteeConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		dhi_council: DhiCouncilConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			phantom: Default::default(),
		},
		aura: AuraConfig { authorities: vec![] },
		grandpa: GrandpaConfig { authorities: vec![] },
		sudo: SudoConfig { key: root_key },
		treasury: Default::default(),
		base: Default::default(),
		nix: Default::default(),
	}
}

/// Staging testnet config.
pub fn cord_staging_config() -> Result<ChainSpec, String> {
	let wasm_binary = cord_runtime::WASM_BINARY.ok_or("CORD development wasm not available")?;
	let boot_nodes = vec![];
	let properties = get_properties("WAY", 12, 29);

	Ok(ChainSpec::from_genesis(
		"Cord Staging Testnet",
		"cord_staging_testnet",
		ChainType::Live,
		move || cord_staging_config_genesis(wasm_binary),
		boot_nodes,
		Some(
			TelemetryEndpoints::new(vec![(STAGING_TELEMETRY_URL.to_string(), 0)])
				.expect("Staging telemetry url is valid; qed"),
		),
		Some(DEFAULT_PROTOCOL_ID),
		Some(properties),
		Default::default(),
	))
}

fn cord_development_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, AuraId, GrandpaId)>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> GenesisConfig {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);
	let num_endowed_accounts = endowed_accounts.len();
	const ENDOWMENT: u128 = 10_000 * WAY;
	const STASH: u128 = 100 * WAY;
	GenesisConfig {
		system: SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		indices: IndicesConfig { indices: vec![] },
		balances: BalancesConfig {
			balances: endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect(),
		},
		authority: AuthorityConfig {
			authorities: initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), session_keys(x.2.clone(), x.3.clone())))
				.collect::<Vec<_>>(),
		},
		phragmen_election: PhragmenElectionConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.map(|member| (member, STASH))
				.collect(),
		},
		democracy: DemocracyConfig::default(),
		council: CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: TechnicalCommitteeConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		dhi_council: DhiCouncilConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			phantom: Default::default(),
		},
		aura: AuraConfig { authorities: vec![] },
		grandpa: GrandpaConfig { authorities: vec![] },
		sudo: SudoConfig { key: root_key },
		treasury: Default::default(),
		base: Default::default(),
		nix: Default::default(),
	}
}
