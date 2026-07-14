// This file is part of CORD - https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

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
use sp_core::{sr25519, Pair as _};
use tokio::net::TcpListener;

use crate::{
	workers::flush_pending_submissions, ChainAuthority, CheckpointSubmitter, CommitInput,
	DiskStore, NodeProfile, RootObservation, SignedCheckpoint, StoreError, PROTOCOL_VERSION,
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
	authority: Arc<A>,
	service_key: sr25519::Pair,
	outbox: Arc<dyn CheckpointSubmitter>,
	root_outbox_lock: tokio::sync::Mutex<()>,
	started_unix_ms: u64,
}

impl<A: ChainAuthority> ProviderService<A> {
	/// Construct the service. The signing key must match the service key registered on Orbis.
	pub fn new(
		store: Arc<DiskStore>,
		authority: Arc<A>,
		service_key: sr25519::Pair,
		outbox: Arc<dyn CheckpointSubmitter>,
	) -> Self {
		Self {
			store,
			authority,
			service_key,
			outbox,
			root_outbox_lock: tokio::sync::Mutex::new(()),
			started_unix_ms: now_ms(),
		}
	}

	/// Access the local store for worker orchestration.
	pub fn store(&self) -> &Arc<DiskStore> {
		&self.store
	}

	/// Access the finalized chain authority for challenge coordination.
	pub fn authority(&self) -> &Arc<A> {
		&self.authority
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
