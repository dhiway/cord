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

//! Genesis config presets for the Origin System runtime

use crate::*;
use origin_hub_system_runtime_constants::genesis_presets::*;
use origin_runtime_constants::system_parachain::ORIGIN_HUB_IN_ID;
use sp_genesis_builder::PresetId;

const SYSTEM_ORIGIN_STAGING_ED: Balance = ExistentialDeposit::get();
/// Default para-id used when no explicit override is supplied.
const DEFAULT_SYSTEM_PARA_ID: u32 = ORIGIN_HUB_IN_ID;

fn system_origin_staging_genesis(
	invulnerables: Vec<(AccountId, parachains_common::AuraId)>,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
	token_network_id: u32,
) -> serde_json::Value {
	serde_json::json!({
		"balances": BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, SYSTEM_ORIGIN_STAGING_ED * 4096 * 4096))
				.collect(),
			dev_accounts: None,
		},
		"parachainInfo": ParachainInfoConfig {
			parachain_id: id,
			..Default::default()
		},
		"token": TokenConfig { network_id: token_network_id as u16, ..Default::default()},
		"collatorSelection": CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: SYSTEM_ORIGIN_STAGING_ED * 16,
			..Default::default()
		},
		"session": SessionConfig {
			keys: invulnerables
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc.clone(),                         // account id
						acc,                                 // validator id
						SessionKeys { aura }, 		// session keys
					)
				})
				.collect(),
			..Default::default()
		},
		"polkadotXcm": {
			"safeXcmVersion": Some(SAFE_XCM_VERSION),
		},
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this. `aura: Default::default()`
	})
}

pub fn system_origin_local_testnet_genesis(para_id: ParaId) -> serde_json::Value {
	system_origin_staging_genesis(invulnerables(), testnet_accounts(), para_id, para_id.into())
}

pub fn system_origin_development_genesis(para_id: ParaId) -> serde_json::Value {
	system_origin_local_testnet_genesis(para_id)
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
			system_origin_development_genesis(DEFAULT_SYSTEM_PARA_ID.into())
		},
		sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => {
			system_origin_local_testnet_genesis(DEFAULT_SYSTEM_PARA_ID.into())
		},
		_ => return None,
	};
	Some(
		serde_json::to_string(&patch)
			.expect("serialization to json is expected to work. qed.")
			.into_bytes(),
	)
}
