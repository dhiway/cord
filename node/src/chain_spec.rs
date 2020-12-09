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
use cord_node_runtime::constants::currency::DCU;
// use hex;

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const DEFAULT_PROTOCOL_ID: &str = "cord";

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

impl Alternative {
	/// Get an actual chain config from one of the alternatives.
	pub(crate) fn load(self) -> Result<ChainSpec, String> {
		let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
		let mut properties = Properties::new();
		properties.insert("tokenSymbol".into(), "DCU".into());
		properties.insert("tokenDecimals".into(), 12.into());
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
					Some(DEFAULT_PROTOCOL_ID),
					Some(properties),
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
								authority_keys_from_seed("Charlie"),
							],
							get_account_id_from_seed::<sr25519::Public>("Alice"),
							vec![
								get_account_id_from_seed::<sr25519::Public>("Alice"),
								get_account_id_from_seed::<sr25519::Public>("Bob"),
								get_account_id_from_seed::<sr25519::Public>("Charlie"),
								get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
								get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
							],
							true,
						)
					},
					vec![],
					None,
					Some(DEFAULT_PROTOCOL_ID),
					Some(properties),
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
								authority_keys_from_seed("Charlie"),
							],
							get_account_id_from_seed::<sr25519::Public>("Alice"),
							vec![
								get_account_id_from_seed::<sr25519::Public>("Alice"),
								get_account_id_from_seed::<sr25519::Public>("Bob"),
								get_account_id_from_seed::<sr25519::Public>("Charlie"),
								get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
								get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
							],
							true,
						)
					},
					vec![],
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
	const ENDOWMENT: u128 = 1_000_000 * DCU;

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k|(k, ENDOWMENT)).collect(),
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
