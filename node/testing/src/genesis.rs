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
use cord_loom_runtime::{
	AccountId, AssetsConfig, AuthorityMembershipConfig, BalancesConfig, IndicesConfig,
	NetworkMembershipConfig, NetworkParametersConfig, NodeAuthorizationConfig,
	RuntimeGenesisConfig, SessionConfig,
};
use cord_loom_runtime_constants::currency::*;
use sp_keyring::{Ed25519Keyring, Sr25519Keyring};
use sp_std::collections::btree_map::BTreeMap;

/// Create genesis runtime configuration for tests.
pub fn config() -> RuntimeGenesisConfig {
	config_endowed(Default::default())
}

/// Create genesis runtime configuration for tests with some extra
/// endowed accounts.
pub fn config_endowed(extra_endowed: Vec<AccountId>) -> RuntimeGenesisConfig {
	let mut endowed = vec![
		(alice(), 111 * UNITS),
		(bob(), 100 * UNITS),
		(charlie(), 100_000_000 * UNITS),
		(dave(), 111 * UNITS),
		(eve(), 101 * UNITS),
		(ferdie(), 100 * UNITS),
	];

	endowed.extend(extra_endowed.into_iter().map(|endowed| (endowed, 100 * UNITS)));

	let members = vec![alice(), bob(), charlie()];

	RuntimeGenesisConfig {
		indices: IndicesConfig { indices: vec![] },
		balances: BalancesConfig { balances: endowed },
		network_parameters: NetworkParametersConfig {
			permissioned: true,
			_marker: Default::default(),
		},
		node_authorization: NodeAuthorizationConfig {
			nodes: vec![
				(b"12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2".to_vec(), alice()),
				(b"12D3KooWQYV9dGMFoRzNStwpXztXaBUjtPqi6aU76ZgUriHhKust".to_vec(), bob()),
				(b"12D3KooWJvyP3VJYymTqG7eH4PM5rN4T2agk5cdNCfNymAqwqcvZ".to_vec(), charlie()),
				(b"12D3KooWPHWFrfaJzxPnqnAYAoRUyAHHKqACmEycGTVmeVhQYuZN".to_vec(), dave()),
			],
		},
		network_membership: NetworkMembershipConfig {
			members: members
				.iter()
				.map(|member| (member.clone(), false))
				.collect::<BTreeMap<_, _>>(),
		},
		authority_membership: AuthorityMembershipConfig { initial_authorities: members },
		session: SessionConfig {
			keys: vec![
				(alice(), dave(), to_session_keys(&Ed25519Keyring::Alice, &Sr25519Keyring::Alice)),
				(bob(), eve(), to_session_keys(&Ed25519Keyring::Bob, &Sr25519Keyring::Bob)),
				(
					charlie(),
					ferdie(),
					to_session_keys(&Ed25519Keyring::Charlie, &Sr25519Keyring::Charlie),
				),
			],
		},
		assets: AssetsConfig { assets: vec![(9, alice(), true, 1)], ..Default::default() },
		..Default::default()
	}
}
