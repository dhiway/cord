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

//! Private desktop host socket and exact bounded host-v2 framing.

use std::{sync::Arc, time::Duration};

use ciborium::value::Value;
use rand::{rngs::OsRng, RngCore};
use sp_core::Pair as _;
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::{UnixListener, UnixStream},
	sync::Semaphore,
};

use crate::{
	capability::ProviderCapabilityV1,
	chain::{ReplicationAuthority, ReplicationTopologySnapshot},
	checkpoint_stack::{PrivateHostKernelError, PrivateHostRootV1, PrivateHostStorageKind},
	storage::streaming::{
		private_query::PrivateObjectRequestV2,
		recovery::{ObjectPutRequestV2, ResponseAckV1, ResumeTokenV1},
	},
	ChainAuthority, FinalizedRuntimeAuthority, ProviderService,
};

const MAX_HOST_FRAME_BYTES: usize = 4_194_304;
const MAX_PRIVATE_HOST_CONNECTIONS: usize = 64;
const HOST_FRAME_TIMEOUT: Duration = Duration::from_secs(30);

#[doc(hidden)]
#[derive(Debug, thiserror::Error)]
pub enum PrivateHostIpcError {
	#[error("DESKTOP_FRAME_TOO_LARGE")]
	FrameTooLarge,
	#[error("DESKTOP_FRAME_TRUNCATED")]
	FrameTruncated,
	#[error("DESKTOP_PEER_BINDING_REJECTED")]
	PeerBinding,
	#[error("PRIVATE_HOST_KERNEL: {0}")]
	Kernel(String),
	#[error("PRIVATE_HOST_CHAIN_AUTHORITY: {0}")]
	Chain(String),
	#[error("PRIVATE_HOST_WIRE_INVALID")]
	Wire,
	#[error("PRIVATE_HOST_FRAME_TIMEOUT")]
	Timeout,
	#[error("DESKTOP_IO: {0}")]
	Io(#[from] std::io::Error),
}

impl From<PrivateHostKernelError> for PrivateHostIpcError {
	fn from(error: PrivateHostKernelError) -> Self {
		Self::Kernel(error.to_string())
	}
}

/// Serve the concrete production authority through the single startup-validated checkpoint stack.
pub async fn serve_private_host_ipc(
	listener: UnixListener,
	allowed_user_id: u32,
	service: Arc<ProviderService<FinalizedRuntimeAuthority>>,
) -> Result<(), PrivateHostIpcError> {
	serve_private_host_ipc_with(listener, allowed_user_id, service).await
}

async fn serve_private_host_ipc_with<A>(
	listener: UnixListener,
	allowed_user_id: u32,
	service: Arc<ProviderService<A>>,
) -> Result<(), PrivateHostIpcError>
where
	A: ChainAuthority + ReplicationAuthority,
{
	let permits = Arc::new(Semaphore::new(MAX_PRIVATE_HOST_CONNECTIONS));
	loop {
		let permit = Arc::clone(&permits)
			.acquire_owned()
			.await
			.map_err(|_| PrivateHostIpcError::Wire)?;
		match PrivateHostConnection::accept(&listener, allowed_user_id).await {
			Ok(connection) => {
				let service = Arc::clone(&service);
				tokio::spawn(async move {
					let _permit = permit;
					let _ = serve_connection(connection, service).await;
				});
			},
			Err(PrivateHostIpcError::PeerBinding) => drop(permit),
			Err(error) => return Err(error),
		}
	}
}

async fn serve_connection<A>(
	mut connection: PrivateHostConnection,
	service: Arc<ProviderService<A>>,
) -> Result<(), PrivateHostIpcError>
where
	A: ChainAuthority + ReplicationAuthority,
{
	loop {
		let request = connection.read_frame().await?;
		if ResponseAckV1::decode(&request).is_ok() {
			let ack = service.checkpoint_stack().acknowledge_private_host_recovery(&request)?;
			connection.write_frame(&ack_confirmation(&ack)).await?;
			continue;
		}
		let authority = connection.read_frame().await?;
		let root = service.checkpoint_stack().private_host_root(&request, &authority)?;
		let snapshot = service
			.authority()
			.capability_authority_snapshot(root.grant_id, root.agreement_id)
			.await
			.map_err(|error| PrivateHostIpcError::Chain(error.to_string()))?;
		let service_key = service.private_host_service_key();
		let public_key = service_key.public().0;
		let generation = if is_cancel(&request)? {
			service.checkpoint_stack().cancel_private_host_generation(
				&root,
				&request,
				&authority,
				&snapshot,
				public_key,
				&service_key,
			)?
		} else {
			let successor_nonce = random_nonce();
			match root.kind {
				PrivateHostStorageKind::Put => {
					let payload = if put_expects_payload(&root, &authority)? {
						Some(connection.read_frame().await?)
					} else {
						None
					};
					service.checkpoint_stack().execute_private_put_generation(
						&root,
						&authority,
						payload.as_deref(),
						snapshot,
						public_key,
						&service_key,
						successor_nonce,
					)?
				},
				PrivateHostStorageKind::Query => {
					let topology = pinned_replication_topology(
						service.authority().as_ref(),
						root.bucket_id,
						&snapshot.finalized_hash,
						snapshot.finalized_number,
					)
					.await?;
					service.checkpoint_stack().execute_private_query_generation(
						&root,
						&authority,
						&snapshot,
						&topology,
						&service_key,
						successor_nonce,
					)?
				},
			}
		};
		for frame in &generation.frames {
			connection.write_frame(&frame).await?;
		}
		if let Some(token) = &generation.successor_token {
			connection.write_frame(&token).await?;
		}
		let exact_ack = connection.read_frame().await?;
		let ack = ResponseAckV1::decode(&exact_ack).map_err(|_| PrivateHostIpcError::Wire)?;
		if ack != generation.expected_ack {
			return Err(PrivateHostIpcError::Wire);
		}
		service.checkpoint_stack().acknowledge_private_host_generation(
			root.kind,
			root.host_key_id,
			&exact_ack,
		)?;
		connection.write_frame(&ack_confirmation(&ack)).await?;
	}
}

async fn pinned_replication_topology<A: ReplicationAuthority>(
	authority: &A,
	bucket_id: [u8; 32],
	finalized_hash: &str,
	finalized_number: u32,
) -> Result<ReplicationTopologySnapshot, PrivateHostIpcError> {
	let finalized_hash = decode_hash(finalized_hash)?;
	let topology = authority
		.replication_topology_at(bucket_id, finalized_hash, finalized_number)
		.await
		.map_err(|error| PrivateHostIpcError::Chain(error.to_string()))?;
	if topology.bucket_id != bucket_id ||
		topology.finalized_hash != finalized_hash ||
		topology.finalized_number != finalized_number
	{
		return Err(PrivateHostIpcError::Chain("pinned replication topology mismatch".into()));
	}
	Ok(topology)
}

fn decode_hash(exact: &str) -> Result<[u8; 32], PrivateHostIpcError> {
	let exact = exact.strip_prefix("0x").unwrap_or(exact);
	if exact.len() != 64 || exact.bytes().any(|byte| !byte.is_ascii_hexdigit()) {
		return Err(PrivateHostIpcError::Wire);
	}
	hex::decode(exact)
		.map_err(|_| PrivateHostIpcError::Wire)?
		.try_into()
		.map_err(|_| PrivateHostIpcError::Wire)
}

fn put_expects_payload(
	root: &PrivateHostRootV1,
	authority: &[u8],
) -> Result<bool, PrivateHostIpcError> {
	if ProviderCapabilityV1::decode(authority).is_ok() {
		return Ok(false)
	}
	let token = ResumeTokenV1::decode(authority).map_err(|_| PrivateHostIpcError::Wire)?;
	let request =
		ObjectPutRequestV2::decode(&root.exact_request).map_err(|_| PrivateHostIpcError::Wire)?;
	Ok(u64::from(token.cursor) < request.object_len.div_ceil(crate::CHUNK_BYTES as u64))
}

fn is_cancel(exact: &[u8]) -> Result<bool, PrivateHostIpcError> {
	if ObjectPutRequestV2::decode(exact).is_ok() || PrivateObjectRequestV2::decode(exact).is_ok() {
		return Ok(false)
	}
	let value: Value = ciborium::from_reader(exact).map_err(|_| PrivateHostIpcError::Wire)?;
	let Value::Map(fields) = &value else { return Err(PrivateHostIpcError::Wire) };
	if fields.len() != 5 || uint_field(&value, 0) != Some(2) || uint_field(&value, 3) != Some(4) {
		return Err(PrivateHostIpcError::Wire)
	}
	let Value::Map(body) = field(&value, 4).ok_or(PrivateHostIpcError::Wire)? else {
		return Err(PrivateHostIpcError::Wire)
	};
	if body.len() != 1 || uint_field(field(&value, 4).unwrap(), 0) != Some(107) {
		return Err(PrivateHostIpcError::Wire)
	}
	let mut canonical = Vec::new();
	ciborium::ser::into_writer(&value, &mut canonical).map_err(|_| PrivateHostIpcError::Wire)?;
	if canonical != exact {
		return Err(PrivateHostIpcError::Wire)
	}
	Ok(true)
}

fn ack_confirmation(ack: &ResponseAckV1) -> Vec<u8> {
	canonical_map(vec![
		(0, Value::Integer(1.into())),
		(1, Value::Bytes(ack.request_id.to_vec())),
		(2, Value::Bytes(ack.operation_id.to_vec())),
		(3, Value::Integer(ack.generation.into())),
		(4, Value::Bytes(ack.response_hash.to_vec())),
		(5, Value::Bool(true)),
	])
}

fn random_nonce() -> [u8; 16] {
	let mut nonce = [0; 16];
	OsRng.fill_bytes(&mut nonce);
	nonce
}

fn field(value: &Value, wanted: u64) -> Option<&Value> {
	let Value::Map(fields) = value else { return None };
	fields.iter().find_map(|(key, value)| {
		matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(wanted))
			.then_some(value)
	})
}

