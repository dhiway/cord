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

//! Private authenticated intake for provider replication requests.
//!
//! No request reaches content or durable reply state until its canonical target signature and its
//! complete finalized topology-derived peer context have both been verified.

use std::sync::Arc;

use sp_core::{ed25519, Pair as _};

use crate::{
	chain::ReplicationAuthority,
	checkpoint_stack::CheckpointStack,
	peer::{PeerChunkRequestV1, PeerSyncPageRequestV1},
	replication_session::ReplicationSessionV1,
	ContentError,
};

/// Private source-side responder. It deliberately owns no route or transport listener.
pub(crate) struct PeerResponder<A> {
	authority: Arc<A>,
	stack: Arc<CheckpointStack>,
	local_provider: [u8; 32],
	local_service: ed25519::Pair,
}

impl<A: ReplicationAuthority> PeerResponder<A> {
	pub(crate) fn new(
		authority: Arc<A>,
		stack: Arc<CheckpointStack>,
		local_provider: [u8; 32],
		local_service: ed25519::Pair,
	) -> Result<Self, ContentError> {
		if local_provider == [0; 32] {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(Self { authority, stack, local_provider, local_service })
	}

	#[cfg(test)]
	pub(crate) fn checkpoint_stack(&self) -> &Arc<CheckpointStack> {
		&self.stack
	}

	/// Authenticate topology and durably build or replay one exact page response.
	pub(crate) async fn page(&self, request_bytes: &[u8]) -> Result<Vec<u8>, ContentError> {
		let request = PeerSyncPageRequestV1::decode_authenticated(request_bytes)?;
		if let Some(bytes) = self.stack.replay_peer_page(&request)? {
			return Ok(bytes);
		}
		self.authenticate_context(request.context()).await?;
		self.stack.serve_peer_page(&request, &self.local_service)
	}

	/// Authenticate topology and durably build or replay one exact committed-object chunk.
	pub(crate) async fn chunk(&self, request_bytes: &[u8]) -> Result<Vec<u8>, ContentError> {
		let request = PeerChunkRequestV1::decode_authenticated(request_bytes)?;
		if let Some(bytes) = self.stack.replay_peer_chunk(&request)? {
			return Ok(bytes);
		}
		self.authenticate_context(request.context()).await?;
		self.stack.serve_peer_chunk(&request, &self.local_service)
	}

	async fn authenticate_context(
		&self,
		claimed: &crate::peer::PeerContextV1,
	) -> Result<(), ContentError> {
		if claimed.source_provider() != self.local_provider ||
			self.local_service.public().0 == [0; 32]
		{
			return Err(ContentError::IntegrityFailed);
		}
		let (finalized_hash, finalized_number) = claimed.finalized();
		let pinned_topology = self
			.authority
			.replication_topology_at(claimed.bucket(), finalized_hash, finalized_number)
			.await
			.map_err(|_| ContentError::IntegrityFailed)?;
		let pinned_session = ReplicationSessionV1::from_topology(
			pinned_topology,
			self.local_provider,
			self.local_service.public().0,
			claimed.source_provider(),
			claimed.target_provider(),
			claimed.candidate_commitment(),
		)
		.map_err(|_| ContentError::IntegrityFailed)?;
		if pinned_session.context() != claimed {
			return Err(ContentError::IntegrityFailed);
		}

		// Historical context authenticates the request; current finalized state separately controls
		// whether either peer may perform new content I/O after revocation or key rotation.
		let current_topology = self
			.authority
			.replication_topology(claimed.bucket())
			.await
			.map_err(|_| ContentError::IntegrityFailed)?;
		if current_topology.finalized_number < finalized_number {
			return Err(ContentError::IntegrityFailed);
		}
		let current_session = ReplicationSessionV1::from_topology(
			current_topology,
			self.local_provider,
			self.local_service.public().0,
			claimed.source_provider(),
			claimed.target_provider(),
			claimed.candidate_commitment(),
		)
		.map_err(|_| ContentError::IntegrityFailed)?;
		if current_session.topology().bucket_version != pinned_session.topology().bucket_version ||
			current_session.source().order() != pinned_session.source().order() ||
			current_session.target().order() != pinned_session.target().order() ||
			current_session.source().endpoint_hash() != pinned_session.source().endpoint_hash() ||
			current_session.target().endpoint_hash() != pinned_session.target().endpoint_hash() ||
			current_session.source().service_key() != pinned_session.source().service_key() ||
			current_session.source().service_key_version() !=
				pinned_session.source().service_key_version() ||
			current_session.target().service_key() != pinned_session.target().service_key() ||
			current_session.target().service_key_version() !=
				pinned_session.target().service_key_version()
		{
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use std::{fs, path::PathBuf, sync::RwLock};

	use async_trait::async_trait;
	use codec::Encode;
	use sp_crypto_hashing::blake2_256;
	use tempfile::TempDir;

	use super::*;
	use crate::{
		chain::{
			ChainError, ReplicationProviderExclusion, ReplicationProviderSnapshot,
			ReplicationTopologySnapshot,
		},
		peer::{
			PeerChunkExpectationV1, PeerChunkResponseV1, PeerContextV1, PeerMmrCommitmentV1,
			PeerObjectV1, PeerPageExpectationV1, PeerRequestIdentityV1, PeerSyncPageResponseV1,
		},
		peer_reply::PeerReplyFault,
		storage::{bucket_mmr::BucketMmrStore, StreamingDescriptor, StreamingStore},
		BucketId, CanonicalCid, OperationId, CHUNK_BYTES,
	};

	const REPLY_ROOT: &str = "peer-replies-v1";

	#[derive(Clone)]
	struct MockAuthority {
		topology: ReplicationTopologySnapshot,
	}

	#[async_trait]
	impl ReplicationAuthority for MockAuthority {
		async fn replication_topology(
			&self,
			bucket_id: [u8; 32],
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			if bucket_id != self.topology.bucket_id {
				return Err(ChainError::Rejected("wrong bucket".into()));
			}
			Ok(self.topology.clone())
		}

		async fn replication_topology_at(
			&self,
			bucket_id: [u8; 32],
			finalized_hash: [u8; 32],
			finalized_number: u32,
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			if bucket_id != self.topology.bucket_id ||
				finalized_hash != self.topology.finalized_hash ||
				finalized_number != self.topology.finalized_number
			{
				return Err(ChainError::Rejected("wrong pinned topology".into()));
			}
			Ok(self.topology.clone())
		}
	}

	#[derive(Clone)]
	struct AdvancingAuthority {
		pinned: ReplicationTopologySnapshot,
		current: Arc<RwLock<ReplicationTopologySnapshot>>,
	}

	#[async_trait]
	impl ReplicationAuthority for AdvancingAuthority {
		async fn replication_topology(
			&self,
			bucket_id: [u8; 32],
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			let topology = self.current.read().unwrap().clone();
			if bucket_id != topology.bucket_id {
				return Err(ChainError::Rejected("wrong bucket".into()));
			}
			Ok(topology)
		}

		async fn replication_topology_at(
			&self,
			bucket_id: [u8; 32],
			finalized_hash: [u8; 32],
			finalized_number: u32,
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			if bucket_id != self.pinned.bucket_id ||
				finalized_hash != self.pinned.finalized_hash ||
				finalized_number != self.pinned.finalized_number
			{
				return Err(ChainError::Rejected("wrong pinned topology".into()));
			}
			Ok(self.pinned.clone())
		}
	}

	struct Fixture {
		temp: TempDir,
		topology: ReplicationTopologySnapshot,
		commitment: PeerMmrCommitmentV1,
		object: PeerObjectV1,
		bytes: Vec<u8>,
		cid: CanonicalCid,
	}

	fn pair(seed: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[seed; 32])
	}

	fn provider(
		provider: [u8; 32],
		order: u8,
		primary: bool,
		key: [u8; 32],
		version: u64,
		endpoint: &[u8],
	) -> ReplicationProviderSnapshot {
		ReplicationProviderSnapshot {
			provider,
			order,
			primary,
			record_present: true,
			endpoint: Some(endpoint.to_vec()),
			endpoint_hash: Some(blake2_256(endpoint)),
			active_service_key: Some(key),
			active_service_key_version: Some(version),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(8),
			overdue_challenges: 0,
			eligible: true,
			usable: true,
			exclusions: Vec::new(),
			confirmed_checkpoint: None,
		}
	}

	fn reseal(topology: &mut ReplicationTopologySnapshot) {
		topology.snapshot_hash = [0; 32];
		let mut input = b"cord/provider/replication-topology/v1".to_vec();
		topology.encode_to(&mut input);
		topology.snapshot_hash = blake2_256(&input);
	}

	fn topology() -> ReplicationTopologySnapshot {
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: [1; 32],
			finalized_hash: [2; 32],
			finalized_number: 10,
			governed_finalized_checkpoint: Some(8),
			bucket_id: [3; 32],
			bucket_version: 4,
			primary: [4; 32],
			replicas: vec![[5; 32]],
			providers: vec![
				provider([4; 32], 0, true, pair(11).public().0, 7, b"https://source.invalid"),
				provider([5; 32], 1, false, pair(12).public().0, 9, b"https://target.invalid"),
			],
			current_checkpoint: None,
			snapshot_hash: [0; 32],
		};
		reseal(&mut topology);
		topology
	}

	fn build_fixture() -> Fixture {
		let temp = TempDir::new().unwrap();
		let bytes = vec![31; CHUNK_BYTES + 7];
		let cid = CanonicalCid::from_digest(blake2_256(&bytes));
		let bucket_id = BucketId::from_bytes([3; 32]);
		let operation_id = OperationId::from_bytes([7; 16]);
		let streaming = StreamingStore::open(temp.path()).unwrap();
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id,
					bucket_id,
					expected_cid: cid.as_str().into(),
					object_len: bytes.len() as u64,
				},
				bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec),
			)
			.unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let candidate = mmr.commitment_candidate(&streaming, bucket_id, 0).unwrap();
		let commitment = PeerMmrCommitmentV1::new(candidate.mmr_root.0, 0, 1, 0).unwrap();
		let (items, terminal) =
			mmr.replication_page(&streaming, bucket_id, commitment, None, 1).unwrap();
		assert_eq!(terminal, None);
		let object = items.into_iter().next().unwrap();
		drop(mmr);
		drop(streaming);
		Fixture { temp, topology: topology(), commitment, object, bytes, cid }
	}

	fn context(
		topology: ReplicationTopologySnapshot,
		commitment: PeerMmrCommitmentV1,
	) -> PeerContextV1 {
		ReplicationSessionV1::from_topology(
			topology,
			[4; 32],
			pair(11).public().0,
			[4; 32],
			[5; 32],
			commitment,
		)
		.unwrap()
		.context()
		.clone()
	}

	fn page_request(
		topology: ReplicationTopologySnapshot,
		commitment: PeerMmrCommitmentV1,
		nonce: u8,
		limit: u16,
	) -> PeerSyncPageRequestV1 {
		let expected = PeerPageExpectationV1::new(
			context(topology, commitment),
			PeerRequestIdentityV1::new([15; 16], [nonce; 16]).unwrap(),
			None,
			limit,
		)
		.unwrap();
		PeerSyncPageRequestV1::new_signed(&expected, &pair(12)).unwrap()
	}

	fn chunk_request(
		topology: ReplicationTopologySnapshot,
		commitment: PeerMmrCommitmentV1,
		object: PeerObjectV1,
		nonce: u8,
		index: u16,
	) -> PeerChunkRequestV1 {
		let expected = PeerChunkExpectationV1::new(
			context(topology, commitment),
			PeerRequestIdentityV1::new([15; 16], [nonce; 16]).unwrap(),
			object,
			index,
		)
		.unwrap();
		PeerChunkRequestV1::new_signed(&expected, &pair(12)).unwrap()
	}

	fn make_responder(fixture: &Fixture) -> PeerResponder<MockAuthority> {
		PeerResponder::new(
			Arc::new(MockAuthority { topology: fixture.topology.clone() }),
			Arc::new(CheckpointStack::open(fixture.temp.path()).unwrap()),
			[4; 32],
			pair(11),
		)
		.unwrap()
	}

	fn object_path(fixture: &Fixture) -> PathBuf {
		fixture
			.temp
			.path()
			.join("streaming-v1")
			.join("objects")
			.join(fixture.cid.as_str())
	}

	#[tokio::test]
	async fn page_and_chunk_are_source_signed_durable_and_byte_identical_after_restart() {
		let fixture = build_fixture();
		let page_request = page_request(fixture.topology.clone(), fixture.commitment, 16, 1);
		let chunk_request = chunk_request(
			fixture.topology.clone(),
			fixture.commitment,
			fixture.object.clone(),
			17,
			1,
		);
		let responder = make_responder(&fixture);
		let page = responder.page(&page_request.encode_wire()).await.unwrap();
		let chunk = responder.chunk(&chunk_request.encode_wire()).await.unwrap();
		assert_eq!(
			PeerSyncPageResponseV1::decode_canonical(&page, &page_request).unwrap().items(),
			&[fixture.object.clone()]
		);
		assert_eq!(
			PeerChunkResponseV1::decode_canonical(&chunk, &chunk_request)
				.unwrap()
				.verified_chunk()
				.2,
			&fixture.bytes[CHUNK_BYTES..]
		);
		drop(responder);
		let mut corrupt = fixture.bytes.clone();
		corrupt[0] ^= 1;
		fs::write(object_path(&fixture), corrupt).unwrap();
		let reopened = make_responder(&fixture);
		assert_eq!(reopened.page(&page_request.encode_wire()).await.unwrap(), page);
		assert_eq!(reopened.chunk(&chunk_request.encode_wire()).await.unwrap(), chunk);
	}

	#[tokio::test]
	async fn pinned_progress_survives_head_advance_and_revocation_only_blocks_new_io() {
		let fixture = build_fixture();
		let current = Arc::new(RwLock::new(fixture.topology.clone()));
		let authority = Arc::new(AdvancingAuthority {
			pinned: fixture.topology.clone(),
			current: Arc::clone(&current),
		});
		let responder = PeerResponder::new(
			authority,
			Arc::new(CheckpointStack::open(fixture.temp.path()).unwrap()),
			[4; 32],
			pair(11),
		)
		.unwrap();
		let first = chunk_request(
			fixture.topology.clone(),
			fixture.commitment,
			fixture.object.clone(),
			40,
			0,
		);
		let first_reply = responder.chunk(&first.encode_wire()).await.unwrap();

		{
			let mut advanced = current.write().unwrap();
			advanced.finalized_hash = [42; 32];
			advanced.finalized_number += 1;
			reseal(&mut advanced);
		}
		let second = chunk_request(
			fixture.topology.clone(),
			fixture.commitment,
			fixture.object.clone(),
			41,
			1,
		);
		assert!(responder.chunk(&second.encode_wire()).await.is_ok());

		{
			let mut revoked = current.write().unwrap();
			revoked.finalized_hash = [43; 32];
			revoked.finalized_number += 1;
			revoked.providers[1].active_service_key = None;
			revoked.providers[1].usable = false;
			revoked.providers[1].exclusions = vec![ReplicationProviderExclusion::InvalidServiceKey];
			reseal(&mut revoked);
		}
		let blocked = page_request(fixture.topology.clone(), fixture.commitment, 42, 1);
		assert_eq!(
			responder.page(&blocked.encode_wire()).await,
			Err(ContentError::IntegrityFailed)
		);
		fs::remove_file(object_path(&fixture)).unwrap();
		assert_eq!(responder.chunk(&first.encode_wire()).await.unwrap(), first_reply);
	}

	#[tokio::test]
	async fn confirmation_invalid_target_can_repair_but_cannot_gain_confirmation_authority() {
		let mut fixture = build_fixture();
		fixture.topology.providers[1].usable = false;
		fixture.topology.providers[1].exclusions =
			vec![ReplicationProviderExclusion::ConfirmationInvalid];
		reseal(&mut fixture.topology);
		let request = page_request(fixture.topology.clone(), fixture.commitment, 18, 1);
		let bytes = make_responder(&fixture).page(&request.encode_wire()).await.unwrap();
		assert!(PeerSyncPageResponseV1::decode_canonical(&bytes, &request).is_ok());
	}

	#[tokio::test]
	async fn topology_source_key_context_and_target_signature_fail_before_stack_io() {
		let fixture = build_fixture();
		let request = page_request(fixture.topology.clone(), fixture.commitment, 20, 1);
		let mut signature = request.encode_wire();
		*signature.last_mut().unwrap() ^= 1;
		assert!(make_responder(&fixture).page(&signature).await.is_err());

		let wrong_key = PeerResponder::new(
			Arc::new(MockAuthority { topology: fixture.topology.clone() }),
			Arc::new(CheckpointStack::open(fixture.temp.path()).unwrap()),
			[4; 32],
			pair(99),
		)
		.unwrap();
		assert!(wrong_key.page(&request.encode_wire()).await.is_err());

		let wrong_source_context = PeerContextV1::new(
			[1; 32],
			[2; 32],
			10,
			[3; 32],
			[6; 32],
			[5; 32],
			10,
			pair(13).public().0,
			9,
			pair(12).public().0,
			[21; 32],
			[22; 32],
			fixture.commitment,
		)
		.unwrap();
		let wrong_source = PeerPageExpectationV1::new(
			wrong_source_context,
			PeerRequestIdentityV1::new([15; 16], [21; 16]).unwrap(),
			None,
			1,
		)
		.unwrap();
		let wrong_source = PeerSyncPageRequestV1::new_signed(&wrong_source, &pair(12)).unwrap();
		assert!(make_responder(&fixture).page(&wrong_source.encode_wire()).await.is_err());

		let wrong_context = PeerContextV1::new(
			[1; 32],
			[99; 32],
			10,
			[3; 32],
			[4; 32],
			[5; 32],
			7,
			pair(11).public().0,
			9,
			pair(12).public().0,
			blake2_256(b"https://source.invalid"),
			blake2_256(b"https://target.invalid"),
			fixture.commitment,
		)
		.unwrap();
		let wrong_context = PeerPageExpectationV1::new(
			wrong_context,
			PeerRequestIdentityV1::new([15; 16], [22; 16]).unwrap(),
			None,
			1,
		)
		.unwrap();
		let wrong_context = PeerSyncPageRequestV1::new_signed(&wrong_context, &pair(12)).unwrap();
		assert!(make_responder(&fixture).page(&wrong_context.encode_wire()).await.is_err());

		let mut tampered_topology = fixture.topology.clone();
		tampered_topology.bucket_version += 1;
		let wrong_topology = PeerResponder::new(
			Arc::new(MockAuthority { topology: tampered_topology }),
			Arc::new(CheckpointStack::open(fixture.temp.path()).unwrap()),
			[4; 32],
			pair(11),
		)
		.unwrap();
		assert!(wrong_topology.page(&request.encode_wire()).await.is_err());
	}

	#[tokio::test]
	async fn target_signed_arbitrary_chunk_object_never_reaches_content_bytes() {
		let fixture = build_fixture();
		let invented_bytes = vec![77; fixture.bytes.len()];
		let invented = PeerObjectV1::new(
			&CanonicalCid::from_digest(blake2_256(&invented_bytes)),
			invented_bytes.len() as u64,
			0,
			invented_bytes.len() as u64,
			invented_bytes.chunks(CHUNK_BYTES).map(blake2_256).collect(),
		)
		.unwrap();
		let request = chunk_request(fixture.topology.clone(), fixture.commitment, invented, 23, 0);
		assert!(make_responder(&fixture).chunk(&request.encode_wire()).await.is_err());
		assert_eq!(fs::read_dir(fixture.temp.path().join(REPLY_ROOT)).unwrap().count(), 0);
	}

	#[tokio::test]
	async fn corrupt_and_quarantined_sources_never_return_or_store_a_reply() {
		let corrupt = build_fixture();
		let request = page_request(corrupt.topology.clone(), corrupt.commitment, 24, 1);
		let mut damaged = corrupt.bytes.clone();
		damaged[0] ^= 1;
		fs::write(object_path(&corrupt), &damaged).unwrap();
		assert!(make_responder(&corrupt).page(&request.encode_wire()).await.is_err());
		assert_eq!(fs::read_dir(corrupt.temp.path().join(REPLY_ROOT)).unwrap().count(), 0);

		let quarantined = build_fixture();
		let request = page_request(quarantined.topology.clone(), quarantined.commitment, 25, 1);
		let path = object_path(&quarantined);
		let mut damaged = quarantined.bytes.clone();
		damaged[0] ^= 1;
		fs::write(&path, damaged).unwrap();
		let streaming = StreamingStore::open(quarantined.temp.path()).unwrap();
		assert!(streaming.verify_installed(quarantined.cid.as_str()).is_err());
		drop(streaming);
		fs::write(path, &quarantined.bytes).unwrap();
		assert!(make_responder(&quarantined).page(&request.encode_wire()).await.is_err());
		assert_eq!(fs::read_dir(quarantined.temp.path().join(REPLY_ROOT)).unwrap().count(), 0);
	}

	#[tokio::test]
	async fn changed_replay_key_input_conflicts_and_persistence_fault_poison_returns_nothing() {
		let fixture = build_fixture();
		let request = page_request(fixture.topology.clone(), fixture.commitment, 26, 1);
		let responder = make_responder(&fixture);
		responder.page(&request.encode_wire()).await.unwrap();
		let changed = page_request(fixture.topology.clone(), fixture.commitment, 26, 2);
		assert_eq!(
			responder.page(&changed.encode_wire()).await,
			Err(ContentError::IdempotencyConflict)
		);

		let faulted = build_fixture();
		let request = page_request(faulted.topology.clone(), faulted.commitment, 27, 1);
		let responder = make_responder(&faulted);
		responder
			.stack
			.inject_peer_reply_fault_once(PeerReplyFault::BeforeTempFsync)
			.unwrap();
		assert!(matches!(responder.page(&request.encode_wire()).await, Err(ContentError::Io(_))));
		assert_eq!(
			responder.page(&request.encode_wire()).await,
			Err(ContentError::IntegrityFailed)
		);
	}
}
