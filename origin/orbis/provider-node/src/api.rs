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

//! Bounded authenticated provider HTTP API.

use std::{
	convert::Infallible,
	net::SocketAddr,
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
	body::{Body as _, Incoming},
	header,
	server::conn::http1,
	service::service_fn,
	Method, Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use sp_core::{ed25519, Pair as _};
use tokio::net::TcpListener;

use crate::{
	chain::ReplicationAuthority, checkpoint_stack::CheckpointStack, peer_http::serve_peer_http,
	peer_responder::PeerResponder, workers::flush_pending_submissions, ChainAuthority,
	CheckpointSubmitter, CommitInput, ContentError, DiskStore, FinalizedRuntimeAuthority,
	NodeProfile, RootObservation, SignedCheckpoint, StoreError, PROTOCOL_VERSION,
};

type Body = Full<Bytes>;

/// HTTP listener limits and authentication policy.
#[derive(Clone, Debug)]
pub struct ApiConfig {
	/// Listener address. Production deployments should terminate TLS in an authenticated proxy.
	pub listen: SocketAddr,
	/// Blake3 hash of the bearer token used for mutating and replica endpoints.
	pub bearer_token_hash: [u8; 32],
	/// Maximum content bytes accepted by `/commit`.
	pub max_content_bytes: usize,
	/// Maximum JSON control-plane body.
	pub max_json_bytes: usize,
}

/// Shared provider service state.
pub struct ProviderService<A: ChainAuthority> {
	store: Arc<DiskStore>,
	checkpoint_stack: Arc<CheckpointStack>,
	authority: Arc<A>,
	service_key: ed25519::Pair,
	outbox: Arc<dyn CheckpointSubmitter>,
	root_outbox_lock: tokio::sync::Mutex<()>,
	started_unix_ms: u64,
}

impl<A: ChainAuthority> ProviderService<A> {
	/// Construct the service. The signing key must match the service key registered on Orbis.
	pub fn new(
		store: Arc<DiskStore>,
		authority: Arc<A>,
		service_key: ed25519::Pair,
		outbox: Arc<dyn CheckpointSubmitter>,
	) -> Result<Self, ContentError> {
		let checkpoint_stack = Arc::new(CheckpointStack::open(store.root())?);
		Ok(Self {
			store,
			checkpoint_stack,
			authority,
			service_key,
			outbox,
			root_outbox_lock: tokio::sync::Mutex::new(()),
			started_unix_ms: now_ms(),
		})
	}

	/// Access the local store for worker orchestration.
	pub fn store(&self) -> &Arc<DiskStore> {
		&self.store
	}

	/// Access the finalized chain authority for challenge coordination.
	pub fn authority(&self) -> &Arc<A> {
		&self.authority
	}

	/// Access the private checkpoint kernels for in-crate orchestration.
	pub(crate) fn checkpoint_stack(&self) -> &Arc<CheckpointStack> {
		&self.checkpoint_stack
	}

	pub(crate) fn outbox(&self) -> &Arc<dyn CheckpointSubmitter> {
		&self.outbox
	}

	pub(crate) fn root_outbox_lock(&self) -> &tokio::sync::Mutex<()> {
		&self.root_outbox_lock
	}

	/// Produce and persist a signed current-root checkpoint.
	pub fn sign_checkpoint(&self) -> Result<SignedCheckpoint, StoreError> {
		let stats = self.store.stats()?;
		self.sign_root_checkpoint(RootObservation {
			root: stats.root,
			leaf_count: stats.proof_leaf_count,
		})
	}

	pub(crate) fn sign_root_checkpoint(
		&self,
		observation: RootObservation,
	) -> Result<SignedCheckpoint, StoreError> {
		let created_unix_ms = now_ms();
		let payload =
			checkpoint_payload(&observation.root, observation.leaf_count, created_unix_ms);
		let signature = self.service_key.sign(&payload);
		let checkpoint = SignedCheckpoint {
			root: observation.root,
			leaves: observation.leaf_count,
			created_unix_ms,
			signature: hex::encode(signature.0),
		};
		self.store.append_checkpoint(checkpoint.clone())?;
		Ok(checkpoint)
	}
}

/// Serve the public API and the separately-bound authenticated peer ingress as one lifecycle.
///
/// This is intentionally specialized to the finalized runtime authority so the private
/// replication authority and responder types never become public API.
pub async fn serve_provider_ingress(
	config: ApiConfig,
	peer_listener: TcpListener,
	service: Arc<ProviderService<FinalizedRuntimeAuthority>>,
	local_provider: [u8; 32],
) -> Result<(), std::io::Error> {
	let responder = peer_responder_for_service(&service, local_provider).map_err(|_| {
		std::io::Error::new(std::io::ErrorKind::InvalidInput, "provider peer identity is invalid")
	})?;
	tokio::select! {
		result = serve(config, service) => result,
		result = serve_peer_http(peer_listener, responder) => result,
	}
}

/// Run the separately bounded target replication lifecycle for the production authority.
pub async fn run_replication_worker(
	service: Arc<ProviderService<FinalizedRuntimeAuthority>>,
	local_provider: [u8; 32],
	cadence: std::time::Duration,
) {
	crate::replication_worker::run(
		Arc::clone(service.authority()),
		Arc::clone(service.checkpoint_stack()),
		Arc::clone(service.store()),
		local_provider,
		service.service_key.clone(),
		cadence,
	)
	.await
}

/// Run the separately bounded checkpoint confirmation quorum lifecycle.
pub async fn run_checkpoint_quorum_worker(
	service: Arc<ProviderService<FinalizedRuntimeAuthority>>,
	local_provider: [u8; 32],
	cadence: std::time::Duration,
) {
	crate::checkpoint_quorum_worker::run(
		Arc::clone(service.authority()),
		Arc::clone(service.checkpoint_stack()),
		Arc::clone(service.store()),
		local_provider,
		service.service_key.clone(),
		cadence,
	)
	.await
}

fn peer_responder_for_service<A>(
	service: &Arc<ProviderService<A>>,
	local_provider: [u8; 32],
) -> Result<Arc<PeerResponder<A>>, ContentError>
where
	A: ChainAuthority + ReplicationAuthority,
{
	Ok(Arc::new(PeerResponder::new_with_store(
		Arc::clone(service.authority()),
		Arc::clone(service.checkpoint_stack()),
		Arc::clone(service.store()),
		local_provider,
		service.service_key.clone(),
	)?))
}

/// Serve the provider protocol until the listener fails or the task is cancelled.
pub async fn serve<A: ChainAuthority>(
	config: ApiConfig,
	service: Arc<ProviderService<A>>,
) -> Result<(), std::io::Error> {
	let listener = TcpListener::bind(config.listen).await?;
	loop {
		let (stream, _) = listener.accept().await?;
		let service = service.clone();
		let config = config.clone();
		tokio::spawn(async move {
			let io = TokioIo::new(stream);
			let connection = http1::Builder::new().serve_connection(
				io,
				service_fn(move |request| route(request, service.clone(), config.clone())),
			);
			if let Err(error) = connection.await {
				eprintln!("provider HTTP connection failed: {error}");
			}
		});
	}
}

async fn route<A: ChainAuthority>(
	request: Request<Incoming>,
	service: Arc<ProviderService<A>>,
	config: ApiConfig,
) -> Result<Response<Body>, Infallible> {
	let response = match handle(request, service, &config).await {
		Ok(response) => response,
		Err(error) => error_response(error),
	};
	Ok(response)
}

async fn handle<A: ChainAuthority>(
	request: Request<Incoming>,
	service: Arc<ProviderService<A>>,
	config: &ApiConfig,
) -> Result<Response<Body>, ApiError> {
	let method = request.method().clone();
	let path = request.uri().path().to_owned();
	let query = parse_query(request.uri().query());
	let requires_auth =
		method != Method::GET
			|| matches!(
				path.as_str(),
				"/read"
					| "/commitment" | "/buckets"
					| "/checkpoint-signature"
					| "/checkpoint/duty"
					| "/mmr_proof" | "/chunk_proof"
					| "/mmr_peaks" | "/mmr_subtree"
					| "/replica/historical_roots"
					| "/replica/sync_status"
			);
	if requires_auth && !authorized(&request, config.bearer_token_hash) {
		return Err(ApiError::unauthorized());
	}
	match (method, path.as_str()) {
		(Method::GET, "/health") => {
			json(StatusCode::OK, &serde_json::json!({"version": PROTOCOL_VERSION, "status": "ok"}))
		},
		(Method::GET, "/info") => json(
			StatusCode::OK,
			&serde_json::json!({"version": PROTOCOL_VERSION, "profile": service.store.profile()?, "service_key": hex::encode(service.service_key.public().0), "started_unix_ms": service.started_unix_ms}),
		),
		(Method::GET, "/stats") => json(StatusCode::OK, &service.store.stats()?),
		(Method::GET, "/node") => json(StatusCode::OK, &service.store.profile()?),
		(Method::PUT, "/node") => {
			let body: NodeUpdate = read_json(request, config.max_json_bytes).await?;
			service.store.update_profile(body.profile, body.capacity_bytes)?;
			json(StatusCode::OK, &serde_json::json!({"updated": true}))
		},
		(Method::POST, "/exists") => {
			let body: ExistsRequest = read_json(request, config.max_json_bytes).await?;
			json(StatusCode::OK, &service.store.exists(&body.commitments)?)
		},
		(Method::POST, "/commit") => {
			let body: CommitRequest = read_json(
				request,
				config.max_content_bytes.saturating_mul(2).saturating_add(config.max_json_bytes),
			)
			.await?;
			let bytes = BASE64
				.decode(body.data.as_bytes())
				.map_err(|_| ApiError::bad_request("data must be canonical base64"))?;
			if bytes.len() > config.max_content_bytes {
				return Err(ApiError::payload_too_large());
			}
			let commitment = decode_hash(&body.commitment)?;
			if DiskStore::content_commitment(&bytes) != commitment {
				return Err(ApiError::bad_request("commitment does not match content"));
			}
			let agreement_id = decode_hash(&body.agreement_id)?;
			let authorization = service
				.authority
				.authorize_commit(agreement_id, commitment, bytes.len() as u64)
				.await
				.map_err(|error| ApiError::forbidden(error.to_string()))?;
			let _root_order = service.root_outbox_lock.lock().await;
			flush_pending_submissions(&service.store, service.outbox.as_ref())
				.await
				.map_err(ApiError::internal)?;
			let record = service.store.commit(CommitInput {
				commitment,
				authorization,
				bucket: body.bucket,
				key: body.key,
				bytes,
			})?;
			flush_pending_submissions(&service.store, service.outbox.as_ref())
				.await
				.map_err(ApiError::internal)?;
			json(StatusCode::CREATED, &record)
		},
		(Method::GET, "/read") => {
			let commitment = required_query(&query, "commitment")?;
			let bytes = service.store.read(commitment)?;
			json(
				StatusCode::OK,
				&serde_json::json!({"commitment": commitment, "data": BASE64.encode(bytes)}),
			)
		},
		(Method::GET, "/commitment") => {
			let commitment = required_query(&query, "commitment")?;
			json(StatusCode::OK, &service.store.record(commitment)?)
		},
		(Method::GET, "/checkpoint-signature") => {
			json(StatusCode::OK, &service.store.latest_checkpoint()?)
		},
		(Method::POST, "/checkpoint/sign") => json(StatusCode::OK, &service.sign_checkpoint()?),
		(Method::GET, "/mmr_proof") => {
			let commitment = required_query(&query, "commitment")?;
			json(
				StatusCode::OK,
				&serde_json::json!({"commitment": commitment, "proof": service.store.proof(commitment)?, "root": service.store.stats()?.root}),
			)
		},
		(Method::GET, "/chunk_proof") => {
			let commitment = required_query(&query, "commitment")?;
			let index = query_usize(&query, "index", 0)?;
			let proof = service.store.chunk_proof(commitment, index)?;
			json(
				StatusCode::OK,
				&serde_json::json!({"commitment": commitment, "index": proof.index, "chunks": proof.chunks, "chunk_hash": proof.chunk_hash, "root": proof.root, "proof": proof.proof, "data": BASE64.encode(proof.bytes)}),
			)
		},
		(Method::GET, "/buckets") => {
			let cursor = query_usize(&query, "cursor", 0)?;
			let limit = query_usize(&query, "limit", 100)?;
			let (items, next_cursor) = service.store.bucket_records(
				query.get("bucket").map(String::as_str),
				cursor,
				limit,
			)?;
			json(StatusCode::OK, &serde_json::json!({"items": items, "next_cursor": next_cursor}))
		},
		(Method::POST, "/delete") => {
			let body: DeleteRequest = read_json(request, config.max_json_bytes).await?;
			let commitment = decode_hash(&body.commitment)?;
			let agreement_id = decode_hash(&body.agreement_id)?;
			let existing = service.store.record(&body.commitment)?;
			if existing.agreement_id.trim_start_matches("0x") != hex::encode(agreement_id) {
				return Err(ApiError::forbidden("agreement does not own commitment"));
			}
			if existing.deleted {
				let _root_order = service.root_outbox_lock.lock().await;
				flush_pending_submissions(&service.store, service.outbox.as_ref())
					.await
					.map_err(ApiError::internal)?;
				return json(StatusCode::OK, &existing);
			}
			let authorization = service
				.authority
				.authorize_delete(agreement_id, commitment)
				.await
				.map_err(|error| ApiError::forbidden(error.to_string()))?;
			let _root_order = service.root_outbox_lock.lock().await;
			flush_pending_submissions(&service.store, service.outbox.as_ref())
				.await
				.map_err(ApiError::internal)?;
			let (deleted, _pending) =
				service.store.prepare_delete(&body.commitment, &authorization)?;
			// Persist a signed observation too, but the runtime proof binds the exact tombstone
			// root captured atomically in the pending-deletion journal.
			let _checkpoint = service.sign_checkpoint()?;
			flush_pending_submissions(&service.store, service.outbox.as_ref())
				.await
				.map_err(ApiError::internal)?;
			json(StatusCode::OK, &deleted)
		},
		(Method::GET, "/mmr_peaks") => json(
			StatusCode::OK,
			&serde_json::json!({"peaks": service.store.peaks()?, "root": service.store.stats()?.root}),
		),
		(Method::GET, "/mmr_subtree") | (Method::POST, "/fetch_nodes") => {
			let (start, limit) = if request.method() == Method::POST {
				let body: NodeRange = read_json(request, config.max_json_bytes).await?;
				(body.start, body.limit)
			} else {
				(query_usize(&query, "start", 0)?, query_usize(&query, "limit", 100)?)
			};
			json(
				StatusCode::OK,
				&serde_json::json!({"start": start, "nodes": service.store.leaf_nodes(start, limit)?}),
			)
		},
		(Method::GET, "/checkpoint/duty") => {
			let after = query
				.get("after")
				.map(|value| {
					value.parse::<u32>().map_err(|_| ApiError::bad_request("invalid after"))
				})
				.transpose()?;
			let batch = service
				.authority
				.challenge_duties(after)
				.await
				.map_err(|error| ApiError::forbidden(error.to_string()))?;
			json(StatusCode::OK, &batch)
		},
		(Method::GET, "/replica/historical_roots") => {
			let limit = query_usize(&query, "limit", 100)?;
			json(
				StatusCode::OK,
				&serde_json::json!({"checkpoints": service.store.checkpoints(limit)?}),
			)
		},
		(Method::GET, "/replica/sync_status") => json(
			StatusCode::OK,
			&serde_json::json!({"root": service.store.stats()?.root, "peaks": service.store.peaks()?, "status": "ready"}),
		),
		_ => Err(ApiError::not_found()),
	}
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeUpdate {
	profile: NodeProfile,
	capacity_bytes: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExistsRequest {
	commitments: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommitRequest {
	agreement_id: String,
	commitment: String,
	bucket: Option<String>,
	key: Option<String>,
	data: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteRequest {
	agreement_id: String,
	commitment: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeRange {
	start: usize,
	limit: usize,
}

#[derive(Debug)]
struct ApiError {
	status: StatusCode,
	message: String,
}
impl ApiError {
	fn internal(message: impl Into<String>) -> Self {
		Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: message.into() }
	}
	fn bad_request(message: impl Into<String>) -> Self {
		Self { status: StatusCode::BAD_REQUEST, message: message.into() }
	}
	fn forbidden(message: impl Into<String>) -> Self {
		Self { status: StatusCode::FORBIDDEN, message: message.into() }
	}
	fn unauthorized() -> Self {
		Self { status: StatusCode::UNAUTHORIZED, message: "valid bearer token required".into() }
	}
	fn not_found() -> Self {
		Self { status: StatusCode::NOT_FOUND, message: "route not found".into() }
	}
	fn payload_too_large() -> Self {
		Self {
			status: StatusCode::PAYLOAD_TOO_LARGE,
			message: "request exceeds configured bound".into(),
		}
	}
}
impl From<StoreError> for ApiError {
	fn from(error: StoreError) -> Self {
		let status = match error {
			StoreError::Invalid(_) => StatusCode::BAD_REQUEST,
			StoreError::NotFound => StatusCode::NOT_FOUND,
			StoreError::Capacity => StatusCode::INSUFFICIENT_STORAGE,
			StoreError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
		};
		Self { status, message: error.to_string() }
	}
}

async fn read_json<T: for<'de> Deserialize<'de>>(
	request: Request<Incoming>,
	limit: usize,
) -> Result<T, ApiError> {
	if request.body().size_hint().upper().is_some_and(|size| size > limit as u64) {
		return Err(ApiError::payload_too_large());
	}
	let bytes = request
		.into_body()
		.collect()
		.await
		.map_err(|_| ApiError::bad_request("request body failed"))?
		.to_bytes();
	if bytes.len() > limit {
		return Err(ApiError::payload_too_large());
	}
	serde_json::from_slice(&bytes)
		.map_err(|error| ApiError::bad_request(format!("invalid JSON: {error}")))
}

fn json(status: StatusCode, value: &impl Serialize) -> Result<Response<Body>, ApiError> {
	let body = serde_json::to_vec(value).map_err(|error| ApiError {
		status: StatusCode::INTERNAL_SERVER_ERROR,
		message: error.to_string(),
	})?;
	Ok(Response::builder()
		.status(status)
		.header(header::CONTENT_TYPE, "application/json")
		.header("x-orbis-provider-version", PROTOCOL_VERSION)
		.body(Full::new(Bytes::from(body)))
		.expect("static response is valid"))
}

fn error_response(error: ApiError) -> Response<Body> {
	json(error.status, &serde_json::json!({"error": error.message, "version": PROTOCOL_VERSION}))
		.unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
}

fn authorized(request: &Request<Incoming>, expected: [u8; 32]) -> bool {
	let token = request
		.headers()
		.get(header::AUTHORIZATION)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.strip_prefix("Bearer "));
	token
		.is_some_and(|token| constant_time_eq(blake3::hash(token.as_bytes()).as_bytes(), &expected))
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
	left.len() == right.len()
		&& left.iter().zip(right).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}

fn parse_query(query: Option<&str>) -> std::collections::BTreeMap<String, String> {
	query
		.unwrap_or_default()
		.split('&')
		.filter(|pair| !pair.is_empty())
		.filter_map(|pair| pair.split_once('=').or(Some((pair, ""))))
		.map(|(key, value)| (key.to_owned(), value.to_owned()))
		.collect()
}
fn required_query<'a>(
	query: &'a std::collections::BTreeMap<String, String>,
	key: &str,
) -> Result<&'a str, ApiError> {
	query
		.get(key)
		.filter(|value| !value.is_empty())
		.map(String::as_str)
		.ok_or_else(|| ApiError::bad_request(format!("missing query parameter {key}")))
}
fn query_usize(
	query: &std::collections::BTreeMap<String, String>,
	key: &str,
	default: usize,
) -> Result<usize, ApiError> {
	query
		.get(key)
		.map(|value| value.parse().map_err(|_| ApiError::bad_request(format!("invalid {key}"))))
		.unwrap_or(Ok(default))
}
fn decode_hash(value: &str) -> Result<[u8; 32], ApiError> {
	let raw = hex::decode(value.strip_prefix("0x").unwrap_or(value))
		.map_err(|_| ApiError::bad_request("hash must be hex"))?;
	raw.try_into()
		.map_err(|_| ApiError::bad_request("hash must be exactly 32 bytes"))
}
fn checkpoint_payload(root: &str, leaves: u64, created: u64) -> Vec<u8> {
	[
		b"orbis/provider-checkpoint/v1".as_slice(),
		root.as_bytes(),
		&leaves.to_le_bytes(),
		&created.to_le_bytes(),
	]
	.concat()
}
fn now_ms() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |value| value.as_millis() as u64)
}