fn uint_field(value: &Value, wanted: u64) -> Option<u64> {
	let Value::Integer(value) = field(value, wanted)? else { return None };
	u64::try_from(*value).ok()
}

fn canonical_map(entries: Vec<(u64, Value)>) -> Vec<u8> {
	let value = Value::Map(
		entries
			.into_iter()
			.map(|(key, value)| (Value::Integer(key.into()), value))
			.collect(),
	);
	let mut exact = Vec::new();
	ciborium::ser::into_writer(&value, &mut exact).expect("bounded private IPC map");
	exact
}

/// One kernel-authenticated local connection. The OS credential is captured before any frame is
/// accepted and cannot be substituted by request bytes.
pub(crate) struct PrivateHostConnection {
	stream: UnixStream,
	pub(crate) peer_process_id: Option<u32>,
	pub(crate) peer_user_id: u32,
}

impl PrivateHostConnection {
	pub(crate) async fn accept(
		listener: &UnixListener,
		allowed_user_id: u32,
	) -> Result<Self, PrivateHostIpcError> {
		let (stream, _) = listener.accept().await?;
		let credential = stream.peer_cred()?;
		if credential.uid() != allowed_user_id {
			return Err(PrivateHostIpcError::PeerBinding);
		}
		Ok(Self {
			peer_process_id: credential.pid().and_then(|pid| pid.try_into().ok()),
			peer_user_id: credential.uid(),
			stream,
		})
	}

