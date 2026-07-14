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

//! Genesis config presets for the Orbis enterprise runtime

use crate::{
	AccountId, Balance, BalancesConfig, CollatorSelectionConfig, DotnsConfig, ExistentialDeposit,
	ParaId, ParachainInfoConfig, Revive, SessionConfig, SessionKeys, TokenConfig,
};
use alloc::{vec, vec::Vec};
use origin_hub_system_runtime_constants::genesis_presets::*;
use origin_runtime_constants::system_parachain::ORBIS_ID;
use sp_core::sr25519;
use sp_genesis_builder::PresetId;

const ORBIS_STAGING_ED: Balance = ExistentialDeposit::get();
/// Default para-id used when no explicit override is supplied.
const DEFAULT_ORBIS_PARA_ID: u32 = ORBIS_ID;

fn orbis_genesis(
	invulnerables: Vec<(AccountId, parachains_common::AuraId)>,
	endowed_accounts: Vec<AccountId>,
	feeless_accounts: Vec<AccountId>,
	id: ParaId,
	token_network_id: u32,
	root_key: AccountId,
) -> serde_json::Value {
	let endowment = ORBIS_STAGING_ED.saturating_mul(4096 * 4096);
	debug_assert!(
		token_network_id <= u16::MAX as u32,
		"token_network_id {} does not fit into u16",
		token_network_id
	);
	// let development_accounts: Vec<AccountId> = endowed_accounts.clone();
	let mut balances: Vec<(AccountId, Balance)> =
		endowed_accounts.iter().cloned().map(|account| (account, endowment)).collect();
	let revive_account = Revive::account_id();
	let dotns_registrar = root_key.clone();
	let dotns_root_reservations = [b"origin".as_slice(), b"orbis", b"system"]
		.into_iter()
		.map(|label| (label.to_vec().try_into().expect("bootstrap DotNS label is bounded"), None))
		.collect();
	if !balances.iter().any(|(account, _)| account == &revive_account) {
		// Code-upload deposits are held on this account. It must exist before the first upload
		// because `transfer_and_hold` cannot create a destination whose entire balance is held.
		balances.push((revive_account, ORBIS_STAGING_ED));
	}

	serde_json::json!({
		"balances": BalancesConfig {
			balances,
			dev_accounts: None,
		},
		"parachainInfo": ParachainInfoConfig {
			parachain_id: id,
			..Default::default()
		},
		"token": TokenConfig {
			network_id: token_network_id as u16,
			..Default::default()
		},
		"dotns": DotnsConfig {
			registrars: vec![dotns_registrar],
			root_reservations: dotns_root_reservations,
		},
		"collatorSelection": CollatorSelectionConfig {
			invulnerables: invulnerables
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect(),
			candidacy_bond: 0,
			..Default::default()
		},
		"session": SessionConfig {
			keys: invulnerables
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc.clone(),
						acc,
						SessionKeys { aura },
					)
				})
				.collect(),
			..Default::default()
		},
		"feeless": {
			"feelessAccounts": feeless_accounts
				.iter()
				.cloned()
				.collect::<Vec<_>>(),
		},
		"sudo": {
			"key": Some(root_key),
		},
		"polkadotXcm": {
			"safeXcmVersion": Some(SAFE_XCM_VERSION),
		},
	})
}

pub fn orbis_local_testnet_genesis(para_id: ParaId) -> serde_json::Value {
	let root_key = get_account_id_from_seed::<sr25519::Public>("Alice");
	let endowed_accounts = testnet_accounts();

	orbis_genesis(
		invulnerables(),
		endowed_accounts.clone(),
		endowed_accounts,
		para_id,
		para_id.into(),
		root_key,
	)
}

pub fn orbis_development_genesis(para_id: ParaId) -> serde_json::Value {
	orbis_local_testnet_genesis(para_id)
}

