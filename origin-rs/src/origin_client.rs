//! Dynamic, runtime-upgrade-safe client for Origin/OriginHub chains with a view-only surface.

use crate::{
	client::nonce::{NonceManager, NonceStrategy},
	extrinsic::{BatchBuilder, ExtrinsicBuilder, MetaTxClient},
	error::Error,
	metadata, params,
};
use jsonrpsee_client_transport::ws::WsTransportClientBuilder;
use jsonrpsee_core::client::{async_client::PingConfig, Client as WsClient};
use log::warn;
use scale_info::PortableRegistry;
use scale_value::{scale as scale_value_scale, Value};
use sp_core::hashing::blake2_256;
use sp_runtime::traits::SaturatedConversion;
use std::{collections::HashMap, sync::Arc, time::Duration};
use subxt::{
	backend::rpc::RpcClient, config::DefaultExtrinsicParamsBuilder, dynamic, utils::AccountId32,
	Metadata, OnlineClient,
};
use tokio::{sync::{mpsc, RwLock, Semaphore}, time::sleep};
use url::Url;

fn spawn_nonce_resync_if_enabled(
	interval: Option<Duration>,
	nonces: Arc<NonceManager>,
	_metadata: Arc<Metadata>,
	_layout: Arc<RuntimeLayout>,
	client: OnlineClient<params::config::OriginConfig>,
) -> Option<tokio::task::JoinHandle<()>> {
	let Some(period) = interval else { return None };
	Some(tokio::spawn(async move {
		let mut ticker = tokio::time::interval(period);
		loop {
			ticker.tick().await;
			let accounts: Vec<AccountId32> = nonces.accounts().await;
			for account in accounts {
				let _ = nonces.refresh(&client, &account).await;
			}
		}
	}))
}

fn expect_composite(value: Value) -> Result<scale_value::Composite<()>, Error> {
	match value.value {
		scale_value::ValueDef::Composite(c) => Ok(c),
		other => Err(Error::Params(format!("expected composite for view args, got {other:?}"))),
	}
}

fn is_future_or_stale(err: &subxt::Error) -> bool {
	let msg = err.to_string().to_lowercase();
	msg.contains("future") || msg.contains("stale")
}

/// Lightweight dynamic event decoded from block subscriptions.
#[derive(Clone, Debug)]
pub struct DynamicEvent {
	pub block_hash: subxt::utils::H256,
	pub block_number: u32,
	pub pallet: String,
	pub variant: String,
	pub fields: scale_value::Composite<u32>,
}

/// JSON-friendly event representation (fields serialized via `scale_value` serde).
#[derive(Clone, Debug, serde::Serialize)]
pub struct JsonEvent {
	pub block_hash: subxt::utils::H256,
	pub block_number: u32,
	pub pallet: String,
	pub variant: String,
	pub fields: serde_json::Value,
}

pub struct EventStream {
	rx: mpsc::Receiver<DynamicEvent>,
}

impl EventStream {
	pub async fn next(&mut self) -> Option<DynamicEvent> {
		self.rx.recv().await
	}

	/// Map the stream to serde_json-friendly events.
	pub async fn next_json(&mut self) -> Option<JsonEvent> {
		while let Some(ev) = self.rx.recv().await {
			if let Ok(fields) = serde_json::to_value(&ev.fields) {
				return Some(JsonEvent {
					block_hash: ev.block_hash,
					block_number: ev.block_number,
					pallet: ev.pallet,
					variant: ev.variant,
					fields,
				});
			}
		}
		None
	}
}

/// Submission retry policy.
#[derive(Clone, Copy, Debug)]
pub struct SubmitRetryPolicy {
	pub attempts: usize,
	pub initial_backoff: Duration,
}

impl Default for SubmitRetryPolicy {
	fn default() -> Self {
		Self { attempts: 3, initial_backoff: Duration::from_millis(100) }
	}
}

