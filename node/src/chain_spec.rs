// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! CORD chain configurations.

pub use cord_primitives::{AccountId, Balance, Signature};
use cord_runtime::constants::currency::*;
use cord_runtime::Block;
pub use cord_runtime::GenesisConfig;
use cord_runtime::{
	wasm_binary_unwrap, AuthorityDiscoveryConfig, BabeConfig, BalancesConfig, CouncilConfig, DemocracyConfig,
	ElectionsConfig, GrandpaConfig, ImOnlineConfig, IndicesConfig, SessionConfig, SessionKeys, StakerStatus,
	StakingConfig, SudoConfig, SystemConfig, TechnicalCommitteeConfig, MAX_NOMINATIONS,
};
use hex_literal::hex;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::ChainSpecExtension;
use sc_service::{ChainType, Properties};
// use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	Perbill,
};

type AccountPublic = <Signature as Verify>::Signer;

pub use cord_runtime::constants::time::*;

// Note this is the URL for the telemetry server
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
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
}

/// Specialized `ChainSpec`.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

fn session_keys(
	grandpa: GrandpaId,
	babe: BabeId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys {
		grandpa,
		babe,
		im_online,
		authority_discovery,
	}
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

fn amber_glow_genesis_config() -> GenesisConfig {
	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		GrandpaId,
		BabeId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)> = vec![(
		//3wF3nbuyb97oSkVBSgZe9dpYcFw5dypX8SPhBWrUcCpZxBWW
		hex!["6ab68082628ad0cfab68b1a00377170ff0dea4da06030cdd0c21a364ecbbc23b"].into(),
		//3yzE5N1DMjibaSesw1hAZ8wwvPJnxM3RzvQFanitVm4rkC8h
		hex!["e41d2833b0b2f629e52a1bc1ace3079c395673bab26a14626b52c132b1fb5f1c"].into(),
		//3xaQXFoMVNgQ2qMCXHazaEiQ4bzWfVX3TowLc1DHMB1sL4nx
		hex!["a5b6331fcff809f2b3419332678fd7b23a2a9320240ec36652337fe66a7337e0"].unchecked_into(),
		//3xuztVAW9ftgcU5FNc3dEXsEgrZW1AnbGWqWmeKKxpGnM4H2
		hex!["b4a78c7de7cc60ed9a99029fcf487f40a3c4b5d5d78a7080387507a680ecb75e"].unchecked_into(),
		//3xE2yQSUQ9hfeX1kZjP1Dg5hoU2EdLc1B9zFjzEcc5fgax2W
		hex!["962cc02d5dddbb2fc03bd8d511844ec47e798b3bc20d9daf7400b3d09533d518"].unchecked_into(),
		//3vL3vWTS2FZ9JDc4SyMFXQRa5TuitFBfSx8ZrygeEMzc7HkV
		hex!["424af4547d488e65307cb14ffae20257b6e000658913f985824da5629afff13c"].unchecked_into(),
	)];

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
	let root_key: AccountId = endowed_accounts[0].clone();
	let num_endowed_accounts = endowed_accounts.len();

	amber_glow_genesis(
		initial_authorities,
		vec![],
		root_key,
		Some(endowed_accounts),
		num_endowed_accounts,
	)
}

/// Staging testnet config.
pub fn amber_glow_config() -> ChainSpec {
	let boot_nodes = vec![];
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "WAY".into());
	properties.insert("tokenDecimals".into(), 12.into());
	ChainSpec::from_genesis(
		"Amber Glow",
		"DevNode",
		ChainType::Local,
		amber_glow_genesis_config,
		boot_nodes,
		None,
		Some(DEFAULT_PROTOCOL_ID),
		Some(properties),
		Default::default(),
	)
}

fn amber_glow_genesis(
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		GrandpaId,
		BabeId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)>,
	initial_nominators: Vec<AccountId>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
	num_endowed_accounts: usize,
) -> GenesisConfig {
	let mut endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(|| {
		vec![
			get_account_id_from_seed::<sr25519::Public>("Ashok"),
			get_account_id_from_seed::<sr25519::Public>("Radha"),
		]
	});
	// endow all authorities and nominators.
	initial_authorities
		.iter()
		.map(|x| &x.0)
		.chain(initial_nominators.iter())
		.for_each(|x| {
			if !endowed_accounts.contains(&x) {
				endowed_accounts.push(x.clone())
			}
		});

	// stakers: all validators and nominators.
	let mut rng = rand::thread_rng();
	let stakers = initial_authorities
		.iter()
		.map(|x| (x.0.clone(), x.1.clone(), CONTROLLER_ENDOWMENT, StakerStatus::Validator))
		.chain(initial_nominators.iter().map(|x| {
			use rand::{seq::SliceRandom, Rng};
			let limit = (MAX_NOMINATIONS as usize).min(initial_authorities.len());
			let count = rng.gen::<usize>() % limit;
			let nominations = initial_authorities
				.as_slice()
				.choose_multiple(&mut rng, count)
				.into_iter()
				.map(|choice| choice.0.clone())
				.collect::<Vec<_>>();
			(
				x.clone(),
				x.clone(),
				CONTROLLER_ENDOWMENT,
				StakerStatus::Nominator(nominations),
			)
		}))
		.collect::<Vec<_>>();

	const CONTROLLER_ENDOWMENT: u128 = 1_100 * WAY;
	const ENDOWMENT: u128 = 11_100 * WAY;
	const CORD_STASH: u128 = 1_110_101_200 * WAY;

	GenesisConfig {
		system: SystemConfig {
			code: wasm_binary_unwrap().to_vec(),
			changes_trie_config: Default::default(),
		},
		balances: BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k| (k, CORD_STASH)).collect(),
		},
		indices: IndicesConfig { indices: vec![] },
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
		staking: StakingConfig {
			validator_count: initial_authorities.len() as u32,
			minimum_validator_count: initial_authorities.len() as u32,
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			stakers,
			..Default::default()
		},
		democracy: DemocracyConfig::default(),
		elections: ElectionsConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.map(|member| (member, ENDOWMENT))
				.collect(),
		},
		council: CouncilConfig::default(),
		technical_committee: TechnicalCommitteeConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			phantom: Default::default(),
		},
		sudo: SudoConfig { key: root_key },
		babe: BabeConfig {
			authorities: vec![],
			epoch_config: Some(cord_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		im_online: ImOnlineConfig { keys: vec![] },
		authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
		grandpa: GrandpaConfig { authorities: vec![] },
		technical_membership: Default::default(),
		treasury: Default::default(),
		vesting: Default::default(),
		transaction_storage: Default::default(),
	}
}