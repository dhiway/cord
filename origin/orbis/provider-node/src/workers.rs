// This file is part of CORD - https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Provider background coordinators and the explicit checkpoint-submission seam.

use std::{
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use async_trait::async_trait;
use codec::Encode;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, H256};
use tokio::{
	io::AsyncWriteExt,
	sync::Mutex,
	time::{interval, MissedTickBehavior},
};

use crate::{ChainAuthority, ChallengeDuty, PendingDeletion, ProviderService, SignedCheckpoint};

/// Durable request consumed by the CORD-owned Orbis signer/nonce/finality pipeline.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointSubmission {
	/// Finalized runtime duty.
	pub duty: ChallengeDuty,
	/// Signed local provider root produced while handling the duty.
	pub checkpoint: SignedCheckpoint,
	/// Exact runtime proof commitment passed to `StorageProvider::submit_checkpoint`.
	pub proof_commitment: String,
}

/// Native provider deletion acknowledgement queued after finalized authorization and byte removal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentDeletionSubmission {
	/// Agreement whose provider must acknowledge content deletion.
	pub agreement_id: String,
	/// Deleted raw-content commitment.
	pub content_commitment: String,
	/// Finalized block that authorized deletion.
	pub authorized_at: String,
	/// Provider root after appending the tombstone leaf.
	pub tombstone_root: String,
	/// Domain-separated agreement/content/provider/root binding checked by the runtime.
	pub proof_commitment: String,
}

/// Versioned typed provider transaction request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "request", rename_all = "snake_case")]
pub enum ProviderSubmission {
	/// Native `StorageProvider::submit_checkpoint` request.
	Checkpoint(CheckpointSubmission),
	/// Native provider content-deletion acknowledgement request.
	ContentDeletion(ContentDeletionSubmission),
}

/// Explicit seam between proof production and signed Orbis extrinsic submission.
#[async_trait]
pub trait CheckpointSubmitter: Send + Sync + 'static {
	/// Durably accept a checkpoint submission. Implementations must be idempotent by challenge id.
	async fn submit(&self, request: CheckpointSubmission) -> Result<(), String>;

	/// Durably accept a content-deletion acknowledgement.
	async fn submit_deletion(&self, request: ContentDeletionSubmission) -> Result<(), String>;
}

/// Append-only JSONL outbox for a separately governed CORD Orbis transaction worker.
///
/// This keeps the provider service from inventing nonce/signer behavior. The downstream worker
/// submits `StorageProvider::submit_checkpoint`, waits for finality, and records the challenge id
/// before acknowledging/removing an outbox item.
pub struct JsonlCheckpointOutbox {
	path: PathBuf,
	write_lock: Mutex<()>,
}

impl JsonlCheckpointOutbox {
	/// Create an outbox at `path`; parent directories are created on first submission.
	pub fn new(path: impl AsRef<Path>) -> Self {
		Self { path: path.as_ref().to_path_buf(), write_lock: Mutex::new(()) }
	}
}

#[async_trait]
impl CheckpointSubmitter for JsonlCheckpointOutbox {
	async fn submit(&self, request: CheckpointSubmission) -> Result<(), String> {
		let _guard = self.write_lock.lock().await;
		let mut encoded = serde_json::to_vec(&ProviderSubmission::Checkpoint(request))
			.map_err(|error| error.to_string())?;
		self.append(&mut encoded).await
	}

	async fn submit_deletion(&self, request: ContentDeletionSubmission) -> Result<(), String> {
		let _guard = self.write_lock.lock().await;
		let mut encoded = serde_json::to_vec(&ProviderSubmission::ContentDeletion(request))
			.map_err(|error| error.to_string())?;
		self.append(&mut encoded).await
	}
}

impl JsonlCheckpointOutbox {
	async fn append(&self, encoded: &mut Vec<u8>) -> Result<(), String> {
		if let Some(parent) = self.path.parent() {
			tokio::fs::create_dir_all(parent).await.map_err(|error| error.to_string())?;
		}
		encoded.push(b'\n');
		let mut file = tokio::fs::OpenOptions::new()
			.create(true)
			.append(true)
			.open(&self.path)
			.await
			.map_err(|error| error.to_string())?;
		file.write_all(&encoded).await.map_err(|error| error.to_string())?;
		file.sync_all().await.map_err(|error| error.to_string())
	}
}

/// Bounded worker cadence.
#[derive(Clone, Debug)]
pub struct WorkerConfig {
	/// Local signed-root checkpoint cadence.
	pub checkpoint_interval: Duration,
	/// Finalized challenge polling cadence.
	pub challenge_interval: Duration,
	/// Replica/index integrity observation cadence.
	pub replica_observation_interval: Duration,
}

impl Default for WorkerConfig {
	fn default() -> Self {
		Self {
			checkpoint_interval: Duration::from_secs(60),
			challenge_interval: Duration::from_secs(6),
			replica_observation_interval: Duration::from_secs(30),
		}
	}
}