/// Per-client tuning knobs.
#[derive(Clone, Debug)]
pub struct ClientConfig {
	pub nonce_strategy: NonceStrategy,
	pub max_in_flight_txs: usize,
	pub submit_retry: SubmitRetryPolicy,
	/// Optional periodic nonce resync interval for LocalCache strategy.
	pub nonce_resync_interval: Option<Duration>,
}

impl Default for ClientConfig {
	fn default() -> Self {
		Self {
			nonce_strategy: NonceStrategy::LocalCache,
			max_in_flight_txs: 1024,
			submit_retry: SubmitRetryPolicy::default(),
			nonce_resync_interval: None,
		}
	}
}

/// Low-level websocket tuning knobs.
#[derive(Clone, Debug)]
pub struct WsConfig {
	pub request_timeout: Duration,
	pub max_buffer_bytes: usize,
	pub ping_interval: Duration,
	pub max_concurrent_requests: usize,
	pub initial_backoff: Duration,
	pub max_backoff: Duration,
	pub max_retries: usize,
}

impl Default for WsConfig {
	fn default() -> Self {
		Self {
			request_timeout: Duration::from_secs(15),
			max_buffer_bytes: 16 * 1024 * 1024,
			ping_interval: Duration::from_secs(10),
			max_concurrent_requests: 10_000,
			initial_backoff: Duration::from_millis(50),
			max_backoff: Duration::from_secs(5),
			max_retries: 5,
		}
	}
}

/// Builder for OriginClient with tunable transport + SDK options.
#[derive(Clone, Debug)]
pub struct OriginClientBuilder {
	url: String,
	client: ClientConfig,
	ws: WsConfig,
}

impl OriginClientBuilder {
	pub fn new(url: impl Into<String>) -> Self {
		Self { url: url.into(), client: ClientConfig::default(), ws: WsConfig::default() }
	}

	pub fn with_client_config(mut self, config: ClientConfig) -> Self {
		self.client = config;
		self
	}

	pub fn with_ws_config(mut self, config: WsConfig) -> Self {
		self.ws = config;
		self
	}

	pub async fn build(self) -> Result<OriginClient, Error> {
		let rpc = self.build_rpc().await?;
		let (genesis_hash, runtime_version, metadata_snapshot, raw_meta) =
			metadata::load_or_fetch(rpc.as_ref()).await?;
		let metadata_hash = blake2_256(&raw_meta);

		let online = OnlineClient::<params::config::OriginConfig>::from_rpc_client_with(
			genesis_hash,
			runtime_version.clone(),
			metadata_snapshot.clone(),
			rpc.as_ref().clone(),
		)
		.map_err(Error::from)?;

		OriginClient::from_online_parts(
			online,
			metadata_snapshot,
			raw_meta,
			metadata_hash,
			self.client,
		)
		.await
	}

	async fn build_rpc(&self) -> Result<Arc<RpcClient>, Error> {
		let url = Url::parse(&self.url).map_err(|e| Error::Params(e.to_string()))?;
		let mut attempt = 0usize;
		let mut backoff = self.ws.initial_backoff;
		loop {
			match WsTransportClientBuilder::default().build(url.clone()).await {
				Ok((sender, receiver)) => {
					let client = WsClient::builder()
						.request_timeout(self.ws.request_timeout)
						.max_buffer_capacity_per_subscription(self.ws.max_buffer_bytes)
						.enable_ws_ping(PingConfig::new().ping_interval(self.ws.ping_interval))
						.set_tcp_no_delay(true)
						.max_concurrent_requests(self.ws.max_concurrent_requests)
						.build_with_tokio(sender, receiver);
					return Ok(Arc::new(RpcClient::new(client)));
				},
				Err(err) => {
					attempt = attempt.saturating_add(1);
					if attempt >= self.ws.max_retries {
						return Err(Error::Transport(format!("ws connect failed: {err}")));
					}
					let wait = backoff.min(self.ws.max_backoff);
					warn!("ws connect failed (attempt #{attempt}): {err}; retrying in {wait:?}");
					sleep(wait).await;
					backoff = (backoff * 2).min(self.ws.max_backoff);
				},
			}
		}
	}
}

