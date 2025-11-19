use crate::{
	error::{Error, Result},
	flavors::ChainFlavor,
	metadata,
	params::config::OriginConfig,
	query::auth::{AuthorizationBuilder, DEFAULT_VIEW_AUTH_TTL},
};
#[allow(unused_imports)]
use futures::StreamExt;
use jsonrpsee_client_transport::ws::WsTransportClientBuilder;
use jsonrpsee_core::client::{async_client::PingConfig, Client as WsClient};
use log::warn;
use origin_primitives::view_api::AuthorizationRequest;
use scale_decode::DecodeAsType;
use sp_core::hashing::blake2_256;
use sp_runtime::traits::SaturatedConversion;
use std::{convert::TryFrom, sync::Arc, time::Duration};
use subxt::{
	backend::rpc::RpcClient,
	config::PolkadotConfig,
	dynamic::DecodedValue,
	ext::{
		subxt_core::client::RuntimeVersion as CoreRuntimeVersion,
		subxt_rpcs::methods::legacy::{LegacyRpcMethods, SystemHealth},
	},
};
use tokio::time::sleep;
use url::Url;

use crate::origin_client::{ClientConfig as DynamicClientConfig, OriginClient};

pub const DEFAULT_RPC_ENDPOINT: &str = "ws://127.0.0.1:9944";

/// Controls how the SDK establishes and maintains its RPC connection.
#[derive(Clone, Debug)]
pub struct ConnectionConfig {
	pub url: String,
	pub flavor: ChainFlavor,
	pub retry: RetryPolicy,
	pub enforce_views: bool,
}

impl ConnectionConfig {
	pub fn new(url: impl Into<String>, flavor: ChainFlavor) -> Self {
		Self { url: url.into(), flavor, ..Default::default() }
	}

	pub fn with_retry(mut self, retry: RetryPolicy) -> Self {
		self.retry = retry;
		self
	}

	/// Disable metadata view-function validation (useful for diagnostics against older runtimes).
	pub fn skip_view_validation(mut self) -> Self {
		self.enforce_views = false;
		self
	}
}

impl Default for ConnectionConfig {
	fn default() -> Self {
		Self {
			url: DEFAULT_RPC_ENDPOINT.into(),
			flavor: ChainFlavor::Auto,
			retry: RetryPolicy::default(),
			enforce_views: true,
		}
	}
}

/// Retry/backoff settings for the reconnecting RPC client.
#[derive(Clone, Debug)]
pub struct RetryPolicy {
	pub initial_backoff: Duration,
	pub max_backoff: Duration,
	pub max_retries: Option<usize>,
}

impl Default for RetryPolicy {
	fn default() -> Self {
		Self {
			initial_backoff: Duration::from_millis(50),
			max_backoff: Duration::from_secs(10),
			max_retries: None,
		}
	}
}

/// High-level handle to a connected Origin-derived chain.
pub struct Client {
	pub(crate) api: subxt::OnlineClient<OriginConfig>,
	pub(crate) rpc: Arc<RpcClient>,
	pub(crate) flavor: ChainFlavor,
	pub(crate) origin: OriginClient,
}

impl Client {
	/// Connect to a node at `url`, optionally forcing the chain flavor.
	pub async fn connect(url: &str, flavor: ChainFlavor) -> Result<Self> {
		Self::connect_with(ConnectionConfig::new(url.to_string(), flavor)).await
	}

