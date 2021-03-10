// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! CORD chain configurations.

use cord_runtime::{
	AuthorityDiscoveryConfig, BalancesConfig, CouncilConfig, DemocracyConfig, GenesisConfig,
	ImOnlineConfig, IndicesConfig, SessionConfig, SessionKeys, StakerStatus, StakingConfig,
	SudoConfig, SystemConfig, TechnicalCommitteeConfig, WASM_BINARY,
};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_service::{ChainType, Properties};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::crypto::UncheckedInto;
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::Perbill;

pub use cord_primitives::{AccountId, Balance, BlockNumber, Signature};
use cord_runtime::constants::currency::*;
pub use cord_runtime::constants::time::*;
use hex_literal::hex;

// Note this is the URL for the telemetry server
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const DEFAULT_PROTOCOL_ID: &str = "cord";

/// Specialised `ChainSpec`. This is a specialisation of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
	BombayBrown,
	IndianTeal,
	RoyalBlue,
}

fn session_keys(
	aura: AuraId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys {
		grandpa,
		aura,
		im_online,
		authority_discovery,
	}
}

impl Alternative {
	/// Get an actual chain config from one of the alternatives.
	/// Chain name using color codes representings strands
	pub(crate) fn load(self) -> Result<ChainSpec, String> {
		let wasm_binary =
			WASM_BINARY.ok_or_else(|| "Development wasm binary not available".to_string())?;
		let boot_nodes = vec![];

		let mut properties = Properties::new();
		properties.insert("tokenSymbol".into(), "CRD".into());
		properties.insert("tokenDecimals".into(), 18.into());
		Ok(match self {
			Alternative::BombayBrown => ChainSpec::from_genesis(
				"Bombay Brown",
				"LocalDevNode",
				ChainType::Local,
				move || bombay_brown_genesis(wasm_binary),
				boot_nodes,
				None,
				Some(DEFAULT_PROTOCOL_ID),
				Some(properties),
				None,
			),
			Alternative::IndianTeal => ChainSpec::from_genesis(
				"Indian Teal",
				"TestNet",
				ChainType::Development,
				move || indian_teal_genesis(wasm_binary),
				boot_nodes,
				None,
				Some(DEFAULT_PROTOCOL_ID),
				Some(properties),
				None,
			),
			Alternative::RoyalBlue => ChainSpec::from_genesis(
				"Royal Blue",
				"MarkStudio",
				ChainType::Live,
				move || royal_blue_genesis(wasm_binary),
				boot_nodes,
				None,
				Some(DEFAULT_PROTOCOL_ID),
				Some(properties),
				None,
			),
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(Alternative::BombayBrown),
			"tnet" => Some(Alternative::IndianTeal),
			"msnet" => Some(Alternative::RoyalBlue),
			_ => None,
		}
	}
}

fn bombay_brown_genesis(wasm_binary: &[u8]) -> GenesisConfig {
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

	let endowed_dev_accounts = vec![
		//3wvaRWBFgEifBUN6z68fuosfPcWHismnVFv5dx5dFbdooUL8 - Alice
		hex!["88dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0ee"].into(),
		//3yZoPm4GdpakE28bPs7EWFkCm1XUxxCC5iC2waLBC9wm2Qre - Bob
		hex!["d17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae69"].into(),
		//3vMkNoL6q49by5JbbGAqGZ8YFycBu2zomU6mssW1jpwWvHaG - Charlie
		hex!["439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234f"].into(),
		//3zDfe2TD5p3WGhHMwJG7aDbjtGzanUuLR2CXpWrZMLN2KqDW - Faucet
		hex!["ee5d6689d78e26bb5b35b0441740a065c7bd8efdd1c15422075c3f2b2021b5d2"].into(),
		//3ydNZgC16DA8zKzxNwB6uW3ufoM4X373vRVpCWFTnAXzFff7 - Alice SR25519
		hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into(),
	];

	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		AuraId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)> = vec![(
		//3wF3nbuyb97oSkVBSgZe9dpYcFw5dypX8SPhBWrUcCpZxBWW
		hex!["6ab68082628ad0cfab68b1a00377170ff0dea4da06030cdd0c21a364ecbbc23b"].into(),
		//3yzE5N1DMjibaSesw1hAZ8wwvPJnxM3RzvQFanitVm4rkC8h
		hex!["e41d2833b0b2f629e52a1bc1ace3079c395673bab26a14626b52c132b1fb5f1c"].into(),
		//3y5rP4K3E9QoyPk8Ax47vrnoJPD89AEBThUJQHqHCA4uRHFL
		hex!["bc2b5d4a95a29479caf0c5a065274b63870f200dbc68b0dc85d2dfe5005f8f32"].unchecked_into(),
		//3xaQXFoMVNgQ2qMCXHazaEiQ4bzWfVX3TowLc1DHMB1sL4nx
		hex!["a5b6331fcff809f2b3419332678fd7b23a2a9320240ec36652337fe66a7337e0"].unchecked_into(),
		//3xE2yQSUQ9hfeX1kZjP1Dg5hoU2EdLc1B9zFjzEcc5fgax2W
		hex!["962cc02d5dddbb2fc03bd8d511844ec47e798b3bc20d9daf7400b3d09533d518"].unchecked_into(),
		//3vL3vWTS2FZ9JDc4SyMFXQRa5TuitFBfSx8ZrygeEMzc7HkV
		hex!["424af4547d488e65307cb14ffae20257b6e000658913f985824da5629afff13c"].unchecked_into(),
	)];

