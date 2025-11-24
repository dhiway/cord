pub(crate) mod connection;
mod events;
pub mod nonce;
pub mod signer;
pub mod submit;
mod view;

use std::sync::Arc;

use crate::{
	extrinsic::{builder::DynamicCallBuilder, metatx::MetaTxClient},
	types::error::OriginSdkError,
};
use connection::{Connection, ConnectionBuilder};
use submit::SubmitClient;
use view::ViewClient;

pub use events::EventEnvelope;
pub use signer::Signer;

/// Default Subxt config used by the Origin SDK.
pub type OriginConfig = subxt::config::PolkadotConfig;

/// High-level entrypoint to interact with Origin nodes.
#[derive(Clone)]
pub struct OriginClient {
	connection: Arc<Connection>,
	signer: Option<Arc<dyn Signer>>,
}

impl OriginClient {
	/// Connect to an endpoint with default retry/backoff policy.
	pub async fn connect(endpoint: impl Into<String>) -> Result<Self, OriginSdkError> {
		Self::builder().endpoint(endpoint).build().await
	}

	/// Convenience: connect and attach a signer in one step.
	pub async fn connect_with_signer(
		endpoint: impl Into<String>,
		signer: impl Signer + 'static,
	) -> Result<Self, OriginSdkError> {
		let client = Self::connect(endpoint).await?;
		Ok(client.with_signer(signer))
	}

	/// Build a client with custom policies.
	pub fn builder() -> ConnectionBuilder {
		ConnectionBuilder::default()
	}

	/// Attach a signer, returning a new client instance.
	pub fn with_signer(mut self, signer: impl Signer + 'static) -> Self {
		self.signer = Some(Arc::new(signer));
		self
	}

	/// Mutable setter for signers (useful when holding the client mutably).
	pub fn set_signer(&mut self, signer: impl Signer + 'static) {
		self.signer = Some(Arc::new(signer));
	}

	fn require_signer(&self) -> Result<Arc<dyn Signer>, OriginSdkError> {
		self.signer.clone().ok_or_else(|| {
			OriginSdkError::InvalidInput("signer is required for this operation".into())
		})
	}

	/// Low-level access to Subxt online client.
	pub fn online(&self) -> &subxt::OnlineClient<OriginConfig> {
		self.connection.online()
	}

	/// View-only calls (pallet view functions, no storage RPCs).
	pub fn view(&self) -> Result<ViewClient, OriginSdkError> {
		let signer = self.require_signer()?;
		Ok(ViewClient::new(self.connection.clone(), Some(signer)))
	}

	/// View-only calls using an explicit signer (does not alter client state).
	pub fn view_with(&self, signer: impl Signer + 'static) -> ViewClient {
		ViewClient::new(self.connection.clone(), Some(Arc::new(signer)))
	}

	/// Extrinsic submission with nonce queue + event-driven completion.
	pub fn tx(&self) -> Result<SubmitClient, OriginSdkError> {
		let signer = self.require_signer()?;
		Ok(SubmitClient::new(self.connection.clone(), Some(signer)))
	}

	/// Extrinsic submission with a one-off signer (does not alter client state).
	pub fn tx_with(&self, signer: impl Signer + 'static) -> SubmitClient {
		SubmitClient::new(self.connection.clone(), Some(Arc::new(signer)))
	}

	/// Build meta-transaction flows.
	pub fn metatx(&self) -> Result<MetaTxClient, OriginSdkError> {
		let signer = self.require_signer()?;
		Ok(MetaTxClient::new(self.connection.clone(), Some(signer)))
	}

	/// Meta-transaction flows with a provided signer (does not alter client state).
	pub fn metatx_with(&self, signer: impl Signer + 'static) -> MetaTxClient {
		MetaTxClient::new(self.connection.clone(), Some(Arc::new(signer)))
	}

	/// Dynamic call builder helper.
	pub fn call(&self) -> DynamicCallBuilder {
		DynamicCallBuilder::new()
	}

	/// Event subscription helpers.
	pub fn events(&self) -> events::EventClient {
		events::EventClient::new(self.connection.clone())
	}

	/// Query helpers (views + tx shortcuts) grouped by pallet.
	pub fn query(&self) -> crate::query::Query<'_> {
		crate::query::Query::new(self)
	}
}

/// Shorthand alias.
pub type Client = OriginClient;

/// Exported builder for user ergonomics.
pub fn connect(endpoint: impl Into<String>) -> ConnectionBuilder {
	ConnectionBuilder::default().endpoint(endpoint)
}