/// Cached lookups for pallets, calls, and storage to avoid repeated string matching.
#[derive(Debug)]
pub struct RuntimeLayout {
	metadata: Arc<Metadata>,
	calls: RwLock<HashMap<(String, String), (u8, u8)>>,
	views: RwLock<HashMap<(String, String), [u8; 32]>>,
	view_output_types: RwLock<HashMap<(String, String), u32>>,
	type_ids: RwLock<HashMap<Vec<String>, u32>>,
	constants: RwLock<HashMap<(String, String), (Arc<Vec<u8>>, u32)>>,
	registry: PortableRegistry,
}

impl RuntimeLayout {
	pub fn new(metadata: Arc<Metadata>) -> Self {
		let registry = metadata.types().clone();
		Self {
			metadata,
			calls: RwLock::new(HashMap::new()),
			views: RwLock::new(HashMap::new()),
			view_output_types: RwLock::new(HashMap::new()),
			type_ids: RwLock::new(HashMap::new()),
			constants: RwLock::new(HashMap::new()),
			registry,
		}
	}

	pub async fn call_index(&self, pallet: &str, call: &str) -> Result<(u8, u8), Error> {
		if let Some(hit) = self.calls.read().await.get(&(pallet.into(), call.into())).cloned() {
			return Ok(hit);
		}
		let pallet_meta = self
			.metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| Error::NotFound(format!("pallet '{pallet}' not found")))?;
		let call_meta = pallet_meta
			.call_variant_by_name(call)
			.ok_or_else(|| Error::NotFound(format!("call '{pallet}.{call}' not found")))?;
		let idx = call_meta.index as u8;
		let pallet_idx = pallet_meta.index();
		self.calls
			.write()
			.await
			.insert((pallet.to_owned(), call.to_owned()), (pallet_idx, idx));
		Ok((pallet_idx, idx))
	}

	pub fn decode_as_type(
		&self,
		bytes: &[u8],
		type_id: u32,
	) -> Result<dynamic::DecodedValue, Error> {
		scale_value_scale::decode_as_type(&mut &*bytes, type_id, &self.registry)
			.map_err(|e| Error::Codec(e.to_string()))
	}

	pub fn registry(&self) -> &PortableRegistry {
		&self.registry
	}

	pub async fn view_id(&self, pallet: &str, view: &str) -> Result<[u8; 32], Error> {
		if let Some(hit) = self.views.read().await.get(&(pallet.into(), view.into())).cloned() {
			return Ok(hit);
		}
		let pallet_meta = self
			.metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| Error::NotFound(format!("pallet '{pallet}' not found")))?;
		let view_meta = pallet_meta
			.view_function_by_name(view)
			.ok_or_else(|| Error::NotFound(format!("view '{pallet}.{view}' not found")))?;
		let id = *view_meta.query_id();
		self.views.write().await.insert((pallet.to_owned(), view.to_owned()), id);
		Ok(id)
	}

	pub async fn view_output_type(&self, pallet: &str, view: &str) -> Result<u32, Error> {
		if let Some(hit) =
			self.view_output_types.read().await.get(&(pallet.into(), view.into())).cloned()
		{
			return Ok(hit);
		}

		let pallet_meta = self
			.metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| Error::NotFound(format!("pallet '{pallet}' not found")))?;
		let view_meta = pallet_meta
			.view_function_by_name(view)
			.ok_or_else(|| Error::NotFound(format!("view '{pallet}.{view}' not found")))?;
		let ty = view_meta.output_ty();
		self.view_output_types
			.write()
			.await
			.insert((pallet.to_owned(), view.to_owned()), ty);
		Ok(ty)
	}

	pub async fn constant_metadata(
		&self,
		pallet: &str,
		constant: &str,
	) -> Result<(Arc<Vec<u8>>, u32), Error> {
		if let Some(hit) =
			self.constants.read().await.get(&(pallet.into(), constant.into())).cloned()
		{
			return Ok(hit);
		}

		let pallet_meta = self
			.metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| Error::NotFound(format!("pallet '{pallet}' not found")))?;
		let constant_meta = pallet_meta
			.constant_by_name(constant)
			.ok_or_else(|| Error::NotFound(format!("constant '{pallet}.{constant}' not found")))?;
		let ty = constant_meta.ty();
		let value = Arc::new(constant_meta.value().to_vec());
		self.constants
			.write()
			.await
			.insert((pallet.to_owned(), constant.to_owned()), (value.clone(), ty));
		Ok((value, ty))
	}

	/// Resolve a type ID by its full path (e.g., ["Runtime", "EntityInfo"]).
	#[allow(deprecated)]
	pub async fn type_id_by_path(&self, path: &[&str]) -> Result<u32, Error> {
		let key: Vec<String> = path.iter().map(|s| s.to_string()).collect();
		if let Some(hit) = self.type_ids.read().await.get(&key).cloned() {
			return Ok(hit);
		}
		let ty = self
			.registry
			.types()
			.iter()
			.find(|t| t.ty.path().segments() == path)
			.ok_or_else(|| {
				Error::NotFound(format!("type path {:?} not found in metadata", path))
			})?;
		let id = ty.id();
		self.type_ids.write().await.insert(key, id);
		Ok(id)
	}

	/// Decode bytes into a concrete type using the registry and a type-id path lookup.
	pub async fn decode_as_path<T: scale_decode::DecodeAsType>(
		&self,
		bytes: &[u8],
		type_path: &[&str],
	) -> Result<T, Error> {
		let id = self.type_id_by_path(type_path).await?;
		let mut cursor = &bytes[..];
		scale_decode::DecodeAsType::decode_as_type(&mut cursor, id, &self.registry)
			.map_err(|e| Error::Codec(e.to_string()))
	}
}