	const CONTROLLER_ENDOWMENT: u128 = 1_000 * CRD;
	const DEV_ENDOWMENT: u128 = 200 * CRD;
	const CONTROLLER_STASH: u128 = 1_000_000 * CRD;
	const STAKED_ENDOWMENT: u128 = 100 * CRD;
	const CORD_STASH: u128 = 10_000_000_000 * CRD;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k: &AccountId| (k.clone(), CORD_STASH))
				.chain(
					endowed_dev_accounts
						.iter()
						.map(|d: &AccountId| (d.clone(), DEV_ENDOWMENT)),
				)
				.chain(
					initial_authorities
						.iter()
						.map(|x| (x.0.clone(), CONTROLLER_STASH)),
				)
				.chain(
					initial_authorities
						.iter()
						.map(|x| (x.1.clone(), CONTROLLER_ENDOWMENT)),
				)
				.collect(),
		}),
		pallet_indices: Some(IndicesConfig { indices: vec![] }),
		pallet_session: Some(SessionConfig {
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
		}),
		pallet_staking: Some(StakingConfig {
			validator_count: 3,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.1.clone(),
						STAKED_ENDOWMENT,
						StakerStatus::Validator,
					)
				})
				.collect(),
			invulnerables: [].to_vec(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_democracy: Some(DemocracyConfig::default()),
		pallet_collective_Instance1: Some(CouncilConfig {
			members: endowed_accounts
				.iter()
				.map(|x| (x.clone()))
				.collect::<Vec<_>>(),
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(TechnicalCommitteeConfig {
			members: endowed_accounts
				.iter()
				.map(|x| (x.clone()))
				.collect::<Vec<_>>(),
			phantom: Default::default(),
		}),
		pallet_membership_Instance1: Some(Default::default()),
		// pallet_membership_Instance2: Some(Default::default()),
		pallet_reserve_Instance1: Some(Default::default()),
		pallet_aura: Some(Default::default()),
		pallet_grandpa: Some(Default::default()),
		pallet_im_online: Some(ImOnlineConfig { keys: vec![] }),
		pallet_authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
		pallet_elections_phragmen: Some(Default::default()),
		pallet_sudo: Some(SudoConfig {
			key: endowed_accounts[0].clone(),
		}),
	}
}

fn indian_teal_genesis(wasm_binary: &[u8]) -> GenesisConfig {
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
			//3y5rP4K3E9QoyPk8Ax47vrnoJPD89AEBThUJQHqHCA4uRHFL
			hex!["bc2b5d4a95a29479caf0c5a065274b63870f200dbc68b0dc85d2dfe5005f8f32"]
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
			//3u5KV5TCUjqhw4td1gAyEQpuKp9etNueNNEvh4EmPtvhxQ5w
			hex!["0ad26cb81f15cfeb79f57fb63ce12a87ba27182301ce5adcbbc11675507c3e09"]
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
			//3vpuUKGMrtP1qpabD63iNLBQm2DCo1LAdPoT9VMZsT6UXYg5
			hex!["584c9ed9c6628d311dbe64069227d3a4c25ff7bee43b18d5dc8b1bf8f69e8878"]
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
		(
			//3ymyaXvRnmR5nXti2g9EZKAnnKmyxPWdCtEy1BrTZYNBpgB5
			hex!["dac569278a02734f9e116cdbb8f0f5f521919b4ed1943aac01c803a3aec08415"].into(),
			//3xWPkL8EuFbY7i9knUkMrnLwUj2komMHtPAjFSY8G6EW65po
			hex!["a2a69c524c40a5fe21127ac02d0dfb79a49bc597ebfc7678f3d71c37eff8b249"].into(),
			//3xHFpW2DvkNVoikFWbHZE3x4HSPCZ17LYQEzGedZsrBifnDm
			hex!["98a1bb66a7438ae07440560a724ef16b5a3291d845cb62938fd514d8071ddf45"]
				.unchecked_into(),
			//3ymcmew8jHa1sgrPgmuV1SRmjgA1KEVJ5HWun4kV9Sb6C8UN
			hex!["da7f5d2948641047cbed0c1baef36932a3768f1f8eb1e987023ab90fb9c629c1"]
				.unchecked_into(),
			//3xKZ3S9aiB1qvEFPrH24Vfy9yVprQRsdVTh39U8P9a7CTD5P
			hex!["9a62335987b82d3e2dd633ad30c5970dc22dafead733cd8d1000aa41d42a1131"]
				.unchecked_into(),
			//3uAGHcs46btmmyfuLWdLkqeQ8a7aArNuaWukrGK48LzrdYWz
			hex!["0e97e3446ed528c83b7c4e2aff8e16bd5c967a7ec8174d671e7695087f9ffa3a"]
				.unchecked_into(),
		),
		(
			//3uHs7SHVp6QtBp8EfS1pNaLY6CbcWjfg8WrWQ5KhkR9jtVop
			hex!["1463d5ca03e5a21e53ffe128e2841cdbf2a6ebf804c8cb87f1a6e996d7e58532"].into(),
			//3uV8hMnKRh2eU5UpCZcjxVMXrV8VXA36KxvVyEtHAejnee1m
			hex!["1cfbff76b4096a9e2aaf9d5629a110adad05129c9d9d76262981b93a79666c42"].into(),
			//3z5nCgpNgZL175b6GQg5EKmDmiaR6iComTkQp82VTy6QaZLk
			hex!["e85987e0508f32503006ca3e9340801c7cd38a0157770edb1a3f5aeb67190a5d"]
				.unchecked_into(),
			//3yXWLAQifgfGvHBstzLS7gVXp4oPucph6mTUCE9PpqCNtNug
			hex!["cfbc403bec438b9128fbf41c37266cec35e6c697a40c2742d52e4f4b5ffe400f"]
				.unchecked_into(),
			//3xVpRwPXX63aZR2d6aEXPXa1dAjn2uha2KxYos6QH9EPD2si
			hex!["a236748c1501b2eecc0ad49e353678636f609fc45176384dbef21c5277bbba36"]
				.unchecked_into(),
			//3xYmhCSNLzZzrCU6SZrhf5G4YugcyYcwSuGwYwpKa82XUfuF
			hex!["a476fa6d3f35d7f1091209daf6a317aa98097cad8e857d9175851e460638a84c"]
				.unchecked_into(),
		),
	];

	const CONTROLLER_ENDOWMENT: u128 = 1_000 * CRD;
	const CONTROLLER_STASH: u128 = 1_000_000 * CRD;
	const STAKED_ENDOWMENT: u128 = 100 * CRD;
	const CORD_STASH: u128 = 10_000_000_000 * CRD;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k: &AccountId| (k.clone(), CORD_STASH))
				.chain(
					initial_authorities
						.iter()
						.map(|x| (x.0.clone(), CONTROLLER_STASH)),
				)
				.chain(
					initial_authorities
						.iter()
						.map(|x| (x.1.clone(), CONTROLLER_ENDOWMENT)),
				)
				.collect(),
		}),
		pallet_indices: Some(IndicesConfig { indices: vec![] }),
		pallet_session: Some(SessionConfig {
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
		}),
		pallet_staking: Some(StakingConfig {
			validator_count: 10,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.1.clone(),
						STAKED_ENDOWMENT,
						StakerStatus::Validator,
					)
				})
				.collect(),
			invulnerables: [].to_vec(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_democracy: Some(DemocracyConfig::default()),
		pallet_collective_Instance1: Some(CouncilConfig {
			members: endowed_accounts
				.iter()
				.map(|x| (x.clone()))
				.collect::<Vec<_>>(),
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(TechnicalCommitteeConfig {
			members: endowed_accounts
				.iter()
				.map(|x| (x.clone()))
				.collect::<Vec<_>>(),
			phantom: Default::default(),
		}),
		pallet_membership_Instance1: Some(Default::default()),
		// pallet_membership_Instance2: Some(Default::default()),
		pallet_reserve_Instance1: Some(Default::default()),
		pallet_aura: Some(Default::default()),
		pallet_grandpa: Some(Default::default()),
		pallet_im_online: Some(ImOnlineConfig { keys: vec![] }),
		pallet_authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
		pallet_elections_phragmen: Some(Default::default()),
		pallet_sudo: Some(SudoConfig {
			key: endowed_accounts[0].clone(),
		}),
	}
}

fn royal_blue_genesis(wasm_binary: &[u8]) -> GenesisConfig {
	let endowed_accounts = vec![
		//3vUo2cZuteV3WrayeXMzZgLCmGqFfKuJ2GCuu9HmWDMBiYrW
		hex!["48f605dd2dfcfa161cd419951d00f9b43bfd3ee81f562039688cb347b0e49d3a"].into(),
		//3xn63E3BrY7X4EyLXRkVeU25tJaqfXGJ2VHWtuDQrwWazWvF
		hex!["ae9eec9dc02f0f2b9553720932d4176dbe55c425ce0f57fe06fa8c974fb0480b"].into(),
		//3ukcr5pvsTQpVDXkbUDcg9hLVaaNkrc5AptdKTEBz8dhFJY6
		hex!["28cb71176accb9169ee13b3358ce49d871a283e154d2fcb939d5adce7ae0567d"].into(),
		//3vxQUq3EbutEfxMkefyf6ZaT7MwXNs1fvriQfM327X9KhZvx
		hex!["5e04fda782ac2f0da75a67c74b93b5b232811a128bb10f456953c03d2d00bb3d"].into(),
		//3yTRYKjqgyBNku2iQcD28b9WZDBQudh2E8XFkUv2AkyJWq9o
		hex!["cc9f24c386202b6a4304c1357d43e9859045cb9bc89fa39dfc4860f1fb22d21d"].into(),
		//3ttNf2ePjeZuNcMdjB1mpNXuXvCAjQKUSRqTCkQFALLFLQPh
		hex!["027966d7686c1aa2ce7b1579f930511da49e97e37402b55c289180446715aa63"].into(),
		//3y8pKYpDKNEpXd4ENDnjRg1AoJ45kpc44Vy3H3ijHL8TGoGP
		hex!["be6e2a15dd25f446f7eae9fde80442b9c9fae7c5b2564c5c854f966e65d2a116"].into(),
		//3unfyQomJwxNvPTd9qJN1ZNKgHHytBSDTFA8ZsCGmkYZ3Eia
		hex!["2a5c74ed1c1193ac0194f90ceb3704be602499787e7b85d5aca31784ca91577f"].into(),
		//3x9azPtRZkPtE7Yow9cCoCvdKpR79ubzf68gZhewtPkq7LYY
		hex!["92c84d81a65e8ad00b8b5784c61e9cac63e289a1a8202d986444725b5214516e"].into(),
		//3uJDeJ8SVcCcyZzKtA6ZHdGwBjmT5ysnjA15toMnmWae8bSD
		hex!["14a8f3c241cb14a8e25953bc5c752909cfc0ede57666290ff6130d25a6d9ad47"].into(),
	];

	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		AuraId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)> = vec![
		(
			//3veNTCYshSrJjJ3Pq7DMrCeMoJTFuwWbLLBdPYNFgXg3hVCM
			hex!["5043bf146632724eb121c79f0e66b328bf669197a4e1d95d4e1d8fcfa2fbd167"].into(),
			//3xEiS56yVLFeeUMYtGE6a8WA7xJvMfHvd33pNHHWhE2NBX1Y
			hex!["96b19588a8a2ce248fabf5b89ca330dc52aec6a169455c7f328c17c0debeec4e"].into(),
			//3x8quZXjt5SyQdcme1UbsWJJR7mPoq5aY7iEzmE1b9dqV5dW
			hex!["9237457470f9a2a77decc718f988bf10f3b668e874bf960e24086863d8fb492c"]
				.unchecked_into(),
			//3vQBDhVG3tL2f7thSv5v5paeqeMtpc1xXMGNM8u2Mo75tHw9
			hex!["45707f72cbcfdad3cc062babff2c9e23db52513325122efc4420327541967146"]
				.unchecked_into(),
			//3vPNeAqhxnEQS7SLamHvyroMmLhhL5QYccFdtexiUgNgdxLG
			hex!["44d3b3ba85af32ddc3ec093677cf74636d58314ab71a0662502907ce4528ce7b"]
				.unchecked_into(),
			//3yxJ5rJarncv5S94vu8BvRRseot6pdnTJXvRWF3FygMbnfYn
			hex!["e2a428e55206ea87a16e60bfcae01876edaeae33cec523debd76d6385abead26"]
				.unchecked_into(),
		),
		(
			//3vf7jrHrK7w6qCLc8GWrsYAwHj4QED3YfuBrG1zb9dTZULzd
			hex!["50d576b5aae41f38b1e585b1524dd6133f79bd61f888b7720e6f7de55eae650a"].into(),
			//3uLxmYuEqZGnprQvzpzzQPFf8PYv9TYWPbBjqcmjS56EPern
			hex!["16c09d6dafa217b65227857749d1debe043aeff6fc7a74200e40e469b6e35703"].into(),
			//3vCvxftLuH1sz5mp1Dfz3wz1LdVMHD6EJEMd37GD6JCX5DKx
			hex!["3cdccc499fdbe7d08fa616397c045daab0a99e31bfad13a67d21c2c45c44d050"]
				.unchecked_into(),
			//3ymkZ6Tur6kYn9MgGwuNh7YTytpQJZAspLXKxYkCZp7hxi1R
			hex!["da9990ba684f19814412fd7b695b43b3859239e12025ff44dafb6a14751eade2"]
				.unchecked_into(),
			//3zN9crtG9MswQ8tXRSfAuwG452Ppm5wEkZJEsET8J5KpuzFr
			hex!["f4d58d74cdfdda529d5b4781ba3e3c6e7cba69987cb24afa7aa55c1fbd15d25f"]
				.unchecked_into(),
			//3wsUkuEvndukwn19KjHuyodRyfMcizszS7yx1r8BTKjPzDXs
			hex!["867f6532e2c1c6c5fd141be9be414abeaf5051270ea53ce79170ec5a313a9e24"]
				.unchecked_into(),
		),
		(
			//3xTuyo6V8zPj8QNaYnnfJRLieNBGy1Qr6cqLyZ4q1q8HPrhF
			hex!["a0c2a519b78cd772bcec7e71e7ee01b7f0f059b39b21f2772bfbb2ee70832c17"].into(),
			//3veznSEUy6Qqq9LhJ7V823spBsfR8wfNRze6inubVYv5sdg5
			hex!["50be0cb616ac9481b544d5bf01a63adfeec835aa1e79be15ff7cb38a271e4d2c"].into(),
			//3zQSuV5Lx5owdECmiCqoN9tSdN6tNdtZzUuaqs3LTHKVP1PB
			hex!["f6963c3e2f97bb0e9850000032c7349c367cf23376efd8c10c1781117c637351"]
				.unchecked_into(),
			//3uAAaQXq3hLUUDSpCXJpHNwd9GEWnUuHCEoj8Wi2krxjtu5t
			hex!["0e84aa0f5a530b31876d4949d600dddb9f5c1e660837f3551f320223ca954e43"]
				.unchecked_into(),
			//3w1X2YD8i3aCJqESvXqp4Fn2r7a9WSmJMRRwcsUbpH4qhiXW
			hex!["6064c3e95252c754eba8b5ea413779c5e8c15ecf259cd45d103e96986dc0e301"]
				.unchecked_into(),
			//3ymtnstjYu3kq4LZRGCigmWPxwR9L2E3xmPQsi6bGVeiowu8
			hex!["dab54bb4464bd856e0b6fb31d5dd98254ca3f1fc4fd17666fcdb6bd57c728509"]
				.unchecked_into(),
		),
		(
			//3zKQorgBaQd1LEaW8RcevTcKvJbBeaSYz3ToBsBWYPfq2bsF
			hex!["f2bef2f6de57996d01a2f97787acea64942baa110402ecbde7e966327aed0008"].into(),
			//3tqYrxr1pKYrHvQZbYrzuq3TynedwaDZ9GyMK5VYEt72YbxD
			hex!["0052056c6ce9bfc7a24ad7212813f3118a824e3a9b244d37c1ed9be7d8e72c40"].into(),
			//3xREotP6Ux2ssrAmfvRCD4VtBaa77r1BtTHPsds9KKEzeZFT
			hex!["9eb84b16106c6b19cb8fe6a5bb6a89234524655b40d5b919ba6b845253410b3d"]
				.unchecked_into(),
			//3wDeazYjUe7QyZNcM1knKhd5ZerXnqe2D72jvF93PMz7rhBM
			hex!["69a528694cef94ee4d248f41ec3d5d48187c0261bbf4124bc3a94b51182e94a0"]
				.unchecked_into(),
			//3yGKnuSKrZt2ALPgh1yZTiirSeS1DztzcqEwtmUuqCYsE8jb
			hex!["c42817b702ff6ee1c894e103b2849672266e415080d1f6a248884d8c738bb947"]
				.unchecked_into(),
			//3zKeKBdKShsmxyNicGJdw5bxXqtnmdPgD1ssBMPfVvkMH9qs
			hex!["f2ec69c0874b1c3d153610984800f40a314a36aaac7f7a67eca31cd30d552d52"]
				.unchecked_into(),
		),
		(
			//3yfv54pjArj89jAYpzQnRkFXjrYHZ2H98coyfxfbRmmKFMJF
			hex!["d62621110b58f09ce48736dcd9f4fa2615f52ffc3523734fee5df85b06a51364"].into(),
			//3xizK35NAnhtL1SpVpZ3nM4aUbAsh44KortTHoJwbCdqXKAN
			hex!["ac41e86e4dd3b9143db8716ac8e0580c0b91af8e816627505641241fb08d0348"].into(),
			//3zMiGjHni46EFg1oW2w3d3c1PVCNBEJnivx8GQ3Eh8VMMqbc
			hex!["f4803a06fa7c860e7188275cf3f8aca965cea285121ec6390f47605dde605808"]
				.unchecked_into(),
			//3u5TwqPuYDXRkJzTsUbHd2mFA595SfqzsyPHFgHGhrXZBHuL
			hex!["0aeee8788c6ff629bbeffb4ea17ebed1f22c5e7d1b6b367894ef1e0088ffc30e"]
				.unchecked_into(),
			//3xE5okiX3pi7eEnCHLEXzJbCVNzkinpqeqQRPiQ6xky5tw8G
			hex!["96364a1bc81c093aefa99a2a3be2ca3a7fb17e4234acffdd2629a7d02230ca16"]
				.unchecked_into(),
			//3yZ1aEbbUMQjLJvLzj2yi5D6C3fY44edZrV9mGdxqcmhdMKy
			hex!["d0e1ef8e6415665ed1e0455134945eac01c164f4f07644228bc71d160310bf19"]
				.unchecked_into(),
		),
	];

	const CONTROLLER_ENDOWMENT: u128 = 1_000 * CRD;
	const CONTROLLER_STASH: u128 = 1_000_000 * CRD;
	const STAKED_ENDOWMENT: u128 = 100 * CRD;
	const CORD_STASH: u128 = 10_000_000_000 * CRD;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k: &AccountId| (k.clone(), CORD_STASH))
				.chain(
					initial_authorities
						.iter()
						.map(|x| (x.0.clone(), CONTROLLER_STASH)),
				)
				.chain(
					initial_authorities
						.iter()
						.map(|x| (x.1.clone(), CONTROLLER_ENDOWMENT)),
				)
				.collect(),
		}),
		pallet_indices: Some(IndicesConfig { indices: vec![] }),
		pallet_session: Some(SessionConfig {
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
		}),
		pallet_staking: Some(StakingConfig {
			validator_count: 3,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.1.clone(),
						STAKED_ENDOWMENT,
						StakerStatus::Validator,
					)
				})
				.collect(),
			invulnerables: [].to_vec(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_democracy: Some(DemocracyConfig::default()),
		pallet_collective_Instance1: Some(CouncilConfig {
			members: endowed_accounts
				.iter()
				.map(|x| (x.clone()))
				.collect::<Vec<_>>(),
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(TechnicalCommitteeConfig {
			members: endowed_accounts
				.iter()
				.map(|x| (x.clone()))
				.collect::<Vec<_>>(),
			phantom: Default::default(),
		}),
		pallet_membership_Instance1: Some(Default::default()),
		// pallet_membership_Instance2: Some(Default::default()),
		pallet_reserve_Instance1: Some(Default::default()),
		pallet_aura: Some(Default::default()),
		pallet_grandpa: Some(Default::default()),
		pallet_im_online: Some(ImOnlineConfig { keys: vec![] }),
		pallet_authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
		pallet_elections_phragmen: Some(Default::default()),
		pallet_sudo: Some(SudoConfig {
			key: endowed_accounts[0].clone(),
		}),
	}
}

pub fn load_spec(id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
	Ok(match Alternative::from(id) {
		Some(spec) => Box::new(spec.load()?),
		None => Box::new(ChainSpec::from_json_file(std::path::PathBuf::from(id))?),
	})
}
