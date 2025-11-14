use crate::{
	error::{Error, Result},
	flavors::ChainFlavor,
	params::config::CordConfig,
	query::auth::DEFAULT_VIEW_AUTH_TTL,
};
#[allow(unused_imports)]
use futures::StreamExt;
use sp_core::hashing::blake2_256;
use sp_runtime::traits::SaturatedConversion;
use std::{convert::TryFrom, sync::Arc, time::Duration};
use subxt::{
	backend::rpc::{
		reconnecting_rpc_client::{
			ExponentialBackoff as RpcBackoff, RpcClient as ReconnectingRpcClient,
		},
		RpcClient,
	},
	config::PolkadotConfig,
	ext::{
		subxt_core::client::RuntimeVersion as CoreRuntimeVersion,
		subxt_rpcs::methods::legacy::{LegacyRpcMethods, SystemHealth},
	},
};

pub const DEFAULT_RPC_ENDPOINT: &str = "ws://127.0.0.1:9944";

/// Controls how the SDK establishes and maintains its RPC connection.
#[derive(Clone, Debug)]
pub struct ConnectionConfig {
	pub url: String,
	pub flavor: ChainFlavor,
	pub retry: RetryPolicy,
}

impl ConnectionConfig {
	pub fn new(url: impl Into<String>, flavor: ChainFlavor) -> Self {
		Self { url: url.into(), flavor, ..Default::default() }
	}

	pub fn with_retry(mut self, retry: RetryPolicy) -> Self {
		self.retry = retry;
		self
	}
}

impl Default for ConnectionConfig {
	fn default() -> Self {
		Self {
			url: DEFAULT_RPC_ENDPOINT.into(),
			flavor: ChainFlavor::Auto,
			retry: RetryPolicy::default(),
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
	pub(crate) api: subxt::OnlineClient<CordConfig>,
	pub(crate) rpc: Arc<RpcClient>,
	pub(crate) flavor: ChainFlavor,
}

impl Client {
	/// Connect to a node at `url`, optionally forcing the chain flavor.
	pub async fn connect(url: &str, flavor: ChainFlavor) -> Result<Self> {
		Self::connect_with(ConnectionConfig::new(url.to_string(), flavor)).await
	}

	/// Connect using a reusable [`ConnectionConfig`].
	pub async fn connect_with(config: ConnectionConfig) -> Result<Self> {
		let rpc = build_reconnecting_rpc(&config).await?;
		let api = subxt::OnlineClient::<CordConfig>::from_rpc_client(rpc.as_ref().clone())
			.await
			.map_err(Error::from)?;
		let flavor = match config.flavor {
			ChainFlavor::Auto => crate::flavors::detect_flavor(&api).await?,
			other => other,
		};
		Ok(Self { api, rpc, flavor })
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
		let raw = self
			.legacy_methods()
			.state_get_metadata(None)
			.await
			.map_err(|e| Error::Transport(e.to_string()))?;
		Ok(blake2_256(&raw.into_raw()))
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
	pub fn online(&self) -> &subxt::OnlineClient<CordConfig> {
		&self.api
	}

	pub async fn fetch_metadata_blob(&self) -> Result<Vec<u8>> {
		self.legacy_methods()
			.state_get_metadata(None)
			.await
			.map(|blob| blob.into_raw())
			.map_err(|e| Error::Transport(e.to_string()))
	}
}

async fn build_reconnecting_rpc(config: &ConnectionConfig) -> Result<Arc<RpcClient>> {
	let strategy = ReconnectBackoff::new(&config.retry);
	let reconnecting = ReconnectingRpcClient::builder()
		.retry_policy(strategy)
		.build(&config.url)
		.await
		.map_err(|e| Error::Transport(e.to_string()))?;
	let rpc = RpcClient::new(reconnecting);
	Ok(Arc::new(rpc))
}

#[derive(Clone)]
struct ReconnectBackoff {
	inner: RpcBackoff,
	remaining: Option<usize>,
}

impl ReconnectBackoff {
	fn new(policy: &RetryPolicy) -> Self {
		let mut inner = RpcBackoff::from_millis(duration_to_millis(policy.initial_backoff));
		inner = inner.max_delay(policy.max_backoff);
		Self { inner, remaining: policy.max_retries }
	}
}

impl Iterator for ReconnectBackoff {
	type Item = Duration;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(rem) = self.remaining.as_mut() {
			if *rem == 0 {
				return None;
			}
			*rem -= 1;
		}
		self.inner.next()
	}
}

fn duration_to_millis(duration: Duration) -> u64 {
	let ms = duration.as_millis();
	if ms == 0 {
		1
	} else {
		ms.min(u64::MAX as u128) as u64
	}
}
