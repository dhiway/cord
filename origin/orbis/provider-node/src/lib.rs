// This file is part of CORD - https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Native off-chain content service for the Orbis storage pallets.
//!
//! The runtime remains authoritative for provider admission and storage agreements. This crate
//! stores content bytes, builds deterministic proofs, signs checkpoints and exposes the bounded
//! provider HTTP protocol. Every write is rejected unless [`ChainAuthority`] confirms an active
//! agreement at a finalized Orbis block.

#![warn(missing_docs)]

mod api;
mod chain;
mod merkle;
mod storage;
mod workers;

pub use api::{serve, ApiConfig, ProviderService};
pub use chain::{
	AgreementAuthorization, ChainAuthority, ChainError, ChallengeBatch, ChallengeDuty,
	FinalizedRuntimeAuthority,
};
pub use storage::{
	ChunkProof, CommitInput, ContentRecord, DiskStore, NodeProfile, PendingDeletion,
	PendingRootSubmission, ProviderStats, RootObservation, SignedCheckpoint, StoreError,
};
pub use workers::{
	run_workers, CheckpointSubmission, CheckpointSubmitter, ContentDeletionSubmission,
	JsonlCheckpointOutbox, ProviderRootSubmission, ProviderSubmission, WorkerConfig,
};

/// Protocol version shared by persisted records and HTTP responses.
pub const PROTOCOL_VERSION: u16 = 4;
