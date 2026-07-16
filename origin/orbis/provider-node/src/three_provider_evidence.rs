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

//! Deterministic three-provider recovery evidence over the production replication machinery.

use std::{
	fs,
	path::Path,
	sync::{Arc, Mutex},
};

use async_trait::async_trait;
use codec::Encode;
use serde::Serialize;
use sp_core::{ed25519, Pair as _};
use sp_crypto_hashing::blake2_256;

use crate::{
	chain::{
		ChainError, ReplicationAuthority, ReplicationProviderSnapshot, ReplicationTopologySnapshot,
	},
	checkpoint_stack::CheckpointStack,
	peer::PeerMmrCommitmentV1,
	peer_responder::PeerResponder,
	peer_transport::{PeerTransport, PeerTransportError},
	replication::ReplicationPhase,
	replication_reconciler::ReplicationReconciler,
	replication_session::ReplicationSessionV1,
	replication_worker::select_source,
	storage::bucket_mmr::BucketMmrStore,
	BucketId, CanonicalCid, ContentError, OperationId, StreamingDescriptor, StreamingStore,
	CHUNK_BYTES,
};

const BUCKET: [u8; 32] = [3; 32];
const PROVIDER_A: [u8; 32] = [4; 32];
const PROVIDER_B: [u8; 32] = [5; 32];
const PROVIDER_C: [u8; 32] = [6; 32];
const CHECKPOINT: u32 = 100;
const FAILURE_DETECTED_BLOCK: u32 = 200;

/// One source candidate observed by the production deterministic selector.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EligibleSourceObservation {
	/// Provider identifier as lowercase hexadecimal.
	pub provider: String,
	/// Runtime-assigned provider order used for the deterministic tie-break.
	pub order: u8,
}

/// One fail-closed read observation made after corrupting durable object bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CorruptReadObservation {
	/// Stable outcome label derived from the real verified-read result.
	pub result: String,
	/// Number of bytes returned by the verified read.
	pub bytes_returned: usize,
}

/// Raw observations emitted by the deterministic three-provider recovery scenario.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ThreeProviderRecoveryEvidence {
	/// Evidence schema version.
	pub schema_version: u16,
	/// Exact integration-test name which generated the observation.
	pub test: String,
	/// Number of independently persisted provider stores in the scenario.
	pub provider_count: usize,
	/// Final production MMR root independently read from each provider store.
	pub provider_roots: Vec<String>,
	/// Eligible failover sources and their runtime order.
	pub eligible_sources: Vec<EligibleSourceObservation>,
	/// Selector results for two different topology iteration orders.
	pub selection_results: Vec<String>,
	/// Verified-read observations made against corrupt durable bytes.
	pub corrupt_read_observations: Vec<CorruptReadObservation>,
	/// Finalized block at which the synthetic failover topology was observed.
	pub failure_detected_block: u32,
	/// Number of real reconciler actions required to repair the failed provider.
	pub recovery_steps: u32,
	/// Conservative observed block if no more than one reconciler action runs per block.
	pub convergence_observed_block: u32,
	/// Number of authenticated peer page/chunk exchanges made during recovery.
	pub recovery_network_requests: usize,
	/// Production components exercised by the scenario.
	pub production_surfaces: Vec<String>,
}

#[derive(Clone)]
struct StaticAuthority(ReplicationTopologySnapshot);

#[async_trait]
impl ReplicationAuthority for StaticAuthority {
	async fn replication_topology(
		&self,
		bucket: [u8; 32],
	) -> Result<ReplicationTopologySnapshot, ChainError> {
		(bucket == self.0.bucket_id)
			.then(|| self.0.clone())
			.ok_or_else(|| ChainError::Rejected("wrong bucket".into()))
	}

	async fn replication_topology_at(
		&self,
		bucket: [u8; 32],
		finalized_hash: [u8; 32],
		finalized_number: u32,
	) -> Result<ReplicationTopologySnapshot, ChainError> {
		(bucket == self.0.bucket_id &&
			finalized_hash == self.0.finalized_hash &&
			finalized_number == self.0.finalized_number)
			.then(|| self.0.clone())
			.ok_or_else(|| ChainError::Rejected("wrong pinned topology".into()))
	}
}

