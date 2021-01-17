/*
 * This file is part of the CORD
 * Copyright (C) 2020-21  Dhiway
 *
 */

use cord_node_runtime::{
	AccountId, BalancesConfig, GenesisConfig, SessionConfig,
	SudoConfig, SystemConfig, WASM_BINARY, ValidatorSetConfig,
	AccountSetConfig,
};

use sc_service::{self, ChainType, Properties};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::UncheckedInto};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use cord_node_runtime::constants::currency::DCU;
use hex_literal::hex;

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const DEFAULT_PROTOCOL_ID: &str = "cord";

/// Specialised `ChainSpec`. This is a specialisation of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
	Development,
	CordTestNet,
	CordMainNet,
}

impl Alternative {
	/// Get an actual chain config from one of the alternatives.
	pub(crate) fn load(self) -> Result<ChainSpec, String> {
		let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
		let boot_nodes = vec![];

		let mut properties = Properties::new();
		properties.insert("tokenSymbol".into(), "DCU".into());
		properties.insert("tokenDecimals".into(), 18.into());
		Ok(match self {
			Alternative::Development => {
				ChainSpec::from_genesis(
					"Development",
					"development",
					ChainType::Development,
					move || local_dev_genesis(wasm_binary),
					boot_nodes,
					None,
					Some(DEFAULT_PROTOCOL_ID),
					Some(properties),
					None,
				)
			}
			Alternative::CordTestNet => {
				ChainSpec::from_genesis(
					"CORD Testnet",
					"tnet",
					ChainType::Development,
					move || cord_testnet_genesis(wasm_binary),
					boot_nodes,
					None,
					Some(DEFAULT_PROTOCOL_ID),
					Some(properties),
					None,
				)
			}
			Alternative::CordMainNet => {
				ChainSpec::from_genesis(
					"CORD Main Net",
					"mnet",
					ChainType::Development,
					move || cord_mainnet_genesis(wasm_binary),
					boot_nodes,
					None,
					Some(DEFAULT_PROTOCOL_ID),
					Some(properties),
					None,
				)
			}
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(Alternative::Development),
			"tnet" => Some(Alternative::CordTestNet),
			"mnet" => Some(Alternative::CordMainNet),
			_ => None,
		}
	}
}

fn local_dev_genesis(wasm_binary: &[u8]) -> GenesisConfig {
	let endowed_accounts = vec![
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

	let initial_authorities: Vec<(
		AccountId,
		AuraId,
		GrandpaId,
	)> = vec![(
			//3yzE5N1DMjibaSesw1hAZ8wwvPJnxM3RzvQFanitVm4rkC8h
			hex!["e41d2833b0b2f629e52a1bc1ace3079c395673bab26a14626b52c132b1fb5f1c"].into(),
			//3y5rP4K3E9QoyPk8Ax47vrnoJPD89AEBThUJQHqHCA4uRHFL
			hex!["bc2b5d4a95a29479caf0c5a065274b63870f200dbc68b0dc85d2dfe5005f8f32"].unchecked_into(),
			//3xaQXFoMVNgQ2qMCXHazaEiQ4bzWfVX3TowLc1DHMB1sL4nx
			hex!["a5b6331fcff809f2b3419332678fd7b23a2a9320240ec36652337fe66a7337e0"].unchecked_into(),
		)];

	const ENDOWMENT: u128 = 8_999 * DCU;
	const STASH: u128 = 9_000_999_000_999 * DCU;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts
			.iter()
			.map(|k: &AccountId| (k.clone(), STASH))
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), ENDOWMENT)))
			.collect(),
		}),
		validatorset: Some(ValidatorSetConfig {
			validators: initial_authorities
				.iter()
				.map(|x| x.0.clone())
				.collect::<Vec<_>>(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						cord_node_runtime::opaque::SessionKeys {
							aura: x.1.clone(),
							grandpa: x.2.clone(),
						},
					)
				})
				.collect::<Vec<_>>(),
		}),
		pallet_aura: Some(Default::default()),
		pallet_grandpa: Some(Default::default()),
		pallet_sudo: Some(SudoConfig {
			key: endowed_accounts[0].clone(),
		}),
		accountset: Some(AccountSetConfig {
			allowed_accounts:endowed_accounts
			.iter()
			.map(|k| (k.clone(), ()))
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), ())))
			.collect(),
		}),
	}
}