/// Dynamic Origin client with high-throughput defaults and runtime-upgrade safety.
#[derive(Clone)]
pub struct OriginClient {
	inner: Arc<OnlineClient<params::config::OriginConfig>>,
	metadata: Arc<Metadata>,
	raw_metadata: Arc<Vec<u8>>,
	metadata_hash: [u8; 32],
	layout: Arc<RuntimeLayout>,
	nonces: Arc<NonceManager>,
	config: ClientConfig,
	in_flight: Arc<Semaphore>,
}

impl OriginClient {
	/// Connect using default high-throughput config.
	pub async fn connect(url: &str) -> Result<Self, Error> {
		OriginClientBuilder::new(url).build().await
	}

	pub async fn from_online_parts(
		inner: OnlineClient<params::config::OriginConfig>,
		metadata_snapshot: Metadata,
		raw_metadata: Vec<u8>,
		metadata_hash: [u8; 32],
		config: ClientConfig,
	) -> Result<Self, Error> {
		// OnlineClient has already fetched metadata via runtime API; cache it.
		let metadata = Arc::new(metadata_snapshot);
		let layout = Arc::new(RuntimeLayout::new(metadata.clone()));
		let nonces = Arc::new(NonceManager::new(
			config.nonce_strategy,
			Duration::from_secs(30),
		));
		let in_flight = Arc::new(Semaphore::new(config.max_in_flight_txs));
		spawn_nonce_resync_if_enabled(
			config.nonce_resync_interval,
			nonces.clone(),
			metadata.clone(),
			layout.clone(),
			inner.clone(),
		);
		Ok(Self {
			inner: Arc::new(inner),
			metadata,
			raw_metadata: Arc::new(raw_metadata),
			metadata_hash,
			layout,
			nonces,
			config,
			in_flight,
		})
	}

	/// Access cached metadata.
	pub fn metadata(&self) -> Arc<Metadata> {
		self.metadata.clone()
	}

	/// Raw metadata bytes pulled at connection time.
	pub fn raw_metadata(&self) -> Arc<Vec<u8>> {
		self.raw_metadata.clone()
	}

	/// Blake2 256 hash of the metadata blob.
	pub fn metadata_hash(&self) -> [u8; 32] {
		self.metadata_hash
	}

	/// Access cached layout (pallet/call indices).
	pub fn layout(&self) -> Arc<RuntimeLayout> {
		self.layout.clone()
	}

