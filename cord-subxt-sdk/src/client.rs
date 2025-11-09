use crate::{
	error::{Error, Result},
	flavors::ChainFlavor,
	params::config::CordConfig,
};
use futures::StreamExt;
use std::sync::Arc;
use subxt::backend::{
	legacy::rpc_methods::{LegacyRpcMethods, SystemHealth},
	rpc::RpcClient,
};
use subxt::config::PolkadotConfig;
use subxt::ext::subxt_core::client::RuntimeVersion;

/// High-level handle to a connected Origin-derived chain.
pub struct Client {
	pub(crate) api: subxt::OnlineClient<CordConfig>,
	pub(crate) rpc: Arc<RpcClient>,
	pub(crate) flavor: ChainFlavor,
}

impl Client {
	/// Connect to a node at `url`, optionally forcing the chain flavor.
	pub async fn connect(url: &str, flavor: ChainFlavor) -> Result<Self> {
		let rpc = Arc::new(RpcClient::from_insecure_url(url).await.map_err(Error::from)?);
		let api = subxt::OnlineClient::<CordConfig>::from_rpc_client(rpc.as_ref().clone())
			.await
			.map_err(Error::from)?;
		let flavor = match flavor {
			ChainFlavor::Auto => crate::flavors::detect_flavor(&api).await?,
			other => other,
		};
		Ok(Self { api, rpc, flavor })
	}

	fn legacy(&self) -> LegacyRpcMethods<PolkadotConfig> {
		LegacyRpcMethods::new((*self.rpc).clone())
	}

	/// Fetch the current node health info.
	pub async fn health(&self) -> Result<SystemHealth> {
		self.legacy().system_health().await.map_err(|e| Error::Transport(e.to_string()))
	}

	/// Returns the runtime version.
	pub async fn runtime_version(&self) -> Result<RuntimeVersion> {
		self.legacy()
			.state_get_runtime_version(None)
			.await
			.map_err(|e| Error::Transport(e.to_string()))
	}

	/// Compute the metadata hash for the latest runtime.
	pub async fn metadata_hash(&self) -> Result<[u8; 32]> {
		Ok(self.api.metadata().as_latest().hash().0)
	}

	/// Wait for a runtime upgrade notification and return once one is observed.
	pub async fn watch_runtime_upgrades(&self) -> Result<()> {
		let mut sub = self
			.legacy()
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

	/// Access the JSON view facade.
	pub fn views(&self) -> crate::views::Views<'_> {
		crate::views::Views { api: &self.api }
	}

	/// Access the extrinsic builder facade.
	pub fn tx(&self) -> crate::tx::Transactions<'_> {
		crate::tx::Transactions { client: self }
	}

	/// Access lightweight state helpers.
	pub fn state(&self) -> crate::state::State<'_> {
		crate::state::State { api: &self.api }
	}
}
