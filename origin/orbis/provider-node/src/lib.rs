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
// Deliberately compiled but not routed until durable RequestV2 recovery is implemented.
#[allow(dead_code)]
mod capability;
mod chain;
#[allow(dead_code)]
mod checkpoint;
mod checkpoint_stack;
mod content;
mod merkle;
mod peer;
// Private HTTP/1 listener for the service-key-authenticated replication wire protocol.
mod peer_http;
// Private durable replay table for exact authenticated peer responses.
mod peer_reply;
// Private topology-authenticated responder behind the dedicated peer listener.
mod peer_responder;
// Private outbound transport pinned to one exact finalized replication session.
#[allow(dead_code)]
mod peer_transport;
// Deliberately private until authenticated peer replication orchestration is activated.
#[allow(dead_code)]
mod replication;
// Private bounded target reconciler; deliberately not wired into the public node lifecycle yet.
#[allow(dead_code)]
mod replication_reconciler;
// Private deterministic bridge from finalized topology evidence into authenticated peer context.
mod replication_session;
mod storage;
mod workers;

pub use api::{serve, serve_provider_ingress, ApiConfig, ProviderService};
pub use chain::{
	AgreementAuthorization, CapabilityAuthoritySnapshot, ChainAuthority, ChainError,
	ChallengeBatch, ChallengeDuty, CheckpointDuty, CheckpointDutyBatch, CheckpointDutyMode,
	CheckpointDutyPageRequest, CheckpointDutyPhase, CheckpointDutyRole, CheckpointDutyScanCursor,
	FinalizedRuntimeAuthority,
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
