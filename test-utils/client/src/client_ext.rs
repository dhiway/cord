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

//! Client extension for tests.

use sc_client_api::{backend::Finalizer, client::BlockBackend};
use sc_consensus::{BlockImport, BlockImportParams, ForkChoiceStrategy};
use sc_service::client::Client;
use sp_consensus::Error as ConsensusError;
use sp_runtime::{traits::Block as BlockT, Justification, Justifications};

pub use sp_consensus::BlockOrigin;

/// Extension trait for a test client.
pub trait ClientExt<Block: BlockT>: Sized {
	/// Finalize a block.
	fn finalize_block(
		&self,
		hash: Block::Hash,
		justification: Option<Justification>,
	) -> sp_blockchain::Result<()>;

	/// Returns hash of the genesis block.
	fn genesis_hash(&self) -> <Block as BlockT>::Hash;
}

/// Extension trait for a test client around block importing.
#[async_trait::async_trait]
pub trait ClientBlockImportExt<Block: BlockT>: Sized {
	/// Import block to the chain. No finality.
	async fn import(&mut self, origin: BlockOrigin, block: Block) -> Result<(), ConsensusError>;

	/// Import a block and make it our best block if possible.
	async fn import_as_best(
		&mut self,
		origin: BlockOrigin,
		block: Block,
	) -> Result<(), ConsensusError>;

	/// Import a block and finalize it.
	async fn import_as_final(
		&mut self,
		origin: BlockOrigin,
		block: Block,
	) -> Result<(), ConsensusError>;

	/// Import block with justification(s), finalizes block.
	async fn import_justified(
		&mut self,
		origin: BlockOrigin,
		block: Block,
		justifications: Justifications,
	) -> Result<(), ConsensusError>;
}

impl<B, E, RA, Block> ClientExt<Block> for Client<B, E, Block, RA>
where
	B: sc_client_api::backend::Backend<Block>,
	E: sc_client_api::CallExecutor<Block> + sc_executor::RuntimeVersionOf + 'static,
	Self: BlockImport<Block, Error = ConsensusError>,
	Block: BlockT,
{
	fn finalize_block(
		&self,
		hash: Block::Hash,
		justification: Option<Justification>,
	) -> sp_blockchain::Result<()> {
		Finalizer::finalize_block(self, hash, justification, true)
	}

	fn genesis_hash(&self) -> <Block as BlockT>::Hash {
		self.block_hash(0u32.into()).unwrap().unwrap()
	}
}

/// This implementation is required, because of the weird api requirements around `BlockImport`.
#[async_trait::async_trait]
impl<Block: BlockT, T> ClientBlockImportExt<Block> for std::sync::Arc<T>
where
	for<'r> &'r T: BlockImport<Block, Error = ConsensusError>,
	T: Send + Sync,
{
	async fn import(&mut self, origin: BlockOrigin, block: Block) -> Result<(), ConsensusError> {
		let (header, extrinsics) = block.deconstruct();
		let mut import = BlockImportParams::new(origin, header);
		import.body = Some(extrinsics);
		import.fork_choice = Some(ForkChoiceStrategy::LongestChain);

		BlockImport::import_block(self, import).await.map(|_| ())
	}

	async fn import_as_best(
		&mut self,
		origin: BlockOrigin,
		block: Block,
	) -> Result<(), ConsensusError> {
		let (header, extrinsics) = block.deconstruct();
		let mut import = BlockImportParams::new(origin, header);
		import.body = Some(extrinsics);
		import.fork_choice = Some(ForkChoiceStrategy::Custom(true));

		BlockImport::import_block(self, import).await.map(|_| ())
	}

	async fn import_as_final(
		&mut self,
		origin: BlockOrigin,
		block: Block,
	) -> Result<(), ConsensusError> {
		let (header, extrinsics) = block.deconstruct();
		let mut import = BlockImportParams::new(origin, header);
		import.body = Some(extrinsics);
		import.finalized = true;
		import.fork_choice = Some(ForkChoiceStrategy::Custom(true));

		BlockImport::import_block(self, import).await.map(|_| ())
	}

	async fn import_justified(
		&mut self,
		origin: BlockOrigin,
		block: Block,
		justifications: Justifications,
	) -> Result<(), ConsensusError> {
		let (header, extrinsics) = block.deconstruct();
		let mut import = BlockImportParams::new(origin, header);
		import.justifications = Some(justifications);
		import.body = Some(extrinsics);
		import.finalized = true;
		import.fork_choice = Some(ForkChoiceStrategy::LongestChain);

		BlockImport::import_block(self, import).await.map(|_| ())
	}
}

#[async_trait::async_trait]
impl<B, E, RA, Block: BlockT> ClientBlockImportExt<Block> for Client<B, E, Block, RA>
where
	Self: BlockImport<Block, Error = ConsensusError>,
	RA: Send,
	B: Send + Sync,
	E: Send + Sync,
{
	async fn import(&mut self, origin: BlockOrigin, block: Block) -> Result<(), ConsensusError> {
		let (header, extrinsics) = block.deconstruct();
		let mut import = BlockImportParams::new(origin, header);
		import.body = Some(extrinsics);
		import.fork_choice = Some(ForkChoiceStrategy::LongestChain);

		BlockImport::import_block(self, import).await.map(|_| ())
	}

	async fn import_as_best(
		&mut self,
		origin: BlockOrigin,
		block: Block,
	) -> Result<(), ConsensusError> {
		let (header, extrinsics) = block.deconstruct();
		let mut import = BlockImportParams::new(origin, header);
		import.body = Some(extrinsics);
		import.fork_choice = Some(ForkChoiceStrategy::Custom(true));

		BlockImport::import_block(self, import).await.map(|_| ())
	}

	async fn import_as_final(
		&mut self,
		origin: BlockOrigin,
		block: Block,
	) -> Result<(), ConsensusError> {
		let (header, extrinsics) = block.deconstruct();
		let mut import = BlockImportParams::new(origin, header);
		import.body = Some(extrinsics);
		import.finalized = true;
		import.fork_choice = Some(ForkChoiceStrategy::Custom(true));

		BlockImport::import_block(self, import).await.map(|_| ())
	}

	async fn import_justified(
		&mut self,
		origin: BlockOrigin,
		block: Block,
		justifications: Justifications,
	) -> Result<(), ConsensusError> {
		let (header, extrinsics) = block.deconstruct();
		let mut import = BlockImportParams::new(origin, header);
		import.justifications = Some(justifications);
		import.body = Some(extrinsics);
		import.finalized = true;
		import.fork_choice = Some(ForkChoiceStrategy::LongestChain);

		BlockImport::import_block(self, import).await.map(|_| ())
	}
}