	pub(crate) async fn read_frame(&mut self) -> Result<Vec<u8>, PrivateHostIpcError> {
		self.read_frame_with_timeout(HOST_FRAME_TIMEOUT).await
	}

	async fn read_frame_with_timeout(
		&mut self,
		timeout: Duration,
	) -> Result<Vec<u8>, PrivateHostIpcError> {
		tokio::time::timeout(timeout, async {
			let mut header = [0u8; 4];
			self.stream.read_exact(&mut header).await.map_err(map_frame_read_error)?;
			let length = u32::from_be_bytes(header) as usize;
			if length > MAX_HOST_FRAME_BYTES {
				return Err(PrivateHostIpcError::FrameTooLarge);
			}
			let mut payload = vec![0; length];
			self.stream.read_exact(&mut payload).await.map_err(map_frame_read_error)?;
			Ok(payload)
		})
		.await
		.map_err(|_| PrivateHostIpcError::Timeout)?
	}

	pub(crate) async fn write_frame(&mut self, payload: &[u8]) -> Result<(), PrivateHostIpcError> {
		self.write_frame_with_timeout(payload, HOST_FRAME_TIMEOUT).await
	}

	async fn write_frame_with_timeout(
		&mut self,
		payload: &[u8],
		timeout: Duration,
	) -> Result<(), PrivateHostIpcError> {
		if payload.len() > MAX_HOST_FRAME_BYTES {
			return Err(PrivateHostIpcError::FrameTooLarge);
		}
		let length: u32 =
			payload.len().try_into().map_err(|_| PrivateHostIpcError::FrameTooLarge)?;
		tokio::time::timeout(timeout, async {
			self.stream.write_all(&length.to_be_bytes()).await?;
			self.stream.write_all(payload).await?;
			self.stream.flush().await?;
			Ok(())
		})
		.await
		.map_err(|_| PrivateHostIpcError::Timeout)?
	}
}

fn map_frame_read_error(error: std::io::Error) -> PrivateHostIpcError {
	if error.kind() == std::io::ErrorKind::UnexpectedEof {
		PrivateHostIpcError::FrameTruncated
	} else {
		PrivateHostIpcError::Io(error)
	}
}

#[cfg(test)]
mod tests {
	use std::{
		os::unix::fs::MetadataExt,
		sync::atomic::{AtomicBool, Ordering},
	};

	use super::*;
	use crate::{
		capability::NORMATIVE_REGISTRY_SHA256,
		chain::{ChainError, ReplicationProviderSnapshot},
		storage::{
			bucket_mmr::BucketMmrStore, streaming::local_put_session::ProviderTransferChunkV1,
			StreamingStore,
		},
		AgreementAuthorization, BucketId, ChallengeBatch, CheckpointDutyBatch,
		CheckpointDutyPageRequest, NodeProfile, OperationId, StreamingDescriptor, CHUNK_BYTES,
	};
	use async_trait::async_trait;
	use blake2::{digest::consts::U32, Blake2b, Digest as _};
	use codec::Encode;
	use orbis_storage_runtime_api::{
		AgreementInfo, AgreementStatus, BucketGrantInfo, BucketRole, CheckpointInfo,
		CommitmentInfo, ControlBucketInfo, HostDelegationInfo,
	};
	use sha2::Sha256;
	use sp_core::{crypto::AccountId32, ed25519, H256};

	struct MovingHeadAuthority {
		pinned: ReplicationTopologySnapshot,
		latest_called: AtomicBool,
	}

