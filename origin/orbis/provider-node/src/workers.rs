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

use crate::{
	ChainAuthority, ChallengeDuty, PendingDeletion, PendingRootSubmission, ProviderService,
	SignedCheckpoint,
};

const MAX_CHECKPOINT_DUTY_PAGES_PER_POLL: usize = 4_096;

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
	/// Exact tombstone leaf appended by the preceding provider-root call.
	pub tombstone_leaf: String,
	/// Provider root sequence which must already be finalized on Orbis.
	pub root_sequence: u64,
	/// Canonical tombstone leaf index.
	pub leaf_index: u64,
	/// Total leaves covered by the committed root.
	pub leaf_count: u64,
	/// Bounded leaf-to-root sibling hashes.
	pub inclusion_proof: Vec<String>,
}

/// Provider-authenticated append-only root which must finalize before a deletion acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRootSubmission {
	/// Monotonic provider-authenticated submission sequence.
	pub sequence: u64,
	/// Exact leaf values appended and folded by the runtime accumulator.
	pub appended_leaves: Vec<String>,
	/// Locally derived expected root retained for finalized-result audit.
	pub expected_root: String,
	/// Locally derived expected leaf count retained for finalized-result audit.
	pub expected_leaf_count: u64,
}

/// Versioned typed provider transaction request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "request", rename_all = "snake_case")]
pub enum ProviderSubmission {
	/// Native `StorageProvider::submit_checkpoint` request.
	Checkpoint(CheckpointSubmission),
	/// Native `StorageProvider::commit_provider_root` request.
	ProviderRoot(ProviderRootSubmission),
	/// Native provider content-deletion acknowledgement request.
	ContentDeletion(ContentDeletionSubmission),
}

/// Explicit seam between proof production and signed Orbis extrinsic submission.
#[async_trait]
pub trait CheckpointSubmitter: Send + Sync + 'static {
	/// Durably accept a checkpoint submission. Implementations must be idempotent by challenge id.
	async fn submit(&self, request: CheckpointSubmission) -> Result<(), String>;

	/// Durably accept an append-only provider root update.
	async fn submit_root(&self, request: ProviderRootSubmission) -> Result<(), String>;

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
	#[cfg(test)]
	fail_next_directory_sync: std::sync::atomic::AtomicBool,
}

