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

//! Deterministic three-provider duty, promotion and repair evidence.

use std::{
	collections::BTreeSet,
	fs,
	path::Path,
	sync::{
		atomic::{AtomicUsize, Ordering},
		Arc, Mutex,
	},
	time::Duration,
};

use async_trait::async_trait;
use codec::{Decode, Encode};
use frame_metadata::v15::{
	CustomMetadata, ExtrinsicMetadata, OuterEnums, PalletCallMetadata, PalletMetadata,
	RuntimeMetadataV15,
};
use orbis_storage_runtime_api::{
	CheckpointDutyInfo, CheckpointDutyMode as RuntimeMode, CheckpointDutyPhase as RuntimePhase,
	CheckpointInfo, CommitmentInfo, ProviderDutyAuthority, ProviderDutyExclusion, ProviderDutyRole,
	Versioned, RESPONSE_VERSION,
};
use pallet_orbis_storage_provider::{
	CheckpointFallbackPromotionV1, CommitmentPayloadV2, ReplicaSignature,
};
use scale_info::{meta_type, TypeInfo};
use serde::Serialize;
use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
use sp_crypto_hashing::blake2_256;

use crate::{
	chain::{
		validate_checkpoint_duty, ChainError, CheckpointPublicationAuthority,
		FinalizedCheckpointObservation, ReplicationAuthority, ReplicationProviderExclusion,
		ReplicationProviderSnapshot, ReplicationTopologySnapshot,
	},
	checkpoint::{
		checkpoint_outbox::{
			finality_attestation_digest, CheckpointSubmissionV2, FINALITY_ATTESTATION_VERSION,
			FINALIZED_STATE,
		},
		checkpoint_promotion::{promotion_finality_attestation_digest, FallbackPromotionIntentV2},
		checkpoint_promotion_submitter::PromotionFinalityLane,
		checkpoint_quorum::ReplicaConfirmationRequestV1,
		checkpoint_submitter::{CheckpointFinalityLane, FinalizedEvidence},
	},
	checkpoint_promotion_worker::PromotionDiscoveryScheduler,
	checkpoint_stack::CheckpointStack,
	checkpoint_transport::{
		CheckpointConfirmationEndpoint, CheckpointConfirmationTransport, CheckpointTransportError,
	},
	content::MAX_STORED_BYTES,
	peer::PeerMmrCommitmentV1,
	peer_responder::PeerResponder,
	peer_transport::{PeerTransport, PeerTransportError},
	replication::ReplicationPhase,
	replication_reconciler::ReplicationReconciler,
	replication_session::ReplicationSessionV1,
	replication_worker::select_source,
	storage::bucket_mmr::BucketMmrStore,
	workers::CheckpointSubmitter,
	AgreementAuthorization, BucketId, CanonicalCid, ChainAuthority, ChallengeBatch,
	CheckpointDutyBatch, CheckpointDutyPageRequest, ContentError, DiskStore, NodeProfile,
	OperationId, ProviderService, StreamingDescriptor, StreamingStore, CHUNK_BYTES,
};

const BUCKET: [u8; 32] = [3; 32];
const PROVIDER_A: [u8; 32] = [4; 32];
const PROVIDER_B: [u8; 32] = [5; 32];
const PROVIDER_C: [u8; 32] = [6; 32];
const INITIAL_SNAPSHOT: u32 = 100;
const INITIAL_DUTY_FINALIZED: u32 = 110;
const INITIAL_CHECKPOINT_FINALIZED: u32 = 120;
const FAILURE_DUTY_FINALIZED: u32 = 220;
const PROMOTION_FINALIZED: u32 = 230;
const PROMOTED_DUTY_FINALIZED: u32 = 240;
const PROMOTED_CHECKPOINT_FINALIZED: u32 = 260;

