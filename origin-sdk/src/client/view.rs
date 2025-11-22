use std::sync::Arc;

use super::connection::Connection;
use super::Signer;
use crate::types::error::OriginSdkError;
use crate::util::{retry::RetryPolicy, ttl};
use codec::{Decode, Encode};
use origin_primitives::authorization::AuthorizationError;
use scale_value::{Composite as SvComposite, Primitive as SvPrimitive, ValueDef};
use sp_core::{ecdsa, ed25519, sr25519};

type Auth = origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
>;

/// View (read) API limited to pallet view functions.
#[derive(Clone)]
pub struct ViewClient {
	connection: Arc<Connection>,
	signer: Arc<dyn Signer>,
}

impl ViewClient {
	pub(crate) fn new(connection: Arc<Connection>, signer: Arc<dyn Signer>) -> Self {
		Self { connection, signer }
	}

	/// Invoke a pallet view function dynamically and decode via metadata.
	pub async fn call<T: Decode>(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<T, OriginSdkError> {
		let backoff = RetryPolicy::default();
		let connection = self.connection.clone();
		let auth = self.auth_for(pallet, function);
		backoff
			.retry(|| {
				let connection = connection.clone();
				let raw_args = raw_args.clone();
				let auth = auth.clone();
				async move {
					let metadata = connection.metadata();
					let pallet_meta = metadata.pallet_by_name(pallet).ok_or_else(|| {
						OriginSdkError::View(format!("pallet {pallet} not found"))
					})?;
					let vf =
						pallet_meta.view_functions().find(|vf| vf.name() == function).ok_or_else(
							|| OriginSdkError::View(format!("view {pallet}.{function} not found")),
						)?;
					let query_id = *vf.query_id();
					let inputs: Vec<_> = vf.inputs().collect();
					let mut args_with_auth = vec![auth.encode()];
					args_with_auth.extend(raw_args.clone());
					if inputs.len() != args_with_auth.len() {
						return Err(OriginSdkError::InvalidInput(format!(
							"expected {} args, got {}",
							inputs.len(),
							args_with_auth.len()
						)));
					}
					let mut values = Vec::with_capacity(args_with_auth.len());
					for (bytes, input) in args_with_auth.iter().cloned().zip(inputs) {
						let mut cursor = &bytes[..];
						let val = scale_value::scale::decode_as_type(
							&mut cursor,
							input.ty,
							metadata.types(),
						)
						.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
						values.push(val.remove_context());
					}
					let args = scale_value::Composite::unnamed(values);
					let payload = subxt::dynamic::view_function_call(query_id, args);
					let value = Self::call_value_inner(&connection, payload).await?;
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

	async fn call_value_inner(
		connection: &Arc<Connection>,
		payload: subxt::view_functions::DefaultPayload<
			scale_value::Composite<()>,
			subxt::dynamic::DecodedValueThunk,
		>,
	) -> Result<subxt::dynamic::DecodedValue, OriginSdkError> {
		let api = connection
			.online()
			.view_functions()
			.at_latest()
			.await
			.map_err(|e| OriginSdkError::View(e.to_string()))?;
		let thunk = api.call(payload).await.map_err(|e| OriginSdkError::View(e.to_string()))?;
		thunk.to_value().map_err(|e| OriginSdkError::Decode(e.to_string()))
	}

	/// Invoke and return raw DecodedValue (no further decode).
	pub async fn call_value(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<subxt::dynamic::DecodedValue, OriginSdkError> {
		let metadata = self.connection.metadata();
		let pallet_meta = metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| OriginSdkError::View(format!("pallet {pallet} not found")))?;
		let vf = pallet_meta
			.view_functions()
			.find(|vf| vf.name() == function)
			.ok_or_else(|| OriginSdkError::View(format!("view {pallet}.{function} not found")))?;
		let query_id = *vf.query_id();
		let inputs: Vec<_> = vf.inputs().collect();
		let mut args_with_auth = vec![self.auth_for(pallet, function).encode()];
		args_with_auth.extend(raw_args);
		if inputs.len() != args_with_auth.len() {
			return Err(OriginSdkError::InvalidInput(format!(
				"expected {} args, got {}",
				inputs.len(),
				args_with_auth.len()
			)));
		}
		let mut values = Vec::with_capacity(args_with_auth.len());
		for (bytes, input) in args_with_auth.into_iter().zip(inputs) {
			let mut cursor = &bytes[..];
			let val = scale_value::scale::decode_as_type(&mut cursor, input.ty, metadata.types())
				.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
			values.push(val.remove_context());
		}
		let args = scale_value::Composite::unnamed(values);
		let payload = subxt::dynamic::view_function_call(query_id, args);
		Self::call_value_inner(&self.connection, payload).await
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
	pub(crate) inner: ViewClient,
}

impl EntityViews {
	pub async fn overview(
		&self,
		entity_id: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::EntityStateView, OriginSdkError> {
		let res: Result<
			Result<crate::types::EntityStateView, AuthorizationError>,
			OriginSdkError,
		> = self
			.inner
			.call("Entity", "overview", vec![entity_id.encode(), Option::<u32>::None.encode()])
			.await;
		match res {
			Ok(Ok(v)) => Ok(v),
			Ok(Err(e)) => Err(OriginSdkError::View(format!("overview err: {e:?}"))),
			Err(e) => Err(e),
		}
	}

	pub async fn account_token(
		&self,
		account: subxt::utils::AccountId32,
	) -> Result<Option<origin_primitives::Ss58Identifier>, OriginSdkError> {
		let res: Result<Result<origin_primitives::Ss58Identifier, AuthorizationError>, OriginSdkError> =
			self.inner.call("Entity", "account_token", vec![account.encode()]).await;
		match res {
			Ok(Ok(id)) => Ok(Some(id)),
			Ok(Err(AuthorizationError::NotFound)) => Ok(None),
			Ok(Err(e)) => Err(OriginSdkError::View(format!("account_token err: {e:?}"))),
			Err(e) => Err(e),
		}
	}

	pub async fn details(
		&self,
		entity_id: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::EntityStateView, OriginSdkError> {
		self.inner.call("Entity", "details", vec![entity_id.encode()]).await
	}
}

#[derive(Clone)]
pub struct RegistryViews {
	pub(crate) inner: ViewClient,
}

impl RegistryViews {
	pub async fn details(
		&self,
		registry: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::RegistryStateView, OriginSdkError> {
		self.inner.call("Register", "details", vec![registry.encode()]).await
	}
}

#[derive(Clone)]
pub struct PacketViews {
	pub(crate) inner: ViewClient,
}

impl PacketViews {
	pub async fn lookup(
		&self,
		key: Vec<u8>,
	) -> Result<subxt::dynamic::DecodedValue, OriginSdkError> {
		self.inner.call_value("Packet", "lookup", vec![key.encode()]).await
	}

	pub async fn state(
		&self,
		packet: origin_primitives::PacketPointer,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.inner.call("Packet", "state", vec![packet.encode()]).await
	}
}

#[derive(Clone)]
pub struct TokenViews {
	pub(crate) inner: ViewClient,
}

impl TokenViews {
	pub async fn timeline(
		&self,
		token: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::TokenTimelineView, OriginSdkError> {
		self.inner.call_value("Token", "timeline", vec![token.encode()]).await
	}

	pub async fn lookup(
		&self,
		token: origin_primitives::Ss58Identifier,
		key: Vec<u8>,
	) -> Result<crate::types::TokenLookupView, OriginSdkError> {
		self.inner
			.call_value("Token", "lookup", vec![token.encode(), key.encode()])
			.await
	}
}

impl ViewClient {
	fn auth_for(&self, pallet: &str, function: &str) -> Auth {
		let expires = ttl::expires_in(std::time::Duration::from_secs(30));
		let payload = format!("view:{pallet}.{function}:{expires}").into_bytes();
		let sig = self.signer.sign(&payload);
		let account = origin_primitives::AccountId::from(self.signer.account_id().0);
		let signature = match sig {
			subxt::utils::MultiSignature::Ed25519(raw) => {
				sp_runtime::MultiSignature::Ed25519(ed25519::Signature::from_raw(raw))
			},
			subxt::utils::MultiSignature::Sr25519(raw) => {
				sp_runtime::MultiSignature::Sr25519(sr25519::Signature::from_raw(raw))
			},
			subxt::utils::MultiSignature::Ecdsa(raw) => {
				sp_runtime::MultiSignature::Ecdsa(ecdsa::Signature::from_raw(raw))
			},
		};
		origin_primitives::Authorization { account, payload, signature }
	}
}

fn decode_result_ss58(
	value: &subxt::dynamic::DecodedValue,
) -> Option<origin_primitives::Ss58Identifier> {
	if let ValueDef::Variant(v) = &value.value {
		if v.name == "Ok" {
			match &v.values {
				SvComposite::Unnamed(vals) => vals.get(0).and_then(decode_ss58),
				SvComposite::Named(vals) => vals.get(0).map(|(_, v)| v).and_then(decode_ss58),
			}.map(|id| return id);
		}
	}
	None
}

fn decode_ss58(value: &subxt::dynamic::DecodedValue) -> Option<origin_primitives::Ss58Identifier> {
	match &value.value {
		ValueDef::Primitive(SvPrimitive::String(s)) => {
			origin_primitives::Ss58Identifier::try_from(s.clone()).ok()
		},
		ValueDef::Composite(SvComposite::Unnamed(vals)) => {
			let mut bytes = Vec::with_capacity(vals.len());
			for v in vals {
				if let ValueDef::Primitive(SvPrimitive::U128(n)) = &v.value {
					if *n <= 255 {
						bytes.push(*n as u8);
						continue;
					}
				}
				return None;
			}
			origin_primitives::Ss58Identifier::try_from(bytes).ok()
		},
		_ => None,
	}
}