	#[async_trait]
	impl ReplicationAuthority for MovingHeadAuthority {
		async fn replication_topology(
			&self,
			_: [u8; 32],
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			self.latest_called.store(true, Ordering::SeqCst);
			Err(ChainError::Rejected("moving head must not be read".into()))
		}

		async fn replication_topology_at(
			&self,
			bucket_id: [u8; 32],
			finalized_hash: [u8; 32],
			finalized_number: u32,
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			assert_eq!(bucket_id, self.pinned.bucket_id);
			assert_eq!(finalized_hash, self.pinned.finalized_hash);
			assert_eq!(finalized_number, self.pinned.finalized_number);
			Ok(self.pinned.clone())
		}
	}

	fn pinned_topology() -> ReplicationTopologySnapshot {
		ReplicationTopologySnapshot {
			genesis_hash: [1; 32],
			finalized_hash: [2; 32],
			finalized_number: 10,
			governed_finalized_checkpoint: None,
			bucket_id: [3; 32],
			bucket_version: 4,
			primary: [5; 32],
			replicas: Vec::new(),
			providers: Vec::new(),
			current_checkpoint: None,
			snapshot_hash: [6; 32],
		}
	}

	#[derive(Clone)]
	struct PrivateHostAuthority {
		snapshot: crate::CapabilityAuthoritySnapshot,
		topology: ReplicationTopologySnapshot,
	}

	#[async_trait]
	impl ChainAuthority for PrivateHostAuthority {
		async fn capability_authority_snapshot(
			&self,
			_: [u8; 32],
			_: Option<[u8; 32]>,
		) -> Result<crate::CapabilityAuthoritySnapshot, ChainError> {
			Ok(self.snapshot.clone())
		}

		async fn authorize_commit(
			&self,
			_: [u8; 32],
			_: [u8; 32],
			_: u64,
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("unused".into()))
		}

		async fn authorize_delete(
			&self,
			_: [u8; 32],
			_: [u8; 32],
		) -> Result<AgreementAuthorization, ChainError> {
			Err(ChainError::Rejected("unused".into()))
		}

		async fn challenge_duties(&self, _: Option<u32>) -> Result<ChallengeBatch, ChainError> {
			Err(ChainError::Rejected("unused".into()))
		}