	/// Connect using a reusable [`ConnectionConfig`].
	pub async fn connect_with(config: ConnectionConfig) -> Result<Self> {
		let rpc = build_reconnecting_rpc(&config).await?;
		let (genesis_hash, runtime_version, metadata_snapshot, metadata_bytes) =
			metadata::load_or_fetch(rpc.as_ref()).await?;

		let flavor = match config.flavor {
			ChainFlavor::Auto => crate::flavors::detect_flavor_from_metadata(&metadata_snapshot)?,
			other => other,
		};

		if config.enforce_views {
			validate_required_views(&metadata_snapshot, flavor)?;
		}

		let api = subxt::OnlineClient::<OriginConfig>::from_rpc_client_with(
			genesis_hash,
			runtime_version.clone(),
			metadata_snapshot.clone(),
			rpc.as_ref().clone(),
		)
		.map_err(Error::from)?;

		let metadata_hash = blake2_256(&metadata_bytes);
		let origin = OriginClient::from_online_parts(
			api.clone(),
			metadata_snapshot.clone(),
			metadata_bytes.clone(),
			metadata_hash,
			DynamicClientConfig::default(),
		)
		.await?;

		let client = Self { api, rpc, flavor, origin };
		// Best-effort metadata caching for future runs.
		let _ = metadata::cache_metadata(client.flavor, &runtime_version, &metadata_bytes).await;
		Ok(client)
	}

	pub(crate) fn legacy_methods(&self) -> LegacyRpcMethods<PolkadotConfig> {
		LegacyRpcMethods::new((*self.rpc).clone())
	}

	/// Fetch the current node health info.
	pub async fn health(&self) -> Result<SystemHealth> {
		self.legacy_methods()
			.system_health()
			.await
			.map_err(|e| Error::Transport(e.to_string()))
	}

	/// Returns the runtime version.
	pub async fn runtime_version(&self) -> Result<CoreRuntimeVersion> {
		Ok(self.api.runtime_version())
	}

	/// Compute the metadata hash for the latest runtime by hashing the raw metadata bytes.
	pub async fn metadata_hash(&self) -> Result<[u8; 32]> {
		let (_, raw) = crate::metadata::fetch_metadata_latest(self.rpc.as_ref(), None).await?;
		Ok(blake2_256(&raw))
	}

	pub async fn view_auth_reference_block(&self) -> Result<u32> {
		let header = self
			.legacy_methods()
			.chain_get_header(None)
			.await
			.map_err(|e| Error::Transport(e.to_string()))?;
		let header = header.ok_or_else(|| Error::NotFound("latest header".into()))?;
		Ok(header.number.saturated_into())
	}

	pub async fn view_auth_valid_until(&self) -> Result<u32> {
		let number = self.view_auth_reference_block().await?;
		Ok(number.saturating_add(DEFAULT_VIEW_AUTH_TTL))
	}

	/// Wait for a runtime upgrade notification and return once one is observed.
	pub async fn watch_runtime_upgrades(&self) -> Result<()> {
		let mut sub = self
			.legacy_methods()
			.state_subscribe_runtime_version()
			.await
			.map_err(|e| Error::Transport(e.to_string()))?;
		let mut baseline = None;
		while let Some(version) =
			sub.next().await.transpose().map_err(|e| Error::Transport(e.to_string()))?
		{
			if baseline.is_none() {
				baseline = Some(version.clone());
				continue;
			}
			if baseline.as_ref().map(|v| v.spec_version) != Some(version.spec_version) {
				return Ok(());
			}
		}
		Err(Error::Timeout)
	}

