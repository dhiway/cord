// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2022 Dhiway
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

pub use cord_primitives::{AccountId, Balance, Signature, DEFAULT_SESSION_PERIOD};
pub use cord_runtime::GenesisConfig;
use cord_runtime::{
	constants::currency::*, AuraConfig, AuthoritiesConfig, AuthorityDiscoveryConfig,
	BalancesConfig, Block, CouncilConfig, DemocracyConfig, ElectionsConfig, GrandpaConfig,
	IndicesConfig, SessionConfig, SessionKeys, SudoConfig, SystemConfig, TechnicalMembershipConfig,
};
use hex_literal::hex;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::ChainSpecExtension;
use sc_service::{ChainType, Properties};
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
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

fn session_keys(
	aura: AuraId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys { aura, grandpa, im_online, authority_discovery }
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
pub fn get_authority_keys_from_seed(
	seed: &str,
) -> (AccountId, AccountId, AuraId, GrandpaId, ImOnlineId, AuthorityDiscoveryId) {
	let keys = get_authority_keys(seed);
	(keys.0, keys.1, keys.2, keys.3, keys.4, keys.5)
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys(
	seed: &str,
) -> (AccountId, AccountId, AuraId, GrandpaId, ImOnlineId, AuthorityDiscoveryId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<AuraId>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<ImOnlineId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
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
		"Dev. Node",
		"cord_dev",
		ChainType::Development,
		move || cord_development_config_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		None,
		Some(properties),
		Default::default(),
	))
}

pub fn cord_local_testnet_config() -> Result<ChainSpec, String> {
	let wasm_binary = cord_runtime::WASM_BINARY.ok_or("CORD development wasm not available")?;
	let properties = get_properties("WAYT", 12, 29);
	Ok(ChainSpec::from_genesis(
		"Local",
		"cord_local",
		ChainType::Local,
		move || cord_local_testnet_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		None,
		Some(properties),
		Default::default(),
	))
}

