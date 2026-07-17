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
	path::Path,
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
	body::{Body as HttpBody, Incoming},
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
	peer_responder::PeerResponder, ChainAuthority, ContentError, DiskStore,
	FinalizedRuntimeAuthority, ManifestDeletionSubmitter, NodeProfile, StoreError,
	PROTOCOL_VERSION,
};

type Body = Full<Bytes>;

/// Failure to validate or apply the co-located provider stores during startup.
#[derive(Debug, thiserror::Error)]
pub enum ProviderOpenError {
	/// The public disk store rejected its persisted state or recovery plan.
	#[error(transparent)]
	Store(#[from] StoreError),
	/// A private checkpoint kernel rejected its persisted state or recovery plan.
	#[error(transparent)]
	Content(#[from] ContentError),
}

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
	outbox: Arc<dyn ManifestDeletionSubmitter>,
	started_unix_ms: u64,
}

impl<A: ChainAuthority> ProviderService<A> {
	/// Validate the disk store and every private kernel before applying any startup recovery.
	pub fn open(
		root: impl AsRef<Path>,
		profile: NodeProfile,
		capacity_bytes: u64,
		authority: Arc<A>,
		service_key: ed25519::Pair,
		outbox: Arc<dyn ManifestDeletionSubmitter>,
	) -> Result<Self, ProviderOpenError> {
		let root = root.as_ref();
		let store = DiskStore::prepare_open(root, profile, capacity_bytes)?;
		let checkpoint_stack = CheckpointStack::prepare_open(root)?;
		let store = Arc::new(store.apply()?);
		let checkpoint_stack = Arc::new(checkpoint_stack.apply()?);
		Ok(Self {
			store,
			checkpoint_stack,
			authority,
			service_key,
			outbox,
			started_unix_ms: now_ms(),
		})
	}