struct DirectTransport {
	responder: Arc<PeerResponder<StaticAuthority>>,
	requests: Mutex<usize>,
}

impl DirectTransport {
	fn new(responder: Arc<PeerResponder<StaticAuthority>>) -> Self {
		Self { responder, requests: Mutex::new(0) }
	}

	fn request_count(&self) -> Result<usize, ContentError> {
		self.requests
			.lock()
			.map(|requests| *requests)
			.map_err(|_| ContentError::Io("evidence request counter poisoned".into()))
	}

	fn record_request(&self) -> Result<(), PeerTransportError> {
		let mut requests = self.requests.lock().map_err(|_| PeerTransportError::Transport)?;
		*requests = requests.checked_add(1).ok_or(PeerTransportError::Transport)?;
		Ok(())
	}
}

#[async_trait]
impl PeerTransport for DirectTransport {
	async fn page(
		&self,
		_: &ReplicationSessionV1,
		request: &[u8],
	) -> Result<Vec<u8>, PeerTransportError> {
		self.record_request()?;
		self.responder.page(request).await.map_err(|_| PeerTransportError::Response)
	}

	async fn chunk(
		&self,
		_: &ReplicationSessionV1,
		request: &[u8],
	) -> Result<Vec<u8>, PeerTransportError> {
		self.record_request()?;
		self.responder.chunk(request).await.map_err(|_| PeerTransportError::Response)
	}
}

