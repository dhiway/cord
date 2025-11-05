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

//! Genesis Configuration.

use crate::keyring::*;
use cord_orb_runtime::{
	AccountId, AuthorityManagerConfig, BalancesConfig, IndicesConfig, RuntimeGenesisConfig,
	SessionConfig, TokenConfig,
};
use cord_orb_runtime_constants::currency::*;
use sp_keyring::Ed25519Keyring;

/// Create genesis runtime configuration for tests.
pub fn config() -> RuntimeGenesisConfig {
	config_endowed(Default::default())
}

/// Create genesis runtime configuration for tests with some extra
/// endowed accounts.
pub fn config_endowed(extra_endowed: Vec<AccountId>) -> RuntimeGenesisConfig {
	let initial_authorities: Vec<AccountId> =
		vec![alice(), bob(), charlie(), dave(), eve(), ferdie()];

	let mut endowed = vec![
		(alice(), 111 * UNITS),
		(bob(), 100 * UNITS),
		(charlie(), 100_000_000 * UNITS),
		(dave(), 112 * UNITS),
		(eve(), 101 * UNITS),
		(ferdie(), 101 * UNITS),
	];

	endowed.extend(extra_endowed.into_iter().map(|endowed| (endowed, 100 * UNITS)));

	RuntimeGenesisConfig {
		indices: IndicesConfig { indices: vec![] },
		balances: BalancesConfig { balances: endowed, ..Default::default() },
		token: TokenConfig {
			protocol_id: "c0rd".to_string(),
			network_id: 100,
			..Default::default()
		},
		authority_manager: AuthorityManagerConfig {
			initial_authorities: initial_authorities.clone(),
			..Default::default()
		},
		session: SessionConfig {
			keys: vec![
				(alice(), dave(), session_keys_from_seed(Ed25519Keyring::Alice.into())),
				(bob(), eve(), session_keys_from_seed(Ed25519Keyring::Bob.into())),
				(charlie(), ferdie(), session_keys_from_seed(Ed25519Keyring::Charlie.into())),
			],
			..Default::default()
		},
		..Default::default()
	}
}