	/// Construct around a store already opened by tests or the evidence harness.
	#[cfg(any(test, feature = "evidence"))]
	pub(crate) fn new_preopened(
		store: Arc<DiskStore>,
		authority: Arc<A>,
		service_key: ed25519::Pair,
		outbox: Arc<dyn ManifestDeletionSubmitter>,
	) -> Result<Self, ContentError> {
		let checkpoint_stack = Arc::new(CheckpointStack::open(store.root())?);
		Ok(Self {
			store,
			checkpoint_stack,
			authority,
			service_key,
			outbox,
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

	pub(crate) fn sign_manifest_deletion_digest(&self, digest: [u8; 32]) -> ([u8; 32], [u8; 64]) {
		(self.service_key.public().0, self.service_key.sign(&digest).0)
	}

	/// Audit the private byte plane and return only redacted readiness counts.
	fn integrity_summary(&self) -> Result<crate::IntegritySummary, ContentError> {
		self.checkpoint_stack.integrity_summary()
	}

	pub(crate) fn outbox(&self) -> &Arc<dyn ManifestDeletionSubmitter> {
		&self.outbox
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
) -> Result<(), ContentError> {
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

/// Run one long-lived account-serialized finality lane and exact checkpoint publication lifecycle.
#[cfg(feature = "checkpoint-live")]
pub async fn run_checkpoint_live_worker(
	service: Arc<ProviderService<FinalizedRuntimeAuthority>>,
	local_provider: [u8; 32],
	orbis_native_rpc: String,
	account_suri: String,
	cadence: std::time::Duration,
) -> Result<(), ContentError> {
	use oc::{product_sdk::OrbisNativeClient, types::OriginAccount, OriginSigner};

	let account = OriginAccount::from_uri(&account_suri, None)
		.map_err(|_| ContentError::Io("invalid provider account secret URI".into()))?;
	drop(account_suri);
	let signer = OriginSigner::from_account(&account)
		.map_err(|_| ContentError::Io("provider account signer initialization failed".into()))?;
	drop(account);
	let signer_account: [u8; 32] = signer.account_id().into();
	if signer_account != local_provider {
		return Err(ContentError::IntegrityFailed);
	}
	let client = OrbisNativeClient::connect(&orbis_native_rpc)
		.await
		.map_err(|error| ContentError::Io(error.to_string()))?;
	let lane = crate::checkpoint::checkpoint_live::OriginRsCheckpointFinalityLane::new(
		client,
		signer,
		service.service_key.clone(),
	)?;
	crate::checkpoint_live_worker::run(
		Arc::clone(service.authority()),
		Arc::clone(service.checkpoint_stack()),
		Arc::clone(service.store()),
		lane,
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
	Ok(dispatch(request, service, &config).await)
}

async fn dispatch<A, B>(
	request: Request<B>,
	service: Arc<ProviderService<A>>,
	config: &ApiConfig,
) -> Response<Body>
where
	A: ChainAuthority,
	B: HttpBody<Data = Bytes>,
{
	match handle(request, service, config).await {
		Ok(response) => response,
		Err(error) => error_response(error),
	}
}

async fn handle<A, B>(
	request: Request<B>,
	service: Arc<ProviderService<A>>,
	config: &ApiConfig,
) -> Result<Response<Body>, ApiError>
where
	A: ChainAuthority,
	B: HttpBody<Data = Bytes>,
{
	let method = request.method().clone();
	let path = request.uri().path().to_owned();
	let query = parse_query(request.uri().query());
	let requires_auth = method != Method::GET ||
		matches!(
				path.as_str(),
			"/read" |
				"/commitment" |
				"/buckets" | "/mmr_proof" |
				"/chunk_proof" |
				"/mmr_peaks" | "/mmr_subtree" |
				"/replica/sync_status" |
				"/stats"
			);
	if requires_auth && !authorized(&request, config.bearer_token_hash) {
		return Err(ApiError::unauthorized());
	}
	match (method, path.as_str()) {
		(Method::GET, "/health") => provider_health(&service),
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
			let _ = request;
			Err(ApiError::service_unavailable(
				"provider commit is reserved until the P4 atomic object-completion cutover",
			))
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
			let _ = request;
			Err(ApiError::service_unavailable(
				"provider delete is reserved until the P4 atomic object-completion cutover",
			))
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
		(Method::GET, "/replica/sync_status") => replica_sync_status(&service),
		_ => Err(ApiError::not_found()),
	}
}

#[derive(Serialize)]
struct ProviderHealth {
	version: u16,
	status: &'static str,
	ready: bool,
}

#[derive(Serialize)]
struct ReplicaSyncStatus {
	version: u16,
	status: &'static str,
	ready: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	installed_objects: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	ready_objects: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	quarantined_objects: Option<u64>,
}

fn provider_health<A: ChainAuthority>(
	service: &ProviderService<A>,
) -> Result<Response<Body>, ApiError> {
	let ready = service.integrity_summary().is_ok_and(|summary| summary.ready);
	let status = if ready { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
	json(
		status,
		&ProviderHealth {
			version: PROTOCOL_VERSION,
			status: if ready { "ready" } else { "degraded" },
			ready,
		},
	)
}

fn replica_sync_status<A: ChainAuthority>(
	service: &ProviderService<A>,
) -> Result<Response<Body>, ApiError> {
	let summary = service.integrity_summary();
	let ready = summary.as_ref().is_ok_and(|summary| summary.ready);
	let response = match summary {
		Ok(summary) => ReplicaSyncStatus {
			version: PROTOCOL_VERSION,
			status: if ready { "ready" } else { "degraded" },
			ready,
			installed_objects: Some(summary.installed_objects),
			ready_objects: Some(summary.ready_objects),
			quarantined_objects: Some(summary.quarantined_objects),
		},
		Err(_) => ReplicaSyncStatus {
			version: PROTOCOL_VERSION,
			status: "degraded",
			ready: false,
			installed_objects: None,
			ready_objects: None,
			quarantined_objects: None,
		},
	};
	let status = if ready { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
	json(status, &response)
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
	fn bad_request(message: impl Into<String>) -> Self {
		Self { status: StatusCode::BAD_REQUEST, message: message.into() }
	}
	fn service_unavailable(message: impl Into<String>) -> Self {
		Self { status: StatusCode::SERVICE_UNAVAILABLE, message: message.into() }
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

async fn read_json<T, B>(request: Request<B>, limit: usize) -> Result<T, ApiError>
where
	T: for<'de> Deserialize<'de>,
	B: HttpBody<Data = Bytes>,
{
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

fn authorized<B>(request: &Request<B>, expected: [u8; 32]) -> bool {
	let token = request
		.headers()
		.get(header::AUTHORIZATION)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.strip_prefix("Bearer "));
	token
		.is_some_and(|token| constant_time_eq(blake3::hash(token.as_bytes()).as_bytes(), &expected))
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
	left.len() == right.len() &&
		left.iter().zip(right).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
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
fn now_ms() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |value| value.as_millis() as u64)
}

#[cfg(test)]
mod lifecycle_tests {
	use std::fs;

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
		CheckpointDutyBatch, CheckpointDutyPageRequest, JsonlManifestDeletionOutbox, OperationId,
	};

	struct Authority(ReplicationTopologySnapshot);
	struct RouteAuthority;

	#[test]
	fn production_open_preserves_all_prepared_cleanup_when_a_late_kernel_rejects() {
		let temp = tempfile::tempdir().unwrap();
		let profile = || NodeProfile {
			provider: hex::encode([0x21; 32]),
			endpoint: "http://127.0.0.1:8080".into(),
			service_key: hex::encode(ed25519::Pair::from_seed(&[0x31; 32]).public().0),
			region: None,
		};
		let store = Arc::new(DiskStore::open(temp.path(), profile(), 1024).unwrap());
		drop(
			ProviderService::new_preopened(
				store,
				Arc::new(RouteAuthority),
				ed25519::Pair::from_seed(&[0x31; 32]),
				Arc::new(JsonlManifestDeletionOutbox::new(temp.path().join("outbox.jsonl"))),
			)
			.unwrap(),
		);

		let disk_temp = temp.path().join("provider-index-v6.tmp-777");
		let disk_temp_bytes = b"stale-disk-index-evidence";
		fs::write(&disk_temp, disk_temp_bytes).unwrap();
		let streaming = temp.path().join("streaming-v1");
		let journal_temp = streaming.join("journal.json.tmp-777");
		let journal_temp_bytes = b"stale-streaming-journal-evidence";
		fs::write(&journal_temp, journal_temp_bytes).unwrap();
		let staging_orphan = streaming.join("staging").join("unowned-part");
		let staging_bytes = b"unowned-staging-evidence";
		fs::write(&staging_orphan, staging_bytes).unwrap();
		fs::write(temp.path().join("checkpoint-proposals-v2").join("invalid.json"), b"not-json")
			.unwrap();

		assert!(matches!(
			ProviderService::open(
				temp.path(),
				profile(),
				1024,
				Arc::new(RouteAuthority),
				ed25519::Pair::from_seed(&[0x31; 32]),
				Arc::new(JsonlManifestDeletionOutbox::new(temp.path().join("outbox.jsonl"))),
			),
			Err(ProviderOpenError::Content(ContentError::IntegrityFailed))
		));
		assert_eq!(fs::read(disk_temp).unwrap(), disk_temp_bytes);
		assert_eq!(fs::read(journal_temp).unwrap(), journal_temp_bytes);
		assert_eq!(fs::read(staging_orphan).unwrap(), staging_bytes);
	}

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
	impl ChainAuthority for RouteAuthority {
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
			ProviderService::new_preopened(
				store,
				Arc::new(Authority(topology.clone())),
				key,
				Arc::new(JsonlManifestDeletionOutbox::new(temp.path().join("outbox.jsonl"))),
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

	fn route_fixture(
		corrupt: bool,
	) -> (tempfile::TempDir, Arc<ProviderService<RouteAuthority>>, ApiConfig, String) {
		let temp = tempfile::tempdir().unwrap();
		let key = ed25519::Pair::from_seed(&[0x31; 32]);
		let bytes = b"private provider health fixture".to_vec();
		let cid = CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(&bytes));
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let receipt = streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: OperationId::from_bytes([0x41; 16]),
					bucket_id: BucketId::from_bytes([0x51; 32]),
					expected_cid: cid.to_string(),
					object_len: bytes.len() as u64,
				},
				[bytes],
			)
			.unwrap();
		drop(streaming);
		let store = Arc::new(
			DiskStore::open(
				temp.path(),
				NodeProfile {
					provider: hex::encode([0x21; 32]),
					endpoint: "http://127.0.0.1:8080".into(),
					service_key: hex::encode(key.public().0),
					region: None,
				},
				1024,
			)
			.unwrap(),
		);
		let service = Arc::new(
			ProviderService::new_preopened(
				store,
				Arc::new(RouteAuthority),
				key,
				Arc::new(JsonlManifestDeletionOutbox::new(temp.path().join("outbox.jsonl"))),
			)
			.unwrap(),
		);
		if corrupt {
			fs::write(
				temp.path().join("streaming-v1").join("objects").join(&receipt.cid),
				b"corrupt",
			)
			.unwrap();
		}
		let token = "route-health-test-token";
		let config = ApiConfig {
			listen: "127.0.0.1:0".parse().unwrap(),
			bearer_token_hash: *blake3::hash(token.as_bytes()).as_bytes(),
			max_content_bytes: 1024,
			max_json_bytes: 1024,
		};
		(temp, service, config, receipt.cid)
	}

	fn get(path: &str, token: Option<&str>) -> Request<Body> {
		let mut builder = Request::builder().method(Method::GET).uri(path);
		if let Some(token) = token {
			builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
		}
		builder.body(Full::new(Bytes::new())).unwrap()
	}

	fn post(path: &str, token: &str, body: &'static [u8]) -> Request<Body> {
		Request::builder()
			.method(Method::POST)
			.uri(path)
			.header(header::AUTHORIZATION, format!("Bearer {token}"))
			.body(Full::new(Bytes::from_static(body)))
			.unwrap()
	}

	async fn response_json(response: Response<Body>) -> serde_json::Value {
		let body = response.into_body().collect().await.unwrap().to_bytes();
		serde_json::from_slice(&body).unwrap()
	}

	fn assert_integrity_response_is_redacted(value: &serde_json::Value, cid: &str) {
		let private = [
			cid.to_string(),
			hex::encode([0x51; 32]),
			hex::encode([0x41; 16]),
			"proof".into(),
			"fingerprint".into(),
			"root".into(),
			"error".into(),
		];
		let encoded = serde_json::to_string(value).unwrap();
		for private in private {
			assert!(!encoded.contains(&private), "integrity response leaked {private}");
		}
	}

	#[tokio::test]
	async fn healthy_routes_require_auth_for_details_and_redact_integrity_evidence() {
		let (_temp, service, config, cid) = route_fixture(false);
		let health = dispatch(get("/health", None), Arc::clone(&service), &config).await;
		assert_eq!(health.status(), StatusCode::OK);
		let health = response_json(health).await;
		assert_eq!(
			health,
			serde_json::json!({"version": PROTOCOL_VERSION, "status": "ready", "ready": true})
		);
		assert_integrity_response_is_redacted(&health, &cid);

		let unauthenticated =
			dispatch(get("/replica/sync_status", None), Arc::clone(&service), &config).await;
		assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
		let sync = dispatch(
			get("/replica/sync_status", Some("route-health-test-token")),
			Arc::clone(&service),
			&config,
		)
		.await;
		assert_eq!(sync.status(), StatusCode::OK);
		let sync = response_json(sync).await;
		assert_eq!(
			sync,
			serde_json::json!({
				"version": PROTOCOL_VERSION,
				"status": "ready",
				"ready": true,
				"installed_objects": 1,
				"ready_objects": 1,
				"quarantined_objects": 0
			})
		);
		assert_integrity_response_is_redacted(&sync, &cid);

		let stats = dispatch(get("/stats", None), Arc::clone(&service), &config).await;
		assert_eq!(stats.status(), StatusCode::UNAUTHORIZED);
		let stats =
			dispatch(get("/stats", Some("route-health-test-token")), Arc::clone(&service), &config)
				.await;
		assert_eq!(stats.status(), StatusCode::OK);
		assert!(response_json(stats).await.get("root").is_some());
	}

	#[tokio::test]
	async fn reserved_object_mutations_fail_before_store_or_journal_changes() {
		let (temp, service, config, _cid) = route_fixture(false);
		let before = service.store.stats().unwrap();

		for (path, error) in [
			(
				"/commit",
				"provider commit is reserved until the P4 atomic object-completion cutover",
			),
			(
				"/delete",
				"provider delete is reserved until the P4 atomic object-completion cutover",
			),
		] {
			let response = dispatch(
				post(path, "route-health-test-token", b"not parsed"),
				Arc::clone(&service),
				&config,
			)
			.await;
			assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
			assert_eq!(
				response_json(response).await,
				serde_json::json!({"error": error, "version": PROTOCOL_VERSION})
			);
			assert_eq!(service.store.stats().unwrap(), before);
			assert!(service.store.pending_root_submissions().unwrap().is_empty());
			assert!(service.store.pending_deletions().unwrap().is_empty());
			assert!(!temp.path().join("outbox.jsonl").exists());
		}
	}

	#[tokio::test]
	async fn generic_checkpoint_http_routes_are_absent() {
		let (_temp, service, config, _cid) = route_fixture(false);
		for request in [
			get("/checkpoint-signature", None),
			get("/checkpoint/duty", None),
			get("/replica/historical_roots", None),
			post("/checkpoint/sign", "route-health-test-token", b"{}"),
		] {
			let response = dispatch(request, Arc::clone(&service), &config).await;
			assert_eq!(response.status(), StatusCode::NOT_FOUND);
		}
	}

	#[tokio::test]
	async fn quarantine_degrades_health_and_reports_only_redacted_authenticated_counts() {
		let (_temp, service, config, cid) = route_fixture(true);
		let health = dispatch(get("/health", None), Arc::clone(&service), &config).await;
		assert_eq!(health.status(), StatusCode::SERVICE_UNAVAILABLE);
		let health = response_json(health).await;
		assert_eq!(
			health,
			serde_json::json!({"version": PROTOCOL_VERSION, "status": "degraded", "ready": false})
		);
		assert_integrity_response_is_redacted(&health, &cid);

		let sync = dispatch(
			get("/replica/sync_status", Some("route-health-test-token")),
			service,
			&config,
		)
		.await;
		assert_eq!(sync.status(), StatusCode::SERVICE_UNAVAILABLE);
		let sync = response_json(sync).await;
		assert_eq!(
			sync,
			serde_json::json!({
				"version": PROTOCOL_VERSION,
				"status": "degraded",
				"ready": false,
				"installed_objects": 1,
				"ready_objects": 0,
				"quarantined_objects": 1
			})
		);
		assert_integrity_response_is_redacted(&sync, &cid);
	}
}
