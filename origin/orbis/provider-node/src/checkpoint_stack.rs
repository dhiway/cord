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

//! Private ownership boundary for the provider checkpoint control plane.

use std::{
	path::Path,
	sync::{Mutex, MutexGuard},
};

use crate::{
	checkpoint::{
		checkpoint_outbox::{CheckpointOutboxV2, CheckpointSubmissionV2},
		checkpoint_primary::CheckpointPrimaryQuorumStore,
		checkpoint_promotion::CheckpointPromotionStoreV1,
		checkpoint_publication::CheckpointPublicationStoreV1,
		checkpoint_quorum::ReplicaConfirmationStore,
		CheckpointProposalStore,
	},
	storage::bucket_mmr::BucketMmrStore,
	ContentError, StreamingStore,
};

/// All durable checkpoint kernels opened against one provider root.
///
/// Operations which cross the chain or network boundary must first clone an owned intent from
/// this state, release the guard, perform the external work, and then reacquire the stack to
/// persist the result. The stack intentionally exposes no async guard.
struct CheckpointStackState {
	streaming: StreamingStore,
	bucket_mmr: BucketMmrStore,
	proposals: CheckpointProposalStore,
	replica_confirmations: ReplicaConfirmationStore,
	primary_quorum: CheckpointPrimaryQuorumStore,
	outbox: CheckpointOutboxV2,
	publications: CheckpointPublicationStoreV1,
	fallback_promotions: CheckpointPromotionStoreV1,
}

/// Cohesive private checkpoint state owned through one synchronization boundary.
pub(crate) struct CheckpointStack {
	state: Mutex<CheckpointStackState>,
}

impl CheckpointStack {
	/// Open every checkpoint kernel against the same provider root.
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let root = root.as_ref();
		let streaming = StreamingStore::open(root)?;
		let bucket_mmr = BucketMmrStore::open(root, &streaming)?;
		let state = CheckpointStackState {
			streaming,
			bucket_mmr,
			proposals: CheckpointProposalStore::open(root)?,
			replica_confirmations: ReplicaConfirmationStore::open(root)?,
			primary_quorum: CheckpointPrimaryQuorumStore::open(root)?,
			outbox: CheckpointOutboxV2::open(root)?,
			publications: CheckpointPublicationStoreV1::open(root)?,
			fallback_promotions: CheckpointPromotionStoreV1::open(root)?,
		};
		Ok(Self { state: Mutex::new(state) })
	}

	/// Clone the next durable submission so external finality work never borrows the stack guard.
	pub(crate) fn next_submission(&self) -> Result<Option<CheckpointSubmissionV2>, ContentError> {
		Ok(self.lock()?.outbox.pending_submissions()?.into_iter().next())
	}

	fn lock(&self) -> Result<MutexGuard<'_, CheckpointStackState>, ContentError> {
		self.state.lock().map_err(|_| ContentError::IntegrityFailed)
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	const DURABLE_ROOTS: [&str; 10] = [
		"streaming-v1",
		"bucket-mmr-v3",
		"checkpoint-proposals-v2",
		"checkpoint-confirmations-v1",
		"checkpoint-primary-quorum-v1",
		"checkpoint-submissions-v2",
		"checkpoint-receipts-v2",
		"checkpoint-finalized-receipts-v2",
		"checkpoint-publications-v1",
		"checkpoint-promotions-v1",
	];

	#[test]
	fn one_stack_opens_every_kernel_and_releases_owned_intents() {
		let temp = TempDir::new().unwrap();
		let stack = CheckpointStack::open(temp.path()).unwrap();

		for root in DURABLE_ROOTS {
			assert!(temp.path().join(root).is_dir(), "missing durable root {root}");
		}
		assert!(stack.next_submission().unwrap().is_none());
		assert!(stack.state.try_lock().is_ok());
		let state = stack.lock().unwrap();
		assert!(state.proposals.pending_checkpoint_proposals().unwrap().is_empty());
		assert!(state.outbox.pending_submissions().unwrap().is_empty());
		let _owned_kernels = (
			&state.streaming,
			&state.bucket_mmr,
			&state.replica_confirmations,
			&state.primary_quorum,
			&state.publications,
			&state.fallback_promotions,
		);
	}

	#[test]
	fn stacks_are_isolated_by_provider_root() {
		let first = TempDir::new().unwrap();
		let second = TempDir::new().unwrap();
		drop(CheckpointStack::open(first.path()).unwrap());
		drop(CheckpointStack::open(second.path()).unwrap());

		fs::write(first.path().join("checkpoint-submissions-v2/unbounded.bin"), b"invalid")
			.unwrap();
		assert!(matches!(CheckpointStack::open(first.path()), Err(ContentError::IntegrityFailed)));
		assert!(CheckpointStack::open(second.path()).is_ok());
	}
}
