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

use std::time::Duration;

/// Strategy for obtaining nonces while submitting extrinsics.
#[derive(Clone, Copy, Debug)]
pub enum NonceMode {
	/// Query the node for the next nonce before each transaction.
	RpcPerTx,
	/// Cache locally and increment; refresh periodically from chain.
	LocalCache,
}

/// How submissions are routed through the pipeline.
#[derive(Clone, Copy, Debug)]
pub enum TxSubmitMode {
	/// Default: queued per-account with managed nonce allocation.
	ManagedQueue,
	/// Same as `ManagedQueue`, but allow explicit nonce override.
	ManagedQueueWithOverride,
	/// Caller bypasses queue and handles sequencing manually.
	Manual,
}

#[derive(Clone, Debug)]
pub struct TxPipelineConfig {
	pub nonce_mode: NonceMode,
	pub submit_mode: TxSubmitMode,
	pub queue_capacity: usize,
	pub nonce_refresh_after: Duration,
	pub retry_on_stale_nonce: bool,
	pub max_retry_attempts: u8,
	pub prewarm_on_first_use: bool,
}

impl Default for TxPipelineConfig {
	fn default() -> Self {
		Self {
			nonce_mode: NonceMode::LocalCache,
			submit_mode: TxSubmitMode::ManagedQueue,
			queue_capacity: 1024,
			nonce_refresh_after: Duration::from_secs(30),
			retry_on_stale_nonce: true,
			max_retry_attempts: 2,
			prewarm_on_first_use: false,
		}
	}
}