		async fn checkpoint_duties(
			&self,
			_: Option<CheckpointDutyPageRequest>,
		) -> Result<CheckpointDutyBatch, ChainError> {
			Err(ChainError::Rejected("unused".into()))
		}
	}

	#[async_trait]
	impl ReplicationAuthority for PrivateHostAuthority {
		async fn replication_topology(
			&self,
			_: [u8; 32],
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			panic!("private host must use pinned topology")
		}

		async fn replication_topology_at(
			&self,
			_: [u8; 32],
			_: [u8; 32],
			_: u32,
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			Ok(self.topology.clone())
		}
	}

	struct PutIpcFixture {
		_temp: tempfile::TempDir,
		service: Arc<ProviderService<PrivateHostAuthority>>,
		request: ObjectPutRequestV2,
		capability: ProviderCapabilityV1,
		host: ed25519::Pair,
		query_cid: crate::CanonicalCid,
		query_len: u64,
	}

	fn put_ipc_fixture() -> PutIpcFixture {
		const PUT: u16 = 1010;
		let temp = tempfile::tempdir().unwrap();
		let host = ed25519::Pair::from_seed(&[9; 32]);
		let service_key = ed25519::Pair::from_seed(&[8; 32]);
		let query_bytes = b"query object bytes".to_vec();
		let query_cid =
			crate::CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(&query_bytes));
		let streaming = StreamingStore::open(temp.path()).unwrap();
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: OperationId::from_bytes([12; 16]),
					bucket_id: BucketId::from_bytes([5; 32]),
					expected_cid: query_cid.as_str().into(),
					object_len: query_bytes.len() as u64,
				},
				query_bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec).collect::<Vec<_>>(),
			)
			.unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		mmr.append_verified(
			&streaming,
			BucketId::from_bytes([5; 32]),
			OperationId::from_bytes([12; 16]),
		)
		.unwrap();
		let commitment =
			mmr.commitment_candidate(&streaming, BucketId::from_bytes([5; 32]), 0).unwrap();
		drop(mmr);
		drop(streaming);
		let bytes = b"first";
		let cid = crate::CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(bytes));
		let request = ObjectPutRequestV2 {
			request_id: [1; 16],
			product_id: "festival".into(),
			grant_id: [3; 32],
			operation_id: [4; 16],
			trace_context: None,
			deadline: 120,
			bucket_id: [5; 32],
			cid: cid.clone(),
			object_len: bytes.len() as u64,
			mode: 0,
		};
		let mut capability = ProviderCapabilityV1 {
			version: 1,
			registry_sha256: NORMATIVE_REGISTRY_SHA256,
			genesis_hash: [2; 32],
			grant_id: [3; 32],
			issuer_key_id: [6; 32],
			product_id: "festival".into(),
			bucket_id: [5; 32],
			agreement_id: Some([11; 32]),
			provider: [7; 32],
			methods: vec![PUT],
			cid: Some(cid),
			max_bytes: bytes.len() as u64,
			issued_at: 100,
			expires_at: 128,
			nonce: [9; 16],
			signature: [0; 64],
		};
		capability.signature = host.sign(&capability.signed_preimage()).0;
		let local = AccountId32::new([7; 32]);
		let snapshot = crate::CapabilityAuthoritySnapshot {
			finalized_hash: format!("0x{}", hex::encode([10; 32])),
			finalized_number: 110,
			genesis_hash: [2; 32],
			registry_sha256: NORMATIVE_REGISTRY_SHA256,
			local_provider: [7; 32],
			delegation: HostDelegationInfo {
				grant_id: H256([3; 32]),
				bucket_id: H256([5; 32]),
				owner: AccountId32::new([1; 32]),
				issuance_nonce: 0,
				issuer_key_id: H256([6; 32]),
				issuer_public_key: host.public().0,
				key_version: 1,
				state_version: 1,
				key_activated_at: 90,
				product_id: b"festival".to_vec(),
				methods: vec![PUT, 1011, 1012, 1014],
				cid: None,
				max_bytes: 1024,
				issued_at: 90,
				expires_at: 200,
				revoked_at: None,
			},
			bucket: ControlBucketInfo {
				bucket_id: H256([5; 32]),
				owner: AccountId32::new([1; 32]),
				version: 1,
				policy: H256([1; 32]),
				primary: local.clone(),
				replicas: vec![],
				grants: vec![BucketGrantInfo {
					account: AccountId32::new([1; 32]),
					role: BucketRole::Admin,
				}],
				created_at: 1,
			},
			agreement: Some(AgreementInfo {
				agreement_id: H256([11; 32]),
				owner: AccountId32::new([1; 32]),
				bucket_id: H256([5; 32]),
				primary: local,
				replicas: vec![],
				bytes: 1024,
				created_at: 90,
				expires_at: 180,
				release_at: None,
				state_version: 1,
				status: AgreementStatus::Active,
			}),
		};
		let endpoint = b"https://origin-provider.invalid".to_vec();
		let provider = ReplicationProviderSnapshot {
			provider: [7; 32],
			order: 0,
			primary: true,
			record_present: true,
			endpoint_hash: Some(sp_crypto_hashing::blake2_256(&endpoint)),
			endpoint: Some(endpoint),
			active_service_key: Some(service_key.public().0),
			active_service_key_version: Some(1),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(108),
			overdue_challenges: 0,
			eligible: true,
			usable: true,
			exclusions: Vec::new(),
			confirmed_checkpoint: Some(108),
		};
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: [2; 32],
			finalized_hash: [10; 32],
			finalized_number: 110,
			governed_finalized_checkpoint: Some(108),
			bucket_id: [5; 32],
			bucket_version: 1,
			primary: [7; 32],
			replicas: vec![],
			providers: vec![provider],
			current_checkpoint: Some(CheckpointInfo {
				bucket_id: H256([5; 32]),
				commitment: CommitmentInfo {
					mmr_root: commitment.mmr_root,
					start_seq: commitment.start_seq,
					leaf_count: commitment.leaf_count,
				},
				checkpoint_block: 108,
				primary_signers: 1,
				commitment_nonce: 108,
				replica_confirmations: vec![],
			}),
			snapshot_hash: [0; 32],
		};
		let mut topology_input = b"cord/provider/replication-topology/v1".to_vec();
		topology.encode_to(&mut topology_input);
		topology.snapshot_hash = sp_crypto_hashing::blake2_256(&topology_input);
		let profile = NodeProfile {
			provider: hex::encode([7; 32]),
			endpoint: "https://origin-provider.invalid".into(),
			service_key: hex::encode(service_key.public().0),
			region: None,
		};
		let service = Arc::new(
			ProviderService::open(
				temp.path(),
				profile,
				1024 * 1024,
				Arc::new(PrivateHostAuthority { snapshot, topology }),
				service_key,
			)
			.unwrap(),
		);
		PutIpcFixture {
			_temp: temp,
			service,
			request,
			capability,
			host,
			query_cid,
			query_len: query_bytes.len() as u64,
		}
	}

	async fn client_write(stream: &mut UnixStream, bytes: &[u8]) {
		stream.write_all(&(bytes.len() as u32).to_be_bytes()).await.unwrap();
		stream.write_all(bytes).await.unwrap();
	}

	async fn client_read(stream: &mut UnixStream) -> Vec<u8> {
		let mut header = [0; 4];
		stream.read_exact(&mut header).await.unwrap();
		let mut bytes = vec![0; u32::from_be_bytes(header) as usize];
		stream.read_exact(&mut bytes).await.unwrap();
		bytes
	}

	fn query_material(
		fixture: &PutIpcFixture,
		method: u16,
		range: Option<(u64, u64)>,
		nonce: u8,
	) -> (Vec<u8>, Vec<u8>, [u8; 16], [u8; 16]) {
		let request_id = [nonce; 16];
		let request = PrivateObjectRequestV2 {
			request_id,
			product_id: "festival".into(),
			method,
			grant_id: [3; 32],
			trace_context: None,
			deadline: 120,
			bucket_id: [5; 32],
			cid: fixture.query_cid.clone(),
			range,
		}
		.canonical_bytes();
		let authorized_bytes = range.map_or_else(
			|| if method == 1014 { 0 } else { fixture.query_len },
			|(_, length)| length,
		);
		let mut capability = ProviderCapabilityV1 {
			version: 1,
			registry_sha256: NORMATIVE_REGISTRY_SHA256,
			genesis_hash: [2; 32],
			grant_id: [3; 32],
			issuer_key_id: [6; 32],
			product_id: "festival".into(),
			bucket_id: [5; 32],
			agreement_id: Some([11; 32]),
			provider: [7; 32],
			methods: vec![method],
			cid: Some(fixture.query_cid.clone()),
			max_bytes: authorized_bytes,
			issued_at: 100,
			expires_at: 128,
			nonce: [nonce; 16],
			signature: [0; 64],
		};
		capability.signature = fixture.host.sign(&capability.signed_preimage()).0;
		let mut operation_material = b"cord/provider/private-object-query/v1".to_vec();
		operation_material.extend_from_slice(&method.to_be_bytes());
		operation_material.extend_from_slice(&request_id);
		let operation_id = Sha256::digest(operation_material)[..16].try_into().unwrap();
		(request, capability.canonical_bytes(), request_id, operation_id)
	}

	fn terminal_query_hash(frames: &[Vec<u8>]) -> [u8; 32] {
		Sha256::digest(canonical_map(vec![
			(0, Value::Integer(2.into())),
			(1, Value::Integer(0.into())),
			(2, Value::Array(frames.iter().cloned().map(Value::Bytes).collect())),
		]))
		.into()
	}

	#[tokio::test]
	async fn unix_put_lifecycle_binds_each_generation_ack_and_recovers_lost_confirmation() {
		let fixture = put_ipc_fixture();
		let socket = fixture._temp.path().join("private-host-test.sock");
		let listener = UnixListener::bind(&socket).unwrap();
		let uid = std::fs::symlink_metadata(&socket).unwrap().uid();
		let server =
			tokio::spawn(serve_private_host_ipc_with(listener, uid, Arc::clone(&fixture.service)));
		let request = fixture.request.canonical_bytes();
		let capability = fixture.capability.canonical_bytes();

		let mut first = UnixStream::connect(&socket).await.unwrap();
		client_write(&mut first, &request).await;
		client_write(&mut first, &capability).await;
		let accepted = client_read(&mut first).await;
		let first_token = client_read(&mut first).await;
		let first_ack = ResponseAckV1 {
			request_id: fixture.request.request_id,
			operation_id: fixture.request.operation_id,
			generation: 0,
			response_hash: Sha256::digest(&accepted).into(),
		};
		client_write(&mut first, &first_ack.canonical_bytes()).await;
		assert_eq!(client_read(&mut first).await, ack_confirmation(&first_ack));
		drop(first);

		let payload = ProviderTransferChunkV1 {
			operation_id: fixture.request.operation_id,
			index: 0,
			bytes: b"first".to_vec(),
			hash: Blake2b::<U32>::digest(b"first").into(),
		}
		.canonical_bytes();
		let mut progress_connection = UnixStream::connect(&socket).await.unwrap();
		client_write(&mut progress_connection, &request).await;
		client_write(&mut progress_connection, &first_token).await;
		client_write(&mut progress_connection, &payload).await;
		let progress = client_read(&mut progress_connection).await;
		let final_token = client_read(&mut progress_connection).await;
		let progress_ack = ResponseAckV1 {
			request_id: fixture.request.request_id,
			operation_id: fixture.request.operation_id,
			generation: 1,
			response_hash: Sha256::digest(&progress).into(),
		};
		// A valid prior-generation ACK on the same authenticated host must not acknowledge the
		// generation which was just emitted.
		client_write(&mut progress_connection, &first_ack.canonical_bytes()).await;
		let mut closed = [0; 1];
		assert_eq!(progress_connection.read(&mut closed).await.unwrap(), 0);
		drop(progress_connection);

		// A reconnect may carry the exact ACK as its first and only request frame after the
		// provider confirmation was lost.
		let mut ack_only = UnixStream::connect(&socket).await.unwrap();
		client_write(&mut ack_only, &progress_ack.canonical_bytes()).await;
		assert_eq!(client_read(&mut ack_only).await, ack_confirmation(&progress_ack));
		drop(ack_only);

		let mut finalize = UnixStream::connect(&socket).await.unwrap();
		client_write(&mut finalize, &request).await;
		client_write(&mut finalize, &final_token).await;
		let installed = client_read(&mut finalize).await;
		let terminal_ack = ResponseAckV1 {
			request_id: fixture.request.request_id,
			operation_id: fixture.request.operation_id,
			generation: 2,
			response_hash: Sha256::digest(&installed).into(),
		};
		client_write(&mut finalize, &terminal_ack.canonical_bytes()).await;
		assert_eq!(client_read(&mut finalize).await, ack_confirmation(&terminal_ack));
		drop(finalize);

		let mut duplicate = UnixStream::connect(&socket).await.unwrap();
		client_write(&mut duplicate, &terminal_ack.canonical_bytes()).await;
		assert_eq!(client_read(&mut duplicate).await, ack_confirmation(&terminal_ack));
		server.abort();
	}

	#[tokio::test]
	async fn unix_put_cancel_is_terminal_acknowledged_and_recoverable() {
		let fixture = put_ipc_fixture();
		let socket = fixture._temp.path().join("private-host-cancel-test.sock");
		let listener = UnixListener::bind(&socket).unwrap();
		let uid = std::fs::symlink_metadata(&socket).unwrap().uid();
		let server =
			tokio::spawn(serve_private_host_ipc_with(listener, uid, Arc::clone(&fixture.service)));
		let request = fixture.request.canonical_bytes();

		let mut accept = UnixStream::connect(&socket).await.unwrap();
		client_write(&mut accept, &request).await;
		client_write(&mut accept, &fixture.capability.canonical_bytes()).await;
		let accepted = client_read(&mut accept).await;
		let token = client_read(&mut accept).await;
		let accepted_ack = ResponseAckV1 {
			request_id: fixture.request.request_id,
			operation_id: fixture.request.operation_id,
			generation: 0,
			response_hash: Sha256::digest(&accepted).into(),
		};
		client_write(&mut accept, &accepted_ack.canonical_bytes()).await;
		assert_eq!(client_read(&mut accept).await, ack_confirmation(&accepted_ack));
		drop(accept);

		let accepted_value: Value = ciborium::from_reader(accepted.as_slice()).unwrap();
		let cancel_sequence = uint_field(&accepted_value, 2).unwrap().checked_add(1).unwrap();
		let cancel = canonical_map(vec![
			(0, Value::Integer(2.into())),
			(1, Value::Bytes(fixture.request.request_id.to_vec())),
			(2, Value::Integer(cancel_sequence.into())),
			(3, Value::Integer(4.into())),
			(4, Value::Map(vec![(Value::Integer(0.into()), Value::Integer(107.into()))])),
		]);
		let token_dto = ResumeTokenV1::decode(&token).unwrap();
		let mut cancel_connection = UnixStream::connect(&socket).await.unwrap();
		client_write(&mut cancel_connection, &cancel).await;
		client_write(&mut cancel_connection, &token).await;
		let terminal = client_read(&mut cancel_connection).await;
		assert_eq!(terminal, cancel);
		let cancel_ack = ResponseAckV1 {
			request_id: fixture.request.request_id,
			operation_id: token_dto.operation_id,
			generation: token_dto.generation,
			response_hash: Sha256::digest(&terminal).into(),
		};
		client_write(&mut cancel_connection, &cancel_ack.canonical_bytes()).await;
		assert_eq!(client_read(&mut cancel_connection).await, ack_confirmation(&cancel_ack));
		drop(cancel_connection);

		let mut duplicate = UnixStream::connect(&socket).await.unwrap();
		client_write(&mut duplicate, &cancel_ack.canonical_bytes()).await;
		assert_eq!(client_read(&mut duplicate).await, ack_confirmation(&cancel_ack));
		server.abort();
	}

	#[tokio::test]
	async fn unix_get_range_and_status_share_the_startup_validated_streaming_store() {
		let fixture = put_ipc_fixture();
		let socket = fixture._temp.path().join("private-query-host-test.sock");
		let listener = UnixListener::bind(&socket).unwrap();
		let uid = std::fs::symlink_metadata(&socket).unwrap().uid();
		let server =
			tokio::spawn(serve_private_host_ipc_with(listener, uid, Arc::clone(&fixture.service)));
		let mut status_recovery = None;

		for (method, range, nonce, expected_kinds) in [
			(1011, None, 31, vec![0, 1, 2]),
			(1012, Some((2, 5)), 32, vec![0, 1, 2]),
			(1014, None, 33, vec![0, 2]),
		] {
			let (request, capability, request_id, operation_id) =
				query_material(&fixture, method, range, nonce);
			let mut client = UnixStream::connect(&socket).await.unwrap();
			client_write(&mut client, &request).await;
			client_write(&mut client, &capability).await;
			let mut frames = Vec::new();
			for expected_kind in expected_kinds {
				let frame = client_read(&mut client).await;
				let value: Value = ciborium::from_reader(frame.as_slice()).unwrap();
				assert_eq!(uint_field(&value, 3), Some(expected_kind));
				frames.push(frame);
			}
			let ack = ResponseAckV1 {
				request_id,
				operation_id,
				generation: 0,
				response_hash: terminal_query_hash(&frames),
			};
			client_write(&mut client, &ack.canonical_bytes()).await;
			assert_eq!(client_read(&mut client).await, ack_confirmation(&ack));
			if method == 1014 {
				status_recovery = Some((ack.canonical_bytes(), ack_confirmation(&ack)));
			}
			drop(client);
		}

		let (ack, confirmation) = status_recovery.expect("STATUS acknowledgement captured");
		let mut ack_only = UnixStream::connect(&socket).await.unwrap();
		client_write(&mut ack_only, &ack).await;
		assert_eq!(client_read(&mut ack_only).await, confirmation);
		server.abort();
	}

	#[tokio::test]
	async fn unix_listener_binds_kernel_peer_and_round_trips_exact_big_endian_frames() {
		let root = tempfile::tempdir().unwrap();
		let path = root.path().join("origin-host-v2.sock");
		let listener = UnixListener::bind(&path).unwrap();
		let allowed_user_id = std::fs::symlink_metadata(&path).unwrap().uid();
		let client = tokio::spawn(async move {
			let mut stream = UnixStream::connect(path).await.unwrap();
			stream.write_all(&5u32.to_be_bytes()[..2]).await.unwrap();
			stream.write_all(&5u32.to_be_bytes()[2..]).await.unwrap();
			stream.write_all(b"exact").await.unwrap();
			let mut header = [0; 4];
			stream.read_exact(&mut header).await.unwrap();
			assert_eq!(u32::from_be_bytes(header), 5);
			let mut response = [0; 5];
			stream.read_exact(&mut response).await.unwrap();
			response
		});
		let mut connection =
			PrivateHostConnection::accept(&listener, allowed_user_id).await.unwrap();
		assert_eq!(connection.peer_user_id, allowed_user_id);
		assert_eq!(connection.read_frame().await.unwrap(), b"exact");
		connection.write_frame(b"bound").await.unwrap();
		assert_eq!(client.await.unwrap(), *b"bound");
	}

	#[tokio::test]
	async fn unix_listener_rejects_disallowed_uid_before_reading_frames() {
		let root = tempfile::tempdir().unwrap();
		let path = root.path().join("origin-host-v2.sock");
		let listener = UnixListener::bind(&path).unwrap();
		let actual_user_id = std::fs::symlink_metadata(&path).unwrap().uid();
		let _client = UnixStream::connect(path).await.unwrap();
		assert!(matches!(
			PrivateHostConnection::accept(&listener, actual_user_id.wrapping_add(1)).await,
			Err(PrivateHostIpcError::PeerBinding)
		));
	}

	#[tokio::test]
	async fn unix_listener_rejects_oversized_frame_before_payload_allocation() {
		let root = tempfile::tempdir().unwrap();
		let path = root.path().join("origin-host-v2.sock");
		let listener = UnixListener::bind(&path).unwrap();
		let allowed_user_id = std::fs::symlink_metadata(&path).unwrap().uid();
		tokio::spawn(async move {
			let mut stream = UnixStream::connect(path).await.unwrap();
			stream
				.write_all(&((MAX_HOST_FRAME_BYTES as u32) + 1).to_be_bytes())
				.await
				.unwrap();
		});
		let mut connection =
			PrivateHostConnection::accept(&listener, allowed_user_id).await.unwrap();
		assert!(matches!(connection.read_frame().await, Err(PrivateHostIpcError::FrameTooLarge)));
	}

	#[tokio::test]
	async fn unix_listener_times_out_an_idle_frame() {
		let root = tempfile::tempdir().unwrap();
		let path = root.path().join("origin-host-v2.sock");
		let listener = UnixListener::bind(&path).unwrap();
		let allowed_user_id = std::fs::symlink_metadata(&path).unwrap().uid();
		let client = UnixStream::connect(path).await.unwrap();
		let mut connection =
			PrivateHostConnection::accept(&listener, allowed_user_id).await.unwrap();
		assert!(matches!(
			connection.read_frame_with_timeout(Duration::from_millis(10)).await,
			Err(PrivateHostIpcError::Timeout)
		));
		drop(client);
	}

	#[tokio::test]
	async fn query_topology_is_loaded_at_the_capability_snapshot_not_a_moving_head() {
		let pinned = pinned_topology();
		let authority =
			MovingHeadAuthority { pinned: pinned.clone(), latest_called: AtomicBool::new(false) };
		assert_eq!(
			pinned_replication_topology(
				&authority,
				pinned.bucket_id,
				&format!("0x{}", hex::encode(pinned.finalized_hash)),
				pinned.finalized_number,
			)
			.await
			.unwrap(),
			pinned
		);
		assert!(!authority.latest_called.load(Ordering::SeqCst));
	}

	#[test]
	fn provider_confirmation_is_exactly_bound_to_the_ack_identity_and_hash() {
		let ack = ResponseAckV1 {
			request_id: [1; 16],
			operation_id: [2; 16],
			generation: 3,
			response_hash: [4; 32],
		};
		assert_eq!(
			ack_confirmation(&ack),
			canonical_map(vec![
				(0, Value::Integer(1.into())),
				(1, Value::Bytes(vec![1; 16])),
				(2, Value::Bytes(vec![2; 16])),
				(3, Value::Integer(3.into())),
				(4, Value::Bytes(vec![4; 32])),
				(5, Value::Bool(true)),
			])
		);
	}
}