fn cord_testnet_genesis(wasm_binary: &[u8]) -> GenesisConfig {
	let endowed_accounts = vec![
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

	let initial_authorities: Vec<(
		AccountId,
		AuraId,
		GrandpaId,
	)> = vec![(
			//3yzE5N1DMjibaSesw1hAZ8wwvPJnxM3RzvQFanitVm4rkC8h
			hex!["e41d2833b0b2f629e52a1bc1ace3079c395673bab26a14626b52c132b1fb5f1c"].into(),
			//3y5rP4K3E9QoyPk8Ax47vrnoJPD89AEBThUJQHqHCA4uRHFL
			hex!["bc2b5d4a95a29479caf0c5a065274b63870f200dbc68b0dc85d2dfe5005f8f32"].unchecked_into(),
			//3xaQXFoMVNgQ2qMCXHazaEiQ4bzWfVX3TowLc1DHMB1sL4nx
			hex!["a5b6331fcff809f2b3419332678fd7b23a2a9320240ec36652337fe66a7337e0"].unchecked_into(),
			),
			(
			//3uPAkKYpvJwYFzasFfEoj6K4hwRiKGbbX4qDsuXmngRcRDE8
			hex!["186f6e121c08e7d2951f086cec0d6cf90e5b964a321175914ab5cb938cb51006"].into(),
			//3u5KV5TCUjqhw4td1gAyEQpuKp9etNueNNEvh4EmPtvhxQ5w
			hex!["0ad26cb81f15cfeb79f57fb63ce12a87ba27182301ce5adcbbc11675507c3e09"].unchecked_into(),
			//3yPbpB1VCL1mna4UFXqhcnepQuXJmoJFgfgedZXqteucf1W3
			hex!["c9b4beb11d90a463dbf7dfc9a20d00538333429e1f93874bf3937de98e49939f"].unchecked_into(),
			),
			(
			//3yDhdkwPaAp1fghGhPW5KwL6xKDCmvM7LGtvtiYvLHMrtBXp
			hex!["c227e25885b199a75429484278681c276062e6b0639c75aba6d7eba622ae773d"].into(),
			//3vpuUKGMrtP1qpabD63iNLBQm2DCo1LAdPoT9VMZsT6UXYg5
			hex!["584c9ed9c6628d311dbe64069227d3a4c25ff7bee43b18d5dc8b1bf8f69e8878"].unchecked_into(),
			//3zJUM1FL1xjSVZhcJhhYEeiHLwrJucC5XAWZpyJQr9XyDmgR
			hex!["f2079c41fe0f05f17138e205da91e90958212daf50605d99699baf081daae49d"].unchecked_into(),
		)];

	const ENDOWMENT: u128 = 8_999 * DCU;
	const STASH: u128 = 9_000_999_000_999 * DCU;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts
			.iter()
			.map(|k: &AccountId| (k.clone(), STASH))
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), ENDOWMENT)))
			.collect(),
		}),
		validatorset: Some(ValidatorSetConfig {
			validators: initial_authorities
				.iter()
				.map(|x| x.0.clone())
				.collect::<Vec<_>>(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						cord_node_runtime::opaque::SessionKeys {
							aura: x.1.clone(),
							grandpa: x.2.clone(),
						},
					)
				})
				.collect::<Vec<_>>(),
		}),
		pallet_aura: Some(Default::default()),
		pallet_grandpa: Some(Default::default()),
		pallet_sudo: Some(SudoConfig {
			key: endowed_accounts[0].clone(),
		}),
		accountset: Some(AccountSetConfig {
			allowed_accounts:endowed_accounts
			.iter()
			.map(|k| (k.clone(), ()))
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), ())))
			.collect(),
		}),
	}
}