#[cfg(test)]
mod lifecycle_tests {
	use async_trait::async_trait;
	use sp_core::Pair as _;

	use super::*;
	use crate::{
		chain::{ReplicationProviderSnapshot, ReplicationTopologySnapshot},
		peer::{
			PeerMmrCommitmentV1, PeerPageExpectationV1, PeerRequestIdentityV1,
			PeerSyncPageRequestV1,
		},
		replication_session::ReplicationSessionV1,
		storage::{bucket_mmr::BucketMmrStore, StreamingDescriptor, StreamingStore},
		AgreementAuthorization, BucketId, CanonicalCid, ChainError, ChallengeBatch,
		CheckpointDutyBatch, CheckpointDutyPageRequest, JsonlCheckpointOutbox, OperationId,
	};

	struct Authority(ReplicationTopologySnapshot);

	#[async_trait]
	impl ChainAuthority for Authority {
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
	impl ReplicationAuthority for Authority {
		async fn replication_topology(
			&self,
			_: [u8; 32],
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			Ok(self.0.clone())
		}

		async fn replication_topology_at(
			&self,
			_: [u8; 32],
			_: [u8; 32],
			_: u32,
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			Ok(self.0.clone())
		}
	}

	#[tokio::test]
	async fn peer_responder_clones_services_stack_and_replays_its_exact_reply() {
		let temp = tempfile::tempdir().unwrap();
		let key = ed25519::Pair::from_seed(&[8; 32]);
		let target = ed25519::Pair::from_seed(&[9; 32]);
		let bytes = b"shared checkpoint stack".to_vec();
		let cid = CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(&bytes));
		let streaming = StreamingStore::open(temp.path()).unwrap();
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: OperationId::from_bytes([3; 16]),
					bucket_id: BucketId::from_bytes([4; 32]),
					expected_cid: cid.to_string(),
					object_len: bytes.len() as u64,
				},
				[bytes],
			)
			.unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let candidate =
			mmr.commitment_candidate(&streaming, BucketId::from_bytes([4; 32]), 0).unwrap();
		let commitment = PeerMmrCommitmentV1::new(candidate.mmr_root.0, 0, 1, 0).unwrap();
		let provider = |id: u8, order: u8, service_key: [u8; 32]| {
			let endpoint = format!("https://provider-{id}.invalid").into_bytes();
			ReplicationProviderSnapshot {
				provider: [id; 32],
				order,
				primary: order == 0,
				record_present: true,
				endpoint_hash: Some(sp_crypto_hashing::blake2_256(&endpoint)),
				endpoint: Some(endpoint),
				active_service_key: Some(service_key),
				active_service_key_version: Some(u64::from(order) + 1),
				status_active: true,
				organization_valid: true,
				authority_validated_at: Some(8),
				overdue_challenges: 0,
				eligible: true,
				usable: true,
				exclusions: Vec::new(),
				confirmed_checkpoint: None,
			}
		};
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: [1; 32],
			finalized_hash: [2; 32],
			finalized_number: 10,
			governed_finalized_checkpoint: Some(8),
			bucket_id: [4; 32],
			bucket_version: 1,
			primary: [7; 32],
			replicas: vec![[9; 32]],
			providers: vec![provider(7, 0, key.public().0), provider(9, 1, target.public().0)],
			current_checkpoint: None,
			snapshot_hash: [0; 32],
		};
		let mut topology_bytes = b"cord/provider/replication-topology/v1".to_vec();
		codec::Encode::encode_to(&topology, &mut topology_bytes);
		topology.snapshot_hash = sp_crypto_hashing::blake2_256(&topology_bytes);
		let store = Arc::new(
			DiskStore::open(
				temp.path(),
				NodeProfile {
					provider: hex::encode([7; 32]),
					endpoint: "http://127.0.0.1:8080".into(),
					service_key: hex::encode(key.public().0),
					region: None,
				},
				1024,
			)
			.unwrap(),
		);
		let service = Arc::new(
			ProviderService::new(
				store,
				Arc::new(Authority(topology.clone())),
				key,
				Arc::new(JsonlCheckpointOutbox::new(temp.path().join("outbox.jsonl"))),
			)
			.unwrap(),
		);
		let responder = peer_responder_for_service(&service, [7; 32]).unwrap();
		assert!(Arc::ptr_eq(service.checkpoint_stack(), responder.checkpoint_stack()));
		let session = ReplicationSessionV1::from_topology(
			topology,
			[7; 32],
			key.public().0,
			[7; 32],
			[9; 32],
			commitment,
		)
		.unwrap();
		let expectation = PeerPageExpectationV1::new(
			session.context().clone(),
			PeerRequestIdentityV1::new([5; 16], [6; 16]).unwrap(),
			None,
			1,
		)
		.unwrap();
		let request = PeerSyncPageRequestV1::new_signed(&expectation, &target).unwrap();
		let request = codec::Encode::encode(&request);
		let first = responder.page(&request).await.unwrap();
		let replay = responder.page(&request).await.unwrap();
		assert_eq!(replay, first);
	}
}
