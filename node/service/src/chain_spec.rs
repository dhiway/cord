// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! CORD chain configurations.

pub use cord_primitives::{AccountId, Balance, Signature};
pub use cord_runtime::GenesisConfig;
use cord_runtime::{
	AuthorityDiscoveryConfig, AuthorityMembershipConfig, BabeConfig, BalancesConfig, Block,
	CouncilMembershipConfig, IndicesConfig, NetworkMembershipConfig, SessionConfig, SessionKeys,
	SudoConfig, SystemConfig, TechnicalMembershipConfig,
};
use network_membership::MemberData;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::ChainSpecExtension;
use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sc_service::{ChainType, Properties};
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
type AccountPublic = <Signature as Verify>::Signer;

pub use cord_runtime_constants::{currency::*, time::*};

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
	/// The light sync state extension used by the sync-state rpc.
	pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

/// Specialized `ChainSpec`.
pub type CordChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

fn session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys { babe, grandpa, im_online, authority_discovery }
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
) -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId) {
	let keys = get_authority_keys(seed);
	(keys.0, keys.1, keys.2, keys.3, keys.4, keys.5)
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys(
	seed: &str,
) -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId, AuthorityDiscoveryId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<BabeId>(seed),
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
	]
}

fn member_accounts() -> Vec<AccountId> {
	vec![
		(get_account_id_from_seed::<sr25519::Public>("Alice")),
		(get_account_id_from_seed::<sr25519::Public>("Bob")),
		(get_account_id_from_seed::<sr25519::Public>("Charlie")),
	]
}

/// Development config.
fn cord_dev_config_genesis(wasm_binary: &[u8]) -> cord_runtime::GenesisConfig {
	cord_local_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

fn cord_local_config_genesis(wasm_binary: &[u8]) -> cord_runtime::GenesisConfig {
	cord_local_genesis(
		wasm_binary,
		vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
			get_authority_keys_from_seed("Charlie"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

pub fn cord_dev_config() -> Result<CordChainSpec, String> {
	let wasm_binary = cord_runtime::WASM_BINARY.ok_or("CORD development wasm not available")?;
	let properties = get_properties("WAY", 12, 29);
	Ok(CordChainSpec::from_genesis(
		"Cord Ignite",
		"cord_dev",
		ChainType::Development,
		move || cord_dev_config_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		None,
		Some(properties),
		Default::default(),
	))
}

pub fn cord_local_config() -> Result<CordChainSpec, String> {
	let wasm_binary = cord_runtime::WASM_BINARY.ok_or("CORD development wasm not available")?;
	let properties = get_properties("WAY", 12, 29);
	Ok(CordChainSpec::from_genesis(
		"Cord Spin",
		"local",
		ChainType::Local,
		move || cord_local_config_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		None,
		Some(properties),
		Default::default(),
	))
}

// pub fn cord_config() -> Result<CordChainSpec, String> {
// CordChainSpec::from_json_bytes(&include_bytes!("../chain-specs/cord.json")[..
// ]) }

pub fn cord_staging_config() -> Result<CordChainSpec, String> {
	CordChainSpec::from_json_bytes(&include_bytes!("../../chain-specs/sprint.json")[..])
}

pub fn cord_builder_config() -> Result<CordChainSpec, String> {
	CordChainSpec::from_json_bytes(&include_bytes!("../../chain-specs/spark.json")[..])
}

fn cord_local_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> GenesisConfig {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);
	// let member_candidates = member_accounts()
	// .into_iter()
	// .map(|account_id| (account_id, MemberData { expire_on: 0 }))
	// .collect::<BTreeMap<AccountId, MemberData<BlockNumber>>>();

	// let members = member_accounts()
	// 	.into_iter()
	// 	.map(|account_id| {
	// 		let member_data = ;
	// 		(account_id, member_data)
	// 	})
	// .collect::<BTreeMap<AccountId, MemberData<BlockNumber>>>();

	let num_endowed_accounts = endowed_accounts.len();
	const ENDOWMENT: u128 = 50_000 * WAY;

	GenesisConfig {
		system: SystemConfig { code: wasm_binary.to_vec() },
		balances: BalancesConfig {
			balances: endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect(),
		},
		indices: IndicesConfig { indices: vec![] },
		network_membership: NetworkMembershipConfig {
			members: member_accounts()
				.into_iter()
				.map(|member| (member, MemberData { expire_on: 0 }))
				.collect(),
		},
		authority_membership: AuthorityMembershipConfig {
			initial_authorities: initial_authorities
				.iter()
				.map(|x| x.0.clone())
				.collect::<Vec<_>>(),
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
		babe: BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(cord_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		council: Default::default(),
		council_membership: CouncilMembershipConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect::<Vec<_>>()
				.try_into()
				.unwrap_or_else(|e| panic!("Failed to add council memebers: {:?}", e)),
			phantom: Default::default(),
		},
		technical_committee: Default::default(),
		technical_membership: TechnicalMembershipConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect::<Vec<_>>()
				.try_into()
				.unwrap_or_else(|e| panic!("Failed to add committee members: {:?}", e)),
			phantom: Default::default(),
		},
		authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
		sudo: SudoConfig { key: Some(root_key) },
	}
}