fn cord_mainnet_genesis(wasm_binary: &[u8]) -> GenesisConfig {
	let endowed_accounts = vec![
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

	let initial_authorities: Vec<(
		AccountId,
		AuraId,
		GrandpaId,
	)> = vec![(
			//3yzE5N1DMjibaSesw1hAZ8wwvPJnxM3RzvQFanitVm4rkC8h
			hex!["e41d2833b0b2f629e52a1bc1ace3079c395673bab26a14626b52c132b1fb5f1c"].into(),
			//3y5rP4K3E9QoyPk8Ax47vrnoJPD89AEBThUJQHqHCA4uRHFL
			hex!["bc2b5d4a95a29479caf0c5a065274b63870f200dbc68b0dc85d2dfe5005f8f32"].unchecked_into(),
			//3xaQXFoMVNgQ2qMCXHazaEiQ4bzWfVX3TowLc1DHMB1sL4nx
			hex!["a5b6331fcff809f2b3419332678fd7b23a2a9320240ec36652337fe66a7337e0"].unchecked_into(),
			),
			(
			//3uPAkKYpvJwYFzasFfEoj6K4hwRiKGbbX4qDsuXmngRcRDE8
			hex!["186f6e121c08e7d2951f086cec0d6cf90e5b964a321175914ab5cb938cb51006"].into(),
			//3u5KV5TCUjqhw4td1gAyEQpuKp9etNueNNEvh4EmPtvhxQ5w
			hex!["0ad26cb81f15cfeb79f57fb63ce12a87ba27182301ce5adcbbc11675507c3e09"].unchecked_into(),
			//3yPbpB1VCL1mna4UFXqhcnepQuXJmoJFgfgedZXqteucf1W3
			hex!["c9b4beb11d90a463dbf7dfc9a20d00538333429e1f93874bf3937de98e49939f"].unchecked_into(),
			),
			(
			//3yDhdkwPaAp1fghGhPW5KwL6xKDCmvM7LGtvtiYvLHMrtBXp
			hex!["c227e25885b199a75429484278681c276062e6b0639c75aba6d7eba622ae773d"].into(),
			//3vpuUKGMrtP1qpabD63iNLBQm2DCo1LAdPoT9VMZsT6UXYg5
			hex!["584c9ed9c6628d311dbe64069227d3a4c25ff7bee43b18d5dc8b1bf8f69e8878"].unchecked_into(),
			//3zJUM1FL1xjSVZhcJhhYEeiHLwrJucC5XAWZpyJQr9XyDmgR
			hex!["f2079c41fe0f05f17138e205da91e90958212daf50605d99699baf081daae49d"].unchecked_into(),
		)];

	const ENDOWMENT: u128 = 8_999 * DCU;
	const STASH: u128 = 9_000_999_000_999 * DCU;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts
			.iter()
			.map(|k: &AccountId| (k.clone(), STASH))
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), ENDOWMENT)))
			.collect(),
		}),
		validatorset: Some(ValidatorSetConfig {
			validators: initial_authorities
				.iter()
				.map(|x| x.0.clone())
				.collect::<Vec<_>>(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						cord_node_runtime::opaque::SessionKeys {
							aura: x.1.clone(),
							grandpa: x.2.clone(),
						},
					)
				})
				.collect::<Vec<_>>(),
		}),
		pallet_aura: Some(Default::default()),
		pallet_grandpa: Some(Default::default()),
		pallet_sudo: Some(SudoConfig {
			key: endowed_accounts[0].clone(),
		}),
		accountset: Some(AccountSetConfig {
			allowed_accounts:endowed_accounts
			.iter()
			.map(|k| (k.clone(), ()))
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), ())))
			.collect(),
		}),
	}
}

pub fn load_spec(id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
	Ok(match Alternative::from(id) {
		Some(spec) => Box::new(spec.load()?),
		None => Box::new(ChainSpec::from_json_file(std::path::PathBuf::from(id))?),
	})
}
