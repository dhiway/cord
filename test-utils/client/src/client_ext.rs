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

/// Extension trait for a test client.
pub trait ClientExt<Block: BlockT>: Sized {
    /// Finalize a block with the given hash and justification.
    fn finalize_block(
        &self,
        block_hash: Block::Hash,
        justification: Option<Justification>,
    ) -> Result<(), ConsensusError>;

    /// Get the hash of the genesis block.
    fn genesis_hash(&self) -> Block::Hash;
}

/// Extension trait for a test client around block importing.
#[async_trait::async_trait]
pub trait ClientBlockImportExt<Block: BlockT>: Sized {
    /// Import a block to the chain without finalization.
    async fn import(&mut self, origin: BlockOrigin, block: Block) -> Result<(), ConsensusError>;

    /// Import a block and make it the best block if possible.
    async fn import_as_best(&mut self, origin: BlockOrigin, block: Block) -> Result<(), ConsensusError>;

    /// Import a block and finalize it.
    async fn import_as_final(&mut self, origin: BlockOrigin, block: Block) -> Result<(), ConsensusError>;

    /// Import a block with justifications and finalize it.
    async fn import_justified(
        &mut self,
        origin: BlockOrigin,
        block: Block,
        justifications: Justifications,
    ) -> Result<(), ConsensusError>;
}

impl<B, E, RA, Block> ClientExt<Block> for Client<B, E, Block, RA>
where
    B: BlockBackend<Block>,
    E: sc_client_api::CallExecutor<Block> + sc_executor::RuntimeVersionOf + 'static,
    Self: BlockImport<Block, Error = ConsensusError>,
    Block: BlockT,
{
    /// Finalize a block with the given hash and justification.
    fn finalize_block(
        &self,
        block_hash: Block::Hash,
        justification: Option<Justification>,
    ) -> Result<(), ConsensusError> {
        Finalizer::finalize_block(self, block_hash, justification, true)
    }

    /// Get the hash of the genesis block.
    fn genesis_hash(&self) -> Block::Hash {
        self.block_hash(0u32.into()).unwrap().unwrap()
    }
}

#[async_trait::async_trait]
impl<B, Block> ClientBlockImportExt<Block> for B
where
    B: BlockImport<Block, Error = ConsensusError>,
{
    /// Import a block to the chain without finalization.
    async fn import(&mut self, origin: BlockOrigin, block: Block) -> Result<(), ConsensusError> {
        let (header, extrinsics) = block.deconstruct();
        let mut import_params = BlockImportParams::new(origin, header);
        import_params.body = Some(extrinsics);
        import_params.fork_choice = Some(ForkChoiceStrategy::LongestChain);

        BlockImport::import_block(self, import_params).await.map(|_| ())
    }

    /// Import a block and make it the best block if possible.
    async fn import_as_best(&mut self, origin: BlockOrigin, block: Block) -> Result<(), ConsensusError> {
        let (header, extrinsics) = block.deconstruct();
        let mut import_params = BlockImportParams::new(origin, header);
        import_params.body = Some(extrinsics);
        import_params.fork_choice = Some(ForkChoiceStrategy::Custom(true));

        BlockImport::import_block(self, import_params).await.map(|_| ())
    }

    /// Import a block and finalize it.
    async fn import_as_final(&mut self, origin: BlockOrigin, block: Block) -> Result<(), ConsensusError> {
        let (header, extrinsics) = block.deconstruct();
        let mut import_params = BlockImportParams::new(origin, header);
        import_params.body = Some(extrinsics);
        import_params.finalized = true;
        import_params.fork_choice = Some(ForkChoiceStrategy::Custom(true));

        BlockImport::import_block(self, import_params).await.map(|_| ())
    }

    /// Import a block with justifications and finalize it.
    async fn import_justified(
        &mut self,
        origin: BlockOrigin,
        block: Block,
        justifications: Justifications,
    ) -> Result<(), ConsensusError> {
        let (header, extrinsics) = block.deconstruct();
        let mut import_params = BlockImportParams::new(origin, header);
        import_params.justifications = Some(justifications);
        import_params.body = Some(extrinsics);
        import_params.finalized = true;
        import_params.fork_choice = Some(ForkChoiceStrategy::LongestChain);

        BlockImport::import_block(self, import_params).await.map(|_| ())
    }
}

