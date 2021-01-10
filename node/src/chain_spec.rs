/*
 * This file is part of the CORD
 * Copyright (C) 2020  Dhiway
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
			//5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P
			hex!["4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49"].into(),
			//5F6ybobhyyQGdXKy76L1oboVeywzH79jSqDbeRD4kX5ffVAH
			hex!["86703071df386fa21ce085228476b0656bda12c0f05f4749be65830d8eae4158"].into(),
			//5DZwZQUShKreSzfA78EvCBYzpc5Hi5VGTNg47Toq87fLT5pC
			hex!["428879f73dab6604bc954c486a309bd5dc65a20c9c2ba911b34691fa2f60e733"].into(),
			//5CcGEF5APCYQo4JwRd9yUWWFMDWToHHDgSKfYbFvAdJhvRun
			hex!["181194d1b25a985a5b25c83ed3c22b3e5157ef6a34ac8789c809dd24cb7ab84b"].into(),
	];

	let initial_authorities: Vec<(
		AccountId,
		AuraId,
		GrandpaId,
	)> = vec![(
			//5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P
			hex!["4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49"].into(),
			//5FgrrDx5H3jx38Xe3mD883FPFdRmDyEEoNLA3nMbPd5uzt26
			hex!["a047ce4974347c3972ae4a2292ba1fad2b4ece9d6eaffd5286c185985dc92401"].unchecked_into(),
			//5C9dfj4zZdFVg8ndFEXzKwiqd9FtYiracERZStwNEXefWZfv
			hex!["03c22070787b00627c5817c34451e10a181f5c257dc44d49d0c6a9e46581ba27"].unchecked_into(),
		)];

	// const ENDOWMENT: u128 = 1_000_000_000_000 * DCU;
	// const STASH: u128 = 1_000 * DCU;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts
			.iter()
			.cloned()
			.map(|k| (k, 1u128 << 90))
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
			.map(|x| (x.clone(), (),))
			.collect(),
		}),
	}
}

fn cord_testnet_genesis(wasm_binary: &[u8]) -> GenesisConfig {
	let endowed_accounts = vec![
			//5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P
			hex!["4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49"].into(),
			//5F6ybobhyyQGdXKy76L1oboVeywzH79jSqDbeRD4kX5ffVAH
			hex!["86703071df386fa21ce085228476b0656bda12c0f05f4749be65830d8eae4158"].into(),
			//5DZwZQUShKreSzfA78EvCBYzpc5Hi5VGTNg47Toq87fLT5pC
			hex!["428879f73dab6604bc954c486a309bd5dc65a20c9c2ba911b34691fa2f60e733"].into(),
			//5CcGEF5APCYQo4JwRd9yUWWFMDWToHHDgSKfYbFvAdJhvRun
			hex!["181194d1b25a985a5b25c83ed3c22b3e5157ef6a34ac8789c809dd24cb7ab84b"].into(),
	];

	let initial_authorities: Vec<(
		AccountId,
		AuraId,
		GrandpaId,
	)> = vec![(
		//5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P
		hex!["4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49"].into(),
		//5FgrrDx5H3jx38Xe3mD883FPFdRmDyEEoNLA3nMbPd5uzt26
		hex!["a047ce4974347c3972ae4a2292ba1fad2b4ece9d6eaffd5286c185985dc92401"].unchecked_into(),
		//5C9dfj4zZdFVg8ndFEXzKwiqd9FtYiracERZStwNEXefWZfv
		hex!["03c22070787b00627c5817c34451e10a181f5c257dc44d49d0c6a9e46581ba27"].unchecked_into(),
		),
		(
		//5DhYRwjpfvxYnBVD2QbPWrK5s9aJeji33nVNC99jQgwGSWpM
		hex!["485494ec577ef5807683a52144343112af183ae54b95e349f0a4b122d3428014"].into(),
		//5Gmq8xxxypVMA5YUjZQvsKFw71GyucC3ZnGrLTQ35C6ifcvQ
		hex!["d04e815f6b73d22e5d5948e7d05bab6c8cda3cb10e37660582fa918c61f4cc7b"].unchecked_into(),
		//5EQQMTTz6a4686C86NsC55LTFKXYUA8KAx1KythaW8q67am2
		hex!["677e73d451feffdc46384fbc0688ab08cb8c66c660b8e598aa8bbbb6c38b7695"].unchecked_into(),
		),
		(
		//5DkEcWqE9CAj2Cusx1XSXhmLvGQQ8qChQnDJuTYMCukSjNWU
		hex!["4a62568acc66d594cb24744a24968c21decb3d824ac3cde58561a30ea4e33026"].into(),
		//5EWvkWim11zojxkiRX9SouCAq67jKC6HiGJGEBZrPEhqMXnc
		hex!["6c784086bd9a33ca691e73f14ac0fd28a71a653f7d0bf3d1c8f06efc3bc88059"].unchecked_into(),
		//5HczPzT6FTLYBD6rNB85TgZUpa5tupRUmkJSd15xQrtyCwba
		hex!["f5ccb2eedfa6adb37bae3f28aa388c5e1e4e87b0684f77020e89879a2299fbf3"].unchecked_into(),
		)];

	const ENDOWMENT: u128 = 1_000_000_000_000 * DCU;
	const STASH: u128 = 1_000 * DCU;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts
			.iter()
			.map(|k: &AccountId| (k.clone(), ENDOWMENT))
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
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
			.map(|x| (x.clone(), (),))
			.collect(),
		}),
	}
}

fn cord_mainnet_genesis(wasm_binary: &[u8]) -> GenesisConfig {
	let endowed_accounts = vec![
			//5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P
			hex!["4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49"].into(),
			//5F6ybobhyyQGdXKy76L1oboVeywzH79jSqDbeRD4kX5ffVAH
			hex!["86703071df386fa21ce085228476b0656bda12c0f05f4749be65830d8eae4158"].into(),
			//5DZwZQUShKreSzfA78EvCBYzpc5Hi5VGTNg47Toq87fLT5pC
			hex!["428879f73dab6604bc954c486a309bd5dc65a20c9c2ba911b34691fa2f60e733"].into(),
			//5CcGEF5APCYQo4JwRd9yUWWFMDWToHHDgSKfYbFvAdJhvRun
			hex!["181194d1b25a985a5b25c83ed3c22b3e5157ef6a34ac8789c809dd24cb7ab84b"].into(),
	];

	let initial_authorities: Vec<(
		AccountId,
		AuraId,
		GrandpaId,
	)> = vec![(
		//5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P
		hex!["4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49"].into(),
		//5FgrrDx5H3jx38Xe3mD883FPFdRmDyEEoNLA3nMbPd5uzt26
		hex!["a047ce4974347c3972ae4a2292ba1fad2b4ece9d6eaffd5286c185985dc92401"].unchecked_into(),
		//5C9dfj4zZdFVg8ndFEXzKwiqd9FtYiracERZStwNEXefWZfv
		hex!["03c22070787b00627c5817c34451e10a181f5c257dc44d49d0c6a9e46581ba27"].unchecked_into(),
		),
		(
		//5DhYRwjpfvxYnBVD2QbPWrK5s9aJeji33nVNC99jQgwGSWpM
		hex!["485494ec577ef5807683a52144343112af183ae54b95e349f0a4b122d3428014"].into(),
		//5Gmq8xxxypVMA5YUjZQvsKFw71GyucC3ZnGrLTQ35C6ifcvQ
		hex!["d04e815f6b73d22e5d5948e7d05bab6c8cda3cb10e37660582fa918c61f4cc7b"].unchecked_into(),
		//5EQQMTTz6a4686C86NsC55LTFKXYUA8KAx1KythaW8q67am2
		hex!["677e73d451feffdc46384fbc0688ab08cb8c66c660b8e598aa8bbbb6c38b7695"].unchecked_into(),
		),
		(
		//5DkEcWqE9CAj2Cusx1XSXhmLvGQQ8qChQnDJuTYMCukSjNWU
		hex!["4a62568acc66d594cb24744a24968c21decb3d824ac3cde58561a30ea4e33026"].into(),
		//5EWvkWim11zojxkiRX9SouCAq67jKC6HiGJGEBZrPEhqMXnc
		hex!["6c784086bd9a33ca691e73f14ac0fd28a71a653f7d0bf3d1c8f06efc3bc88059"].unchecked_into(),
		//5HczPzT6FTLYBD6rNB85TgZUpa5tupRUmkJSd15xQrtyCwba
		hex!["f5ccb2eedfa6adb37bae3f28aa388c5e1e4e87b0684f77020e89879a2299fbf3"].unchecked_into(),
		)];

	const ENDOWMENT: u128 = 1_000_000_000_000 * DCU;
	const STASH: u128 = 1_000 * DCU;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts
			.iter()
			.map(|k: &AccountId| (k.clone(), ENDOWMENT))
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
			.collect(),
			// balances: initial_authorities.iter().cloned().map(|k|(k, ENDOWMENT)).collect(),
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
			.map(|x| (x.clone(), (),))
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
