use std::sync::Arc;

use crate::types::error::OriginSdkError;
use crate::types::{EntityOverview, EntityStateView, PacketStateView, RegistryStateView};
use super::connection::Connection;

type Auth = origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
>;

/// View (read) API limited to pallet view functions.
#[derive(Clone)]
pub struct ViewClient {
	connection: Arc<Connection>,
}

impl ViewClient {
	pub(crate) fn new(connection: Arc<Connection>) -> Self {
		Self { connection }
	}

	/// Invoke a pallet view function dynamically using Subxt view RPC.
	pub async fn call<T: Send>(
		&self,
		pallet: &str,
		function: &str,
		_args: impl Send,
	) -> Result<T, OriginSdkError> {
		Err(OriginSdkError::Unimplemented(format!("view {pallet}.{function}")))
	}

	/// Entity view helpers.
	pub fn entity(&self) -> EntityViews {
		EntityViews { inner: self.clone() }
	}

	/// Registry view helpers.
	pub fn registry(&self) -> RegistryViews {
		RegistryViews { inner: self.clone() }
	}

	/// Packet view helpers.
	pub fn packet(&self) -> PacketViews {
		PacketViews { inner: self.clone() }
	}
}

#[derive(Clone)]
pub struct EntityViews {
	inner: ViewClient,
}

impl EntityViews {
	pub async fn overview(
		&self,
		auth: Auth,
		entity_id: origin_primitives::Ss58Identifier,
	) -> Result<EntityOverview, OriginSdkError> {
		self.inner.call("Entity", "overview", (auth, entity_id)).await
	}

	pub async fn details(
		&self,
		auth: Auth,
		entity_id: origin_primitives::Ss58Identifier,
	) -> Result<EntityStateView, OriginSdkError> {
		self.inner.call("Entity", "details", (auth, entity_id)).await
	}
}

#[derive(Clone)]
pub struct RegistryViews {
	inner: ViewClient,
}

impl RegistryViews {
	pub async fn details(
		&self,
		auth: Auth,
		registry: origin_primitives::Ss58Identifier,
	) -> Result<RegistryStateView, OriginSdkError> {
		self.inner.call("Register", "details", (auth, registry)).await
	}
}

#[derive(Clone)]
pub struct PacketViews {
	inner: ViewClient,
}

impl PacketViews {
	pub async fn state(
		&self,
		auth: Auth,
		packet: origin_primitives::PacketPointer,
	) -> Result<PacketStateView, OriginSdkError> {
		self.inner.call("Packet", "state", (auth, packet)).await
	}
}