	pub fn registry(&self) -> &PortableRegistry {
		&self.layout.registry
	}

	/// Expose the registry for helper utilities.
	pub fn registry_ref(&self) -> &PortableRegistry {
		&self.layout.registry
	}

	/// Access to the underlying `OnlineClient` (advanced users).
	pub fn inner(&self) -> Arc<OnlineClient<params::config::OriginConfig>> {
		self.inner.clone()
	}

	/// Subscribe to finalized blocks and stream decoded dynamic events.
	pub async fn subscribe_finalized_events(&self, buffer: usize) -> Result<EventStream, Error> {
		let (tx, rx) = mpsc::channel(buffer);
		let client = self.inner.clone();
		let mut backoff = Duration::from_millis(200);
		let max_backoff = Duration::from_secs(5);

		tokio::spawn(async move {
			loop {
				match client.blocks().subscribe_finalized().await {
					Ok(mut sub) => {
						backoff = Duration::from_millis(200);
						while let Some(block_res) = sub.next().await {
							let Ok(block) = block_res else {
								break;
							};
							let block_hash = block.hash();
							let block_number: u32 = block.number().saturated_into();
							let events = match block.events().await {
								Ok(ev) => ev,
								Err(_) => break,
							};
							for ev in events.iter() {
								if let Ok(ev) = ev {
									if let Ok(fields) = ev.field_values() {
										let dynamic = DynamicEvent {
											block_hash,
											block_number,
											pallet: ev.pallet_name().to_string(),
											variant: ev.variant_name().to_string(),
											fields,
										};
										if tx.send(dynamic).await.is_err() {
											return;
										}
									}
								}
							}
						}
					},
					Err(_) => {},
				}
				tokio::time::sleep(backoff).await;
				backoff = (backoff * 2).min(max_backoff);
			}
		});

		Ok(EventStream { rx })
	}

	/// Decode a runtime constant into a dynamic value.
	pub async fn constant_value(
		&self,
		pallet: &str,
		constant: &str,
	) -> Result<dynamic::DecodedValue, Error> {
		let (bytes, ty) = self.layout.constant_metadata(pallet, constant).await?;
		scale_value_scale::decode_as_type(&mut &bytes[..], ty, self.layout.registry())
			.map_err(|e| Error::Codec(e.to_string()))
	}

	/// Decode a runtime constant into a concrete type using metadata.
	pub async fn constant_value_as<T: scale_decode::DecodeAsType>(
		&self,
		pallet: &str,
		constant: &str,
	) -> Result<T, Error> {
		let (bytes, ty) = self.layout.constant_metadata(pallet, constant).await?;
		let mut cursor = &bytes[..];
		scale_decode::DecodeAsType::decode_as_type(&mut cursor, ty, self.layout.registry())
			.map_err(|e| Error::Codec(e.to_string()))
	}

