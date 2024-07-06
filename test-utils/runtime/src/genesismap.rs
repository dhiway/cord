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

//! Tool for creating the genesis block.

use super::{
	cord_test_pallet, currency, wasm_binary_unwrap, AccountId, Balance, RuntimeGenesisConfig,
};
use codec::Encode;
use sc_service::construct_genesis_block;
use sp_core::{
	sr25519,
	storage::{well_known_keys, StateVersion, Storage},
	Pair,
};
use sp_keyring::{AccountKeyring, Sr25519Keyring};
use sp_runtime::{
	traits::{Block as BlockT, Hash as HashT, Header as HeaderT},
	BuildStorage,
};

/// Builder for generating storage from cord-test-runtime genesis config.
///
/// Default storage can be extended with additional key-value pairs.
pub struct GenesisStorageBuilder {
	/// Authorities accounts used by any component requiring an authority set
	/// (e.g. babe).
	authorities: Vec<AccountId>,
	/// Accounts to be endowed with some funds.
	balances: Vec<(AccountId, u64)>,
	/// Override default number of heap pages.
	heap_pages_override: Option<u64>,
	/// Additional storage key pairs that will be added to the genesis map.
	extra_storage: Storage,
	/// Optional wasm code override.
	wasm_code: Option<Vec<u8>>,
}

impl Default for GenesisStorageBuilder {
	/// Creates a builder with default settings for `cord_test_runtime`.
	fn default() -> Self {
		Self::new(
			vec![
				Sr25519Keyring::Alice.into(),
				Sr25519Keyring::Bob.into(),
				Sr25519Keyring::Charlie.into(),
			],
			(0..16_usize)
				.into_iter()
				.map(|i| AccountKeyring::numeric(i).public())
				.chain(vec![
					AccountKeyring::Alice.into(),
					AccountKeyring::Bob.into(),
					AccountKeyring::Charlie.into(),
				])
				.collect(),
			1000 * currency::UNITS,
		)
	}
}

impl GenesisStorageBuilder {
	/// Creates a storage builder for genesis config. `substrage test runtime`
	/// [`RuntimeGenesisConfig`] is initialized with provided `authorities`,
	/// `endowed_accounts` with given balance. Key-value pairs from
	/// `extra_storage` will be injected into built storage. `HEAP_PAGES` key
	/// and value will also be placed into storage.
	pub fn new(
		authorities: Vec<AccountId>,
		endowed_accounts: Vec<AccountId>,
		balance: Balance,
	) -> Self {
		GenesisStorageBuilder {
			authorities,
			balances: endowed_accounts.into_iter().map(|a| (a, balance)).collect(),
			heap_pages_override: None,
			extra_storage: Default::default(),
			wasm_code: None,
		}
	}

	/// Override default wasm code to be placed into RuntimeGenesisConfig.
	pub fn with_wasm_code(mut self, wasm_code: &Option<Vec<u8>>) -> Self {
		self.wasm_code = wasm_code.clone();
		self
	}

	pub fn with_heap_pages(mut self, heap_pages_override: Option<u64>) -> Self {
		self.heap_pages_override = heap_pages_override;
		self
	}

	pub fn with_extra_storage(mut self, storage: Storage) -> Self {
		self.extra_storage = storage;
		self
	}

	/// A `RuntimeGenesisConfig` from internal configuration
	pub fn genesis_config(&self) -> RuntimeGenesisConfig {
		let authorities_sr25519: Vec<_> = self
			.authorities
			.clone()
			.into_iter()
			.map(|id| sr25519::Public::from(id))
			.collect();

		RuntimeGenesisConfig {
			system: Default::default(),
			babe: pallet_babe::GenesisConfig {
				authorities: authorities_sr25519
					.clone()
					.into_iter()
					.map(|x| (x.into(), 1))
					.collect(),
				..Default::default()
			},
			cord_test: cord_test_pallet::GenesisConfig {
				authorities: authorities_sr25519.clone(),
				..Default::default()
			},
			balances: pallet_balances::GenesisConfig { balances: self.balances.clone() },
		}
	}

	/// Builds the `RuntimeGenesisConfig` and returns its storage.
	pub fn build(self) -> Storage {
		let mut storage = self
			.genesis_config()
			.build_storage()
			.expect("Build storage from cord-test-runtime RuntimeGenesisConfig");

		if let Some(heap_pages) = self.heap_pages_override {
			storage.top.insert(well_known_keys::HEAP_PAGES.into(), heap_pages.encode());
		}

		storage.top.insert(
			well_known_keys::CODE.into(),
			self.wasm_code.clone().unwrap_or(wasm_binary_unwrap().to_vec()),
		);

		storage.top.extend(self.extra_storage.top.clone());
		storage.children_default.extend(self.extra_storage.children_default.clone());

		storage
	}
}

pub fn insert_genesis_block(storage: &mut Storage) -> sp_core::hash::H256 {
	let child_roots = storage.children_default.iter().map(|(sk, child_content)| {
		let state_root =
			<<<crate::Block as BlockT>::Header as HeaderT>::Hashing as HashT>::trie_root(
				child_content.data.clone().into_iter().collect(),
				sp_runtime::StateVersion::V1,
			);
		(sk.clone(), state_root.encode())
	});
	// add child roots to storage
	storage.top.extend(child_roots);
	let state_root = <<<crate::Block as BlockT>::Header as HeaderT>::Hashing as HashT>::trie_root(
		storage.top.clone().into_iter().collect(),
		sp_runtime::StateVersion::V1,
	);
	let block: crate::Block = construct_genesis_block(state_root, StateVersion::V1);
	let genesis_hash = block.header.hash();

	genesis_hash
}
