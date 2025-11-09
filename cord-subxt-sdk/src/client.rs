use crate::{
	error::{Error, Result},
	flavors::ChainFlavor,
	params::config::CordConfig,
};
#[allow(unused_imports)]
use futures::StreamExt;
use sp_core::hashing::blake2_256;
use std::sync::Arc;
use subxt::backend::rpc::RpcClient;
use subxt::config::PolkadotConfig;
use subxt::ext::subxt_core::client::RuntimeVersion as CoreRuntimeVersion;
use subxt::ext::subxt_rpcs::methods::legacy::{LegacyRpcMethods, SystemHealth};

/// High-level handle to a connected Origin-derived chain.
pub struct Client {
	pub(crate) api: subxt::OnlineClient<CordConfig>,
	pub(crate) rpc: Arc<RpcClient>,
	pub(crate) flavor: ChainFlavor,
}

impl Client {
	/// Connect to a node at `url`, optionally forcing the chain flavor.
	pub async fn connect(url: &str, flavor: ChainFlavor) -> Result<Self> {
		let rpc = Arc::new(
			RpcClient::from_insecure_url(url)
				.await
				.map_err(|e| Error::Transport(e.to_string()))?,
		);
		let api = subxt::OnlineClient::<CordConfig>::from_rpc_client(rpc.as_ref().clone())
			.await
			.map_err(Error::from)?;
		let flavor = match flavor {
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

	/// Access the JSON view facade.
	pub fn views(&self) -> crate::views::Views<'_> {
		crate::views::Views { client: self }
	}

	/// Access the extrinsic builder facade.
	pub fn tx(&self) -> crate::tx::Transactions<'_> {
		crate::tx::Transactions { client: self }
	}

	/// Access lightweight state helpers.
	pub fn state(&self) -> crate::state::State<'_> {
		crate::state::State { client: self }
	}
}
