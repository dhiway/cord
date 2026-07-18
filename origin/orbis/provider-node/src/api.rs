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

use bytes::Bytes;
use http_body_util::Full;
use hyper::{
	body::{Body as HttpBody, Incoming},
	header,
	server::conn::http1,
	service::service_fn,
	Method, Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use sp_core::{ed25519, Pair as _};
use tokio::net::TcpListener;

use crate::{
	chain::ReplicationAuthority,
	checkpoint_quorum_worker::CheckpointQuorumScheduler,
	checkpoint_stack::CheckpointStack,
	peer_http::serve_peer_http,
	peer_responder::PeerResponder,
	ChainAuthority, ContentError, DiskStore, FinalizedRuntimeAuthority,
	JsonlManifestDeletionOutbox, ManifestDeletionSubmitter, NodeProfile, StoreError, PROTOCOL_VERSION,
};

type Body = Full<Bytes>;

fn now_ms() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
}


/// Failure to validate or apply the co-located provider stores during startup.
#[derive(Debug, thiserror::Error)]
pub enum ProviderOpenError {
	/// The public disk store rejected its persisted state or recovery plan.
	#[error(transparent)]
	Store(#[from] StoreError),
	/// A private checkpoint kernel rejected its persisted state or recovery plan.
	#[error(transparent)]
	Content(#[from] ContentError),
	/// The configured manifest-deletion submitter rejected its startup recovery view.
	#[error("manifest-deletion submitter startup failed: {0}")]
	Outbox(String),
}

/// HTTP listener limits and authentication policy.
#[derive(Clone, Debug)]
pub struct ApiConfig {
	/// Listener address. Production deployments should terminate TLS in an authenticated proxy.
	pub listen: SocketAddr,
}

/// Shared provider service state.
pub struct ProviderService<A: ChainAuthority> {
	store: Arc<DiskStore>,
	checkpoint_stack: Arc<CheckpointStack>,
	checkpoint_quorum_scheduler: Arc<CheckpointQuorumScheduler>,
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
		authority: Arc<A>,
		service_key: ed25519::Pair,
	) -> Result<Self, ProviderOpenError> {
		let root = root.as_ref();
		let outbox = Arc::new(JsonlManifestDeletionOutbox::for_provider_root(root));
		Self::open_with_submitter(root, profile, authority, service_key, outbox)
	}

	fn open_with_submitter(
		root: &Path,
		profile: NodeProfile,
		authority: Arc<A>,
		service_key: ed25519::Pair,
		outbox: Arc<dyn ManifestDeletionSubmitter>,
	) -> Result<Self, ProviderOpenError> {
		DiskStore::validate_root(root)?;
		let store = DiskStore::prepare_open(root, profile)?;
		let checkpoint_stack = CheckpointStack::prepare_open(root)?;
		let checkpoint_quorum_scheduler = CheckpointQuorumScheduler::prepare_open(root)?;
		let outbox_startup = outbox
			.prepare_startup(root, store.prepared_root_directory().map(|root| root.file()))
			.map_err(ProviderOpenError::Outbox)?;
		let store = store.arm()?;
		let outbox_startup = outbox_startup
			.arm(store.root_directory()?)
			.map_err(ProviderOpenError::Outbox)?;
		let store = match store.apply() {
			Ok(store) => Arc::new(store),
			Err(error) => {
				outbox_startup.rollback().map_err(ProviderOpenError::Outbox)?;
				return Err(error.into())
			},
		};
		let checkpoint_stack = match checkpoint_stack.apply() {
			Ok(stack) => Arc::new(stack),
			Err(error) => {
				outbox_startup.rollback().map_err(ProviderOpenError::Outbox)?;
				return Err(error.into())
			},
		};
		let checkpoint_quorum_scheduler = match checkpoint_quorum_scheduler.apply() {
			Ok(scheduler) => Arc::new(scheduler),
			Err(error) => {
				outbox_startup.rollback().map_err(ProviderOpenError::Outbox)?;
				return Err(error.into())
			},
		};
		outbox_startup.apply().map_err(ProviderOpenError::Outbox)?;
		Ok(Self {
			store,
			checkpoint_stack,
			checkpoint_quorum_scheduler,
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
		let checkpoint_stack = CheckpointStack::prepare_open(store.root())?;
		let checkpoint_quorum_scheduler = CheckpointQuorumScheduler::prepare_open(store.root())?;
		let root_directory = store.root_directory().map_err(|error| ContentError::Io(error.to_string()))?;
		let outbox_startup = outbox
			.prepare_startup(store.root(), Some(root_directory.file()))
			.map_err(ContentError::Io)?;
		let outbox_startup = outbox_startup.arm(root_directory).map_err(ContentError::Io)?;
		let checkpoint_stack = match checkpoint_stack.apply() {
			Ok(stack) => Arc::new(stack),
			Err(error) => {
				outbox_startup.rollback().map_err(ContentError::Io)?;
				return Err(error)
			},
		};
		let checkpoint_quorum_scheduler = match checkpoint_quorum_scheduler.apply() {
			Ok(scheduler) => Arc::new(scheduler),
			Err(error) => {
				outbox_startup.rollback().map_err(ContentError::Io)?;
				return Err(error)
			},
		};
		outbox_startup.apply().map_err(ContentError::Io)?;
		Ok(Self {
			store,
			checkpoint_stack,
			checkpoint_quorum_scheduler,
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

	pub(crate) fn private_host_service_key(&self) -> ed25519::Pair {
		self.service_key.clone()
	}

	/// Access the startup-validated quorum scheduler for worker orchestration.
	pub(crate) fn checkpoint_quorum_scheduler(&self) -> &Arc<CheckpointQuorumScheduler> {
		&self.checkpoint_quorum_scheduler
	}

	pub(crate) fn sign_manifest_deletion_digest(&self, digest: [u8; 32]) -> ([u8; 32], [u8; 64]) {
		(self.service_key.public().0, self.service_key.sign(&digest).0)
	}

	/// Audit the private byte plane and return only redacted readiness counts.
	fn integrity_summary(&self) -> Result<crate::IntegritySummary, ContentError> {
		self.checkpoint_stack.integrity_summary()
	}

	/// Return one redacted typed outcome derived from durable byte-plane and Commons duty state.
	pub fn recovery_status(&self) -> crate::ProviderRecoveryStatus {
		let integrity = self.integrity_summary();
		let inventory = self.store.checkpoint_duty_inventory();
		let local_state_available = integrity.is_ok() && inventory.is_ok();
		crate::recovery_status::status(
			integrity.ok(),
			inventory.ok().flatten(),
			local_state_available,
		)
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
		Arc::clone(service.checkpoint_quorum_scheduler()),
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
			if connection.await.is_err() {
				crate::observability::emit_failure(
					crate::observability::ProviderFailureCode::ProviderHttpConnectionFailed,
				);
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
	_config: &ApiConfig,
) -> Result<Response<Body>, ApiError>
where
	A: ChainAuthority,
	B: HttpBody<Data = Bytes>,
{
	match (request.method(), request.uri().path()) {
		(&Method::GET, "/health") => provider_health(&service),
		(&Method::GET, "/info") => json(
			StatusCode::OK,
			&serde_json::json!({"version": PROTOCOL_VERSION, "profile": service.store.profile()?, "service_key": hex::encode(service.service_key.public().0), "started_unix_ms": service.started_unix_ms}),
		),
		_ => Err(ApiError::not_found()),
	}
}

#[derive(Serialize)]
struct ProviderHealth {
	version: u16,
	status: &'static str,
	ready: bool,
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


#[derive(Debug)]
struct ApiError {
	status: StatusCode,
	message: String,
}
impl ApiError {
	fn not_found() -> Self {
		Self { status: StatusCode::NOT_FOUND, message: "route not found".into() }
	}
}
impl From<StoreError> for ApiError {
	fn from(error: StoreError) -> Self {
		let status = match error {
			StoreError::Invalid(_) => StatusCode::BAD_REQUEST,
			StoreError::Capacity => StatusCode::INSUFFICIENT_STORAGE,
			StoreError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
		};
		Self { status, message: error.to_string() }
	}
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
