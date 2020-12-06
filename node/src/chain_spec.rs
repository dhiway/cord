// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019  BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

//! KILT chain specification

use cord_node_runtime::{
	AccountId, AuraConfig, BalancesConfig, GenesisConfig, GrandpaConfig, 
	SessionConfig, opaque::SessionKeys, Signature, SudoConfig, SystemConfig,
	WASM_BINARY,
};

use sc_service::{self, ChainType, Properties};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::{IdentifyAccount, Verify};

use hex;

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

type AccountPublic = <Signature as Verify>::Signer;

/// Specialised `ChainSpec`. This is a specialisation of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// Whatever the current runtime is, with simple Alice/Bob auths.
	CordTestnet,
	CordDevnet,
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

fn session_keys(
	aura: AuraId,
	grandpa: GrandpaId,
) -> SessionKeys {
	SessionKeys { aura, grandpa }
}

/// Helper function to generate an authority key for Aura
pub fn authority_keys_from_seed(s: &str) -> (AccountId, AuraId, GrandpaId) {
	(
		// get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", s)),
		get_account_id_from_seed::<sr25519::Public>(s),
		get_from_seed::<AuraId>(s),
		get_from_seed::<GrandpaId>(s),
	)
}


/// Build a pair of public keys from a given hex string. This method will panic if the hex string is malformed.
///
/// public_key – the public key formatted as a hex string
// fn as_authority_key(public_key: [u8; 32]) -> (AccountId, AuraId, GrandpaId) {
// 	(
// 		public_key.into(),
// 		public_key.unchecked_into(),
// 		public_key.unchecked_into(),
// 	)
// }

// const TEST_AUTH_ALICE: [u8; 32] =
// 	hex!("58d3bb9e9dd245f3dec8d8fab7b97578c00a10cf3ca9d224caaa46456f91c46c");
// const TEST_AUTH_BOB: [u8; 32] =
// 	hex!("d660b4470a954ecc99496d4e4b012ee9acac3979e403967ef09de20da9bdeb28");
// const TEST_AUTH_CHARLIE: [u8; 32] =
// 	hex!("2ecb6a4ce4d9bc0faab70441f20603fcd443d6d866e97c9e238a2fb3e982ae2f");
// const TEST_FAUCET: [u8; 32] =
// 	hex!("3cd78d9e468030ac8eff5b5d2b40e35aa9db01a9e48997e61f97f0da8c572411");

// const DEV_AUTH_ALICE: [u8; 32] =
// 	hex!("d44da634611d9c26837e3b5114a7d460a4cb7d688119739000632ed2d3794ae9");
// const DEV_AUTH_BOB: [u8; 32] =
// 	hex!("06815321f16a5ae0fe246ee19285f8d8858fe60d5c025e060922153fcf8e54f9");
// const DEV_AUTH_CHARLIE: [u8; 32] =
// 	hex!("6d2d775fdc628134e3613a766459ccc57a29fd380cd410c91c6c79bc9c03b344");
// const DEV_FAUCET: [u8; 32] =
// 	hex!("2c9e9c40e15a2767e2d04dc1f05d824dd76d1d37bada3d7bb1d40eca29f3a4ff");

