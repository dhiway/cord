//! Dynamic, runtime-upgrade-safe client for Origin/OriginHub chains.
//!
//! This sits alongside the existing static `Client` and uses Subxt's
//! `DynamicConfig` to fetch metadata at runtime, assemble calls/storage
//! lookups by name, and manage nonces with a high-throughput default.

use crate::{error::Error, params};
use futures::stream::StreamExt;
use lru::LruCache;
use scale_info::PortableRegistry;
use scale_value::scale as scale_value_scale;
use scale_value::Value;
use std::{collections::HashMap, num::NonZeroUsize, sync::Arc, time::Duration};
use subxt::{config::DefaultExtrinsicParamsBuilder, dynamic, storage::DynamicAddress, Metadata, OnlineClient};
use subxt::dynamic::DecodedValueThunk;
use subxt::utils::AccountId32;
use tokio::sync::{Mutex, RwLock, Semaphore};

/// Nonce handling strategy for submissions.
#[derive(Clone, Copy, Debug)]
pub enum NonceStrategy {
	RpcPerTx,
	LocalCache,
}

fn expect_composite(value: Value) -> Result<scale_value::Composite<()>, Error> {
	match value.value {
		scale_value::ValueDef::Composite(c) => Ok(c),
		other => Err(Error::Params(format!("expected composite for view args, got {other:?}"))),
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
	pub storage_cache_capacity: usize,
}

impl Default for ClientConfig {
	fn default() -> Self {
		Self {
			nonce_strategy: NonceStrategy::LocalCache,
			max_in_flight_txs: 1024,
			submit_retry: SubmitRetryPolicy::default(),
			storage_cache_capacity: 256,
		}
	}
}

/// Thread-safe nonce tracker (per account).
#[derive(Debug)]
pub struct NonceManager {
	strategy: NonceStrategy,
	cached: RwLock<HashMap<[u8; 32], u64>>,
}

impl NonceManager {
	fn new(strategy: NonceStrategy) -> Self {
		Self { strategy, cached: RwLock::new(HashMap::new()) }
	}

	pub async fn next_nonce(
		&self,
		client: &OnlineClient<params::config::OriginConfig>,
		account: &AccountId32,
	) -> Result<u64, Error> {
		match self.strategy {
			NonceStrategy::RpcPerTx => client
				.tx()
				.account_nonce(account)
				.await
				.map_err(Error::from),
			NonceStrategy::LocalCache => {
				let key = account.0;
				// Fast path: try read lock first.
				if let Some(nonce) = self.cached.read().await.get(&key).cloned() {
					let mut guard = self.cached.write().await;
					let entry = guard.entry(key).or_insert(nonce);
					let next = *entry;
					*entry = entry.saturating_add(1);
					return Ok(next);
				}

				// Cache miss: fetch from chain once.
				let fetched = client.tx().account_nonce(account).await.map_err(Error::from)?;
				let mut guard = self.cached.write().await;
				let entry = guard.entry(key).or_insert(fetched);
				let next = *entry;
				*entry = entry.saturating_add(1);
				Ok(next)
			},
		}
	}
}

/// Cached lookups for pallets, calls, and storage to avoid repeated string matching.
#[derive(Debug)]
pub struct RuntimeLayout {
	metadata: Arc<Metadata>,
	calls: RwLock<HashMap<(String, String), (u8, u8)>>,
	registry: PortableRegistry,
}

impl RuntimeLayout {
	pub fn new(metadata: Arc<Metadata>) -> Self {
		let registry = metadata.types().clone();
		Self { metadata, calls: RwLock::new(HashMap::new()), registry }
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

	pub fn storage_value_type(&self, pallet: &str, entry: &str) -> Result<u32, Error> {
		let pallet_meta = self
			.metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| Error::NotFound(format!("pallet '{pallet}' not found")))?;
		let storage = pallet_meta
			.storage()
			.ok_or_else(|| Error::NotFound(format!("pallet '{pallet}' has no storage")))?;
		let entry_meta = storage
			.entry_by_name(entry)
			.ok_or_else(|| Error::NotFound(format!("storage '{pallet}.{entry}' not found")))?;
		Ok(entry_meta.entry_type().value_ty())
	}

	pub fn decode_as_type(&self, bytes: &[u8], type_id: u32) -> Result<dynamic::DecodedValue, Error> {
		scale_value_scale::decode_as_type(&mut &*bytes, type_id, &self.registry)
			.map_err(|e| Error::Codec(e.to_string()))
	}
}

/// Dynamic Origin client with high-throughput defaults and runtime-upgrade safety.
#[derive(Clone)]
	pub struct OriginClient {
	inner: Arc<OnlineClient<params::config::OriginConfig>>,
	metadata: Arc<Metadata>,
	layout: Arc<RuntimeLayout>,
	nonces: Arc<NonceManager>,
	config: ClientConfig,
	in_flight: Arc<Semaphore>,
	storage_cache: Arc<Mutex<LruCache<Vec<u8>, dynamic::DecodedValue>>>,
}

impl OriginClient {
	/// Connect using default high-throughput config.
	pub async fn connect(url: &str) -> Result<Self, Error> {
		let inner = OnlineClient::<params::config::OriginConfig>::from_url(url)
			.await
			.map_err(Error::from)?;
		Self::from_online_client(inner, ClientConfig::default()).await
	}

	async fn from_online_client(
		inner: OnlineClient<params::config::OriginConfig>,
		config: ClientConfig,
	) -> Result<Self, Error> {
		// OnlineClient has already fetched metadata via runtime API; cache it.
		let metadata = Arc::new(inner.metadata().clone());
		let layout = Arc::new(RuntimeLayout::new(metadata.clone()));
		let nonces = Arc::new(NonceManager::new(config.nonce_strategy));
		let in_flight = Arc::new(Semaphore::new(config.max_in_flight_txs));
		let cache = LruCache::new(NonZeroUsize::new(config.storage_cache_capacity).unwrap_or_else(|| NonZeroUsize::new(256).unwrap()));
		Ok(Self {
			inner: Arc::new(inner),
			metadata,
			layout,
			nonces,
			config,
			in_flight,
			storage_cache: Arc::new(Mutex::new(cache)),
		})
	}

	/// Access cached metadata.
	pub fn metadata(&self) -> Arc<Metadata> {
		self.metadata.clone()
	}

	/// Access cached layout (pallet/call indices).
	pub fn layout(&self) -> Arc<RuntimeLayout> {
		self.layout.clone()
	}

	pub fn registry(&self) -> &PortableRegistry {
		&self.layout.registry
	}

	/// Fetch a storage item by name and decode with metadata type info.
	pub async fn storage_value(
		&self,
		pallet: &str,
		entry: &str,
		keys: Vec<Value>,
	) -> Result<Option<dynamic::DecodedValue>, Error> {
		let address: DynamicAddress<Vec<Value>> = DynamicAddress::new(pallet, entry, keys);

		// cache key: exact storage key bytes (root + hashed keys) for determinism.
		let key_bytes = subxt::ext::subxt_core::storage::get_address_bytes(&address, &self.metadata)
			.map_err(|e| Error::Codec(e.to_string()))?;

		if let Some(hit) = self.storage_cache.lock().await.get(&key_bytes).cloned() {
			return Ok(Some(hit));
		}

		let raw: Option<DecodedValueThunk> = self
			.inner
			.storage()
			.at_latest()
			.await
			.map_err(Error::from)?
			.fetch(&address)
			.await
			.map_err(Error::from)?;

		if let Some(thunk) = raw {
			let val = thunk.to_value().map_err(|e| Error::Codec(e.to_string()))?;
			self.storage_cache.lock().await.put(key_bytes, val.clone());
			Ok(Some(val))
		} else {
			Ok(None)
		}
	}

	/// Fetch multiple storage values concurrently (best effort) and return decoded outputs.
	pub async fn storage_values_batch(
		&self,
		pallet: &str,
		entry: &str,
		keys_list: Vec<Vec<Value>>,
	) -> Result<Vec<Option<dynamic::DecodedValue>>, Error> {
		let concurrency = 16usize;
		let mut stream = futures::stream::iter(keys_list.into_iter().map(|keys| async move {
			self.storage_value(pallet, entry, keys).await
		}))
		.buffer_unordered(concurrency);

		let mut out = Vec::new();
		while let Some(res) = stream.next().await {
			out.push(res?);
		}
		Ok(out)
	}

	/// Invoke a runtime view function dynamically and decode the output.
	pub async fn call_view(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<dynamic::DecodedValue, Error> {
		let pallet_meta = self
			.metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| Error::NotFound(format!("pallet '{pallet}' not found")))?;
		let view = pallet_meta
			.view_function_by_name(function)
			.ok_or_else(|| Error::NotFound(format!("view '{pallet}.{function}' not found")))?;
		let query_id = *view.query_id();
		let payload = dynamic::view_function_call(query_id, expect_composite(args)?);
		let api = self.inner.view_functions().at_latest().await.map_err(Error::from)?;
		let thunk = api.call(payload).await.map_err(Error::from)?;
		thunk.to_value().map_err(|e| Error::Codec(e.to_string()))
	}

	/// Low-level dynamic call submission using dynamic metadata (args as `Value`s).
	pub async fn submit_dynamic_call<S>(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
		signer: &S,
	) -> Result<
		subxt::tx::TxProgress<params::config::OriginConfig, OnlineClient<params::config::OriginConfig>>,
		Error,
	>
	where
		S: subxt::tx::Signer<params::config::OriginConfig> + Clone + Send + Sync,
	{
		let _permit = self.in_flight.acquire().await.expect("semaphore closed");
		// Build dynamic call by name; Will be encoded using cached metadata.
		let call = dynamic::tx(pallet, call, args);
		let account_id = signer.account_id();
		let nonce = self.nonces.next_nonce(&self.inner, &account_id).await?;

		let mut attempts = 0usize;
		let mut backoff = self.config.submit_retry.initial_backoff;
		loop {
			attempts += 1;
			// Attach explicit nonce using Origin extrinsic params builder.
			let params = params::build_origin_params(
				DefaultExtrinsicParamsBuilder::<params::config::OriginConfig>::new().nonce(nonce),
			);
			match self.inner.tx().sign_and_submit_then_watch(&call, signer, params).await {
				Ok(progress) => return Ok(progress),
				Err(_err) if attempts < self.config.submit_retry.attempts => {
					tokio::time::sleep(backoff).await;
					backoff = backoff.saturating_mul(2);
				},
				Err(err) => return Err(Error::from(err)),
			}
		}
	}
}
