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

//! Native off-chain content service for the Orbis storage pallets.
//!
//! The runtime remains authoritative for provider admission and storage agreements. This crate
//! stores content bytes, builds deterministic proofs, signs checkpoints and exposes the bounded
//! provider HTTP protocol. Every write is rejected unless [`ChainAuthority`] confirms an active
//! agreement at a finalized Orbis block.

#![warn(missing_docs)]

mod api;
mod chain;
mod content;
mod merkle;
mod storage;
mod workers;

pub use api::{serve, ApiConfig, ProviderService};
pub use chain::{
	AgreementAuthorization, ChainAuthority, ChainError, ChallengeBatch, ChallengeDuty,
	CheckpointDuty, CheckpointDutyBatch, CheckpointDutyMode, CheckpointDutyPageRequest,
	CheckpointDutyPhase, CheckpointDutyRole, CheckpointDutyScanCursor, FinalizedRuntimeAuthority,
};
pub use content::{
	BucketId, CanonicalCid, ContentError, OperationId, BLAKE2B_256_CODE, CHUNK_BYTES,
	INGRESS_WINDOW_BYTES, INGRESS_WINDOW_CHUNKS, MAX_CHUNKS, MAX_RANGE_BYTES, MAX_STORED_BYTES,
	MAX_STREAMING_OPERATIONS, RAW_CODEC,
};
pub use storage::{
	BeginStreaming, CheckpointDutyWatermark, ChunkProof, CommitInput, ContentRecord, DiskStore,
	IngressPermit, IntegritySummary, NodeProfile, PendingDeletion, PendingRootSubmission,
	ProgressAck, ProviderStats, RootObservation, SignedCheckpoint, StoreError, StreamingDescriptor,
	StreamingFault, StreamingReceipt, StreamingStore,
};
pub use workers::{
	poll_checkpoint_duties_once, run_workers, CheckpointSubmission, CheckpointSubmitter,
	ContentDeletionSubmission, JsonlCheckpointOutbox, ProviderRootSubmission, ProviderSubmission,
	WorkerConfig,
};

/// Protocol version shared by persisted records and HTTP responses.
pub const PROTOCOL_VERSION: u16 = 5;