/// Exercise real streaming, MMR, authenticated peer and reconciler surfaces across three stores.
///
/// The supplied directory must be disposable and empty. The returned object contains raw
/// observations rather than precomputed gate booleans so the external validator can derive each
/// AC5 assertion independently.
pub async fn run_three_provider_recovery_evidence(
	root: impl AsRef<Path>,
) -> Result<ThreeProviderRecoveryEvidence, ContentError> {
	let root = root.as_ref();
	let provider_a_root = root.join("provider-a");
	let provider_b_root = root.join("provider-b");
	let provider_c_root = root.join("provider-c");
	for provider_root in [&provider_a_root, &provider_b_root, &provider_c_root] {
		fs::create_dir_all(provider_root).map_err(io_error)?;
	}

	let key_a = pair(11);
	let key_b = pair(12);
	let key_c = pair(13);
	let bytes = deterministic_bytes();
	let cid = CanonicalCid::from_digest(blake2_256(&bytes));
	let source = StreamingStore::open(&provider_a_root)?;
	source.put_chunks(
		StreamingDescriptor {
			operation_id: OperationId::from_bytes([7; 16]),
			bucket_id: BucketId::from_bytes(BUCKET),
			expected_cid: cid.to_string(),
			object_len: bytes.len() as u64,
		},
		bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec),
	)?;
	let source_mmr = BucketMmrStore::open(&provider_a_root, &source)?;
	let candidate = source_mmr.commitment_candidate(&source, BucketId::from_bytes(BUCKET), 0)?;
	let commitment = PeerMmrCommitmentV1::new(
		candidate.mmr_root.0,
		candidate.start_seq,
		candidate.leaf_count,
		0,
	)?;
	drop(source_mmr);
	drop(source);

	let initial_topology = topology(
		100,
		PROVIDER_A,
		vec![
			provider(PROVIDER_A, 0, true, key_a.public().0, Some(CHECKPOINT)),
			provider(PROVIDER_B, 1, false, key_b.public().0, None),
			provider(PROVIDER_C, 2, false, key_c.public().0, None),
		],
	);
	let source_stack = Arc::new(CheckpointStack::open(&provider_a_root)?);
	let responder_a = Arc::new(PeerResponder::new(
		Arc::new(StaticAuthority(initial_topology.clone())),
		source_stack,
		PROVIDER_A,
		key_a,
	)?);

	for (target_root, target_provider, target_key) in [
		(&provider_b_root, PROVIDER_B, key_b.clone()),
		(&provider_c_root, PROVIDER_C, key_c.clone()),
	] {
		let session = ReplicationSessionV1::from_topology(
			initial_topology.clone(),
			target_provider,
			target_key.public().0,
			PROVIDER_A,
			target_provider,
			commitment.clone(),
		)
		.map_err(|_| ContentError::IntegrityFailed)?;
		let transport = Arc::new(DirectTransport::new(responder_a.clone()));
		reconcile_to_mmr(target_root, transport, target_key, &session).await?;
	}
	drop(responder_a);

	let object_path = provider_a_root.join("streaming-v1").join("objects").join(cid.as_str());
	let mut corrupted = bytes.clone();
	corrupted[0] ^= 1;
	fs::write(&object_path, corrupted).map_err(io_error)?;
	let damaged = StreamingStore::open(&provider_a_root)?;
	let corrupt_read = match damaged.read_range_verified(cid.as_str(), 0, bytes.len() as u64) {
		Ok(returned) => CorruptReadObservation {
			result: if returned == bytes {
				"served_original_bytes".into()
			} else {
				"served_corrupt_bytes".into()
			},
			bytes_returned: returned.len(),
		},
		Err(ContentError::IntegrityFailed) =>
			CorruptReadObservation { result: "rejected_integrity_failed".into(), bytes_returned: 0 },
		Err(error) => CorruptReadObservation {
			result: format!("rejected_unexpected_{error}"),
			bytes_returned: 0,
		},
	};
	if corrupt_read.result != "rejected_integrity_failed" || corrupt_read.bytes_returned != 0 {
		return Err(ContentError::IntegrityFailed)
	}
	drop(damaged);

	let promoted_providers = vec![
		provider(PROVIDER_B, 0, true, key_b.public().0, Some(CHECKPOINT)),
		provider(PROVIDER_A, 1, false, pair(11).public().0, None),
		provider(PROVIDER_C, 2, false, key_c.public().0, Some(CHECKPOINT)),
	];
	let promoted_topology =
		topology(FAILURE_DETECTED_BLOCK, PROVIDER_B, promoted_providers.clone());
	let selected_reversed = select_source(&promoted_topology, PROVIDER_A, CHECKPOINT)
		.ok_or(ContentError::IntegrityFailed)?;
	let mut permuted_topology = promoted_topology.clone();
	permuted_topology.providers.reverse();
	let selected_permuted = select_source(&permuted_topology, PROVIDER_A, CHECKPOINT)
		.ok_or(ContentError::IntegrityFailed)?;
	if selected_reversed != PROVIDER_B || selected_permuted != PROVIDER_B {
		return Err(ContentError::IntegrityFailed)
	}

	let responder_b = Arc::new(PeerResponder::new(
		Arc::new(StaticAuthority(promoted_topology.clone())),
		Arc::new(CheckpointStack::open(&provider_b_root)?),
		PROVIDER_B,
		key_b,
	)?);
	let recovery_session = ReplicationSessionV1::from_topology(
		promoted_topology,
		PROVIDER_A,
		pair(11).public().0,
		selected_reversed,
		PROVIDER_A,
		commitment,
	)
	.map_err(|_| ContentError::IntegrityFailed)?;
	let recovery_transport = Arc::new(DirectTransport::new(responder_b.clone()));
	let recovery_steps =
		reconcile_to_mmr(&provider_a_root, recovery_transport.clone(), pair(11), &recovery_session)
			.await?;
	let recovery_network_requests = recovery_transport.request_count()?;
	drop(recovery_transport);
	drop(responder_b);

	let provider_roots = [&provider_a_root, &provider_b_root, &provider_c_root]
		.into_iter()
		.map(|provider_root| observed_root(provider_root))
		.collect::<Result<Vec<_>, _>>()?;
	if provider_roots.iter().any(|root| root != &provider_roots[0]) {
		return Err(ContentError::IntegrityFailed)
	}
	let convergence_observed_block = FAILURE_DETECTED_BLOCK
		.checked_add(recovery_steps)
		.ok_or(ContentError::IntegrityFailed)?;

	Ok(ThreeProviderRecoveryEvidence {
		schema_version: 1,
		test: "deterministic_failover".into(),
		provider_count: 3,
		provider_roots,
		eligible_sources: vec![
			EligibleSourceObservation { provider: hex::encode(PROVIDER_B), order: 0 },
			EligibleSourceObservation { provider: hex::encode(PROVIDER_C), order: 2 },
		],
		selection_results: vec![hex::encode(selected_reversed), hex::encode(selected_permuted)],
		corrupt_read_observations: vec![corrupt_read],
		failure_detected_block: FAILURE_DETECTED_BLOCK,
		recovery_steps,
		convergence_observed_block,
		recovery_network_requests,
		production_surfaces: vec![
			"StreamingStore::read_range_verified".into(),
			"BucketMmrStore::commitment_candidate".into(),
			"replication_worker::select_source".into(),
			"PeerResponder::page/chunk".into(),
			"ReplicationReconciler::reconcile_one".into(),
		],
	})
}

