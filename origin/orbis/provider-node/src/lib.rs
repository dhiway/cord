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

#[cfg(not(unix))]
compile_error!("origin-orbis-provider requires Unix no-follow directory-handle semantics");

mod api;
mod bounded_io;
// Deliberately private until the P4 route cutover removes the shared bearer atomically.
#[allow(dead_code)]
mod capability;
mod chain;
#[allow(dead_code)]
mod checkpoint;
#[cfg(feature = "checkpoint-live")]
mod checkpoint_live_worker;
#[cfg(feature = "checkpoint-live")]
mod checkpoint_promotion_worker;
mod checkpoint_quorum_worker;
mod checkpoint_stack;
mod checkpoint_transport;
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
mod peer_transport;
// Private local IPC; it is deliberately absent from the crate's public provider surface.
mod private_host_ipc;
mod replication;
// Private bounded target reconciler driven by the dedicated replication worker.
mod replication_reconciler;
mod replication_worker;
// Private deterministic bridge from finalized topology evidence into authenticated peer context.
mod replication_session;
mod storage;
#[cfg(feature = "evidence")]
mod three_provider_evidence;
mod workers;

#[cfg(feature = "checkpoint-live")]
pub use api::run_checkpoint_live_worker;
pub use api::{
	run_checkpoint_quorum_worker, run_replication_worker, serve, serve_provider_ingress, ApiConfig,
	ProviderOpenError, ProviderService,
};
pub use chain::{
	AgreementAuthorization, CapabilityAuthoritySnapshot, ChainAuthority, ChainError,
	ChallengeBatch, ChallengeDuty, CheckpointDuty, CheckpointDutyBatch, CheckpointDutyMode,
	CheckpointDutyPageRequest, CheckpointDutyPhase, CheckpointDutyRole, CheckpointDutyScanCursor,
	DeletionDuty, DeletionDutyBatch, DeletionDutyPageRequest, DeletionDutyScanCursor,
	FinalizedRuntimeAuthority,
};
pub use content::{
	BucketId, CanonicalCid, ContentError, OperationId, BLAKE2B_256_CODE, CHUNK_BYTES,
	INGRESS_WINDOW_BYTES, INGRESS_WINDOW_CHUNKS, MAX_CHUNKS, MAX_RANGE_BYTES, MAX_STORED_BYTES,
	MAX_STREAMING_OPERATIONS, RAW_CODEC,
};
#[doc(hidden)]
pub use private_host_ipc::serve_private_host_ipc;
pub use storage::{
	BeginStreaming, CheckpointDutyWatermark, ChunkProof, ContentRecord, DiskStore, IngressPermit,
	IntegritySummary, NodeProfile, ProgressAck, ProviderStats, StoreError, StreamingDescriptor,
	StreamingFault, StreamingReceipt,
};
#[cfg(feature = "test-seams")]
#[doc(hidden)]
pub use storage::StreamingStore;
#[cfg(feature = "evidence")]
#[doc(hidden)]
pub use three_provider_evidence::{
	run_three_provider_recovery_evidence, CorruptReadObservation, EligibleSourceObservation,
	ThreeProviderRecoveryEvidence,
};
pub(crate) use workers::{JsonlManifestDeletionOutbox, ManifestDeletionSubmitter};
#[cfg(any(test, feature = "evidence"))]
pub(crate) use workers::ManifestDeletionStartupPlan;
pub use workers::{
	poll_checkpoint_duties_once, poll_manifest_deletions_once, run_workers,
	ManifestDeletionSubmission, ProviderSubmission, WorkerConfig,
};

/// Protocol version shared by persisted records and HTTP responses.
pub const PROTOCOL_VERSION: u16 = 6;
