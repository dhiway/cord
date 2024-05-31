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

//! Client testing utilities.

#![warn(missing_docs)]

pub mod trait_tests;

mod block_builder_ext;

pub use cord_test_client::*;
pub use cord_test_runtime as runtime;
pub use sc_consensus::LongestChain;
use std::sync::Arc;

pub use self::block_builder_ext::BlockBuilderExt;

use cord_test_runtime::genesismap::GenesisStorageBuilder;
use sp_core::storage::ChildInfo;

/// A prelude to import in tests.
pub mod prelude {
	// Trait extensions
	pub use super::{
		BlockBuilderExt, ClientBlockImportExt, ClientExt, DefaultTestClientBuilderExt,
		TestClientBuilderExt,
	};
	// Client structs
	pub use super::{
		Backend, ExecutorDispatch, TestClient, TestClientBuilder, WasmExecutionMethod,
	};
	// Keyring
	pub use super::{AccountKeyring, Sr25519Keyring};
}

/// Test client database backend.
pub type Backend = cord_test_client::Backend<cord_test_runtime::Block>;

/// Test client executor.
pub type ExecutorDispatch =
	client::LocalCallExecutor<cord_test_runtime::Block, Backend, WasmExecutor>;

/// Parameters of test-client builder with test-runtime.
#[derive(Default)]
pub struct GenesisParameters {
	heap_pages_override: Option<u64>,
	extra_storage: Storage,
	wasm_code: Option<Vec<u8>>,
}

impl GenesisParameters {
	/// Set the wasm code that should be used at genesis.
	pub fn set_wasm_code(&mut self, code: Vec<u8>) {
		self.wasm_code = Some(code);
	}

	/// Access extra genesis storage.
	pub fn extra_storage(&mut self) -> &mut Storage {
		&mut self.extra_storage
	}
}

impl GenesisInit for GenesisParameters {
	fn genesis_storage(&self) -> Storage {
		GenesisStorageBuilder::default()
			.with_heap_pages(self.heap_pages_override)
			.with_wasm_code(&self.wasm_code)
			.with_extra_storage(self.extra_storage.clone())
			.build()
	}
}

/// A `TestClient` with `test-runtime` builder.
pub type TestClientBuilder<E, B> =
	cord_test_client::TestClientBuilder<cord_test_runtime::Block, E, B, GenesisParameters>;

/// Test client type with `LocalExecutorDispatch` and generic Backend.
pub type Client<B> = client::Client<
	B,
	client::LocalCallExecutor<cord_test_runtime::Block, B, WasmExecutor>,
	cord_test_runtime::Block,
	cord_test_runtime::RuntimeApi,
>;

/// A test client with default backend.
pub type TestClient = Client<Backend>;

/// A `TestClientBuilder` with default backend and executor.
pub trait DefaultTestClientBuilderExt: Sized {
	/// Create new `TestClientBuilder`
	fn new() -> Self;
}

impl DefaultTestClientBuilderExt for TestClientBuilder<ExecutorDispatch, Backend> {
	fn new() -> Self {
		Self::with_default_backend()
	}
}

/// A `test-runtime` extensions to `TestClientBuilder`.
pub trait TestClientBuilderExt<B>: Sized {
	/// Returns a mutable reference to the genesis parameters.
	fn genesis_init_mut(&mut self) -> &mut GenesisParameters;

	/// Override the default value for Wasm heap pages.
	fn set_heap_pages(mut self, heap_pages: u64) -> Self {
		self.genesis_init_mut().heap_pages_override = Some(heap_pages);
		self
	}

	/// Add an extra value into the genesis storage.
	///
	/// # Panics
	///
	/// Panics if the key is empty.
	fn add_extra_child_storage<K: Into<Vec<u8>>, V: Into<Vec<u8>>>(
		mut self,
		child_info: &ChildInfo,
		key: K,
		value: V,
	) -> Self {
		let storage_key = child_info.storage_key().to_vec();
		let key = key.into();
		assert!(!storage_key.is_empty());
		assert!(!key.is_empty());
		self.genesis_init_mut()
			.extra_storage
			.children_default
			.entry(storage_key)
			.or_insert_with(|| StorageChild {
				data: Default::default(),
				child_info: child_info.clone(),
			})
			.data
			.insert(key, value.into());
		self
	}

	/// Add an extra child value into the genesis storage.
	///
	/// # Panics
	///
	/// Panics if the key is empty.
	fn add_extra_storage<K: Into<Vec<u8>>, V: Into<Vec<u8>>>(mut self, key: K, value: V) -> Self {
		let key = key.into();
		assert!(!key.is_empty());
		self.genesis_init_mut().extra_storage.top.insert(key, value.into());
		self
	}

	/// Build the test client.
	fn build(self) -> Client<B> {
		self.build_with_longest_chain().0
	}

	/// Build the test client and longest chain selector.
	fn build_with_longest_chain(
		self,
	) -> (Client<B>, sc_consensus::LongestChain<B, cord_test_runtime::Block>);

	/// Build the test client and the backend.
	fn build_with_backend(self) -> (Client<B>, Arc<B>);
}

impl<B> TestClientBuilderExt<B>
	for TestClientBuilder<client::LocalCallExecutor<cord_test_runtime::Block, B, WasmExecutor>, B>
where
	B: sc_client_api::backend::Backend<cord_test_runtime::Block> + 'static,
{
	fn genesis_init_mut(&mut self) -> &mut GenesisParameters {
		Self::genesis_init_mut(self)
	}

	fn build_with_longest_chain(
		self,
	) -> (Client<B>, sc_consensus::LongestChain<B, cord_test_runtime::Block>) {
		self.build_with_native_executor(None)
	}

	fn build_with_backend(self) -> (Client<B>, Arc<B>) {
		let backend = self.backend();
		(self.build_with_native_executor(None).0, backend)
	}
}

/// Creates new client instance used for tests.
pub fn new() -> Client<Backend> {
	TestClientBuilder::new().build()
}

/// Create a new native executor.
#[deprecated(note = "Switch to `WasmExecutor:default()`.")]
pub fn new_native_or_wasm_executor() -> WasmExecutor {
	WasmExecutor::default()
}