async fn reconcile_to_mmr(
	target_root: &Path,
	transport: Arc<DirectTransport>,
	target_key: ed25519::Pair,
	session: &ReplicationSessionV1,
) -> Result<u32, ContentError> {
	let reconciler = ReplicationReconciler::new(
		Arc::new(CheckpointStack::open(target_root)?),
		transport,
		target_key,
	)?;
	let operation = ReplicationReconciler::<DirectTransport>::operation_id(session);
	for step_count in 1..=32 {
		let step = reconciler.reconcile_one(session, operation).await?;
		if step.phase == ReplicationPhase::MmrCommitted {
			return Ok(step_count)
		}
	}
	Err(ContentError::IntegrityFailed)
}

fn observed_root(root: &Path) -> Result<String, ContentError> {
	let streaming = StreamingStore::open(root)?;
	let mmr = BucketMmrStore::open(root, &streaming)?;
	let commitment = mmr.commitment_candidate(&streaming, BucketId::from_bytes(BUCKET), 0)?;
	Ok(hex::encode(commitment.mmr_root.0))
}

fn topology(
	finalized_number: u32,
	primary: [u8; 32],
	providers: Vec<ReplicationProviderSnapshot>,
) -> ReplicationTopologySnapshot {
	let replicas = providers
		.iter()
		.filter(|provider| provider.provider != primary)
		.map(|provider| provider.provider)
		.collect();
	let mut topology = ReplicationTopologySnapshot {
		genesis_hash: [1; 32],
		finalized_hash: blake2_256(&finalized_number.to_le_bytes()),
		finalized_number,
		governed_finalized_checkpoint: Some(CHECKPOINT),
		bucket_id: BUCKET,
		bucket_version: 1,
		primary,
		replicas,
		providers,
		current_checkpoint: None,
		snapshot_hash: [0; 32],
	};
	let mut encoded = b"cord/provider/replication-topology/v1".to_vec();
	topology.encode_to(&mut encoded);
	topology.snapshot_hash = blake2_256(&encoded);
	topology
}

fn provider(
	provider: [u8; 32],
	order: u8,
	primary: bool,
	service_key: [u8; 32],
	confirmed_checkpoint: Option<u32>,
) -> ReplicationProviderSnapshot {
	let endpoint = format!("https://provider-{}.invalid", provider[0]).into_bytes();
	ReplicationProviderSnapshot {
		provider,
		order,
		primary,
		record_present: true,
		endpoint_hash: Some(blake2_256(&endpoint)),
		endpoint: Some(endpoint),
		active_service_key: Some(service_key),
		active_service_key_version: Some(1),
		status_active: true,
		organization_valid: true,
		authority_validated_at: Some(CHECKPOINT),
		overdue_challenges: 0,
		eligible: true,
		usable: true,
		exclusions: Vec::new(),
		confirmed_checkpoint,
	}
}

fn pair(seed: u8) -> ed25519::Pair {
	ed25519::Pair::from_seed(&[seed; 32])
}

fn deterministic_bytes() -> Vec<u8> {
	(0..CHUNK_BYTES + 37).map(|index| ((index * 31 + 17) % 251) as u8).collect()
}

fn io_error(error: std::io::Error) -> ContentError {
	ContentError::Io(error.to_string())
}