/// Build a clean production genesis from explicitly supplied launch identities.
///
/// Unlike the development presets this function never derives Alice/Bob keys and never grants
/// feeless status to every endowed account. Launch tooling must provide the reviewed collator,
/// endowment, feeless and root-key sets explicitly.
pub fn orbis_production_genesis(
	invulnerables: Vec<(AccountId, parachains_common::AuraId)>,
	endowed_accounts: Vec<AccountId>,
	feeless_accounts: Vec<AccountId>,
	para_id: ParaId,
	token_network_id: u32,
	root_key: AccountId,
) -> serde_json::Value {
	orbis_genesis(
		invulnerables,
		endowed_accounts,
		feeless_accounts,
		para_id,
		token_network_id,
		root_key,
	)
}

/// Provides the names of the predefined genesis configs for this runtime.
pub fn preset_names() -> Vec<PresetId> {
	vec![
		PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
		PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
	]
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
	let patch = match id.as_ref() {
		sp_genesis_builder::DEV_RUNTIME_PRESET => {
			orbis_development_genesis(DEFAULT_ORBIS_PARA_ID.into())
		},
		sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => {
			orbis_local_testnet_genesis(DEFAULT_ORBIS_PARA_ID.into())
		},
		_ => return None,
	};
	Some(
		serde_json::to_string(&patch)
			.expect("serialization to json is expected to work. qed.")
			.into_bytes(),
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::traits::Get;

	#[test]
	fn default_orbis_preset_is_sudo_only_and_non_staking() {
		let genesis = orbis_development_genesis(ORBIS_ID.into());

		assert_eq!(
			genesis.pointer("/parachainInfo/parachainId").and_then(|value| value.as_u64()),
			Some(ORBIS_ID as u64)
		);
		assert_eq!(
			genesis
				.pointer("/collatorSelection/candidacyBond")
				.and_then(|value| value.as_u64()),
			Some(0)
		);
		assert!(genesis.get("sudo").is_some());
		assert!(genesis.get("staking").is_none());
		assert!(genesis.get("referenda").is_none());
		assert_eq!(
			<<crate::Runtime as pallet_collator_selection::Config>::MaxCandidates as Get<u32>>::get(
			),
			0,
			"permissionless collator candidacy must remain disabled"
		);
		let revive_account = serde_json::to_value(Revive::account_id()).unwrap();
		assert!(
			genesis
				.pointer("/balances/balances")
				.and_then(|value| value.as_array())
				.expect("balances are present")
				.iter()
				.any(|entry| entry.get(0) == Some(&revive_account)),
			"the Revive code-deposit account must exist at genesis"
		);
	}

	#[test]
	fn production_genesis_uses_only_explicit_launch_identities() {
		let root = AccountId::new([0x41; 32]);
		let collator = AccountId::new([0x42; 32]);
		let aura = parachains_common::AuraId::from(sp_core::sr25519::Public::from_raw([0x52; 32]));
		let genesis = orbis_production_genesis(
			vec![(collator.clone(), aura)],
			vec![root.clone(), collator.clone()],
			vec![],
			ORBIS_ID.into(),
			ORBIS_ID,
			root.clone(),
		);

		assert_eq!(genesis.pointer("/sudo/key"), Some(&serde_json::to_value(root).unwrap()));
		assert!(genesis
			.pointer("/feeless/feelessAccounts")
			.and_then(|value| value.as_array())
			.is_some_and(Vec::is_empty));
		assert_eq!(
			genesis.pointer("/collatorSelection/invulnerables/0").cloned(),
			Some(serde_json::to_value(collator).unwrap())
		);
		assert_eq!(
			genesis.pointer("/dotns/registrars/0"),
			genesis.pointer("/sudo/key"),
			"the governed root is the only bootstrap registrar"
		);
		let reservations = genesis
			.pointer("/dotns/rootReservations")
			.and_then(serde_json::Value::as_array)
			.expect("root reservations are explicit");
		assert_eq!(reservations.len(), 3);
		assert_eq!(
			genesis
				.as_object()
				.expect("genesis patch is an object")
				.keys()
				.cloned()
				.collect::<alloc::collections::BTreeSet<_>>(),
			[
				"balances",
				"collatorSelection",
				"dotns",
				"feeless",
				"parachainInfo",
				"polkadotXcm",
				"session",
				"sudo",
				"token",
			]
			.into_iter()
			.map(str::to_owned)
			.collect(),
			"all other native domains start from their empty/default genesis state"
		);
	}
}