#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EligibleSourceObservation {
	pub provider: String,
	pub order: u8,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CorruptReadObservation {
	pub result: String,
	pub bytes_returned: usize,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeObservation {
	pub kind: String,
	pub finalized_number: u32,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CheckpointObservation {
	pub phase: String,
	pub duty_id: String,
	pub primary: String,
	pub confirmation_providers: Vec<String>,
	pub submission_id: String,
	pub submission_record_hash: String,
	pub mmr_root: String,
	pub start_seq: u64,
	pub leaf_count: u64,
	pub call_args_blake2_256: String,
	pub before_restart_blake2_256: String,
	pub after_restart_blake2_256: String,
	pub finality_calls: usize,
	pub finalized_number: u32,
	pub publication_count: usize,
	pub replay_finality_calls: usize,
	pub replay_publication_count: usize,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PromotionObservation {
	pub intent_id: String,
	pub provider: String,
	pub finalized_number: u32,
	pub finality_calls: usize,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ThreeProviderRecoveryEvidence {
	pub schema_version: u16,
	pub test: String,
	pub provider_count: usize,
	pub content_cids: Vec<String>,
	pub provider_roots: Vec<String>,
	pub eligible_sources: Vec<EligibleSourceObservation>,
	pub selection_results: Vec<String>,
	pub corrupt_read_observations: Vec<CorruptReadObservation>,
	pub runtime_observations: Vec<RuntimeObservation>,
	pub runtime_duty_reads: usize,
	pub checkpoints: Vec<CheckpointObservation>,
	pub promotion: PromotionObservation,
	pub repaired_read_lengths: Vec<usize>,
	pub replication_network_requests: usize,
}

#[derive(Clone)]
struct ScriptedCommonsAuthority {
	batch: CheckpointDutyBatch,
	pinned: ReplicationTopologySnapshot,
	current: ReplicationTopologySnapshot,
	observation: Arc<Mutex<Option<FinalizedCheckpointObservation>>>,
	duty_reads: Arc<AtomicUsize>,
}

impl ScriptedCommonsAuthority {
	fn new(batch: CheckpointDutyBatch, topology: ReplicationTopologySnapshot) -> Self {
		Self {
			batch,
			pinned: topology.clone(),
			current: topology,
			observation: Arc::new(Mutex::new(None)),
			duty_reads: Arc::new(AtomicUsize::new(0)),
		}
	}

	fn set_observation(&self, observation: FinalizedCheckpointObservation) {
		*self.observation.lock().expect("scripted observation lock") = Some(observation);
	}
}

#[async_trait]
impl ChainAuthority for ScriptedCommonsAuthority {
	async fn authorize_commit(
		&self,
		_: [u8; 32],
		_: [u8; 32],
		_: u64,
	) -> Result<AgreementAuthorization, ChainError> {
		Err(ChainError::Rejected("unused evidence authority operation".into()))
	}

	async fn authorize_delete(
		&self,
		_: [u8; 32],
		_: [u8; 32],
	) -> Result<AgreementAuthorization, ChainError> {
		Err(ChainError::Rejected("unused evidence authority operation".into()))
	}

	async fn challenge_duties(&self, _: Option<u32>) -> Result<ChallengeBatch, ChainError> {
		Err(ChainError::Rejected("unused evidence authority operation".into()))
	}

	async fn checkpoint_duties(
		&self,
		request: Option<CheckpointDutyPageRequest>,
	) -> Result<CheckpointDutyBatch, ChainError> {
		if request.is_some() {
			return Err(ChainError::DutyProtocol("unexpected evidence duty cursor".into()))
		}
		self.duty_reads.fetch_add(1, Ordering::SeqCst);
		Ok(self.batch.clone())
	}
}

#[async_trait]
impl ReplicationAuthority for ScriptedCommonsAuthority {
	async fn replication_topology(
		&self,
		bucket: [u8; 32],
	) -> Result<ReplicationTopologySnapshot, ChainError> {
		(bucket == BUCKET)
			.then(|| self.current.clone())
			.ok_or_else(|| ChainError::Rejected("wrong evidence bucket".into()))
	}

	async fn replication_topology_at(
		&self,
		bucket: [u8; 32],
		finalized_hash: [u8; 32],
		finalized_number: u32,
	) -> Result<ReplicationTopologySnapshot, ChainError> {
		(bucket == BUCKET &&
			finalized_hash == self.pinned.finalized_hash &&
			finalized_number == self.pinned.finalized_number)
			.then(|| self.pinned.clone())
			.ok_or_else(|| ChainError::Rejected("wrong pinned evidence topology".into()))
	}
}

#[async_trait]
impl CheckpointPublicationAuthority for ScriptedCommonsAuthority {
	async fn checkpoint_observation_at(
		&self,
		bucket: [u8; 32],
		finalized_hash: [u8; 32],
		finalized_number: u32,
	) -> Result<FinalizedCheckpointObservation, ChainError> {
		let observation = self
			.observation
			.lock()
			.map_err(|_| ChainError::Rejected("evidence observation lock".into()))?
			.clone()
			.ok_or_else(|| ChainError::Rejected("evidence observation unavailable".into()))?;
		if bucket != BUCKET ||
			observation.finalized_hash != finalized_hash ||
			observation.finalized_number != finalized_number
		{
			return Err(ChainError::Rejected("wrong evidence finality observation".into()))
		}
		Ok(observation)
	}
}

#[derive(Default)]
struct NoopOutbox;

#[async_trait]
impl CheckpointSubmitter for NoopOutbox {
	async fn submit(&self, _: crate::CheckpointSubmission) -> Result<(), String> {
		Ok(())
	}

	async fn submit_root(&self, _: crate::ProviderRootSubmission) -> Result<(), String> {
		Ok(())
	}

	async fn submit_deletion(&self, _: crate::ContentDeletionSubmission) -> Result<(), String> {
		Ok(())
	}
}

struct DirectPeerTransport {
	responder: Arc<PeerResponder<ScriptedCommonsAuthority>>,
	requests: AtomicUsize,
}

#[async_trait]
impl PeerTransport for DirectPeerTransport {
	async fn page(
		&self,
		_: &ReplicationSessionV1,
		request: &[u8],
	) -> Result<Vec<u8>, PeerTransportError> {
		self.requests.fetch_add(1, Ordering::SeqCst);
		self.responder.page(request).await.map_err(|_| PeerTransportError::Response)
	}

	async fn chunk(
		&self,
		_: &ReplicationSessionV1,
		request: &[u8],
	) -> Result<Vec<u8>, PeerTransportError> {
		self.requests.fetch_add(1, Ordering::SeqCst);
		self.responder.chunk(request).await.map_err(|_| PeerTransportError::Response)
	}
}

struct DirectConfirmationTransport {
	responders: Vec<([u8; 32], Arc<PeerResponder<ScriptedCommonsAuthority>>)>,
	partitioned: BTreeSet<[u8; 32]>,
	requests: AtomicUsize,
}

#[async_trait]
impl CheckpointConfirmationTransport for DirectConfirmationTransport {
	async fn confirm(
		&self,
		_: &CheckpointConfirmationEndpoint,
		request: &[u8],
	) -> Result<Vec<u8>, CheckpointTransportError> {
		let decoded = ReplicaConfirmationRequestV1::decode_canonical(request)
			.map_err(|_| CheckpointTransportError::Rejected)?;
		let target = account_bytes(&decoded.target_provider)
			.map_err(|_| CheckpointTransportError::Rejected)?;
		if self.partitioned.contains(&target) {
			return Err(CheckpointTransportError::Timeout)
		}
		let responder = self
			.responders
			.iter()
			.find(|(provider, _)| *provider == target)
			.map(|(_, responder)| responder)
			.ok_or(CheckpointTransportError::Rejected)?;
		self.requests.fetch_add(1, Ordering::SeqCst);
		responder
			.confirmation(request)
			.await
			.map_err(|_| CheckpointTransportError::Rejected)
	}
}

#[allow(non_camel_case_types, dead_code)]
#[derive(TypeInfo)]
enum ExactCall {
	#[codec(index = 19)]
	submit_checkpoint {
		domain: Vec<u8>,
		payload: CommitmentPayloadV2<H256, u32>,
		window_start: u32,
		window_end: u32,
		service_key: ed25519::Public,
		primary_signature: ed25519::Signature,
		primary_context_signature: ed25519::Signature,
		confirmations: Vec<ReplicaSignature<AccountId32>>,
	},
	#[codec(index = 20)]
	promote_checkpoint_fallback {
		payload: CheckpointFallbackPromotionV1<H256, u32>,
		service_key: ed25519::Public,
		signature: ed25519::Signature,
	},
}

struct ScriptedCheckpointLane {
	metadata: subxt::Metadata,
	provider: [u8; 32],
	key: ed25519::Pair,
	finalized_number: u32,
	calls: AtomicUsize,
}

#[async_trait]
impl CheckpointFinalityLane for ScriptedCheckpointLane {
	fn metadata(&self) -> &subxt::Metadata {
		&self.metadata
	}

	fn signer_account(&self) -> [u8; 32] {
		self.provider
	}

	fn service_key(&self) -> [u8; 32] {
		self.key.public().0
	}

	async fn submit_and_finalize(
		&self,
		intent_id: &str,
		_: subxt::tx::DynamicPayload,
	) -> Result<FinalizedEvidence, ContentError> {
		self.calls.fetch_add(1, Ordering::SeqCst);
		let submission_id = intent_id
			.strip_prefix("orbis-checkpoint-v2-")
			.ok_or(ContentError::IntegrityFailed)?;
		let block_hash = blake2_256(&self.finalized_number.to_le_bytes());
		let extrinsic_hash = blake2_256(intent_id.as_bytes());
		let digest = finality_attestation_digest(
			FINALITY_ATTESTATION_VERSION,
			submission_id,
			block_hash,
			self.finalized_number,
			extrinsic_hash,
			FINALIZED_STATE,
		)?;
		Ok(FinalizedEvidence {
			block_hash,
			block_number: self.finalized_number,
			extrinsic_hash,
			finality_attestation_version: FINALITY_ATTESTATION_VERSION,
			finality_signature: self.key.sign(&digest).0,
		})
	}
}

struct ScriptedPromotionLane {
	metadata: subxt::Metadata,
	provider: [u8; 32],
	key: ed25519::Pair,
	finalized_number: u32,
	calls: AtomicUsize,
}

#[async_trait]
impl PromotionFinalityLane for ScriptedPromotionLane {
	fn metadata(&self) -> &subxt::Metadata {
		&self.metadata
	}

	fn signer_account(&self) -> [u8; 32] {
		self.provider
	}

	fn service_key(&self) -> [u8; 32] {
		self.key.public().0
	}

	async fn submit_and_finalize(
		&self,
		_: &str,
		intent: &FallbackPromotionIntentV2,
		_: subxt::tx::DynamicPayload,
	) -> Result<FinalizedEvidence, ContentError> {
		self.calls.fetch_add(1, Ordering::SeqCst);
		let block_hash = blake2_256(&self.finalized_number.to_le_bytes());
		let extrinsic_hash = blake2_256(intent.intent_id.as_bytes());
		let digest = promotion_finality_attestation_digest(
			FINALITY_ATTESTATION_VERSION,
			&intent.intent_id,
			&intent.record_hash,
			&intent.tuple_key,
			block_hash,
			self.finalized_number,
			extrinsic_hash,
			FINALIZED_STATE,
		)?;
		Ok(FinalizedEvidence {
			block_hash,
			block_number: self.finalized_number,
			extrinsic_hash,
			finality_attestation_version: FINALITY_ATTESTATION_VERSION,
			finality_signature: self.key.sign(&digest).0,
		})
	}
}

/// Execute one exact deterministic duty, promotion, repair and finality scenario.
pub async fn run_three_provider_recovery_evidence(
	root: impl AsRef<Path>,
) -> Result<ThreeProviderRecoveryEvidence, ContentError> {
	let root = root.as_ref();
	let roots = [root.join("provider-a"), root.join("provider-b"), root.join("provider-c")];
	for provider_root in &roots {
		fs::create_dir_all(provider_root).map_err(io_error)?;
	}
	let providers = [PROVIDER_A, PROVIDER_B, PROVIDER_C];
	let keys = [pair(4), pair(5), pair(6)];

	let first = deterministic_bytes(17);
	let first_cid = install_source(&roots[0], [7; 16], &first)?;
	let initial_topology = topology(
		INITIAL_DUTY_FINALIZED,
		INITIAL_SNAPSHOT,
		PROVIDER_A,
		&providers,
		&keys,
		[true, true, true],
		[None, None, None],
		None,
	);
	let first_commitment = commitment(&roots[0], 0)?;
	replicate_suffix(&roots, &providers, &keys, &initial_topology, &first_commitment).await?;

	let initial_runtime_duty = runtime_duty(
		[31; 32],
		&providers,
		&keys,
		PROVIDER_A,
		RuntimePhase::Primary,
		RuntimeMode::Standard,
		INITIAL_SNAPSHOT,
		INITIAL_DUTY_FINALIZED,
		None,
		0,
		[true, true, true],
	);
	let initial_checkpoint = run_checkpoint(
		&roots,
		&providers,
		&keys,
		initial_runtime_duty,
		initial_topology.clone(),
		INITIAL_DUTY_FINALIZED,
		INITIAL_CHECKPOINT_FINALIZED,
		"initial",
		BTreeSet::new(),
	)
	.await?;
	let initial_info = checkpoint_info_from_observation(&initial_checkpoint, &first_commitment)?;

	let second = deterministic_bytes(41);
	let second_cid = install_source(&roots[0], [8; 16], &second)?;
	let replicated_topology = topology(
		130,
		INITIAL_SNAPSHOT,
		PROVIDER_A,
		&providers,
		&keys,
		[true, true, true],
		[Some(INITIAL_CHECKPOINT_FINALIZED); 3],
		Some(initial_info.clone()),
	);
	let second_commitment = commitment(&roots[0], 1)?;
	let replication_network_requests =
		replicate_suffix(&roots, &providers, &keys, &replicated_topology, &second_commitment)
			.await?;

	let object_path = roots[0].join("streaming-v1").join("objects").join(second_cid.as_str());
	let mut corrupted = second.clone();
	corrupted[0] ^= 1;
	fs::write(object_path, corrupted).map_err(io_error)?;
	let damaged = StreamingStore::open(&roots[0])?;
	let corrupt_read =
		match damaged.read_range_verified(second_cid.as_str(), 0, second.len() as u64) {
			Ok(bytes) => CorruptReadObservation {
				result: "served_bytes".into(),
				bytes_returned: bytes.len(),
			},
			Err(ContentError::IntegrityFailed) => CorruptReadObservation {
				result: "rejected_integrity_failed".into(),
				bytes_returned: 0,
			},
			Err(_) =>
				CorruptReadObservation { result: "rejected_unexpected".into(), bytes_returned: 0 },
		};
	if corrupt_read.result != "rejected_integrity_failed" || corrupt_read.bytes_returned != 0 {
		return Err(ContentError::IntegrityFailed)
	}
	drop(damaged);

	let fallback_topology = topology(
		FAILURE_DUTY_FINALIZED,
		FAILURE_DUTY_FINALIZED,
		PROVIDER_A,
		&providers,
		&keys,
		[false, true, true],
		[Some(INITIAL_CHECKPOINT_FINALIZED); 3],
		Some(initial_info.clone()),
	);
	let fallback_duty = runtime_duty(
		[32; 32],
		&providers,
		&keys,
		PROVIDER_B,
		RuntimePhase::ReplicaFallbackPromotion,
		RuntimeMode::Standard,
		FAILURE_DUTY_FINALIZED,
		FAILURE_DUTY_FINALIZED,
		Some(initial_info.commitment.clone()),
		1,
		[false, true, true],
	);
	let (promotion, promotion_reads) =
		run_promotion(&roots[1], PROVIDER_B, keys[1].clone(), fallback_duty, fallback_topology)
			.await?;

	let promoted_providers = [PROVIDER_B, PROVIDER_A, PROVIDER_C];
	let promoted_keys = [keys[1].clone(), keys[0].clone(), keys[2].clone()];
	let promoted_topology = topology(
		PROMOTED_DUTY_FINALIZED,
		PROMOTED_DUTY_FINALIZED,
		PROVIDER_B,
		&promoted_providers,
		&promoted_keys,
		[true, true, true],
		[Some(INITIAL_CHECKPOINT_FINALIZED); 3],
		Some(initial_info.clone()),
	);
	let selected = select_source(&promoted_topology, PROVIDER_A, INITIAL_CHECKPOINT_FINALIZED)
		.ok_or(ContentError::IntegrityFailed)?;
	let mut permuted = promoted_topology.clone();
	permuted.providers.reverse();
	let selected_permuted = select_source(&permuted, PROVIDER_A, INITIAL_CHECKPOINT_FINALIZED)
		.ok_or(ContentError::IntegrityFailed)?;
	if selected != PROVIDER_B || selected_permuted != PROVIDER_B {
		return Err(ContentError::IntegrityFailed)
	}
	repair_provider(
		&roots[0],
		&roots[1],
		keys[0].clone(),
		keys[1].clone(),
		promoted_topology.clone(),
		second_commitment.clone(),
	)
	.await?;

	let promoted_duty = runtime_duty(
		[33; 32],
		&promoted_providers,
		&promoted_keys,
		PROVIDER_B,
		RuntimePhase::Primary,
		RuntimeMode::PromotionPending,
		PROMOTED_DUTY_FINALIZED,
		PROMOTED_DUTY_FINALIZED,
		Some(initial_info.commitment),
		1,
		[true, true, true],
	);
	let promoted_checkpoint = run_checkpoint(
		&[roots[1].clone(), roots[0].clone(), roots[2].clone()],
		&promoted_providers,
		&promoted_keys,
		promoted_duty,
		promoted_topology,
		PROMOTED_DUTY_FINALIZED,
		PROMOTED_CHECKPOINT_FINALIZED,
		"promoted",
		BTreeSet::new(),
	)
	.await?;

	let provider_roots = roots
		.iter()
		.map(|provider_root| observed_root(provider_root))
		.collect::<Result<Vec<_>, _>>()?;
	if provider_roots.iter().collect::<BTreeSet<_>>().len() != 1 {
		return Err(ContentError::IntegrityFailed)
	}
	let repaired_read_lengths = roots
		.iter()
		.map(|provider_root| {
			StreamingStore::open(provider_root)?
				.read_range_verified(second_cid.as_str(), 0, second.len() as u64)
				.map(|bytes| bytes.len())
		})
		.collect::<Result<Vec<_>, _>>()?;
	if repaired_read_lengths.iter().any(|length| *length != second.len()) {
		return Err(ContentError::IntegrityFailed)
	}

	let runtime_duty_reads =
		initial_checkpoint.runtime_reads + promotion_reads + promoted_checkpoint.runtime_reads;
	Ok(ThreeProviderRecoveryEvidence {
		schema_version: 2,
		test: "deterministic_failover".into(),
		provider_count: 3,
		content_cids: vec![first_cid.to_string(), second_cid.to_string()],
		provider_roots,
		eligible_sources: vec![
			EligibleSourceObservation { provider: hex::encode(PROVIDER_B), order: 0 },
			EligibleSourceObservation { provider: hex::encode(PROVIDER_C), order: 2 },
		],
		selection_results: vec![hex::encode(selected), hex::encode(selected_permuted)],
		corrupt_read_observations: vec![corrupt_read],
		runtime_observations: vec![
			RuntimeObservation {
				kind: "initial_duty".into(),
				finalized_number: INITIAL_DUTY_FINALIZED,
			},
			RuntimeObservation {
				kind: "initial_checkpoint_finality".into(),
				finalized_number: INITIAL_CHECKPOINT_FINALIZED,
			},
			RuntimeObservation {
				kind: "fallback_duty".into(),
				finalized_number: FAILURE_DUTY_FINALIZED,
			},
			RuntimeObservation {
				kind: "promotion_finality".into(),
				finalized_number: PROMOTION_FINALIZED,
			},
			RuntimeObservation {
				kind: "repaired_eligible_duty".into(),
				finalized_number: PROMOTED_DUTY_FINALIZED,
			},
			RuntimeObservation {
				kind: "promoted_checkpoint_finality".into(),
				finalized_number: PROMOTED_CHECKPOINT_FINALIZED,
			},
		],
		runtime_duty_reads,
		checkpoints: vec![initial_checkpoint.observation, promoted_checkpoint.observation],
		promotion,
		repaired_read_lengths,
		replication_network_requests,
	})
}

struct CheckpointRun {
	observation: CheckpointObservation,
	runtime_reads: usize,
}

#[allow(clippy::too_many_arguments)]
async fn run_checkpoint(
	roots: &[std::path::PathBuf; 3],
	providers: &[[u8; 32]; 3],
	keys: &[ed25519::Pair; 3],
	duty: CheckpointDutyInfo<AccountId32, H256, u32>,
	topology: ReplicationTopologySnapshot,
	duty_finalized: u32,
	checkpoint_finalized: u32,
	phase: &str,
	partitioned: BTreeSet<[u8; 32]>,
) -> Result<CheckpointRun, ContentError> {
	let mut services = Vec::new();
	for index in 0..3 {
		services.push(
			install_duty(
				&roots[index],
				providers[index],
				keys[index].clone(),
				duty.clone(),
				topology.clone(),
				duty_finalized,
			)
			.await?,
		);
	}
	let responders = [1usize, 2]
		.into_iter()
		.map(|index| {
			PeerResponder::new_with_store(
				services[index].authority().clone(),
				services[index].checkpoint_stack().clone(),
				services[index].store().clone(),
				providers[index],
				keys[index].clone(),
			)
			.map(|responder| (providers[index], Arc::new(responder)))
		})
		.collect::<Result<Vec<_>, _>>()?;
	let transport = Arc::new(DirectConfirmationTransport {
		responders,
		partitioned,
		requests: AtomicUsize::new(0),
	});
	for _ in 0..3 {
		crate::checkpoint_quorum_worker::evidence_tick(
			services[0].authority().clone(),
			services[0].checkpoint_stack().clone(),
			services[0].store().clone(),
			providers[0],
			keys[0].clone(),
			transport.clone(),
		)
		.await
		.map_err(ContentError::Io)?;
		if services[0].checkpoint_stack().submission_heads()?.len() == 1 {
			break
		}
	}
	let submission = services[0]
		.checkpoint_stack()
		.submission_heads()?
		.into_iter()
		.next()
		.ok_or(ContentError::IntegrityFailed)?;
	let confirmation_providers = decode_confirmations(&submission)?;
	let payload = decode_hex_scale::<CommitmentPayloadV2<H256, u32>>(&submission.payload_scale)?;
	if confirmation_providers.len() != 2 ||
		confirmation_providers.iter().collect::<BTreeSet<_>>().len() != 2
	{
		return Err(ContentError::IntegrityFailed)
	}
	let before = serde_json::to_vec(&submission).map_err(io_error)?;
	let before_hash = hex::encode(blake2_256(&before));
	let authority = services[0].authority().clone();
	let runtime_reads = services
		.iter()
		.map(|service| service.authority().duty_reads.load(Ordering::SeqCst))
		.sum();
	drop(services);

	let reopened = Arc::new(CheckpointStack::open(&roots[0])?);
	let reopened_submission = reopened
		.submission_heads()?
		.into_iter()
		.next()
		.ok_or(ContentError::IntegrityFailed)?;
	let after_hash =
		hex::encode(blake2_256(&serde_json::to_vec(&reopened_submission).map_err(io_error)?));
	if submission != reopened_submission || before_hash != after_hash {
		return Err(ContentError::IntegrityFailed)
	}
	let lane = ScriptedCheckpointLane {
		metadata: metadata()?,
		provider: providers[0],
		key: keys[0].clone(),
		finalized_number: checkpoint_finalized,
		calls: AtomicUsize::new(0),
	};
	authority.set_observation(checkpoint_observation(&submission, checkpoint_finalized)?);
	let completed = crate::checkpoint_live_worker::tick(&*authority, &reopened, &lane).await?;
	let receipt = completed.finalized.ok_or(ContentError::IntegrityFailed)?;
	if completed.published.len() != 1 || receipt.finalized_number != checkpoint_finalized {
		return Err(ContentError::IntegrityFailed)
	}
	let calls = lane.calls.load(Ordering::SeqCst);
	let replay = crate::checkpoint_live_worker::tick(&*authority, &reopened, &lane).await?;
	let replay_calls = lane.calls.load(Ordering::SeqCst) - calls;
	if replay.finalized.is_some() || !replay.published.is_empty() || replay_calls != 0 {
		return Err(ContentError::IntegrityFailed)
	}

	Ok(CheckpointRun {
		observation: CheckpointObservation {
			phase: phase.into(),
			duty_id: hex::encode(duty.duty_id.as_bytes()),
			primary: submission.primary.clone(),
			confirmation_providers,
			submission_id: submission.submission_id,
			submission_record_hash: submission.record_hash,
			mmr_root: hex::encode(payload.commitment.mmr_root.as_bytes()),
			start_seq: payload.commitment.start_seq,
			leaf_count: payload.commitment.leaf_count,
			call_args_blake2_256: hex::encode(blake2_256(
				&hex::decode(&submission.call_args_scale)
					.map_err(|_| ContentError::IntegrityFailed)?,
			)),
			before_restart_blake2_256: before_hash,
			after_restart_blake2_256: after_hash,
			finality_calls: calls,
			finalized_number: receipt.finalized_number,
			publication_count: completed.published.len(),
			replay_finality_calls: replay_calls,
			replay_publication_count: replay.published.len(),
		},
		runtime_reads,
	})
}

async fn run_promotion(
	root: &Path,
	provider: [u8; 32],
	key: ed25519::Pair,
	duty: CheckpointDutyInfo<AccountId32, H256, u32>,
	topology: ReplicationTopologySnapshot,
) -> Result<(PromotionObservation, usize), ContentError> {
	let service =
		install_duty(root, provider, key.clone(), duty, topology, FAILURE_DUTY_FINALIZED).await?;
	let scheduler = PromotionDiscoveryScheduler::open(root)?;
	let lane = ScriptedPromotionLane {
		metadata: metadata()?,
		provider,
		key: key.clone(),
		finalized_number: PROMOTION_FINALIZED,
		calls: AtomicUsize::new(0),
	};
	let first = crate::checkpoint_promotion_worker::tick(
		&**service.authority(),
		service.checkpoint_stack(),
		service.store(),
		&lane,
		&scheduler,
		provider,
		&key,
		Duration::from_secs(1),
		Duration::from_secs(1),
	)
	.await?;
	if first.is_some() {
		return Err(ContentError::IntegrityFailed)
	}
	let receipt = crate::checkpoint_promotion_worker::tick(
		&**service.authority(),
		service.checkpoint_stack(),
		service.store(),
		&lane,
		&scheduler,
		provider,
		&key,
		Duration::from_secs(1),
		Duration::from_secs(1),
	)
	.await?
	.ok_or(ContentError::IntegrityFailed)?;
	if lane.calls.load(Ordering::SeqCst) != 1 || receipt.finalized_number != PROMOTION_FINALIZED {
		return Err(ContentError::IntegrityFailed)
	}
	Ok((
		PromotionObservation {
			intent_id: receipt.intent_id,
			provider: receipt.provider,
			finalized_number: receipt.finalized_number,
			finality_calls: lane.calls.load(Ordering::SeqCst),
		},
		service.authority().duty_reads.load(Ordering::SeqCst),
	))
}

async fn install_duty(
	root: &Path,
	provider: [u8; 32],
	key: ed25519::Pair,
	duty: CheckpointDutyInfo<AccountId32, H256, u32>,
	topology: ReplicationTopologySnapshot,
	finalized_number: u32,
) -> Result<ProviderService<ScriptedCommonsAuthority>, ContentError> {
	let profile = NodeProfile {
		provider: hex::encode(provider),
		endpoint: String::from_utf8(endpoint(provider))
			.map_err(|_| ContentError::IntegrityFailed)?,
		service_key: hex::encode(key.public().0),
		region: None,
	};
	let store = Arc::new(
		DiskStore::open(root, profile.clone(), MAX_STORED_BYTES)
			.map_err(|error| ContentError::Io(error.to_string()))?,
	);
	let public = validate_checkpoint_duty(
		duty.clone(),
		&AccountId32::new(provider),
		key.public().0,
		duty.snapshot_checkpoint,
	)
	.map_err(|_| ContentError::IntegrityFailed)?;
	let batch = CheckpointDutyBatch {
		finalized_hash: hex::encode(topology.finalized_hash),
		finalized_number,
		provider: profile.provider,
		snapshot_checkpoint: duty.snapshot_checkpoint,
		requested_cursor: None,
		next_cursor: None,
		duties: vec![public],
	};
	let authority = Arc::new(ScriptedCommonsAuthority::new(batch, topology));
	let service = ProviderService::new(store, authority, key, Arc::new(NoopOutbox))?;
	let discovered =
		crate::poll_checkpoint_duties_once(&service).await.map_err(ContentError::Io)?;
	if discovered != 1 {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(service)
}

fn runtime_duty(
	duty_id: [u8; 32],
	providers: &[[u8; 32]; 3],
	keys: &[ed25519::Pair; 3],
	initiator: [u8; 32],
	phase: RuntimePhase,
	mode: RuntimeMode,
	snapshot: u32,
	finalized_number: u32,
	previous: Option<CommitmentInfo<H256>>,
	expected_start: u64,
	eligible: [bool; 3],
) -> CheckpointDutyInfo<AccountId32, H256, u32> {
	let previous_checkpoint = previous.as_ref().map(|_| INITIAL_CHECKPOINT_FINALIZED);
	let fallback = phase == RuntimePhase::ReplicaFallbackPromotion;
	let due_at = if fallback { snapshot.saturating_sub(20) } else { snapshot };
	let grace_until =
		if fallback { snapshot.saturating_sub(10) } else { snapshot.saturating_add(10) };
	let authorities = (0..3)
		.map(|index| ProviderDutyAuthority {
			provider: AccountId32::new(providers[index]),
			role: if index == 0 { ProviderDutyRole::Primary } else { ProviderDutyRole::Replica },
			order: index as u8,
			active_service_key_version: 1,
			active_service_key: keys[index].public().0,
			endpoint_hash: H256::from(blake2_256(&endpoint(providers[index]))),
			organization_sla_eligible: true,
			overdue_challenge: false,
			eligible: eligible[index],
			may_sign: eligible[index],
			may_initiate: providers[index] == initiator,
			exclusion: (!eligible[index]).then_some(ProviderDutyExclusion::Inactive),
			initiation_exclusion: None,
			confirmed_checkpoint: previous.as_ref().map(|_| INITIAL_CHECKPOINT_FINALIZED),
		})
		.collect();
	CheckpointDutyInfo {
		response_version: RESPONSE_VERSION,
		commons_genesis_hash: H256::repeat_byte(1),
		commons_spec_version: 1,
		commons_transaction_version: 1,
		commons_metadata_hash: H256::repeat_byte(2),
		duty_id: H256::from(duty_id),
		bucket_id: H256::from(BUCKET),
		primary: AccountId32::new(providers[0]),
		replicas: vec![AccountId32::new(providers[1]), AccountId32::new(providers[2])],
		authorities,
		initiator: Some(AccountId32::new(initiator)),
		phase,
		mode,
		snapshot_checkpoint: snapshot,
		snapshot_hash: H256::from(blake2_256(&finalized_number.to_le_bytes())),
		due_at,
		grace_until,
		expected_nonce: snapshot,
		scheduled_at: snapshot.saturating_sub(10),
		previous_commitment: previous,
		previous_checkpoint,
		expected_next_start_seq: expected_start,
		required_primary_confirmations: 1,
		required_replica_confirmations: 2,
	}
}

#[allow(clippy::too_many_arguments)]
fn topology(
	finalized_number: u32,
	governed_checkpoint: u32,
	primary: [u8; 32],
	providers: &[[u8; 32]; 3],
	keys: &[ed25519::Pair; 3],
	eligible: [bool; 3],
	confirmed: [Option<u32>; 3],
	current_checkpoint: Option<CheckpointInfo<AccountId32, H256, u32>>,
) -> ReplicationTopologySnapshot {
	let slots = (0..3)
		.map(|index| {
			let usable = eligible[index];
			ReplicationProviderSnapshot {
				provider: providers[index],
				order: index as u8,
				primary: index == 0,
				record_present: true,
				endpoint: Some(endpoint(providers[index])),
				endpoint_hash: Some(blake2_256(&endpoint(providers[index]))),
				active_service_key: Some(keys[index].public().0),
				active_service_key_version: Some(1),
				status_active: true,
				organization_valid: true,
				authority_validated_at: Some(governed_checkpoint),
				overdue_challenges: 0,
				eligible: usable,
				usable,
				exclusions: (!usable)
					.then_some(ReplicationProviderExclusion::RuntimeIneligible)
					.into_iter()
					.collect(),
				confirmed_checkpoint: confirmed[index],
			}
		})
		.collect();
	let mut value = ReplicationTopologySnapshot {
		genesis_hash: [1; 32],
		finalized_hash: blake2_256(&finalized_number.to_le_bytes()),
		finalized_number,
		governed_finalized_checkpoint: Some(governed_checkpoint),
		bucket_id: BUCKET,
		bucket_version: 1,
		primary,
		replicas: vec![providers[1], providers[2]],
		providers: slots,
		current_checkpoint,
		snapshot_hash: [0; 32],
	};
	let mut encoded = b"cord/provider/replication-topology/v1".to_vec();
	value.encode_to(&mut encoded);
	value.snapshot_hash = blake2_256(&encoded);
	value
}

fn install_source(
	root: &Path,
	operation: [u8; 16],
	bytes: &[u8],
) -> Result<CanonicalCid, ContentError> {
	let cid = CanonicalCid::from_digest(blake2_256(bytes));
	StreamingStore::open(root)?.put_chunks(
		StreamingDescriptor {
			operation_id: OperationId::from_bytes(operation),
			bucket_id: BucketId::from_bytes(BUCKET),
			expected_cid: cid.to_string(),
			object_len: bytes.len() as u64,
		},
		bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec),
	)?;
	Ok(cid)
}

fn commitment(root: &Path, start: u64) -> Result<PeerMmrCommitmentV1, ContentError> {
	let streaming = StreamingStore::open(root)?;
	let mmr = BucketMmrStore::open(root, &streaming)?;
	let candidate = mmr.commitment_candidate(&streaming, BucketId::from_bytes(BUCKET), start)?;
	let predecessor = mmr.commitment_predecessor_total(BucketId::from_bytes(BUCKET), start)?;
	PeerMmrCommitmentV1::new(
		candidate.mmr_root.0,
		candidate.start_seq,
		candidate.leaf_count,
		predecessor,
	)
}

async fn replicate_suffix(
	roots: &[std::path::PathBuf; 3],
	providers: &[[u8; 32]; 3],
	keys: &[ed25519::Pair; 3],
	topology: &ReplicationTopologySnapshot,
	commitment: &PeerMmrCommitmentV1,
) -> Result<usize, ContentError> {
	let authority = Arc::new(ScriptedCommonsAuthority::new(
		empty_batch(providers[0], topology),
		topology.clone(),
	));
	let responder = Arc::new(PeerResponder::new(
		authority,
		Arc::new(CheckpointStack::open(&roots[0])?),
		providers[0],
		keys[0].clone(),
	)?);
	let mut requests = 0;
	for index in 1..3 {
		let session = ReplicationSessionV1::from_topology(
			topology.clone(),
			providers[index],
			keys[index].public().0,
			providers[0],
			providers[index],
			commitment.clone(),
		)
		.map_err(|_| ContentError::IntegrityFailed)?;
		let transport = Arc::new(DirectPeerTransport {
			responder: responder.clone(),
			requests: AtomicUsize::new(0),
		});
		reconcile_to_mmr(&roots[index], transport.clone(), keys[index].clone(), &session).await?;
		requests += transport.requests.load(Ordering::SeqCst);
	}
	Ok(requests)
}

async fn repair_provider(
	target_root: &Path,
	source_root: &Path,
	target_key: ed25519::Pair,
	source_key: ed25519::Pair,
	topology: ReplicationTopologySnapshot,
	commitment: PeerMmrCommitmentV1,
) -> Result<(), ContentError> {
	let authority = Arc::new(ScriptedCommonsAuthority::new(
		empty_batch(PROVIDER_B, &topology),
		topology.clone(),
	));
	let responder = Arc::new(PeerResponder::new(
		authority,
		Arc::new(CheckpointStack::open(source_root)?),
		PROVIDER_B,
		source_key,
	)?);
	let session = ReplicationSessionV1::from_topology(
		topology,
		PROVIDER_A,
		target_key.public().0,
		PROVIDER_B,
		PROVIDER_A,
		commitment,
	)
	.map_err(|_| ContentError::IntegrityFailed)?;
	let transport = Arc::new(DirectPeerTransport { responder, requests: AtomicUsize::new(0) });
	reconcile_to_mmr(target_root, transport, target_key, &session).await.map(|_| ())
}

async fn reconcile_to_mmr(
	target_root: &Path,
	transport: Arc<DirectPeerTransport>,
	target_key: ed25519::Pair,
	session: &ReplicationSessionV1,
) -> Result<u32, ContentError> {
	let reconciler = ReplicationReconciler::new(
		Arc::new(CheckpointStack::open(target_root)?),
		transport,
		target_key,
	)?;
	let operation = ReplicationReconciler::<DirectPeerTransport>::operation_id(session);
	for step in 1..=32 {
		if reconciler.reconcile_one(session, operation).await?.phase ==
			ReplicationPhase::MmrCommitted
		{
			return Ok(step)
		}
	}
	Err(ContentError::IntegrityFailed)
}

fn checkpoint_observation(
	submission: &CheckpointSubmissionV2,
	finalized_number: u32,
) -> Result<FinalizedCheckpointObservation, ContentError> {
	let payload = decode_hex_scale::<CommitmentPayloadV2<H256, u32>>(&submission.payload_scale)?;
	let confirmations = decode_confirmation_accounts(submission)?;
	Ok(FinalizedCheckpointObservation {
		finalized_hash: blake2_256(&finalized_number.to_le_bytes()),
		finalized_number,
		response_scale: Versioned {
			version: RESPONSE_VERSION,
			value: Some(CheckpointInfo {
				bucket_id: payload.bucket_id,
				commitment: CommitmentInfo {
					mmr_root: payload.commitment.mmr_root,
					start_seq: payload.commitment.start_seq,
					leaf_count: payload.commitment.leaf_count,
				},
				checkpoint_block: finalized_number,
				primary_signers: 1,
				commitment_nonce: payload.nonce,
				replica_confirmations: confirmations,
			}),
		}
		.encode(),
	})
}

fn checkpoint_info_from_observation(
	checkpoint: &CheckpointRun,
	commitment: &PeerMmrCommitmentV1,
) -> Result<CheckpointInfo<AccountId32, H256, u32>, ContentError> {
	let (start_seq, end_seq) = commitment.sequence_range();
	Ok(CheckpointInfo {
		bucket_id: H256::from(BUCKET),
		commitment: CommitmentInfo {
			mmr_root: H256::from(commitment.mmr_root()),
			start_seq,
			leaf_count: end_seq.saturating_sub(start_seq),
		},
		checkpoint_block: checkpoint.observation.finalized_number,
		primary_signers: 1,
		commitment_nonce: INITIAL_SNAPSHOT,
		replica_confirmations: vec![AccountId32::new(PROVIDER_B), AccountId32::new(PROVIDER_C)],
	})
}

fn decode_confirmation_accounts(
	submission: &CheckpointSubmissionV2,
) -> Result<Vec<AccountId32>, ContentError> {
	Ok(decode_hex_scale::<Vec<ReplicaSignature<AccountId32>>>(&submission.confirmations_scale)?
		.into_iter()
		.map(|confirmation| confirmation.provider)
		.collect())
}

fn decode_confirmations(submission: &CheckpointSubmissionV2) -> Result<Vec<String>, ContentError> {
	Ok(decode_confirmation_accounts(submission)?
		.into_iter()
		.map(|provider| hex::encode(<AccountId32 as AsRef<[u8]>>::as_ref(&provider)))
		.collect())
}

fn decode_hex_scale<T: Decode + Encode>(value: &str) -> Result<T, ContentError> {
	let bytes = hex::decode(value).map_err(|_| ContentError::IntegrityFailed)?;
	let mut input = &bytes[..];
	let decoded = T::decode(&mut input).map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || decoded.encode() != bytes {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(decoded)
}

fn metadata() -> Result<subxt::Metadata, ContentError> {
	let prefixed: frame_metadata::RuntimeMetadataPrefixed = RuntimeMetadataV15::new(
		vec![PalletMetadata {
			name: "StorageProvider",
			storage: None,
			calls: Some(PalletCallMetadata { ty: meta_type::<ExactCall>() }),
			event: None,
			constants: vec![],
			error: None,
			index: 37,
			docs: vec![],
		}],
		ExtrinsicMetadata {
			version: 4,
			address_ty: meta_type::<()>(),
			call_ty: meta_type::<()>(),
			signature_ty: meta_type::<()>(),
			extra_ty: meta_type::<()>(),
			signed_extensions: vec![],
		},
		meta_type::<()>(),
		vec![],
		OuterEnums {
			call_enum_ty: meta_type::<()>(),
			event_enum_ty: meta_type::<()>(),
			error_enum_ty: meta_type::<()>(),
		},
		CustomMetadata { map: Default::default() },
	)
	.into();
	prefixed.try_into().map_err(|_| ContentError::IntegrityFailed)
}

fn empty_batch(provider: [u8; 32], topology: &ReplicationTopologySnapshot) -> CheckpointDutyBatch {
	CheckpointDutyBatch {
		finalized_hash: hex::encode(topology.finalized_hash),
		finalized_number: topology.finalized_number,
		provider: hex::encode(provider),
		snapshot_checkpoint: topology.governed_finalized_checkpoint.unwrap_or_default(),
		requested_cursor: None,
		next_cursor: None,
		duties: Vec::new(),
	}
}

fn observed_root(root: &Path) -> Result<String, ContentError> {
	let streaming = StreamingStore::open(root)?;
	let mmr = BucketMmrStore::open(root, &streaming)?;
	Ok(hex::encode(
		mmr.commitment_candidate(&streaming, BucketId::from_bytes(BUCKET), 0)?
			.mmr_root
			.0,
	))
}

fn endpoint(provider: [u8; 32]) -> Vec<u8> {
	format!("https://provider-{}.invalid", provider[0]).into_bytes()
}

fn pair(seed: u8) -> ed25519::Pair {
	ed25519::Pair::from_seed(&[seed; 32])
}

fn account_bytes(account: &AccountId32) -> Result<[u8; 32], ContentError> {
	let bytes: &[u8] = account.as_ref();
	bytes.try_into().map_err(|_| ContentError::IntegrityFailed)
}

fn deterministic_bytes(seed: usize) -> Vec<u8> {
	(0..CHUNK_BYTES + 37).map(|index| ((index * 31 + seed) % 251) as u8).collect()
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}
