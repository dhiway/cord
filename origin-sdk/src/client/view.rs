use std::sync::Arc;

use crate::types::error::OriginSdkError;
use super::connection::Connection;
use codec::{Decode, Encode};
use crate::util::retry::RetryPolicy;

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

	/// Invoke a pallet view function dynamically using Subxt view RPC and decode via metadata.
	pub async fn call<T: Decode>(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<T, OriginSdkError> {
		let backoff = RetryPolicy::default();
		let connection = self.connection.clone();
		backoff
			.retry(|| {
				let connection = connection.clone();
				let raw_args = raw_args.clone();
				async move {
					let metadata = connection.metadata();
					let pallet_meta = metadata
						.pallet_by_name(pallet)
						.ok_or_else(|| OriginSdkError::View(format!("pallet {pallet} not found")))?;
					let vf = pallet_meta
						.view_functions()
						.find(|vf| vf.name() == function)
						.ok_or_else(|| OriginSdkError::View(format!("view {pallet}.{function} not found")))?;
					let query_id = *vf.query_id();
					let inputs: Vec<_> = vf.inputs().collect();
					if inputs.len() != raw_args.len() {
						return Err(OriginSdkError::InvalidInput(format!(
							"expected {} args, got {}",
							inputs.len(),
							raw_args.len()
						)));
					}
					let mut values = Vec::with_capacity(raw_args.len());
					for (bytes, input) in raw_args.iter().cloned().zip(inputs) {
						let mut cursor = &bytes[..];
						let val = scale_value::scale::decode_as_type(&mut cursor, input.ty, metadata.types())
							.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
						values.push(val.remove_context());
					}
					let args = scale_value::Composite::unnamed(values);
					let payload = subxt::dynamic::view_function_call(query_id, args);
					let api = connection
						.online()
						.view_functions()
						.at_latest()
						.await
						.map_err(|e| OriginSdkError::View(e.to_string()))?;
					let thunk =
						api.call(payload).await.map_err(|e| OriginSdkError::View(e.to_string()))?;
					let value = thunk.to_value().map_err(|e| OriginSdkError::Decode(e.to_string()))?;
					let mut buf = Vec::new();
			scale_value::scale::encode_as_type(
				&value,
				vf.output_ty(),
				metadata.types(),
				&mut buf,
			)
			.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
		T::decode(&mut &buf[..]).map_err(|e| OriginSdkError::Decode(e.to_string()))
	}
			})
			.await
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
	) -> Result<crate::types::EntityOverview, OriginSdkError> {
		self.inner
			.call(
				"Entity",
				"overview",
				vec![auth.encode(), entity_id.encode()],
			)
			.await
	}

	pub async fn details(
		&self,
		auth: Auth,
		entity_id: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::EntityStateView, OriginSdkError> {
		self.inner
			.call(
				"Entity",
				"details",
				vec![auth.encode(), entity_id.encode()],
			)
			.await
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
	) -> Result<crate::types::RegistryStateView, OriginSdkError> {
		self.inner
			.call(
				"Register",
				"details",
				vec![auth.encode(), registry.encode()],
			)
			.await
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
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.inner
			.call(
				"Packet",
				"state",
				vec![auth.encode(), packet.encode()],
			)
			.await
	}
}