	/// Invoke a runtime view function dynamically and decode the output.
	pub async fn call_view(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<dynamic::DecodedValue, Error> {
		let query_id = self.layout.view_id(pallet, function).await?;
		let payload = dynamic::view_function_call(query_id, expect_composite(args)?);
		let api = self.inner.view_functions().at_latest().await.map_err(Error::from)?;
		let thunk = api.call(payload).await.map_err(Error::from)?;
		match thunk.to_value() {
			Ok(value) => Ok(value),
			Err(err) => {
				if std::env::var_os("OC_VIEW_DECODE_DEBUG").is_some() {
					let bytes = thunk.encoded();
					let preview_len = bytes.len().min(512);
					let mut hex_preview = String::with_capacity(preview_len * 2);
					for b in &bytes[..preview_len] {
						use core::fmt::Write;
						let _ = write!(hex_preview, "{:02x}", b);
					}
					let ctx = format!("{pallet}.{function}");
					log::warn!(
						target: "query::decode",
						"{ctx}: metadata decode failed ({err}); raw len {} preview 0x{}",
						bytes.len(),
						hex_preview
					);
					eprintln!(
						"[query::decode] {ctx}: metadata decode failed ({err}); raw len {} preview 0x{}",
						bytes.len(),
						hex_preview
					);
					eprintln!(
						"[query::decode] {ctx}: falling back to raw SCALE bytes"
					);
				}
				Err(Error::Codec(err.to_string()))
			},
		}
	}

	pub async fn call_view_raw_bytes(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<Vec<u8>, Error> {
		let query_id = self.layout.view_id(pallet, function).await?;
		let payload = dynamic::view_function_call(query_id, expect_composite(args)?);
		let api = self.inner.view_functions().at_latest().await.map_err(Error::from)?;
		let thunk = api.call(payload).await.map_err(Error::from)?;
		Ok(thunk.encoded().to_vec())
	}

	/// Invoke a view and decode the result into a concrete type using metadata output type.
	pub async fn call_view_typed<T: scale_decode::DecodeAsType>(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<T, Error> {
		let value = self.call_view(pallet, function, args).await?;
		let ty = self.layout.view_output_type(pallet, function).await?;
		let mut bytes = Vec::new();
		scale_value_scale::encode_as_type(&value, ty, &self.layout.registry, &mut bytes)
			.map_err(|e| Error::Codec(e.to_string()))?;
		let mut cursor = &bytes[..];
		scale_decode::DecodeAsType::decode_as_type(&mut cursor, ty, &self.layout.registry)
			.map_err(|e| Error::Codec(e.to_string()))
	}

	/// Access the high-level transaction pipeline helper.
	pub fn transactions(&self) -> crate::client::TransactionClient {
		crate::client::TransactionClient::new(self.clone())
	}

	/// Access the view-only helper surface.
	pub fn views(&self) -> crate::client::ViewApi {
		crate::client::ViewApi(self.clone())
	}

	/// Create a fluent extrinsic builder for the given pallet + call.
	pub fn extrinsic(&self, pallet: &str, call: &str) -> ExtrinsicBuilder {
		ExtrinsicBuilder::new(self.clone(), pallet, call)
	}

	/// Build a batch submission pipeline.
	pub fn batch(&self) -> BatchBuilder {
		BatchBuilder::new(self.clone())
	}

	/// Entry point for meta-transaction flows.
	pub fn metatx(&self) -> MetaTxClient {
		MetaTxClient::new(self.clone())
	}

	/// Low-level dynamic call submission using dynamic metadata (args as `Value`s).
	pub async fn submit_dynamic_call<S>(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
		signer: &S,
	) -> Result<
		subxt::tx::TxProgress<
			params::config::OriginConfig,
			OnlineClient<params::config::OriginConfig>,
		>,
		Error,
	>
	where
		S: subxt::tx::Signer<params::config::OriginConfig> + Clone + Send + Sync,
	{
		let _permit = self.in_flight.acquire().await.expect("semaphore closed");
		// Build dynamic call by name; Will be encoded using cached metadata.
		let call = dynamic::tx(pallet, call, args);
		let account_id = signer.account_id();

		let mut attempts = 0usize;
		let mut backoff = self.config.submit_retry.initial_backoff;
		loop {
			attempts += 1;
			let nonce = self.nonces.allocate(&self.inner, &account_id).await?;
			// Attach explicit nonce using Origin extrinsic params builder.
			let params = params::build_origin_params(
				DefaultExtrinsicParamsBuilder::<params::config::OriginConfig>::new().nonce(nonce),
			);
			match self.inner.tx().sign_and_submit_then_watch(&call, signer, params).await {
				Ok(progress) => return Ok(progress),
				Err(err) if is_future_or_stale(&err) => {
					let _ = self.nonces.refresh(&self.inner, &account_id).await;
					if attempts < self.config.submit_retry.attempts {
						sleep(backoff).await;
						backoff = backoff.saturating_mul(2);
						continue;
					}
					return Err(Error::from(err));
				},
				Err(_err) if attempts < self.config.submit_retry.attempts => {
					sleep(backoff).await;
					backoff = backoff.saturating_mul(2);
				},
				Err(err) => return Err(Error::from(err)),
			}
		}
	}
}
