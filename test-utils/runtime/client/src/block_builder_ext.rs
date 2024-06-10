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

//! Block Builder extensions for tests.

use cord_test_runtime::*;
use sc_block_builder::BlockBuilderApi;
use sp_api::{ApiExt, ProvideRuntimeApi};

/// Extension trait for test block builder.
pub trait BlockBuilderExt {
	/// Add transfer extrinsic to the block.
	fn push_transfer(
		&mut self,
		transfer: cord_test_runtime::Transfer,
	) -> Result<(), sp_blockchain::Error>;

	/// Add unsigned storage change extrinsic to the block.
	fn push_storage_change(
		&mut self,
		key: Vec<u8>,
		value: Option<Vec<u8>>,
	) -> Result<(), sp_blockchain::Error>;

	/// Adds an extrinsic which pushes DigestItem to header's log
	fn push_deposit_log_digest_item(
		&mut self,
		log: sp_runtime::generic::DigestItem,
	) -> Result<(), sp_blockchain::Error>;
}

impl<'a, A> BlockBuilderExt for sc_block_builder::BlockBuilder<'a, cord_test_runtime::Block, A>
where
	A: ProvideRuntimeApi<cord_test_runtime::Block>
		+ sp_api::CallApiAt<cord_test_runtime::Block>
		+ 'a,
	A::Api: BlockBuilderApi<cord_test_runtime::Block> + ApiExt<cord_test_runtime::Block>,
{
	fn push_transfer(
		&mut self,
		transfer: cord_test_runtime::Transfer,
	) -> Result<(), sp_blockchain::Error> {
		self.push(transfer.into_unchecked_extrinsic())
	}

	fn push_storage_change(
		&mut self,
		key: Vec<u8>,
		value: Option<Vec<u8>>,
	) -> Result<(), sp_blockchain::Error> {
		self.push(ExtrinsicBuilder::new_storage_change(key, value).build())
	}

	fn push_deposit_log_digest_item(
		&mut self,
		log: sp_runtime::generic::DigestItem,
	) -> Result<(), sp_blockchain::Error> {
		self.push(ExtrinsicBuilder::new_deposit_log_digest_item(log).build())
	}
}
