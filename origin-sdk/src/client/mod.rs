pub(crate) mod connection;
mod events;
pub mod nonce;
pub mod signer;
pub(crate) mod tx_pipeline;
mod view;

use std::sync::Arc;

use crate::{
	config::OriginConfig,
	extrinsic::{builder::DynamicCallBuilder, metatx::MetaTxClient},
	tx::{config::TxPipelineConfig, TxClient},
	types::error::OriginSdkError,
};
use connection::{Connection, ConnectionBuilder};
use tx_pipeline::TxPipeline;

pub use events::EventEnvelope;
pub use signer::{OriginSigner, Signer};

/// High-level entrypoint to interact with Origin nodes.
#[derive(Clone)]
pub struct OriginClient {
	connection: Arc<Connection>,
	tx_pipeline: Arc<TxPipeline>,
	tx_cfg: TxPipelineConfig,
}

impl OriginClient {
	/// Connect to an endpoint with default retry/backoff policy.
	pub async fn connect(endpoint: impl Into<String>) -> Result<Self, OriginSdkError> {
		Self::builder().endpoint(endpoint).build().await
	}

	/// Build a client with custom policies.
	pub fn builder() -> ConnectionBuilder {
		ConnectionBuilder::default()
	}

	/// Low-level access to Subxt online client.
	pub fn online(&self) -> &subxt::OnlineClient<OriginConfig> {
		self.connection.online()
	}

	/// Runtime metadata snapshot (cheap handle).
	pub fn metadata(&self) -> subxt::Metadata {
		self.connection.metadata()
	}

	/// Query helpers (views) grouped by pallet; attach signer with `.using(&signer)`.
	pub fn query(&self) -> crate::query::Query<'_> {
		crate::query::Query::new(self)
	}

	/// Low-level view caller (used internally by query layer).
	pub fn view(&self) -> ViewClient {
		ViewClient::new(self.connection.clone())
	}

	/// Tx helpers grouped by pallet; attach signer with `.using(signer)`.
	pub fn tx(&self) -> TxClient {
		TxClient::new(self.clone(), self.tx_pipeline.clone())
	}

	/// Event subscription helpers.
	pub fn events(&self) -> events::EventClient {
		events::EventClient::new(self.connection.clone())
	}

	/// Dynamic call builder helper.
	pub fn call(&self) -> DynamicCallBuilder {
		DynamicCallBuilder::new()
	}

	/// Build meta-transaction flows (requires explicit signer later).
	pub fn metatx(&self) -> MetaTxClient {
		MetaTxClient::new(self.connection.clone(), None)
	}

	/// Convenience: build an OriginSigner from an OriginAccount.
	pub fn signer_from_account(
		&self,
		account: &crate::types::OriginAccount,
	) -> Result<OriginSigner, String> {
		signer::OriginSigner::from_account(account)
	}

	/// Tx pipeline config in use.
	pub fn tx_config(&self) -> &TxPipelineConfig {
		&self.tx_cfg
	}
}

/// Shorthand alias.
pub type Client = OriginClient;

/// Exported builder for user ergonomics.
pub fn connect(endpoint: impl Into<String>) -> ConnectionBuilder {
	ConnectionBuilder::default().endpoint(endpoint)
}

pub(crate) use view::ViewClient;
