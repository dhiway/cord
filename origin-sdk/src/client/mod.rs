pub(crate) mod connection;
mod events;
pub mod signer;
pub mod submit;
mod view;

use std::sync::Arc;

use crate::extrinsic::builder::DynamicCallBuilder;
use crate::extrinsic::metatx::MetaTxClient;
use crate::types::error::OriginSdkError;
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
	signer: Arc<dyn Signer>,
}

impl OriginClient {
	/// Connect to an endpoint with default retry/backoff policy.
	pub async fn connect(
		endpoint: impl Into<String>,
		signer: impl Signer + 'static,
	) -> Result<Self, OriginSdkError> {
		Self::builder().endpoint(endpoint).signer(signer).build().await
	}

	/// Build a client with custom policies.
	pub fn builder() -> ConnectionBuilder {
		ConnectionBuilder::default()
	}

	/// Low-level access to Subxt online client.
	pub fn online(&self) -> &subxt::OnlineClient<OriginConfig> {
		self.connection.online()
	}

	/// View-only calls (pallet view functions, no storage RPCs).
	pub fn view(&self) -> ViewClient {
		ViewClient::new(self.connection.clone(), self.signer.clone())
	}

	/// Extrinsic submission with nonce queue + event-driven completion.
	pub fn tx(&self) -> SubmitClient {
		SubmitClient::new(self.connection.clone(), self.signer.clone())
	}

	/// Build meta-transaction flows.
	pub fn metatx(&self) -> MetaTxClient {
		MetaTxClient::new(self.connection.clone(), self.signer.clone())
	}

	/// Dynamic call builder helper.
	pub fn call(&self) -> DynamicCallBuilder {
		DynamicCallBuilder::new()
	}

	/// Event subscription helpers.
	pub fn events(&self) -> events::EventClient {
		events::EventClient::new(self.connection.clone())
	}

	/// Token view helpers.
	pub fn token(&self) -> view::TokenViews {
		view::TokenViews { inner: self.view() }
	}

	/// Packet view helpers.
	pub fn packet(&self) -> view::PacketViews {
		view::PacketViews { inner: self.view() }
	}

	/// Registry view helpers.
	pub fn registry(&self) -> view::RegistryViews {
		view::RegistryViews { inner: self.view() }
	}

	/// Entity view helpers.
	pub fn entity(&self) -> view::EntityViews {
		view::EntityViews { inner: self.view() }
	}

	/// Domain-scoped helpers (views + tx shortcuts).
	pub fn domain(&self) -> crate::domain::Domain<'_> {
		crate::domain::Domain::new(self)
	}
}

/// Shorthand alias.
pub type Client = OriginClient;

/// Exported builder for user ergonomics.
pub fn connect(endpoint: impl Into<String>, signer: impl Signer + 'static) -> ConnectionBuilder {
	ConnectionBuilder::default().endpoint(endpoint).signer(signer)
}