/// Run checkpoint, finalized challenge-responder and replica-sync coordinators until cancelled.
pub async fn run_workers<A: ChainAuthority>(
	service: Arc<ProviderService<A>>,
	config: WorkerConfig,
) {
	let mut checkpoint = interval(config.checkpoint_interval);
	checkpoint.set_missed_tick_behavior(MissedTickBehavior::Skip);
	let mut challenge = interval(config.challenge_interval);
	challenge.set_missed_tick_behavior(MissedTickBehavior::Skip);
	let mut replica = interval(config.replica_observation_interval);
	replica.set_missed_tick_behavior(MissedTickBehavior::Skip);
	let mut last_scanned_due_block = None;
	loop {
		tokio::select! {
			_ = checkpoint.tick() => {
				if let Err(error) = service.sign_checkpoint() {
					eprintln!("checkpoint coordinator failed: {error}");
				}
				if let Err(error) = recover_pending_deletions(&service).await {
					eprintln!("deletion outbox recovery failed: {error}");
				}
			},
			_ = challenge.tick() => {
				match service.authority().challenge_duties(last_scanned_due_block).await {
					Ok(batch) => {
						let mut accepted = true;
					for duty in batch.duties {
						let root = duty.expected_commitment.trim_start_matches("0x");
						if !service.store().stats().is_ok_and(|stats| stats.root == root) {
							eprintln!("challenge responder root mismatch for {}", duty.challenge_id);
							accepted = false;
							continue;
						}
						if let Err(error) = service.store().read(&duty.content_commitment) {
							eprintln!(
								"challenge responder content verification failed for {}: {error}",
								duty.challenge_id,
							);
							accepted = false;
							continue;
						}
						match service.sign_checkpoint() {
							Ok(checkpoint) => match challenge_proof_commitment(
								&duty,
								&service.store().profile().map(|profile| profile.provider),
							) {
								Ok(proof_commitment) => {
									let request = CheckpointSubmission { proof_commitment, duty, checkpoint };
									if let Err(error) = service.outbox().submit(request).await {
										eprintln!("challenge responder outbox failed: {error}");
										accepted = false;
									}
								},
								Err(error) => {
									eprintln!("challenge responder proof failed: {error}");
									accepted = false;
								},
							},
							Err(error) => {
								eprintln!("challenge responder checkpoint failed: {error}");
								accepted = false;
							},
						}
						}
						if accepted {
							last_scanned_due_block = Some(batch.scanned_through);
						}
					},
					Err(error) => eprintln!("challenge responder finalized scan failed: {error}"),
				}
			},
			_ = replica.tick() => {
				if let Err(error) = service.store().stats().and_then(|_| service.store().peaks().map(|_| ())) {
					eprintln!("replica sync coordinator detected local inconsistency: {error}");
				}
			},
		}
	}
}

fn challenge_proof_commitment(
	duty: &ChallengeDuty,
	provider: &Result<String, crate::StoreError>,
) -> Result<String, String> {
	let challenge = decode_h256(&duty.challenge_id)?;
	let agreement = decode_h256(&duty.agreement_id)?;
	let content = decode_h256(&duty.content_commitment)?;
	let root = decode_h256(&duty.expected_commitment)?;
	let provider = provider.as_ref().map_err(ToString::to_string)?;
	let raw = hex::decode(provider.trim_start_matches("0x")).map_err(|error| error.to_string())?;
	let provider = AccountId32::new(
		raw.try_into().map_err(|_| "provider must be exactly 32 bytes".to_string())?,
	);
	let encoded =
		(b"orbis/provider-challenge-proof/v1", challenge, agreement, content, &provider, root)
			.encode();
	Ok(format!("0x{}", hex::encode(sp_crypto_hashing::blake2_256(&encoded))))
}

fn decode_h256(value: &str) -> Result<H256, String> {
	let raw = hex::decode(value.trim_start_matches("0x")).map_err(|error| error.to_string())?;
	let raw: [u8; 32] = raw.try_into().map_err(|_| "hash must be exactly 32 bytes".to_string())?;
	Ok(H256::from(raw))
}

pub(crate) fn deletion_submission(
	pending: &PendingDeletion,
	provider: &str,
) -> Result<ContentDeletionSubmission, String> {
	let agreement = decode_h256(&pending.agreement_id)?;
	let content = decode_h256(&pending.commitment)?;
	let root = decode_h256(&pending.tombstone_root)?;
	let raw = hex::decode(provider.trim_start_matches("0x")).map_err(|error| error.to_string())?;
	let provider = AccountId32::new(
		raw.try_into().map_err(|_| "provider must be exactly 32 bytes".to_string())?,
	);
	let encoded =
		(b"orbis/provider-deletion-proof/v1", agreement, content, &provider, root).encode();
	Ok(ContentDeletionSubmission {
		agreement_id: pending.agreement_id.clone(),
		content_commitment: format!("0x{}", pending.commitment),
		authorized_at: pending.authorized_at.clone(),
		tombstone_root: format!("0x{}", pending.tombstone_root),
		proof_commitment: format!("0x{}", hex::encode(sp_crypto_hashing::blake2_256(&encoded))),
	})
}

async fn recover_pending_deletions<A: ChainAuthority>(
	service: &ProviderService<A>,
) -> Result<(), String> {
	let provider = service.store().profile().map_err(|error| error.to_string())?.provider;
	for pending in service.store().pending_deletions().map_err(|error| error.to_string())? {
		let submission = deletion_submission(&pending, &provider)?;
		service.outbox().submit_deletion(submission).await?;
		service
			.store()
			.complete_delete(&pending.commitment)
			.map_err(|error| error.to_string())?;
	}
	Ok(())
}