impl JsonlCheckpointOutbox {
	/// Create an outbox at `path`; parent directories are created on first submission.
	pub fn new(path: impl AsRef<Path>) -> Self {
		Self {
			path: path.as_ref().to_path_buf(),
			write_lock: Mutex::new(()),
			#[cfg(test)]
			fail_next_directory_sync: std::sync::atomic::AtomicBool::new(false),
		}
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

	async fn submit_root(&self, request: ProviderRootSubmission) -> Result<(), String> {
		let _guard = self.write_lock.lock().await;
		let mut encoded = serde_json::to_vec(&ProviderSubmission::ProviderRoot(request))
			.map_err(|error| error.to_string())?;
		self.append(&mut encoded).await
	}

	async fn submit_deletion(&self, request: ContentDeletionSubmission) -> Result<(), String> {
		let _guard = self.write_lock.lock().await;
		let root = ProviderRootSubmission {
			sequence: request.root_sequence,
			appended_leaves: vec![request.tombstone_leaf.clone()],
			expected_root: request.tombstone_root.clone(),
			expected_leaf_count: request.leaf_count,
		};
		let mut encoded = serde_json::to_vec(&ProviderSubmission::ProviderRoot(root))
			.map_err(|error| error.to_string())?;
		self.append(&mut encoded).await?;
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
		repair_incomplete_jsonl_tail(&self.path).await?;
		encoded.push(b'\n');
		let mut file = tokio::fs::OpenOptions::new()
			.create(true)
			.append(true)
			.open(&self.path)
			.await
			.map_err(|error| error.to_string())?;
		file.write_all(&encoded).await.map_err(|error| error.to_string())?;
		file.sync_all().await.map_err(|error| error.to_string())?;
		#[cfg(test)]
		if self.fail_next_directory_sync.swap(false, std::sync::atomic::Ordering::SeqCst) {
			return Err("injected parent directory sync failure".into());
		}
		sync_parent_directory(&self.path).await
	}
}

async fn sync_parent_directory(path: &Path) -> Result<(), String> {
	let parent = path
		.parent()
		.filter(|parent| !parent.as_os_str().is_empty())
		.unwrap_or(Path::new("."));
	let directory = tokio::fs::File::open(parent).await.map_err(|error| error.to_string())?;
	directory.sync_all().await.map_err(|error| error.to_string())
}

async fn repair_incomplete_jsonl_tail(path: &Path) -> Result<(), String> {
	let bytes = match tokio::fs::read(path).await {
		Ok(bytes) => bytes,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
		Err(error) => return Err(error.to_string()),
	};
	if bytes.is_empty() || bytes.ends_with(b"\n") {
		return Ok(());
	}
	let keep = bytes.iter().rposition(|byte| *byte == b'\n').map_or(0, |index| index + 1);
	let file = tokio::fs::OpenOptions::new()
		.write(true)
		.open(path)
		.await
		.map_err(|error| error.to_string())?;
	file.set_len(keep as u64).await.map_err(|error| error.to_string())?;
	file.sync_all().await.map_err(|error| error.to_string())
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
	/// Finalized checkpoint-duty intake cadence.
	pub checkpoint_duty_interval: Duration,
}

impl Default for WorkerConfig {
	fn default() -> Self {
		Self {
			checkpoint_interval: Duration::from_secs(60),
			challenge_interval: Duration::from_secs(6),
			replica_observation_interval: Duration::from_secs(30),
			checkpoint_duty_interval: Duration::from_secs(6),
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
	let mut checkpoint_duty = interval(config.checkpoint_duty_interval);
	checkpoint_duty.set_missed_tick_behavior(MissedTickBehavior::Skip);
	let mut last_scanned_due_block = None;
	loop {
		tokio::select! {
			_ = checkpoint.tick() => {
				if let Err(error) = service.sign_checkpoint() {
					eprintln!("checkpoint coordinator failed: {error}");
				}
				if let Err(error) = recover_pending_submissions(&service).await {
					eprintln!("provider outbox recovery failed: {error}");
				}
			},
			_ = challenge.tick() => {
				match service.authority().challenge_duties(last_scanned_due_block).await {
					Ok(batch) => {
						let mut accepted = true;
					for duty in batch.duties {
						let checkpoint = match checkpoint_for_duty(&service, &duty) {
							Ok(checkpoint) => checkpoint,
							Err(error) => {
								eprintln!("challenge responder local proof failed for {}: {error}", duty.challenge_id);
								accepted = false;
								continue;
							},
						};
						match challenge_proof_commitment(
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
			_ = checkpoint_duty.tick() => {
				if let Err(error) = poll_checkpoint_duties_once(&service).await {
					eprintln!("checkpoint duty intake failed: {error}");
				}
			},
		}
	}
}

/// Poll and durably stage one complete fixed-finalized-state checkpoint-duty snapshot.
///
/// Each non-terminal page is fsynced before its exact cursor is used. A retry or restart resumes
/// the same finalized hash; pending duties become visible only after the terminal page is
/// installed.
pub async fn poll_checkpoint_duties_once<A: ChainAuthority>(
	service: &ProviderService<A>,
) -> Result<usize, String> {
	let mut installed = 0usize;
	for _ in 0..MAX_CHECKPOINT_DUTY_PAGES_PER_POLL {
		let request = service
			.store()
			.checkpoint_duty_resume_request()
			.map_err(|error| error.to_string())?;
		let page = service
			.authority()
			.checkpoint_duties(request)
			.await
			.map_err(|error| error.to_string())?;
		installed = installed.saturating_add(page.duties.len());
		if service
			.store()
			.stage_checkpoint_duty_page(page)
			.map_err(|error| error.to_string())?
		{
			return Ok(installed);
		}
	}
	Err("checkpoint duty page bound exceeded".into())
}

fn checkpoint_for_duty<A: ChainAuthority>(
	service: &ProviderService<A>,
	duty: &ChallengeDuty,
) -> Result<SignedCheckpoint, String> {
	let observation = service
		.store()
		.root_observation(&duty.expected_commitment)
		.map_err(|error| error.to_string())?;
	service
		.store()
		.read(&duty.content_commitment)
		.map_err(|error| error.to_string())?;
	service.sign_root_checkpoint(observation).map_err(|error| error.to_string())
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
) -> Result<ContentDeletionSubmission, String> {
	Ok(ContentDeletionSubmission {
		agreement_id: pending.agreement_id.clone(),
		content_commitment: format!("0x{}", pending.commitment),
		authorized_at: pending.authorized_at.clone(),
		tombstone_root: format!("0x{}", pending.tombstone_root),
		root_sequence: pending.root_sequence,
		tombstone_leaf: format!("0x{}", pending.tombstone_leaf),
		leaf_index: pending.leaf_index,
		leaf_count: pending.leaf_count,
		inclusion_proof: pending.inclusion_proof.iter().map(|hash| format!("0x{hash}")).collect(),
	})
}

async fn recover_pending_submissions<A: ChainAuthority>(
	service: &ProviderService<A>,
) -> Result<(), String> {
	let _root_order = service.root_outbox_lock().lock().await;
	flush_pending_submissions(service.store(), service.outbox().as_ref()).await
}

/// Flush every journaled root in ascending sequence order while the caller holds the service's
/// shared root/outbox ordering lock. A deletion root and acknowledgement are one indivisible
/// ordering unit; failures stop the scan before any later sequence can be queued or cleared.
pub(crate) async fn flush_pending_submissions(
	store: &crate::DiskStore,
	outbox: &dyn CheckpointSubmitter,
) -> Result<(), String> {
	let deletions = store.pending_deletions().map_err(|error| error.to_string())?;
	for pending in store.pending_root_submissions().map_err(|error| error.to_string())? {
		if let Some(deletion) = deletions.iter().find(|item| item.root_sequence == pending.sequence)
		{
			outbox.submit_deletion(deletion_submission(deletion)?).await?;
			store.complete_delete(&deletion.commitment).map_err(|error| error.to_string())?;
			store
				.complete_root_submission(pending.sequence)
				.map_err(|error| error.to_string())?;
		} else {
			outbox.submit_root(root_submission(&pending)).await?;
			store
				.complete_root_submission(pending.sequence)
				.map_err(|error| error.to_string())?;
		}
	}
	Ok(())
}

pub(crate) fn root_submission(pending: &PendingRootSubmission) -> ProviderRootSubmission {
	ProviderRootSubmission {
		sequence: pending.sequence,
		appended_leaves: pending.appended_leaves.iter().map(|leaf| format!("0x{leaf}")).collect(),
		expected_root: format!("0x{}", pending.expected_root),
		expected_leaf_count: pending.expected_leaf_count,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::{
		atomic::{AtomicBool, Ordering},
		Mutex as StdMutex,
	};

	use crate::{
		AgreementAuthorization, ChainError, ChallengeBatch, CommitInput, DiskStore, NodeProfile,
	};
	use sp_core::Pair as _;

	#[derive(Default)]
	struct FaultOutbox {
		log: StdMutex<Vec<String>>,
		fail_next_root: AtomicBool,
		partial_delete_once: AtomicBool,
	}

	struct NoopAuthority;

	#[async_trait]
	impl ChainAuthority for NoopAuthority {
		async fn authorize_commit(
			&self,
			_agreement_id: [u8; 32],
			_content_commitment: [u8; 32],
			_bytes: u64,
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}

		async fn authorize_delete(
			&self,
			_agreement_id: [u8; 32],
			_content_commitment: [u8; 32],
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}

		async fn challenge_duties(
			&self,
			_after_block: Option<u32>,
		) -> Result<ChallengeBatch, ChainError> {
			Err(ChainError::Rejected("unused test authority".into()))
		}

		async fn checkpoint_duties(
			&self,
			_request: Option<crate::CheckpointDutyPageRequest>,
		) -> Result<crate::CheckpointDutyBatch, ChainError> {
			Err(ChainError::Decode("injected checkpoint duty decode failure".into()))
		}
	}

	impl FaultOutbox {
		fn log(&self) -> Vec<String> {
			self.log.lock().unwrap().clone()
		}
	}

	#[async_trait]
	impl CheckpointSubmitter for FaultOutbox {
		async fn submit(&self, _request: CheckpointSubmission) -> Result<(), String> {
			Ok(())
		}

		async fn submit_root(&self, request: ProviderRootSubmission) -> Result<(), String> {
			if self.fail_next_root.swap(false, Ordering::SeqCst) {
				return Err("injected root failure".into());
			}
			self.log.lock().unwrap().push(format!("root-{}", request.sequence));
			Ok(())
		}

		async fn submit_deletion(&self, request: ContentDeletionSubmission) -> Result<(), String> {
			self.log.lock().unwrap().push(format!("root-{}", request.root_sequence));
			if self.partial_delete_once.swap(false, Ordering::SeqCst) {
				return Err("injected failure after deletion root".into());
			}
			self.log.lock().unwrap().push(format!("ack-{}", request.root_sequence));
			Ok(())
		}
	}

	fn profile() -> NodeProfile {
		NodeProfile {
			provider: "01".repeat(32),
			endpoint: "http://127.0.0.1:8080".into(),
			service_key: "02".repeat(32),
			region: None,
		}
	}

	fn authorization(commitment: [u8; 32], bytes: u64) -> AgreementAuthorization {
		AgreementAuthorization {
			finalized_hash: format!("0x{}", "11".repeat(32)),
			agreement_id: format!("0x{}", hex::encode(commitment)),
			provider: profile().provider,
			container_ref: format!("0x{}", "03".repeat(32)),
			bytes,
			expires_at: 100,
		}
	}

	fn commit(store: &DiskStore, value: u8) -> (String, AgreementAuthorization) {
		let bytes = vec![value];
		let commitment = DiskStore::content_commitment(&bytes);
		let authorization = authorization(commitment, bytes.len() as u64);
		let record = store
			.commit(CommitInput {
				commitment,
				authorization: authorization.clone(),
				bucket: None,
				key: None,
				bytes,
			})
			.unwrap();
		(record.commitment, authorization)
	}

	fn service(store: Arc<DiskStore>) -> ProviderService<NoopAuthority> {
		ProviderService::new(
			store,
			Arc::new(NoopAuthority),
			sp_core::sr25519::Pair::from_seed(&[7u8; 32]),
			Arc::new(FaultOutbox::default()),
		)
	}

	#[tokio::test]
	async fn checkpoint_duty_authority_failure_cannot_advance_durable_intake() {
		let temp = tempfile::tempdir().unwrap();
		let store = Arc::new(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let service = service(store.clone());
		assert!(poll_checkpoint_duties_once(&service).await.is_err());
		assert!(store.checkpoint_duty_resume_request().unwrap().is_none());
		assert!(store.checkpoint_duty_watermark().unwrap().is_none());
		assert!(store.pending_checkpoint_duties().unwrap().is_empty());
	}

	#[tokio::test]
	async fn deletion_outbox_durably_orders_root_before_acknowledgement() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("outbox.jsonl");
		let outbox = JsonlCheckpointOutbox::new(&path);
		let request = ContentDeletionSubmission {
			agreement_id: format!("0x{}", "01".repeat(32)),
			content_commitment: format!("0x{}", "02".repeat(32)),
			authorized_at: format!("0x{}", "03".repeat(32)),
			tombstone_root: format!("0x{}", "04".repeat(32)),
			root_sequence: 3,
			tombstone_leaf: format!("0x{}", "06".repeat(32)),
			leaf_index: 2,
			leaf_count: 3,
			inclusion_proof: vec![format!("0x{}", "05".repeat(32))],
		};
		outbox.submit_deletion(request).await.unwrap();
		let lines: Vec<_> = tokio::fs::read_to_string(path)
			.await
			.unwrap()
			.lines()
			.map(|line| serde_json::from_str::<ProviderSubmission>(line).unwrap())
			.collect();
		assert!(matches!(lines[0], ProviderSubmission::ProviderRoot(_)));
		assert!(matches!(lines[1], ProviderSubmission::ContentDeletion(_)));
	}

	#[tokio::test]
	async fn producer_repairs_torn_final_submission_before_ordered_retry() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("outbox.jsonl");
		let outbox = JsonlCheckpointOutbox::new(&path);
		let root = |sequence| ProviderRootSubmission {
			sequence,
			appended_leaves: vec![format!("0x{}", "01".repeat(32))],
			expected_root: format!("0x{}", "02".repeat(32)),
			expected_leaf_count: sequence,
		};
		outbox.submit_root(root(1)).await.unwrap();
		let mut file = tokio::fs::OpenOptions::new().append(true).open(&path).await.unwrap();
		file.write_all(b"{\"kind\":\"provider_root\"").await.unwrap();
		file.sync_all().await.unwrap();
		outbox.submit_root(root(2)).await.unwrap();
		let submissions: Vec<_> = tokio::fs::read_to_string(&path)
			.await
			.unwrap()
			.lines()
			.map(|line| serde_json::from_str::<ProviderSubmission>(line).unwrap())
			.collect();
		assert_eq!(submissions.len(), 2);
		assert!(
			matches!(&submissions[0], ProviderSubmission::ProviderRoot(root) if root.sequence == 1)
		);
		assert!(
			matches!(&submissions[1], ProviderSubmission::ProviderRoot(root) if root.sequence == 2)
		);
	}

	#[tokio::test]
	async fn producer_never_reports_success_before_parent_directory_sync() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("outbox.jsonl");
		let outbox = JsonlCheckpointOutbox::new(&path);
		outbox.fail_next_directory_sync.store(true, Ordering::SeqCst);
		let result = outbox
			.submit_root(ProviderRootSubmission {
				sequence: 1,
				appended_leaves: vec![format!("0x{}", "01".repeat(32))],
				expected_root: format!("0x{}", "02".repeat(32)),
				expected_leaf_count: 1,
			})
			.await;
		assert_eq!(result.unwrap_err(), "injected parent directory sync failure");
		assert!(tokio::fs::read(&path).await.unwrap().ends_with(b"\n"));
	}

	#[test]
	fn deletion_checkpoint_uses_exact_append_log_leaf_count() {
		let temp = tempfile::tempdir().unwrap();
		let store = Arc::new(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let (commitment, authorization) = commit(&store, 9);
		store.prepare_delete(&commitment, &authorization).unwrap();
		let stats = store.stats().unwrap();
		assert_eq!(stats.live_objects, 0);
		assert_eq!(stats.deleted_objects, 1);
		assert_eq!(stats.proof_leaf_count, 2);
		let checkpoint = service(store.clone()).sign_checkpoint().unwrap();
		assert_eq!(checkpoint.root, stats.root);
		assert_eq!(checkpoint.leaves, 2);
		assert_eq!(store.latest_checkpoint().unwrap(), checkpoint);
	}

	#[test]
	fn historical_root_checkpoint_survives_later_append_and_unknown_root_fails() {
		let temp = tempfile::tempdir().unwrap();
		let store = Arc::new(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		let (first, _) = commit(&store, 10);
		let first_stats = store.stats().unwrap();
		commit(&store, 11);
		assert_ne!(store.stats().unwrap().root, first_stats.root);
		assert!(store.read(&first).is_ok());
		let observation = store.root_observation(&first_stats.root).unwrap();
		assert_eq!(observation.leaf_count, 1);
		let checkpoint = service(store.clone()).sign_root_checkpoint(observation).unwrap();
		assert_eq!(checkpoint.root, first_stats.root);
		assert_eq!(checkpoint.leaves, 1);
		assert!(store.root_observation(&"ff".repeat(32)).is_err());
	}

	#[tokio::test]
	async fn failed_root_is_flushed_before_a_later_commit_or_delete_sequence() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let outbox = FaultOutbox::default();
		outbox.fail_next_root.store(true, Ordering::SeqCst);
		let (_first, _) = commit(&store, 1);
		assert!(flush_pending_submissions(&store, &outbox).await.is_err());
		assert_eq!(store.pending_root_submissions().unwrap()[0].sequence, 1);

		// This is the pre-mutation flush performed by /commit and /delete.
		flush_pending_submissions(&store, &outbox).await.unwrap();
		let (second, second_authorization) = commit(&store, 2);
		flush_pending_submissions(&store, &outbox).await.unwrap();
		assert_eq!(outbox.log(), vec!["root-1", "root-2"]);

		store.prepare_delete(&second, &second_authorization).unwrap();
		flush_pending_submissions(&store, &outbox).await.unwrap();
		assert_eq!(outbox.log(), vec!["root-1", "root-2", "root-3", "ack-3"]);
		assert!(store.pending_root_submissions().unwrap().is_empty());
		assert!(store.pending_deletions().unwrap().is_empty());
	}

	#[tokio::test]
	async fn failed_root_is_flushed_before_delete_is_created() {
		let temp = tempfile::tempdir().unwrap();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let outbox = FaultOutbox::default();
		outbox.fail_next_root.store(true, Ordering::SeqCst);
		let (commitment, authorization) = commit(&store, 4);
		assert!(flush_pending_submissions(&store, &outbox).await.is_err());
		flush_pending_submissions(&store, &outbox).await.unwrap();
		store.prepare_delete(&commitment, &authorization).unwrap();
		flush_pending_submissions(&store, &outbox).await.unwrap();
		assert_eq!(outbox.log(), vec!["root-1", "root-2", "ack-2"]);
	}

	#[tokio::test]
	async fn partial_deletion_submission_recovers_after_restart_before_later_sequence() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = FaultOutbox::default();
		let store = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		let (commitment, authorization) = commit(&store, 5);
		flush_pending_submissions(&store, &outbox).await.unwrap();
		store.prepare_delete(&commitment, &authorization).unwrap();
		outbox.partial_delete_once.store(true, Ordering::SeqCst);
		assert!(flush_pending_submissions(&store, &outbox).await.is_err());
		assert_eq!(outbox.log(), vec!["root-1", "root-2"]);
		assert_eq!(store.pending_root_submissions().unwrap()[0].sequence, 2);
		drop(store);

		let reopened = DiskStore::open(temp.path(), profile(), 1024).unwrap();
		flush_pending_submissions(&reopened, &outbox).await.unwrap();
		commit(&reopened, 6);
		flush_pending_submissions(&reopened, &outbox).await.unwrap();
		assert_eq!(outbox.log(), vec!["root-1", "root-2", "root-2", "ack-2", "root-3"]);
		assert!(reopened.pending_root_submissions().unwrap().is_empty());
	}

	#[test]
	fn shared_deletion_vector_recomputes_leaf_proof_and_root_cryptographically() {
		let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../../docs/sdk/vectors/storage-provider-deletion-v1.json");
		let vector: serde_json::Value =
			serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
		let hash = |value: &str| -> [u8; 32] {
			hex::decode(value.trim_start_matches("0x")).unwrap().try_into().unwrap()
		};
		let input = &vector["input"];
		let agreement = H256::from(hash(input["agreement_id"].as_str().unwrap()));
		let content = H256::from(hash(input["content_commitment"].as_str().unwrap()));
		let provider = AccountId32::new(hash(input["provider_account_id32"].as_str().unwrap()));
		let nonce = input["deletion_nonce"].as_str().unwrap().parse::<u64>().unwrap();
		let tombstone = sp_crypto_hashing::blake2_256(
			&(b"orbis/provider-deletion-leaf/v1", agreement, content, &provider, nonce).encode(),
		);
		assert_eq!(
			hex::encode(tombstone),
			vector["expected"]["tombstone_leaf"].as_str().unwrap().trim_start_matches("0x")
		);
		let leaves: Vec<[u8; 32]> = vector["append_only_accumulator"]["sequence_1"]
			["appended_leaves"]
			.as_array()
			.unwrap()
			.iter()
			.map(|value| hash(value.as_str().unwrap()))
			.collect();
		assert_eq!(leaves[1], tombstone);
		let proof = crate::merkle::proof(&leaves, 1).unwrap();
		let expected_proof: Vec<[u8; 32]> = vector["expected"]["inclusion_proof"]
			.as_array()
			.unwrap()
			.iter()
			.map(|value| hash(value.as_str().unwrap()))
			.collect();
		assert_eq!(proof, expected_proof);
		assert_eq!(
			crate::merkle::root(&leaves),
			hash(vector["expected"]["tombstone_root"].as_str().unwrap())
		);
	}
}
