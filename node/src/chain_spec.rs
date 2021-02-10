// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! CORD chain configurations.

use cord_runtime::{
	AuthorityDiscoveryConfig, BalancesConfig, CouncilConfig,
	DemocracyConfig, ImOnlineConfig, IndicesConfig, SessionConfig,
	GenesisConfig, SessionKeys, StakerStatus, StakingConfig, SudoConfig, SystemConfig,
	TechnicalCommitteeConfig,  WASM_BINARY,
};

use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_service::{ChainType, Properties};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::crypto::UncheckedInto;
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::Perbill;

// use sp_runtime::{
// 	traits::{IdentifyAccount, One, Verify},
// 	Perbill,
// };

pub use cord_primitives::{AccountId, Balance, BlockNumber, Signature};
pub use cord_runtime::constants::time::*;
use cord_runtime::constants::currency::DCU;
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
	Development,
	CordTestNet,
	// CordMainNet,
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
		let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm binary not available".to_string())?;
		let boot_nodes = vec![];

		let mut properties = Properties::new();
		properties.insert("tokenSymbol".into(), "DCU".into());
		properties.insert("tokenDecimals".into(), 18.into());
		Ok(match self {
			Alternative::Development => {
				ChainSpec::from_genesis(
					"Bombay Brown",
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
					"Indian Teal",
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
			// Alternative::CordMainNet => {
			// 	ChainSpec::from_genesis(
			// 		"Royal Blue",
			// 		"mnet",
			// 		ChainType::Development,
			// 		// move || cord_mainnet_genesis(wasm_binary),
			// 		boot_nodes,
			// 		None,
			// 		Some(DEFAULT_PROTOCOL_ID),
			// 		Some(properties),
			// 		None,
			// 	)
			// }
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(Alternative::Development),
			"tnet" => Some(Alternative::CordTestNet),
			// "mnet" => Some(Alternative::CordMainNet),
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

	// const ENDOWMENT: u128 = 8_999 * DCU;
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
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
			.chain(initial_authorities.iter().map(|x| (x.1.clone(), STASH)))
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
			validator_count: 20,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
				.collect(),
			invulnerables: [].to_vec(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_democracy: Some(DemocracyConfig::default()),
		pallet_collective_Instance1: Some(CouncilConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		pallet_membership_Instance1: Some(Default::default()),
		pallet_aura: Some(Default::default()),
		pallet_grandpa: Some(Default::default()),

		pallet_im_online: Some(ImOnlineConfig { keys: vec![] }),
		pallet_authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
		pallet_treasury: Some(Default::default()),
		pallet_elections_phragmen: Some(Default::default()),
		pallet_sudo: Some(SudoConfig {
			key: endowed_accounts[0].clone(),
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
			),
			(
			//3wLfSLg4AbbfZggDsZ2BScSjkF8XEC7gCtoHTDrUr28hSbMG
			hex!["6efebd6198dc606b9074d7b3cd205261f36e143701a393ee880d29ebab55e92d"].into(),
			//3uPAkKYpvJwYFzasFfEoj6K4hwRiKGbbX4qDsuXmngRcRDE8
			hex!["186f6e121c08e7d2951f086cec0d6cf90e5b964a321175914ab5cb938cb51006"].into(),
			//3u5KV5TCUjqhw4td1gAyEQpuKp9etNueNNEvh4EmPtvhxQ5w
			hex!["0ad26cb81f15cfeb79f57fb63ce12a87ba27182301ce5adcbbc11675507c3e09"].unchecked_into(),
			//3yPbpB1VCL1mna4UFXqhcnepQuXJmoJFgfgedZXqteucf1W3
			hex!["c9b4beb11d90a463dbf7dfc9a20d00538333429e1f93874bf3937de98e49939f"].unchecked_into(),
			//3uWjtNikmuwLVKkLD1opoR2U92YAoExgaxDoKfA5S9N8S7GY
			hex!["1e35b40417a5631c4762974cfd37128985aa626366d659eb37b7d19eca5ce676"].unchecked_into(),
			//3ur2S4iPwFJfehHCRBRQoTR171GrohDHK7ent21xF5YjRSxE
			hex!["2ceb10e043fd67269c33758d0f65d245a2edcd293049b2cb78a807106643ed4c"].unchecked_into(),
			),
			(
			//3tssweCjh9wU7A33RJ1WhTsmXkdUJwyhrE3h7AwHum7YXy5M
			hex!["0218be44e37405b283cd8e2ddf9fb73ec9bde2efc1b6567f2df55fc311bd4502"].into(),
			//3yDhdkwPaAp1fghGhPW5KwL6xKDCmvM7LGtvtiYvLHMrtBXp
			hex!["c227e25885b199a75429484278681c276062e6b0639c75aba6d7eba622ae773d"].into(),
			//3vpuUKGMrtP1qpabD63iNLBQm2DCo1LAdPoT9VMZsT6UXYg5
			hex!["584c9ed9c6628d311dbe64069227d3a4c25ff7bee43b18d5dc8b1bf8f69e8878"].unchecked_into(),
			//3zJUM1FL1xjSVZhcJhhYEeiHLwrJucC5XAWZpyJQr9XyDmgR
			hex!["f2079c41fe0f05f17138e205da91e90958212daf50605d99699baf081daae49d"].unchecked_into(),
			//3x8xZQoUYS9LdQp6NX4SuvWEPq3zsUqibM51Gc6W4y4Z9mjX
			hex!["924daa7728eab557869188f55b30fd8d4810cbd60ad3280c6562e0a8cad3943a"].unchecked_into(),
			//3v9USUnkQpKLYGsDAbzncF6PsHQdCHJqAgt2gKYfmZvdGKEi
			hex!["3a39c922f4c6f6efe8893260b7d326964b12686c28b84a3b83b973c279215243"].unchecked_into(),
			),
			(
			//3ymyaXvRnmR5nXti2g9EZKAnnKmyxPWdCtEy1BrTZYNBpgB5
			hex!["dac569278a02734f9e116cdbb8f0f5f521919b4ed1943aac01c803a3aec08415"].into(),
			//3xWPkL8EuFbY7i9knUkMrnLwUj2komMHtPAjFSY8G6EW65po
			hex!["a2a69c524c40a5fe21127ac02d0dfb79a49bc597ebfc7678f3d71c37eff8b249"].into(),
			//3xHFpW2DvkNVoikFWbHZE3x4HSPCZ17LYQEzGedZsrBifnDm
			hex!["98a1bb66a7438ae07440560a724ef16b5a3291d845cb62938fd514d8071ddf45"].unchecked_into(),
			//3ymcmew8jHa1sgrPgmuV1SRmjgA1KEVJ5HWun4kV9Sb6C8UN
			hex!["da7f5d2948641047cbed0c1baef36932a3768f1f8eb1e987023ab90fb9c629c1"].unchecked_into(),
			//3xKZ3S9aiB1qvEFPrH24Vfy9yVprQRsdVTh39U8P9a7CTD5P
			hex!["9a62335987b82d3e2dd633ad30c5970dc22dafead733cd8d1000aa41d42a1131"].unchecked_into(),
			//3uAGHcs46btmmyfuLWdLkqeQ8a7aArNuaWukrGK48LzrdYWz
			hex!["0e97e3446ed528c83b7c4e2aff8e16bd5c967a7ec8174d671e7695087f9ffa3a"].unchecked_into(),
			),
			(
			//3uHs7SHVp6QtBp8EfS1pNaLY6CbcWjfg8WrWQ5KhkR9jtVop
			hex!["1463d5ca03e5a21e53ffe128e2841cdbf2a6ebf804c8cb87f1a6e996d7e58532"].into(),
			//3uV8hMnKRh2eU5UpCZcjxVMXrV8VXA36KxvVyEtHAejnee1m
			hex!["1cfbff76b4096a9e2aaf9d5629a110adad05129c9d9d76262981b93a79666c42"].into(),
			//3z5nCgpNgZL175b6GQg5EKmDmiaR6iComTkQp82VTy6QaZLk
			hex!["e85987e0508f32503006ca3e9340801c7cd38a0157770edb1a3f5aeb67190a5d"].unchecked_into(),
			//3yXWLAQifgfGvHBstzLS7gVXp4oPucph6mTUCE9PpqCNtNug
			hex!["cfbc403bec438b9128fbf41c37266cec35e6c697a40c2742d52e4f4b5ffe400f"].unchecked_into(),
			//3xVpRwPXX63aZR2d6aEXPXa1dAjn2uha2KxYos6QH9EPD2si
			hex!["a236748c1501b2eecc0ad49e353678636f609fc45176384dbef21c5277bbba36"].unchecked_into(),
			//3xYmhCSNLzZzrCU6SZrhf5G4YugcyYcwSuGwYwpKa82XUfuF
			hex!["a476fa6d3f35d7f1091209daf6a317aa98097cad8e857d9175851e460638a84c"].unchecked_into(),
			),
			(
			//3vsKjsz8ikbmD7PS4XP3VxEipip7W9Jo8L98EtMv3UoZo4Hw
			hex!["5a24ce4cbd76ac348cb430caed8d533672901efdd125609fe233a115e2edfe50"].into(),
			//3ubmvZzZtxtS6D3TJEXNLBqMtNnT8JEytza3jXX2sFV6vgA1
			hex!["220cca4884ddada4bbaaa3ce505706f51b07ce8738d57379fb081fe4d01b7244"].into(),
			//3z3bN8RPJEamkoBsWLKVxbxfiMs2vN1kS6SFuFKTMuWVxQGp
			hex!["e6ae8f02b450a0600b14343cf60ba132b7b8426a96bce30889795f645853c102"].unchecked_into(),
			//3u6U3ZAxiBdsDraMWTmszMEPKpgLwdHAeB55aCJRmS9gsAE2
			hex!["0bb27c498f64ecf5c5bae860f15e4515ae8b2e774cdad776259eb309ab8cafde"].unchecked_into(),
			//3xGmRTpYFVjQ98V8k8xsNFKRL28Gqo6kn71Kddi5rtU58GVo
			hex!["98422375e9ae92cf70a156b84fcd57e47d38716908c58fb53890420b17b5be52"].unchecked_into(),
			//3u25LmRqCAGc48VAX1UDajY3DkEhy8c2FLDJCHmuihMFwS8j
			hex!["085913f2617e8eb4e671dd624e6aa7c61de595389aec27578a15f9fe1d0ac843"].unchecked_into(),
			),
			(
			//3xYsytLCTStLSwfWxh91Ad2tL8otmtF1dJ3tMTiygUiSvuNz
			hex!["a48c25038b20cdc6f630b14c85372b2ae5762eb74fb274102b7a2b14b7051809"].into(),
			//3v7Y2hnUCrXhrZboNFiNgiAmTcgNYYBtsLRxKE38mUJdtD21
			hex!["38bf61506e50f1ac9738cf4d88b0eb05a1a51b95181bc1b944335e3c15ac0034"].into(),
			//3vpakjQ9HrnhfA54ju6hHvP5x8keyagw7iMKZ1S5n3NDvkd9
			hex!["580d9d14834f57975125c15d7b6dcca00bccae1490679113789515dc9cc32878"].unchecked_into(),
			//3yeT1r6gsGaPZebVaodL7G8tosHBie3m7Mix8DrKZDveZmTL
			hex!["d507ce8aeeb63223e6baafc98251d851a92102639939bacae6148f64077fb073"].unchecked_into(),
			//3vLZjdQhQxqEtfnvABJ2vEzwNgbacSWdpunWHyWSfZNk3up2
			hex!["42af4fdb289e0a64db364f95434a170ef92d62ba33a969b144927191765c613b"].unchecked_into(),
			//3x1XcTm9rwRisuaASHDfrQF361sKEpX9N5KNjWgudzKbXJze
			hex!["8ca2f7b68d700760d169d8aee894bb3e97bce6a18999aab57740634878e3f91d"].unchecked_into(),
			),
			(
			//3xGoxDcLGavFEqgEt88o6qtVd8c4FMmCcnnoGnKAa2aBhkzV
			hex!["984aa7f7f5289e1138f81247bf6d75c991dea374acf8ab3777ddf33651a12a23"].into(),
			//3xuLTjczDvAVdAhwtjshLrssziykQL7UDftQ88J6SUEGJnbt
			hex!["b426315fd2d2b6e883f4daa098c2f8174caf4fb609de4c37e446d03cb2b5e93c"].into(),
			//3ymp2Svzrbb3BJEApu6bz7BSxmQamXmHcw6LcijsKfaFyjgY
			hex!["daa54075b8f4115a2d1cc00b5533091e06a99e185d643ff2db38d2f9a8265048"].unchecked_into(),
			//3vQxmHK7MKm2v1UbBXaTEVjuvfvRNdXfVgGwoSu3DSmEVfGg
			hex!["4609d07e3d50fa6f46e5e570882772245bfeebd033c9e2a5e37ebee1528b28c8"].unchecked_into(),
			//3y67uX59HnKteFy3SbTVsyL6Q6BAS4DTHNzTTAzL6nMrFNoR
			hex!["bc5fa07bc9d5968cf83e535d92daa252e302c319f3af79b97089178cbf32a379"].unchecked_into(),
			//3xx9LhmiysYYBeZ4NLuNxuwfTHuzEAvhUp3rrEHpR21AjKRR
			hex!["b64a7ddcef317ab26e4e047d1db7656e3c7fe5ba23711d53b141795216f32547"].unchecked_into(),
			),
			(
			//3tz2n6orkbCjz15r6t4xbDBhmVxLCG2adLePfRs6mmfD8qay
			hex!["06c9f566545dfa5351cdf993695ac2354e1fb82679a08ddc1cc3548370ab1674"].into(),
			//3xmTii6st2t5J2chxYqL9Cpx6q8o1DPuT5waZyZbzkaKBryV
			hex!["ae24a9add4fb2177fcedafe66b94a9755f4e92960be0386a30bdcc7b1a185a15"].into(),
			//3yJuGWECWK3phD54edp7TokSE3X9P73yztTTV8YqBPN5WkxC
			hex!["c61f45f297c5b76f55ff19b1ad259434fca71a9bd5874291bfe065eefe209322"].unchecked_into(),
			//3ymt2AGeYMerycTx6cn1uc3tnJ1DaigEiQSUBQKtvWuFUwjP
			hex!["dab2b3486f0575d06121d7580fd0818e067fe10b9ed8be57bbcbf5248c7dfddb"].unchecked_into(),
			//3y3GcnQmgHqVkQmC233DwNd4wQTLYFregLckuEdJxFsX4MTk
			hex!["ba33374f215f5d46277cd70b12847fda1b83c291ff3a246cacf6c249848d1d02"].unchecked_into(),
			//3z3q3ioMiQwxSx5s3XVJZ571bZMvUzaxp77kMLdxzwQArJxP
			hex!["e6dc9e550771a2b6e20ec7fde549ec7999c7af795e1ed974d54ed41203e4db44"].unchecked_into(),
			),
			(
			//3vWjMuNsATnGWSGDBGQqU9NEXUE6KjL3tVMLjk9JzYDv4kgd
			hex!["4a702b07e33e887a3b6ed3ae024ec024fa1d169ea8013dbac425c52532444110"].into(),
			//3wLF4w3Wj9ipMrVrDJ6ium9DFd5jBc7jxRAZMtD8FbyWLKpc
			hex!["6eacb4c24b1893b903f4d064d888d44ed701d53116765e7a9455fb9b9adbb266"].into(),
			//3wnBzUwj4qt2h8SSkt5hTPVfViFAwPWehoGGZx4JhvJx2ejm
			hex!["8276babbb3c5d36a223d7694a915d1a218dd63ac410ee03fdc9e3ef267a0aa79"].unchecked_into(),
			//3xEHMAoE6EQogdNcyh92dG8jAEaUmKyfWjTV2NAFXEVTZrmn
			hex!["965d246199057a2edbda2bf21c07732055cd1ae5f91d2575186d307f32532e8a"].unchecked_into(),
			//3wQpQAB9Gs8xJ7baU8z3Nz6WF7TY9jWca8GpMprxbSd5EXBY
			hex!["7229e43994b5d2f05e10dfe12ee7a537b6031a839eb18e0e64ba61ef79839938"].unchecked_into(),
			//3zEUR19DtFpQ9Pb9UHm8JqrdcXomukPu9A6Zp6epxR9SF49e
			hex!["eefadc5e8e2b00b89919b6b09443d5f26345c82bd75f50faf4cf08d5543b9a0e"].unchecked_into(),
			),
			(
			//3yod5kt32zYW9Uby2KgAND7nDVM5U9ksMiExgaH9cPLog7b5
			hex!["dc06e7c745097836f1452f0e876f3fed71686e9d0607a5cdb11a855f142f5f12"].into(),
			//3wKS3moZu2uGNSRApNpeFJLdyQHyP4LSNCzHam5UuXEeQ1PW
			hex!["6e0e6c360cc34c9b41acbd6fd61cf7dcd12674ececbd28079153b741a3ab0e39"].into(),
			//3wCLCyUwfmBWREk2PBUu67m1beQxZF7drUiUbrmRFQPCcvmX
			hex!["68a40a8659d0db974d58697a340fd16ce1d9d2af3f5ce3859b6354de1df3b406"].unchecked_into(),
			//3vSNUezMJN5yAMto5jPfpxwGARLbrfAUG1GJBSBwncRL1teU
			hex!["471ce2d4265d966a00c06a95b2d08e85ea68f9acffd7198e2bc4e946fda7652d"].unchecked_into(),
			//3wcAL72XWuSUWmDWekMqjxBHW9qQh7katac2ExzyoPku4wio
			hex!["7ad0ae8034920377ab4221b01c4d013668b0f1b078d68c563a794693f39d6f47"].unchecked_into(),
			//3v7CHY1snDLTwbETtCd2PEfstoZ4YEskUNhjGMWkDaCHnxq4
			hex!["387cea3a403afefb987ad1bf4f7bb4b686775f4f78750663027c47d3a8fe2770"].unchecked_into(),
			),
			(
			//3w95FdpQrG2DdfrbEpZBcKSETXAF7vQpTKjYMXCMHiPdLvnV
			hex!["6627f6ffb8d69e329c93a417f761cc5e652f785c9907da3011fc6d494869db34"].into(),
			//3z36YpUKi9HewJpbf4sE6KrDrBpZ7nF1Rp1cUovPXm7jBcBJ
			hex!["e64d8e70b7d074524ad95de2d1deeece12ce7494fb1c5f6c611fd4e25692f012"].into(),
			//3v2GZkMv1BAgcv4vbQUCRHZDc2iWtxDPBM1aKhHFGWp1Ahyy
			hex!["34bb1818226faa73cf7c34a1cb2c941c6fd0f9719d1e4016118c9eb5c75e1143"].unchecked_into(),
			//3ypZeLc29ZYu4rqctGzSb2JTcyz8VGA2xMurqVkxsQwcrtGk
			hex!["dcbe939c57f76424b8d0dd2935bd62728aa7f2599afa94568c9ee8fb7c6bff3a"].unchecked_into(),
			//3yuNfhNbyQTxX4bYwdAG1XtT3LJeFz7VcNzbdkewtgtoXaW7
			hex!["e069da67e63a998df91287fcdeee649ee0aef18a4cff161bb9200a81d11a696c"].unchecked_into(),
			//3z2n2xkVzajVTmav3yr9uM5D9q2L2H1nC9n7Fd1fp8kRkeae
			hex!["e60f3ae7e5b73217b2c9baa1a351621cc5e2502c76fd1c97b2ca6cebb62ebe53"].unchecked_into(),
			),
			(
			//3vcNEBqySVqsJBQxNJqaVxwHa4MzcT13Y7TqzE43srwcScKP
			hex!["4ebc801894ee6b5979de2378a6f3f076aba0307fd1cce0041e4bb362ed987e24"].into(),
			//3xKKjf6nhyS2SVhDNH3MPasppE2wYkjbWicnLuEKkTeP1ikY
			hex!["9a35682c9d839020ec765d1c626f5f3dec61cd3b64a5ef3b3c7d1d5c00a6d03c"].into(),
			//3yNFK8bjSSwhX73avnLBxhUtRXTBfwTj6TeDKLizVF58dUwb
			hex!["c8ac7ce884db951586f5045dba0b9456c17e60616d7afc9cb5c5f20390611154"].unchecked_into(),
			//3wzxoSL7qfPX47Yfm8JNwaiGGYp6juRRWVbvUabhVFEJXFQ3
			hex!["8c348448719e74425bbb04bb8248741254958441d3c597f8b17120b4c799ec1b"].unchecked_into(),
			//3v2DZpV4RSAnsN6BwfXPMvAQoNqb3W8rkTski6bfrreTmXy8
			hex!["34b0ffd502f8942f1d53cd92ed8cefa198ee12c206164295d88c8c89bfacb440"].unchecked_into(),
			//3wF8NMDakNcJAc8pamorBoUFkc2AN3wovUBfQPzwY8EsBs6u
			hex!["6ac5ed034d7688c86cf30d90679e89eda8c78c9c17994f950332dad10a3f5645"].unchecked_into(),
			),
			(
			//3w1VX2e8WucG97Q9eFZ6q6YqbC7yMyJUxvNoTvwSopCrMYA2
			hex!["605fafad6607dac3f945da160685409d599d3e3079976b1248fa27c565712338"].into(),
			//3uLfPoJ3RiWR4wJsXHTcSiKj532aKwGbr3FyhXQFjVL8uDf6
			hex!["168620192dea52a0617b7dde28491724c65e15a83050a04d0617cbd62116a039"].into(),
			//3wGsG66wXxLg6PxUd61KM6zpSMYb4s1C63shfUm9je5chPsv
			hex!["6c198f0bf9486ba82b50b0b1015487f94726ff125a9026999b6fe221a60dba18"].unchecked_into(),
			//3tz19JBCTNdzKNuZxgj8aEhg3gF6yAghjuyfsxk7RtZVnWuN
			hex!["06c474c996df1e008f046c4dcce8b815025ec7ad83fa89f0c9ed192869eec180"].unchecked_into(),
			//3w6kH39VC4GBAtWTwhhGjzVwtHonBafcHygE1wo25tP56u2j
			hex!["64619880b3c4e3fff2b8ba449d86f518ed2ba007f048bc3c7ba76ff033f5bb28"].unchecked_into(),
			//3wCdtUbcwwqAYuKpTyyBZhKrMjowhTzV6zHgAm2kfm735nc7
			hex!["68df8f96d801ee71e7a3368169b3c5c7285ecfd0ce59ad12c23e2c6ca7a60f31"].unchecked_into(),
			),
			(
			//3uycP1NE8gAPy4hyiDMFcTQbTx23K7pmkxpdc4eBQNjimeuD
			hex!["32b40f7c0488c7913732d0289c283635138fc6e41a3cb6ad8d37108961f7c421"].into(),
			//3xKioqT5h3UV59HxVw53NUsEtLsyWNygV7GqNe3SqqE2hozT
			hex!["9a83130879dbe3be7f4c84e35da8afcf62688f752517bbab814a1be7e07cb424"].into(),
			//3zV7WSzQtz1Rdk949njJj39XjAVyYsEb6vXdhbo3LH1tAXCZ
			hex!["fa252a67c1be5c4fe0f3603ce66b2bb2d6c817e95a49a6265f60def8f909942d"].unchecked_into(),
			//3zZBFazgVXaxfUx3qcPH2TYahuiwN1QrWVD5SJDEaMskrMho
			hex!["fd3ebff80d2438aa465edc592eccb42c13b4ea3a3537191453ed611a61a92eb0"].unchecked_into(),
			//3u2eWZchnmYCDUd2dcz8HL6rRRfvzTimC6iuN29VuRJhzWWZ
			hex!["08c8bc0159d0df6c59aec9f615b49baa4c3875e3561f4994504fbb02a148cd65"].unchecked_into(),
			//3y8xTeAFHtRYwg7BSi8G8JgoeyCWcgTjjPv34NqvY9qeYMks
			hex!["be899080a6534e47fc6174df4721588e001ca8094f502dae7118300a342c2a53"].unchecked_into(),
			),
			(
			//3y3GYT5uTi3wQpUaZGhe1smEbX1raVSYA6kyAuL2w7kQ1pDT
			hex!["ba32f6ed04014f08e99415f1b86cf3e50b3e69cf522fcbd35856a95586161b0e"].into(),
			//3zSZjmPXMYRHA15H3anQRysLzbCu1TXZMKLkEAeL3kXSP7rm
			hex!["f833b9e5b29ba13d77a512e365f5e0c9e256e31515759930be43269676fa7703"].into(),
			//3yorBfn1c2ckSL34QG7GW7nDWiD4x6BmpY76rcM2ghhhr7pe
			hex!["dc3302aa9e17cd832bde8fc4b675074da3bd6ec3d709e4787998b3630fad7b7c"].unchecked_into(),
			//3z44Y2YAQDznLX9UWe9Xu2KofZVqXhVUEFdwwThTuunu5HgM
			hex!["e70a05f37a9b9fd7a277affd153df579f97bbc5c88c01a0949027bfe04a58042"].unchecked_into(),
			//3z6CKkgcKGzTr16vKozxjXSivBVB12B3vqhAEU9UxcFfZgUt
			hex!["e8aabb6bde378624b7daf4ff0be1e04eef3748d2c7cc3400db5cb3dc4ac7f530"].unchecked_into(),
			//3yrEWffcTNRtLdQR66tvz55tC7npDJxn8hcDepfYuzZw3ESB
			hex!["de04a9979797e4e2ebcdc6e4bd70e2967ffcd142ce2cbbc62d93967bafbe0126"].unchecked_into(),
			),
			(
			//3upuPhqYEfZGizV3bGXeSJ7NSAaY1tepkLPe8fw3gxT1tygz
			hex!["2c101e940f234f852e78cc72116c2dac8775996dab2cab949a8580cf29c82d4a"].into(),
			//3xbkPwrrLsUX55yNSmEPDmS6BhNAwMkJxPg7qNLJZCxVUEq4
			hex!["a6bc58cd28a40df814f50627d72607278f3b2e67886223704bfdff95f092d21e"].into(),
			//3xU6pYP1ssSzoS5D8aAoPXgfaP8gniaWTa1qBtFQKayDWVMR
			hex!["a0e7231bc5a50a86b905c0ee46e556b49fc6d93d8cf8f59c4182035699417874"].unchecked_into(),
			//3xfEyfDCfNgcQn9rT4egn1DCNv7JDFhFptkwLRwJZ9WnZsqZ
			hex!["a9664bd2ae094120664e917418b41c148c075ab2d95f410d5c9b940e4fa75d07"].unchecked_into(),
			//3ygDqNZAwHisrAyUUU94yfApNuQ2z7jGa7XnXz9m5a8aDUGy
			hex!["d661ed8171af47b1decb0d27be160dadd92aad5152da39d33069d668be96ba0a"].unchecked_into(),
			//3utNJHT1ctEbqZw5e2Yg1TZy6qBrx4rDy14TBQctMNyDYqwm
			hex!["2eb46e60666ffaecb3099fae9d16e5875e63fd3783c0e548d8000ff55f569765"].unchecked_into(),
			),
			(
			//3w9oKcH6AVrT1FkiXuj3in7w6GzcqFqDPxzbSFX4wPV3fZtv
			hex!["66b5947ed34f751c03dd5cee95df8c8cb31dee64d543fabf7b4f1bf0d498927c"].into(),
			//3uCqsRgBvJX7PmE3c77i5PGg3aQ38szS25aZgCkpbcnNzgSo
			hex!["108f6dba8826142eb64630cb87c67a3176c9ad4abd9d7e2d49f2471bac6e8571"].into(),
			//3zMaWERxz3c9jiupYY5HDp55QSe7kt8ZwcQxyCvJAb5q8M85
			hex!["f4661477439ba477c5ba77b317ead731f110076b15a6e12a97c4bc2379582b36"].unchecked_into(),
			//3zWtfvHemeVvUx3ngv2KkY2fMc4bcj9EVCN55UFBmGgaMP6d
			hex!["fb8071b8cfbaeddffe1229450307b2c3206392c099521df253851269effa2c03"].unchecked_into(),
			//3wxAKjPnQfjeDXFy8YVAL7HCaTEt5fNFjqp3HcaogUxfvsLm
			hex!["8a11916dde1dec9ca6ecd1b24a2d3c58379a552768a2700e231f12601fb12378"].unchecked_into(),
			//3wbv7xjzzYCHW5e41mAkF3gkDS92dps7KFnFpqywrbdNGx3R
			hex!["7aa0d942d862a171bff26bd59f225f46772e6a64fa53f079b39eef0cda38ce65"].unchecked_into(),
			),
			(
			//3uSKfGxThnwrrNXBCkqMv1HWPQsCQJMXcqxCskBqtUE5gqoe
			hex!["1ad72b8ab129b430d42399173f4262eb68e8e41ecdda5e94ee87c4e26aa60560"].into(),
			//3vye99Unj7bS6f1umm85jFCHjAMkg5YUGhdojCSfho8LQcda
			hex!["5ef638dbb98a96605a8e679d1be23e633e0172c55d6c676f30d360f0cfe92d2b"].into(),
			//3yuHVnzHz9NDAdLyGdVrcmqvRhrpxU1AEjucWXZ7W3VJzR1w
			hex!["e058726614842a919f875f48b9d822f94c8607f68444566b78e4423ff19a0f17"].unchecked_into(),
			//3x2u2ajzPPqjZpWirywS48ChCatchUJt69V5LYLYg4X6udFT
			hex!["8dae4e264917aab7814eceea2f1ca8e01a6b8baa22b8dd7602c89aee755dce45"].unchecked_into(),
			//3vTYKY9DzSM7uXua4rwHgUjvCx311H75XC6VvwRWeij5NPKo
			hex!["480143e72c598c02c8a858ed89eae08c1180ef81d115ffab60bf87f843257a3b"].unchecked_into(),
			//3vcW4ycM1hBeUnN7mKZsyXwKTaziVb41He1UoWBrB835XB2Y
			hex!["4ed6e56bc116273bff92a10e26806f51a4c0df450497f8e9cbf8dadfc8d39468"].unchecked_into(),
			),
			(
			//3ufbyhPbGrTCLPgCz1iaenSN26NBU6gDYoRpVgZHXr1USTB1
			hex!["24f84a518acd844aa3d5ffd97e79b40cb23ac9e72c2704986f01558a82bfe374"].into(),
			//3vLME2GNbRUi6z159KfS8uJHprrEdPDLFNk549nupjckZJcB
			hex!["428532b1656ed2a3b22c10fd69612a6958495568e054d26bf01de39fafd5db23"].into(),
			//3uh3GM9TxVN4zLZ3mWo6bDKbVF2oLtQcQdRKwevPhHo2mZw5
			hex!["2610a8dd7ea729ffc8549cc29b42967400645ab7e3970126596c3dfc9ce53623"].unchecked_into(),
			//3u9r4fH6KJJExZipxyXeyauWcKL7f272Fk7fFB43uWFbLD7y
			hex!["0e46582ea4a73f31e1d438ce055a24cce25d310f6d42cd1c438f06f8b8f482dc"].unchecked_into(),
			//3z5YCFo94oEyvLh2GuArL7ascTaeEJhv7TTwtJDTyP97HzwW
			hex!["e82a60a59d35ddc063a8bf98e24e64c1174279bfd0c4427ec97d88e0b196192f"].unchecked_into(),
			//3ve6C5bjDZe5ExsVwgv1fD1hN7oXbnYdzjJNjsXfAJKkV6kY
			hex!["500d020fc9ed098774d93966972c69c8100846e709ce2936ef2f6ffbd2530c44"].unchecked_into(),
			),
			(
			//3z8pX8QizDyMr2pNaooHUd8hDgGSj4a8H7gqLFoDpXjbxo43
			hex!["eaab11e52efd1b512b869c114f758b18f5482b19f54f3ef2063dee7c433a0600"].into(),
			//3tyGaD29H3553Y8Q9TEYnEHu2fup9Fb3yEg7hpQHkBpWaaJx
			hex!["063526b06d03e8ee1e63e5d1a1c39a5a497293a1bb8e4bee18d432420854b75f"].into(),
			//3zKePJSUF24GxPR9bEZVuqtPvRi5vCR72Fkz7jMM3MjGENap
			hex!["f2eca6ee135d79a94abf56137cfeec14e1d1d1bad843f9c89fc529003650d06f"].unchecked_into(),
			//3xR2A8KaocxzMNbKEFTSdNEkSdurQgkDXH9EEVaueiZUs4qu
			hex!["9e8db4c60556e24a549d509469701922c3041d174750d9d17327626fa242d52b"].unchecked_into(),
			//3y8yKjASKXKWNnH22zYGkr2kTBiLNbdQXF37vkmRt3hih5Tq
			hex!["be8c78b11d74ec6eae1ad6a7620c4a455c2572bb0c910b702a700866752dfe38"].unchecked_into(),
			//3yuFz4RP1xm1gPoXtzBVYtNGkGJyPwGGnKCnarbaAx9xDZZX
			hex!["e0535ad5b55bf0c372138a917d7947e8b2e7e50b4afe781ff97163f4d94a1543"].unchecked_into(),
			),
			(
			//3vASi5eyM9E9xevun4nwpM4FdY3yJ9s2hgvHs2fA5FBK9uS2
			hex!["3af73437ee864402d4f839f1cc66797ef913c2c0c958ac5d78935604132b4d72"].into(),
			//3vtCpKyECuEAgp6KHCJ5oVtB61sDzd552LMQrzWVhVkVS1nY
			hex!["5ad0becd7e4365d54ffb9f4c33e9c3e2ed2c334a5764cd3982d2912e6879507c"].into(),
			//3vUX6P2ZJDUzwTPEiyCkHmrwotaKumwNjk1MbPXVDAzakZGd
			hex!["48c0617674b54b6cb13028702290b7b4ce01219293d75e5abafba67f4fbaed71"].unchecked_into(),
			//3vXiU7DtLyLovYc3ntuTU9q5rDUfWVGnd7FwetTKdZjQw1uU
			hex!["4b306843cddd284f1cc43091b59c6ed5f34ae25b459fd9339f86e5ffa0fd5d58"].unchecked_into(),
			//3y3rSXNJCUnu4JaEJKYVEDDMLHuAESQgDkn16vyMWiVVLmXp
			hex!["baa513265ee0662de4ee2d714195f43f755debc32a85966428d8c81c9b39d913"].unchecked_into(),
			//3uXSpr8LJipQZWbt3yFcG34bnEEVdMkS4BzoKwMrrGE3VKd2
			hex!["1ebf844a327e7afc86548eb2aa84091af6909ef0f0a4e91c9479d696c26fce02"].unchecked_into(),
			),
			(
			//3wU7kHzbLf7HZSmipKcgZR3j1g5hx5yFA8sV2vi2WnvaSb66
			hex!["74ae05ffce72637468b9dd7a0a695ed244dd588ff1633093b7332c451a7d4532"].into(),
			//3xQqSuQxpLDWs62cHZQWktX6PQ6tnsVwgXSXNpHAWEsy2JCV
			hex!["9e69a68df8c290ed1e1e617f18cbfae1ab8b73836083990cae01baf291579515"].into(),
			//3yKcXT5wr5giwtaiuAx4JQ4rpXQkXpuv3YjVMgweDmJDUNXM
			hex!["c6aa28b3557317ae1090e939ee378d633af659a0404643166abe25dfd1991e50"].unchecked_into(),
			//3zES1JrZ8Z27nHTrUkM72RLH3T82Amgp1T1gWdQjDxgF9U8f
			hex!["eef2c0c3fe53b34527bf38d2a449241eb86ccdf09f633772d1d85f0f462eb20c"].unchecked_into(),
			//3uZeDVkLFH7ULGJiD8mdo1q3SG827UuAbTp3CvLaCgWp3BrN
			hex!["206c59f4673ade5bf086fc795eca2fc3abfecc03385ecc6b9cfecbb91ca15860"].unchecked_into(),
			//3w1gMyUnetaC9oyniPqwkaGuRc8KcHZSdXLYSBD58z4Ftuxw
			hex!["608430a52b548619efcc8d742c06890b47ff94e7b186c90460163992ef814d48"].unchecked_into(),
			),
			(
			//3u88f2PPH4qcJRunWYRjabXrP8M864VsDU44z3YJQSStogi8
			hex!["0cf7b57c2340966f768549a2c6bf467c4cf2b398f53abfc278b6ffbf0078a308"].into(),
			//3wHbGntq6W3caUC58i9XF9EVbSn6YvNe1WyQJevLPQbaaiKB
			hex!["6ca6fbf4721035597ed30aa5c712821ff2f00f242898e62938676148cf77ae35"].into(),
			//3uw5NFMjN3jC9VCyyq4yYaN4S1KDpLG3ih4zLqT2LbjQjz6n
			hex!["30c52b9b2acbfd92e150976e262dbeeef0a9eda9f523688cf0de2cbbc2344035"].unchecked_into(),
			//3yKwnQFMWMJRQw3PnBgbUPrhzsdyd8f3xJH836vGBEm4Pdid
			hex!["c6eafc8786928d78fb9bdf56c2ba5cec928ec27dabaa65191f03b30dcb0abbef"].unchecked_into(),
			//3z5UYRvYxneVpgAjUCwrXCmXLn4xyqtM5gV7i3FWYHwH1p59
			hex!["e81e15502cb1e869a64c87880d23f5b59702d81db07be7ac8f539c0814f62a54"].unchecked_into(),
			//3uRb8Nf1kKRxMGpcXmk8mRh9YHo6yDoJvSXNh8ztUT2LtK16
			hex!["1a47fdf0cd3610c7fba2ea76c75542e2b89dd67e67138283dfd0163f7369d14e"].unchecked_into(),
			),
			(
			//3z5iRztoHF2STRvxVyfPb7LcvcByTUsmypNjEV9DnvrSVZdv
			hex!["e84cd69255b0abffe4f0471db6abe9033c0650da121de457ce013aa1d9d43061"].into(),
			//3zYntqkHE1fBvz1UvuUXY5AYySZvuwHbDysaHVs2s6AFVQrt
			hex!["fcf37cb9895ae00588eedf776d16ecae698ea5a8ee490809abfdfc0f17196a42"].into(),
			//3ws1EKk9qYF7fnja7bFQie8pxHHky4cmAkwKu6kXQcgWFb16
			hex!["8622bb01d93ee580de4a72663cc19ecdd7df03668d23978372adf33c65666604"].unchecked_into(),
			//3yJ8gcEWsGnwKPBEqLyCxkE4Nm5Z5kXJgHSuxWXL6V9FgXvD
			hex!["c589304e0b4b88a2a33802fa7edcd490276a88a383f56f705c3eb6ad8ab5ab56"].unchecked_into(),
			//3zBfTXvdGCZui9F6GCXR16q9z4Wyh3ijHzX9ippJVtBCcrBQ
			hex!["ecd64d007d392da79cb1e8543bf84bbc6bb49d876fc5cb8a4eed653ba50fd17b"].unchecked_into(),
			//3vZaiQ8ZXE5uazyuAS2L2csQnezb9wDqp6Hmc3iDXEB2jhcL
			hex!["4c9ccc132e5affbd464a58e668fd048268f2fae5fba75f040d4395c27bc1321a"].unchecked_into(),
			),
			(
			//3zYSoshA8hVZRgKJMpk5YA7LtpMWq9cYHKhLpjWetmYqKz3S
			hex!["fcafdf79e60a9fd535621aa3e21757568862a87cfe5f8b81e4a4ff2c80e1e01a"].into(),
			//3y4JEf1ALzxKDMeSQpMToZPPqKKcKgBuZrxeudaLFxfwpNyz
			hex!["bafbe8e6c2d3456ac77ae7621ca19106a754d54730e5e352458ea096d7c27314"].into(),
			//3vpyZka6WNCyFv6hUuLB3WJ6pWohtXSdpHUurdmRv7BBehPg
			hex!["585a66b7266a330023692013c1124ba944ca10234eb4b51443911f97604f8317"].unchecked_into(),
			//3v9mx9KavCkPEa7veBax13ooq6UoacM4Perczycs69V3djYJ
			hex!["3a74bc081fb780698041bd1ad9b8f3f499905f189a3696837def374605f92a31"].unchecked_into(),
			//3xE34pSXwFPbybTdBtWE3c2zTjchoCt26aQgbPcLwhYWX8pD
			hex!["962d109dcf4551c485f2d3e2fb6bb5275dd8d3f6c032aab41293f973d3b4945b"].unchecked_into(),
			//3vFU6v4fPuSyS9ZnDchq86mDMnz4MPomu3a7iNR4TWPNzWcb
			hex!["3ecc1f63a72020de5a87b5e3abf94bc4cb67bc0c8f46e898460d84a4a7af0a7f"].unchecked_into(),
			),
			(
			//3zFyeC9P3GBYDVkmBZEY9Fm7dNfFHa2uijiyCU7c8RDHTpu1
			hex!["f0207e94487bf3ac4c6c4ea90b40eeacdc8fc04a23ebb064d4eee4baca421d5a"].into(),
			//3zBVNZ7tBEyNGgYzVbtGzHEeoVMGr8DqvWeUwV1P7gpcRdGz
			hex!["ecb45949f250a44af62beebb2d622b5370a5a4840ed1056c89a530e5ce438a3a"].into(),
			//3xL45fRaKNABdMSQcT5u1Ar1rnNn3e7ksBGu9aEoRRHTa3ba
			hex!["9ac3f3e0f2bfeb79d679695b16ba245ebc9e26a734089445e7af7414366a3349"].unchecked_into(),
			//3wG6GvhkcCNWwaSzc1nsxUfeobxSSAV62fw8GjrP3Cz18ZYN
			hex!["6b821fb8561ff91c780ea7fc9149d6aaf950607623e73e72735d71e8b7437166"].unchecked_into(),
			//3uy7CprpTHMYVsrxsYZntDJczoYPNPLzHGgUe7keNX6fCrNJ
			hex!["3251d8ccad9edcfecfb238787c52a455f984f8cbb9f930d9fc9bbf5a0c2a6962"].unchecked_into(),
			//3y6KaLsYTVaaDfXQpXQaAFpEUpmNUWwQU8GEjZiujADwGkoZ
			hex!["bc86e8d6e9a4a4873653fe2a79fc4c17325f13afb2421915ee6c214d7c057d48"].unchecked_into(),
			),
			(
			//3x13eBHvtijDesUxcLVg3KfzRfZrpVdmzGUNjCnXPB5hD5jW
			hex!["8c44cf92487dcec87994d2a81ae979b12db0cb638e4cf2e6fccdef1661536b59"].into(),
			//3xp3Z1tACPNiEfQHhBGw8revDdS9GBcsa7rexEdAyewaJCkJ
			hex!["b01d0b934ba371f613eb8b338a8652c1df7732c2031585b733b70683551bac46"].into(),
			//3vJDetebS2Nku8cWficBT7Lgq8BJxy9cu8n5E8MXaF3M7fYd
			hex!["40e5385daca14a9af7947dc678e8077dcd270f55b54d384f574a7f948c640568"].unchecked_into(),
			//3xo7qz4tcnxBwYVgaaePGy1yS95ud8vqkWnDV9jvSR9oNoJV
			hex!["af683ff77e062e8234ed7bf53cbd8167213dc807eb0efb88e52a3747da86fd97"].unchecked_into(),
			//3xmNL73sXq9SuFFFAXGcvUivX7ohg2yaWWLeHobtLRbG1Wu4
			hex!["ae1284f4c8a19fc21b8862082876a7aec0690ae9034a0adc5b7c7f841ade4174"].unchecked_into(),
			//3ubmH4nvJvKFBniiUeLwueFTEMCkCQ9DrgkVGq1oWMszzT69
			hex!["220a9d0d16ba02f385ea1ba0f86cfc6c790463a94924317b8283765cfe7cb131"].unchecked_into(),
			),
			(
			//3vEWyFny1nbqdkSAj3iQ7tCHt1iPcaQgmLn7Fs8vF1YarymU
			hex!["3e128812bb12ddf8b613d2eca90c5b0b288fbb86d63634f159bc3bbec3c35f0e"].into(),
			//3w6WHxyDKAdt52rJM81N755W9xLLKBPfHyRLygGg3EYR9EWH
			hex!["64328578aa474d2a0ed996390444d07713dff121652cfa1d9a5703fb0ed7f44b"].into(),
			//3xDwfKeoo7exbSag89dYxhfoqp2LNp9x6PHhEidRST618SVX
			hex!["961adea3d5fb6d61ee0fce39b01c84733f4174052a58809beedb334a7b0e514a"].unchecked_into(),
			//3zXDEXS6FDiuYzGKd1RgnQ8EJEz2XgLL4NwYft8ehh14FWiA
			hex!["fbbeee18d095ecc4f2e958550c628c1deac86e569ddd9a2c1a3b06f3603ea9a6"].unchecked_into(),
			//3uaB9WvPc69ccxisjVNSNfzPjCoppTZfqs7CT3ErXdhbGSFo
			hex!["20d479c7b3d34cc18c158a652fb82129f9afda9f675b5713d563339b9d283330"].unchecked_into(),
			//3xuMvmg6BAffFstHsF7LU6eYfWNdhn8BmsC7sZj1i8tJDhj3
			hex!["b42b20d7e1cd7bfefa2848d38d50d907c43b2379371e8cdf2ce0fb291f67f220"].unchecked_into(),
			),
			(
			//3xVbx82hUtjM1PVxxbR6L6LykEdnMvGcEdJdNZUmoLAuViuo
			hex!["a20c71f895f14e0fb9d2b304c31ac28201583f7df62d84064aec807ecffb4050"].into(),
			//3x98QZVX5UzC9Z1etKpop1mK3cN1vGgRoVb3nkCrEm7JjcMj
			hex!["926ed0b2519e45be13bbe78d9921fd361064af62ca51c65c23bccb47a512fa42"].into(),
			//3ymeXRBNZ2jS4uDazz61jgviBQU1CGMQY3vtnbY4VRJKqwQR
			hex!["da85452b513892c19a19a20c63fd754c320461aecddeb0bad9f27e636d11ac1e"].unchecked_into(),
			//3y7DGTksM1sJLtuxipaJxQAaEe3z7VTzRdNikLMJTYxeQxCG
			hex!["bd34eb57d63aeff5974a21bce53517d85a4c33546c08e6f8d5cce7175e683409"].unchecked_into(),
			//3vgt1v28fVhqyuYJUBhkK89t2n8oqmwXFTxJQLSdKGduPe8g
			hex!["522dc23910bec78af11db4a2d9ee3fcddb8d97e015181e8e1676d0ef2db96647"].unchecked_into(),
			//3v1S2DiLwDsrrboubmfbko3E2dwRzVdSvjZr2bbrF2eCqPB3
			hex!["3417ae8b88288bde91b322d508918b3c0faebfe52a2670767796d94de0d34a07"].unchecked_into(),
			),
		];

	// const ENDOWMENT: u128 = 8_999 * DCU;
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
			.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
			.chain(initial_authorities.iter().map(|x| (x.1.clone(), STASH)))
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
			validator_count: 20,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
				.collect(),
			invulnerables: [].to_vec(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_democracy: Some(DemocracyConfig::default()),
		pallet_collective_Instance1: Some(CouncilConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		pallet_membership_Instance1: Some(Default::default()),
		pallet_aura: Some(Default::default()),
		pallet_grandpa: Some(Default::default()),

		pallet_im_online: Some(ImOnlineConfig { keys: vec![] }),
		pallet_authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
		pallet_treasury: Some(Default::default()),
		pallet_elections_phragmen: Some(Default::default()),
		pallet_sudo: Some(SudoConfig {
			key: endowed_accounts[0].clone(),
		}),
	}
}


// fn cord_mainnet_genesis(wasm_binary: &[u8]) -> GenesisConfig {
	// let endowed_accounts = vec![
	// ];

	// let initial_authorities: Vec<(
	// 	AccountId,
	// 	AuraId,
	// 	GrandpaId,
	// )> = vec![(
	//	)];

	// const ENDOWMENT: u128 = 8_999 * DCU;
	// const STASH: u128 = 9_000_999_000_999 * DCU;

// 	GenesisConfig {
// 		frame_system: Some(SystemConfig {
// 			code: wasm_binary.to_vec(),
// 			changes_trie_config: Default::default(),
// 		}),
// 		pallet_balances: Some(BalancesConfig {
// 			balances: endowed_accounts
// 			.iter()
// 			.map(|k: &AccountId| (k.clone(), STASH))
// 			.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
// 			.chain(initial_authorities.iter().map(|x| (x.1.clone(), STASH)))
// 			.collect(),
// 		}),
// 		pallet_indices: Some(IndicesConfig { indices: vec![] }),
// 		pallet_session: Some(SessionConfig {
// 			keys: initial_authorities
// 				.iter()
// 				.map(|x| {
// 					(
// 						x.0.clone(),
// 						x.0.clone(),
// 						session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
// 					)
// 				})
// 				.collect::<Vec<_>>(),
// 		}),
// 		pallet_staking: Some(StakingConfig {
// 			validator_count: 20,
// 			minimum_validator_count: initial_authorities.len() as u32,
// 			stakers: initial_authorities
// 				.iter()
// 				.map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
// 				.collect(),
// 			invulnerables: [].to_vec(),
// 			slash_reward_fraction: Perbill::from_percent(10),
// 			..Default::default()
// 		}),
// 		pallet_democracy: Some(DemocracyConfig::default()),
// 		pallet_collective_Instance1: Some(CouncilConfig {
// 			members: vec![],
// 			phantom: Default::default(),
// 		}),
// 		pallet_collective_Instance2: Some(TechnicalCommitteeConfig {
// 			members: vec![],
// 			phantom: Default::default(),
// 		}),
// 		pallet_membership_Instance1: Some(Default::default()),
// 		pallet_aura: Some(Default::default()),
// 		pallet_grandpa: Some(Default::default()),

// 		pallet_im_online: Some(ImOnlineConfig { keys: vec![] }),
// 		pallet_authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
// 		pallet_treasury: Some(Default::default()),
// 		pallet_elections_phragmen: Some(Default::default()),
// 		pallet_sudo: Some(SudoConfig {
// 			key: endowed_accounts[0].clone(),
// 		}),
// 	}
// }

pub fn load_spec(id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
	Ok(match Alternative::from(id) {
		Some(spec) => Box::new(spec.load()?),
		None => Box::new(ChainSpec::from_json_file(std::path::PathBuf::from(id))?),
	})
}
