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

//! tests that should hold for all implementations of certain traits.
//! to test implementations without duplication.

#![allow(missing_docs)]

use std::sync::Arc;

use crate::{
	AccountKeyring, BlockBuilderExt, ClientBlockImportExt, TestClientBuilder, TestClientBuilderExt,
};
use cord_test_runtime::Transfer;
use futures::executor::block_on;
use sc_block_builder::BlockBuilderBuilder;
use sc_client_api::{
	backend,
	blockchain::{Backend as BlockChainBackendT, HeaderBackend},
};
use sp_consensus::BlockOrigin;
use sp_runtime::traits::Block as BlockT;

/// helper to test the `leaves` implementation for various backends
pub fn test_leaves_for_backend<B: 'static>(backend: Arc<B>)
where
	B: backend::Backend<cord_test_runtime::Block>,
{
	// block tree:
	// G -> A1 -> A2 -> A3 -> A4 -> A5
	// 		A1 -> B2 -> B3 -> B4
	// 			  B2 -> C3
	// 		A1 -> D2

	let mut client = TestClientBuilder::with_backend(backend.clone()).build();
	let blockchain = backend.blockchain();

	let genesis_hash = client.chain_info().genesis_hash;

	assert_eq!(blockchain.leaves().unwrap(), vec![genesis_hash]);

	// G -> A1
	let a1 = BlockBuilderBuilder::new(&client)
		.on_parent_block(genesis_hash)
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a1.clone())).unwrap();
	assert_eq!(blockchain.leaves().unwrap(), vec![a1.hash()]);

	// A1 -> A2
	let a2 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a1.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a2.clone())).unwrap();

	assert_eq!(blockchain.leaves().unwrap(), vec![a2.hash()]);

	// A2 -> A3
	let a3 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a2.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a3.clone())).unwrap();

	assert_eq!(blockchain.leaves().unwrap(), vec![a3.hash()]);

	// A3 -> A4
	let a4 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a3.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a4.clone())).unwrap();
	assert_eq!(blockchain.leaves().unwrap(), vec![a4.hash()]);

	// A4 -> A5
	let a5 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a4.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;

	block_on(client.import(BlockOrigin::Own, a5.clone())).unwrap();
	assert_eq!(blockchain.leaves().unwrap(), vec![a5.hash()]);

	// A1 -> B2
	let mut builder = BlockBuilderBuilder::new(&client)
		.on_parent_block(a1.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap();

	// this push is required as otherwise B2 has the same hash as A2 and won't get
	// imported
	builder
		.push_transfer(Transfer {
			from: AccountKeyring::Alice.into(),
			to: AccountKeyring::Ferdie.into(),
			amount: 41,
			nonce: 0,
		})
		.unwrap();
	let b2 = builder.build().unwrap().block;
	block_on(client.import(BlockOrigin::Own, b2.clone())).unwrap();
	assert_eq!(blockchain.leaves().unwrap(), vec![a5.hash(), b2.hash()]);

	// B2 -> B3
	let b3 = BlockBuilderBuilder::new(&client)
		.on_parent_block(b2.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;

	block_on(client.import(BlockOrigin::Own, b3.clone())).unwrap();
	assert_eq!(blockchain.leaves().unwrap(), vec![a5.hash(), b3.hash()]);

	// B3 -> B4
	let b4 = BlockBuilderBuilder::new(&client)
		.on_parent_block(b3.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, b4.clone())).unwrap();
	assert_eq!(blockchain.leaves().unwrap(), vec![a5.hash(), b4.hash()]);

	// // B2 -> C3
	let mut builder = BlockBuilderBuilder::new(&client)
		.on_parent_block(b2.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap();
	// this push is required as otherwise C3 has the same hash as B3 and won't get
	// imported
	builder
		.push_transfer(Transfer {
			from: AccountKeyring::Alice.into(),
			to: AccountKeyring::Ferdie.into(),
			amount: 1,
			nonce: 1,
		})
		.unwrap();
	let c3 = builder.build().unwrap().block;
	block_on(client.import(BlockOrigin::Own, c3.clone())).unwrap();
	assert_eq!(blockchain.leaves().unwrap(), vec![a5.hash(), b4.hash(), c3.hash()]);

	// A1 -> D2
	let mut builder = BlockBuilderBuilder::new(&client)
		.on_parent_block(a1.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap();
	// this push is required as otherwise D2 has the same hash as B2 and won't get
	// imported
	builder
		.push_transfer(Transfer {
			from: AccountKeyring::Alice.into(),
			to: AccountKeyring::Ferdie.into(),
			amount: 1,
			nonce: 0,
		})
		.unwrap();
	let d2 = builder.build().unwrap().block;
	block_on(client.import(BlockOrigin::Own, d2.clone())).unwrap();
	assert_eq!(blockchain.leaves().unwrap(), vec![a5.hash(), b4.hash(), c3.hash(), d2.hash()]);
}

/// helper to test the `children` implementation for various backends
pub fn test_children_for_backend<B: 'static>(backend: Arc<B>)
where
	B: backend::LocalBackend<cord_test_runtime::Block>,
{
	// block tree:
	// G -> A1 -> A2 -> A3 -> A4 -> A5
	// 		A1 -> B2 -> B3 -> B4
	// 			  B2 -> C3
	// 		A1 -> D2

	let mut client = TestClientBuilder::with_backend(backend.clone()).build();
	let blockchain = backend.blockchain();
	let genesis_hash = client.chain_info().genesis_hash;

	// G -> A1
	let a1 = BlockBuilderBuilder::new(&client)
		.on_parent_block(genesis_hash)
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a1.clone())).unwrap();

	// A1 -> A2
	let a2 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a1.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a2.clone())).unwrap();

	// A2 -> A3
	let a3 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a2.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a3.clone())).unwrap();

	// A3 -> A4
	let a4 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a3.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a4.clone())).unwrap();

	// A4 -> A5
	let a5 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a4.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a5.clone())).unwrap();

	// A1 -> B2
	let mut builder = BlockBuilderBuilder::new(&client)
		.on_parent_block(a1.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap();
	// this push is required as otherwise B2 has the same hash as A2 and won't get
	// imported
	builder
		.push_transfer(Transfer {
			from: AccountKeyring::Alice.into(),
			to: AccountKeyring::Ferdie.into(),
			amount: 41,
			nonce: 0,
		})
		.unwrap();
	let b2 = builder.build().unwrap().block;
	block_on(client.import(BlockOrigin::Own, b2.clone())).unwrap();

	// B2 -> B3
	let b3 = BlockBuilderBuilder::new(&client)
		.on_parent_block(b2.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, b3.clone())).unwrap();

	// B3 -> B4
	let b4 = BlockBuilderBuilder::new(&client)
		.on_parent_block(b3.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, b4)).unwrap();

	// // B2 -> C3
	let mut builder = BlockBuilderBuilder::new(&client)
		.on_parent_block(b2.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap();
	// this push is required as otherwise C3 has the same hash as B3 and won't get
	// imported
	builder
		.push_transfer(Transfer {
			from: AccountKeyring::Alice.into(),
			to: AccountKeyring::Ferdie.into(),
			amount: 1,
			nonce: 1,
		})
		.unwrap();
	let c3 = builder.build().unwrap().block;
	block_on(client.import(BlockOrigin::Own, c3.clone())).unwrap();

	// A1 -> D2
	let mut builder = BlockBuilderBuilder::new(&client)
		.on_parent_block(a1.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap();
	// this push is required as otherwise D2 has the same hash as B2 and won't get
	// imported
	builder
		.push_transfer(Transfer {
			from: AccountKeyring::Alice.into(),
			to: AccountKeyring::Ferdie.into(),
			amount: 1,
			nonce: 0,
		})
		.unwrap();
	let d2 = builder.build().unwrap().block;
	block_on(client.import(BlockOrigin::Own, d2.clone())).unwrap();

	let genesis_hash = client.chain_info().genesis_hash;

	let children1 = blockchain.children(a4.hash()).unwrap();
	assert_eq!(vec![a5.hash()], children1);

	let children2 = blockchain.children(a1.hash()).unwrap();
	assert_eq!(vec![a2.hash(), b2.hash(), d2.hash()], children2);

	let children3 = blockchain.children(genesis_hash).unwrap();
	assert_eq!(vec![a1.hash()], children3);

	let children4 = blockchain.children(b2.hash()).unwrap();
	assert_eq!(vec![b3.hash(), c3.hash()], children4);
}

pub fn test_blockchain_query_by_number_gets_canonical<B: 'static>(backend: Arc<B>)
where
	B: backend::LocalBackend<cord_test_runtime::Block>,
{
	// block tree:
	// G -> A1 -> A2 -> A3 -> A4 -> A5
	// 		A1 -> B2 -> B3 -> B4
	// 			  B2 -> C3
	// 		A1 -> D2
	let mut client = TestClientBuilder::with_backend(backend.clone()).build();
	let blockchain = backend.blockchain();
	let genesis_hash = client.chain_info().genesis_hash;

	// G -> A1
	let a1 = BlockBuilderBuilder::new(&client)
		.on_parent_block(genesis_hash)
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a1.clone())).unwrap();

	// A1 -> A2
	let a2 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a1.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a2.clone())).unwrap();

	// A2 -> A3
	let a3 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a2.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a3.clone())).unwrap();

	// A3 -> A4
	let a4 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a3.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a4.clone())).unwrap();

	// A4 -> A5
	let a5 = BlockBuilderBuilder::new(&client)
		.on_parent_block(a4.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, a5.clone())).unwrap();

	// A1 -> B2
	let mut builder = BlockBuilderBuilder::new(&client)
		.on_parent_block(a1.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap();
	// this push is required as otherwise B2 has the same hash as A2 and won't get
	// imported
	builder
		.push_transfer(Transfer {
			from: AccountKeyring::Alice.into(),
			to: AccountKeyring::Ferdie.into(),
			amount: 41,
			nonce: 0,
		})
		.unwrap();
	let b2 = builder.build().unwrap().block;
	block_on(client.import(BlockOrigin::Own, b2.clone())).unwrap();

	// B2 -> B3
	let b3 = BlockBuilderBuilder::new(&client)
		.on_parent_block(b2.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, b3.clone())).unwrap();

	// B3 -> B4
	let b4 = BlockBuilderBuilder::new(&client)
		.on_parent_block(b3.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap()
		.build()
		.unwrap()
		.block;
	block_on(client.import(BlockOrigin::Own, b4)).unwrap();

	// // B2 -> C3
	let mut builder = BlockBuilderBuilder::new(&client)
		.on_parent_block(b2.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap();
	// this push is required as otherwise C3 has the same hash as B3 and won't get
	// imported
	builder
		.push_transfer(Transfer {
			from: AccountKeyring::Alice.into(),
			to: AccountKeyring::Ferdie.into(),
			amount: 1,
			nonce: 1,
		})
		.unwrap();
	let c3 = builder.build().unwrap().block;
	block_on(client.import(BlockOrigin::Own, c3)).unwrap();

	// A1 -> D2
	let mut builder = BlockBuilderBuilder::new(&client)
		.on_parent_block(a1.hash())
		.fetch_parent_block_number(&client)
		.unwrap()
		.build()
		.unwrap();
	// this push is required as otherwise D2 has the same hash as B2 and won't get
	// imported
	builder
		.push_transfer(Transfer {
			from: AccountKeyring::Alice.into(),
			to: AccountKeyring::Ferdie.into(),
			amount: 1,
			nonce: 0,
		})
		.unwrap();
	let d2 = builder.build().unwrap().block;
	block_on(client.import(BlockOrigin::Own, d2)).unwrap();

	let genesis_hash = client.chain_info().genesis_hash;

	assert_eq!(blockchain.hash(0).unwrap().unwrap(), genesis_hash);
	assert_eq!(blockchain.hash(1).unwrap().unwrap(), a1.hash());
	assert_eq!(blockchain.hash(2).unwrap().unwrap(), a2.hash());
	assert_eq!(blockchain.hash(3).unwrap().unwrap(), a3.hash());
	assert_eq!(blockchain.hash(4).unwrap().unwrap(), a4.hash());
	assert_eq!(blockchain.hash(5).unwrap().unwrap(), a5.hash());
}