impl Alternative {
	/// Get an actual chain config from one of the alternatives.
	pub(crate) fn load(self) -> Result<ChainSpec, String> {
		let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
		// let mut properties = Properties::new();
		// properties.insert("tokenSymbol".into(), "CRD".into());
		// properties.insert("tokenDecimals".into(), 18.into());
		Ok(match self {
			Alternative::Development => {
				ChainSpec::from_genesis(
					"Development",
					"development",
					ChainType::Development,
					move || {
						testnet_genesis(
							wasm_binary,
							vec![
								authority_keys_from_seed("Alice"),
							],
							get_account_id_from_seed::<sr25519::Public>("Alice"),
							vec![
								get_account_id_from_seed::<sr25519::Public>("Alice"),
								get_account_id_from_seed::<sr25519::Public>("Bob"),
								get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
								get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
							],
							true,
						)
					},
					vec![],
					None,
					None,
					None, //Some(properties),
					None,
				)
			}
			Alternative::CordTestnet => {
				ChainSpec::from_genesis(
					"CORD Testnet",
					"cord_testnet",
					ChainType::Live,
					move || {
						testnet_genesis(
							wasm_binary,
							vec![
								authority_keys_from_seed("Alice"),
								authority_keys_from_seed("Bob"),
							],
							get_account_id_from_seed::<sr25519::Public>("Alice"),
							vec![
								get_account_id_from_seed::<sr25519::Public>("Alice"),
								get_account_id_from_seed::<sr25519::Public>("Bob"),
								get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
								get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
							],
							true,
						)
					},
					vec![],
					None,
					None,
					None, //Some(properties),
					None,
				)
			}
			Alternative::CordDevnet => {
				ChainSpec::from_genesis(
					"CORD Devnet",
					"cord_devnet",
					ChainType::Live,
					move || {
						testnet_genesis(
							wasm_binary,
							// Initial Authorities
							vec![
								authority_keys_from_seed("Alice"),
								authority_keys_from_seed("Bob"),
							],
							get_account_id_from_seed::<sr25519::Public>("Alice"),
							vec![
								get_account_id_from_seed::<sr25519::Public>("Alice"),
								get_account_id_from_seed::<sr25519::Public>("Bob"),
								get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
								get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
							],
							true,
						)
					},
					vec![],
					None,
					None,
					None,// Some(properties),
					None,
				)
			}
			// Alternative::KiltTestnet => {
			// 	ChainSpec::from_genesis(
			// 		"KILT Testnet",
			// 		"kilt_testnet",
			// 		ChainType::Live,
			// 		move || {
			// 			testnet_genesis(
			// 				wasm_binary,
			// 				vec![
			// 					as_authority_key(TEST_AUTH_ALICE),
			// 					as_authority_key(TEST_AUTH_BOB),
			// 					as_authority_key(TEST_AUTH_CHARLIE),
			// 				],
			// 				TEST_AUTH_ALICE.into(),
			// 				vec![
			// 					// Testnet Faucet accounts
			// 					TEST_FAUCET.into(),
			// 					TEST_AUTH_ALICE.into(),
			// 					TEST_AUTH_BOB.into(),
			// 					TEST_AUTH_CHARLIE.into(),
			// 				],
			// 			)
			// 		},
			// 		vec![],
			// 		None,
			// 		None,
			// 		None, // Some(properties),
			// 		None,
			// 	)
			// }
			// Alternative::KiltDevnet => {
			// 	ChainSpec::from_genesis(
			// 		"KILT Devnet",
			// 		"kilt_devnet",
			// 		ChainType::Live,
			// 		move || {
			// 			testnet_genesis(
			// 				wasm_binary,
			// 				// Initial Authorities
			// 				vec![
			// 					as_authority_key(DEV_AUTH_ALICE),
			// 					as_authority_key(DEV_AUTH_BOB),
			// 					as_authority_key(DEV_AUTH_CHARLIE),
			// 				],
			// 				DEV_AUTH_ALICE.into(),
			// 				vec![
			// 					DEV_FAUCET.into(),
			// 					DEV_AUTH_ALICE.into(),
			// 					DEV_AUTH_BOB.into(),
			// 					DEV_AUTH_CHARLIE.into(),
			// 				],
			// 			)
			// 		},
			// 		vec![],
			// 		None,
			// 		None,
			// 		None, //Some(properties),
			// 		None,
			// 	)
			// }
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(Alternative::Development),
			"cord-testnet" => Some(Alternative::CordTestnet),
			"cord-devnet" => Some(Alternative::CordDevnet),
			_ => None,
		}
	}
}

fn testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AuraId, GrandpaId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	_enable_println: bool,
) -> GenesisConfig {
	const INITIAL_BALANCE: u128 = 1_000_000_000_000_000;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k|(k, INITIAL_BALANCE)).collect(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities.iter().map(|x| {
				(x.0.clone(), x.0.clone(), session_keys(x.1.clone(), x.2.clone()))
			}).collect::<Vec<_>>(),
		}),
		pallet_aura: Some(AuraConfig {
			authorities: vec![],
		}),
		pallet_grandpa: Some(GrandpaConfig {
			authorities: vec![],
		}),
		pallet_sudo: Some(SudoConfig {
			key: root_key,
		}),
	}
}

pub fn load_spec(id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
	Ok(match Alternative::from(id) {
		Some(spec) => Box::new(spec.load()?),
		None => Box::new(ChainSpec::from_json_file(std::path::PathBuf::from(id))?),
	})
}