	/// Access the query facade.
	pub fn query(&self) -> crate::query::Query<'_> {
		crate::query::Query { client: self }
	}

	/// Access the dynamic Origin client facade.
	pub fn origin(&self) -> OriginClient {
		self.origin.clone()
	}

	/// The detected/selected chain flavor.
	pub fn flavor(&self) -> ChainFlavor {
		self.flavor
	}

	pub async fn chain_prefix(&self) -> sp_core::crypto::Ss58AddressFormat {
		let default = sp_core::crypto::Ss58AddressFormat::from(self.flavor.ss58_prefix());
		match self.legacy_methods().system_properties().await {
			Ok(props) => props
				.get("ss58Format")
				.and_then(|value| value.as_u64())
				.and_then(|fmt| u16::try_from(fmt).ok())
				.map(sp_core::crypto::Ss58AddressFormat::from)
				.unwrap_or(default),
			Err(_) => default,
		}
	}

	/// Access the extrinsic builder facade.
	pub fn tx(&self) -> crate::tx::Transactions<'_> {
		crate::tx::Transactions { client: self }
	}

	/// Access lightweight state helpers.
	pub fn state(&self) -> crate::state::State<'_> {
		crate::state::State { client: self }
	}

	/// Obtain a clone of the cached runtime metadata.
	pub fn metadata(&self) -> subxt::Metadata {
		self.api.metadata().clone()
	}

	/// Expose the underlying Subxt client for advanced flows.
	pub fn online(&self) -> &subxt::OnlineClient<OriginConfig> {
		&self.api
	}

	/// Decode a runtime constant into a dynamic value.
	pub async fn constant_value(&self, pallet: &str, constant: &str) -> Result<DecodedValue> {
		self.origin.constant_value(pallet, constant).await
	}

	/// Decode a runtime constant into a concrete type using metadata.
	pub async fn constant_value_as<T: DecodeAsType>(
		&self,
		pallet: &str,
		constant: &str,
	) -> Result<T> {
		self.origin.constant_value_as(pallet, constant).await
	}

	/// Build a scoped authorization for a specific view function using the latest reference block.
	pub async fn build_view_authorization<S: subxt::tx::Signer<PolkadotConfig>>(
		&self,
		pallet: &str,
		view: &str,
		signer: &S,
	) -> Result<AuthorizationRequest> {
		let reference_block = self.view_auth_reference_block().await?;
		let context = AuthorizationBuilder::view_context(pallet, view);
		AuthorizationBuilder::generate_view_authorization(signer, &context, reference_block, None)
			.map_err(|e| Error::Signer(e.to_string()))
	}

	pub async fn fetch_metadata_blob(&self) -> Result<Vec<u8>> {
		let (_, raw) = crate::metadata::fetch_metadata_latest(self.rpc.as_ref(), None).await?;
		Ok(raw)
	}
}

fn validate_required_views(metadata: &subxt::Metadata, flavor: ChainFlavor) -> Result<()> {
	if matches!(flavor, ChainFlavor::OriginHub) {
		for (pallet, view) in [
			("Entity", "details"),
			("Entity", "account_token"),
			("Register", "details"),
			("Register", "packet_snapshot"),
			("Token", "timeline"),
		] {
			let pallet_meta = metadata.pallet_by_name(pallet).ok_or_else(|| {
				Error::NotFound(format!("pallet '{pallet}' not found in metadata"))
			})?;
			if pallet_meta.view_function_by_name(view).is_none() {
				return Err(Error::NotFound(format!(
					"required view '{pallet}.{view}' missing in runtime metadata; \
					 rebuild @origin-hub-system-runtime with view_functions enabled \
					 and refresh local metadata (cargo run -p origin-rs --example fetch-metadata -- --node <ws-url> --flavor origin-hub)"
				)));
			}
		}
	}
	Ok(())
}

async fn build_reconnecting_rpc(config: &ConnectionConfig) -> Result<Arc<RpcClient>> {
	let url = Url::parse(&config.url).map_err(|e| Error::Params(e.to_string()))?;
	let mut attempt = 0usize;
	let mut backoff = config.retry.initial_backoff;
	loop {
		match WsTransportClientBuilder::default().build(url.clone()).await {
			Ok((sender, receiver)) => {
				// Tune the ws client for high throughput and low latency.
				let client = WsClient::builder()
					.request_timeout(Duration::from_secs(10))
					.max_buffer_capacity_per_subscription(16 * 1024 * 1024)
					.enable_ws_ping(PingConfig::new().ping_interval(Duration::from_secs(10)))
					.set_tcp_no_delay(true)
					.max_concurrent_requests(1024 * 10)
					.build_with_tokio(sender, receiver);
				return Ok(Arc::new(RpcClient::new(client)));
			},
			Err(err) => {
				attempt = attempt.saturating_add(1);
				let fallible = matches!(config.retry.max_retries, Some(max) if attempt >= max);
				if fallible {
					return Err(Error::Transport(format!("ws connect failed: {err}")));
				}
				let wait = backoff.min(config.retry.max_backoff);
				warn!("ws connect failed (attempt #{attempt}): {err}; retrying in {wait:?}");
				sleep(wait).await;
				backoff = (backoff * 2).min(config.retry.max_backoff);
			},
		}
	}
}