fn cord_staging_config_genesis(wasm_binary: &[u8]) -> cord_runtime::GenesisConfig {
	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		AuraId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)> = vec![
		(
			//3wF3nbuyb97oSkVBSgZe9dpYcFw5dypX8SPhBWrUcCpZxBWW
			hex!["6ab68082628ad0cfab68b1a00377170ff0dea4da06030cdd0c21a364ecbbc23b"].into(),
			//3yzE5N1DMjibaSesw1hAZ8wwvPJnxM3RzvQFanitVm4rkC8h
			hex!["e41d2833b0b2f629e52a1bc1ace3079c395673bab26a14626b52c132b1fb5f1c"].into(),
			//3xuztVAW9ftgcU5FNc3dEXsEgrZW1AnbGWqWmeKKxpGnM4H2
			hex!["b4a78c7de7cc60ed9a99029fcf487f40a3c4b5d5d78a7080387507a680ecb75e"]
				.unchecked_into(),
			//3xaQXFoMVNgQ2qMCXHazaEiQ4bzWfVX3TowLc1DHMB1sL4nx
			hex!["a5b6331fcff809f2b3419332678fd7b23a2a9320240ec36652337fe66a7337e0"]
				.unchecked_into(),
			//3xE2yQSUQ9hfeX1kZjP1Dg5hoU2EdLc1B9zFjzEcc5fgax2W
			hex!["962cc02d5dddbb2fc03bd8d511844ec47e798b3bc20d9daf7400b3d09533d518"]
				.unchecked_into(),
			//3vL3vWTS2FZ9JDc4SyMFXQRa5TuitFBfSx8ZrygeEMzc7HkV
			hex!["424af4547d488e65307cb14ffae20257b6e000658913f985824da5629afff13c"]
				.unchecked_into(),
		),
		(
			//3wLfSLg4AbbfZggDsZ2BScSjkF8XEC7gCtoHTDrUr28hSbMG
			hex!["6efebd6198dc606b9074d7b3cd205261f36e143701a393ee880d29ebab55e92d"].into(),
			//3uPAkKYpvJwYFzasFfEoj6K4hwRiKGbbX4qDsuXmngRcRDE8
			hex!["186f6e121c08e7d2951f086cec0d6cf90e5b964a321175914ab5cb938cb51006"].into(),
			//3yBxXXsizEhxj5sMbxZ6iJtVAo5iJp4faNKzvEyua2waD9bB
			hex!["c0d386cbb0f71fd8c22fe5724b02bb747a92d5241cfcb7ee81f2611491a4ec2f"]
				.unchecked_into(),
			//3yPbpB1VCL1mna4UFXqhcnepQuXJmoJFgfgedZXqteucf1W3
			hex!["c9b4beb11d90a463dbf7dfc9a20d00538333429e1f93874bf3937de98e49939f"]
				.unchecked_into(),
			//3uWjtNikmuwLVKkLD1opoR2U92YAoExgaxDoKfA5S9N8S7GY
			hex!["1e35b40417a5631c4762974cfd37128985aa626366d659eb37b7d19eca5ce676"]
				.unchecked_into(),
			//3ur2S4iPwFJfehHCRBRQoTR171GrohDHK7ent21xF5YjRSxE
			hex!["2ceb10e043fd67269c33758d0f65d245a2edcd293049b2cb78a807106643ed4c"]
				.unchecked_into(),
		),
		(
			//3tssweCjh9wU7A33RJ1WhTsmXkdUJwyhrE3h7AwHum7YXy5M
			hex!["0218be44e37405b283cd8e2ddf9fb73ec9bde2efc1b6567f2df55fc311bd4502"].into(),
			//3yDhdkwPaAp1fghGhPW5KwL6xKDCmvM7LGtvtiYvLHMrtBXp
			hex!["c227e25885b199a75429484278681c276062e6b0639c75aba6d7eba622ae773d"].into(),
			//3yRFafgrJNPfx5FNEBaBiMkdDpQksQCQ6GiA5MwNQuxJxqjV
			hex!["caf72037137297537c8e00dfe6259a640801d62c71a55d825d9994a26d743b7d"]
				.unchecked_into(),
			//3zJUM1FL1xjSVZhcJhhYEeiHLwrJucC5XAWZpyJQr9XyDmgR
			hex!["f2079c41fe0f05f17138e205da91e90958212daf50605d99699baf081daae49d"]
				.unchecked_into(),
			//3x8xZQoUYS9LdQp6NX4SuvWEPq3zsUqibM51Gc6W4y4Z9mjX
			hex!["924daa7728eab557869188f55b30fd8d4810cbd60ad3280c6562e0a8cad3943a"]
				.unchecked_into(),
			//3v9USUnkQpKLYGsDAbzncF6PsHQdCHJqAgt2gKYfmZvdGKEi
			hex!["3a39c922f4c6f6efe8893260b7d326964b12686c28b84a3b83b973c279215243"]
				.unchecked_into(),
		),
	];

	let endowed_accounts: Vec<AccountId> = vec![
		//3x6FHDirZzxP1BPic2hqkA6LfLC5LHXD2ZS8B618R7rTWNBD
		hex!["903c379067968d241b2293784ff353d533837f77bcb72154e278ed06e1026a4b"].into(),
		//3zBmeQHiZ65FzXmHx8ZvvW8FSfvRU4xgsuqw4rhFeiMrXGJa
		hex!["eceb211f4c13366434d1b8d96f91099e4810e5ce7f195d2de489baf207ce4576"].into(),
		//3tygFJbrVhB9Fpe2g6bEqKDjWd5gRzioRxqtikruN6P37Sb6
		hex!["0684d85c98b64e8af9cb23db1e5e5ed9acc2b65c4dbefc6c3feaba8176da3f13"].into(),
		//3ttmwJLAfo3dCaoAHB11Cvv8vNzZhiBqTjtMZ4jsZrvceedD
		hex!["02c7c55d71abbaffb9590bcaf48ad687b783c035f9ad1e94208b776ff4a6e13f"].into(),
		//3xmViQrSRdQJoNE5GzBmEZAPBFkSsbxnjH4FVAgSbB7CoKC4
		hex!["ae2b60ce50c8a6a0f9f1eba33eec5106facfb366e946a59591633bd30c090d7d"].into(),
	];
	let session_period = DEFAULT_SESSION_PERIOD;
	let root_key: AccountId = endowed_accounts[0].clone();
	let num_endowed_accounts = endowed_accounts.len();
	const STASH: u128 = 100 * WAY;
	const ENDOWMENT: u128 = 1_110_101_200 * WAY;

	GenesisConfig {
		system: SystemConfig { code: wasm_binary.to_vec() },
		indices: IndicesConfig { indices: vec![] },
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, ENDOWMENT))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
				.collect(),
		},
		authorities: AuthoritiesConfig {
			validators: initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
			session_period,
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
					)
				})
				.collect::<Vec<_>>(),
		},
		elections: ElectionsConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.map(|member| (member, STASH))
				.collect(),
		},
		democracy: DemocracyConfig::default(),
		council: CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: Default::default(),
		technical_membership: TechnicalMembershipConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			phantom: Default::default(),
		},
		aura: AuraConfig { authorities: vec![] },
		grandpa: GrandpaConfig { authorities: vec![] },
		im_online: Default::default(),
		authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
		sudo: SudoConfig { key: Some(root_key) },
		treasury: Default::default(),
		transaction_payment: Default::default(),
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
		None,
		Some(properties),
		Default::default(),
	))
}

fn cord_development_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		AuraId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> GenesisConfig {
	let session_period = DEFAULT_SESSION_PERIOD;
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);
	let num_endowed_accounts = endowed_accounts.len();
	const ENDOWMENT: u128 = 10_000 * WAY;
	const STASH: u128 = 100 * WAY;
	GenesisConfig {
		system: SystemConfig { code: wasm_binary.to_vec() },
		indices: IndicesConfig { indices: vec![] },
		balances: BalancesConfig {
			balances: endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect(),
		},
		authorities: AuthoritiesConfig {
			validators: initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
			session_period,
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
					)
				})
				.collect::<Vec<_>>(),
		},
		elections: ElectionsConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.map(|member| (member, STASH))
				.collect(),
		},
		democracy: DemocracyConfig::default(),
		council: CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: Default::default(),
		technical_membership: TechnicalMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		aura: AuraConfig { authorities: vec![] },
		grandpa: GrandpaConfig { authorities: vec![] },
		im_online: Default::default(),
		authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
		sudo: SudoConfig { key: Some(root_key) },
		treasury: Default::default(),
		transaction_payment: Default::default(),
	}
}
