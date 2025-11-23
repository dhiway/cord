use std::{
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

use super::connection::Connection;
use super::Signer;
use crate::types::entity::ElementView;
use crate::types::error::OriginSdkError;
use crate::util::retry::RetryPolicy;
use codec::{Decode, Encode};
use origin_primitives::authorization::AuthorizationError;
use scale_value::{Composite as SvComposite, Primitive as SvPrimitive, ValueDef};
use sp_core::hashing::twox_128;
use sp_runtime::traits::SaturatedConversion;

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

	/// Build the dynamic payload for a view call, given raw SCALE-encoded args (without auth).
	async fn build_view_payload(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<
		subxt::view_functions::DefaultPayload<
			scale_value::Composite<()>,
			subxt::dynamic::DecodedValueThunk,
		>,
		OriginSdkError,
	> {
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

		// Prepend auth
		let mut args_with_auth = vec![self.auth_for(pallet, function).await?.encode()];
		args_with_auth.extend(raw_args);

		if inputs.len() != args_with_auth.len() {
			return Err(OriginSdkError::InvalidInput(format!(
				"expected {} args, got {}",
				inputs.len(),
				args_with_auth.len()
			)));
		}

		// Decode each arg into a dynamic Value using type info
		let mut values = Vec::with_capacity(args_with_auth.len());
		for (bytes, input) in args_with_auth.into_iter().zip(inputs) {
			let mut cursor = &bytes[..];
			let val = scale_value::scale::decode_as_type(&mut cursor, input.ty, metadata.types())
				.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
			values.push(val.remove_context());
		}

		let args = scale_value::Composite::unnamed(values);
		Ok(subxt::dynamic::view_function_call(query_id, args))
	}

	pub async fn call<T: Decode>(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<T, OriginSdkError> {
		let backoff = RetryPolicy::default();
		let connection = self.connection.clone();
		let payload = self.build_view_payload(pallet, function, raw_args).await?;

		backoff
			.retry(|| {
				let connection = connection.clone();
				let payload = payload.clone();
				async move {
					let thunk = Self::call_value_inner(&connection, payload).await?;
					let bytes = thunk.into_encoded();
					println!("Result Bytes (if any): {:?}", bytes);
					T::decode(&mut &bytes[..]).map_err(|e| OriginSdkError::Decode(e.to_string()))
				}
			})
			.await
	}

	pub async fn call_value(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<subxt::dynamic::DecodedValue, OriginSdkError> {
		let payload = self.build_view_payload(pallet, function, raw_args).await?;
		let thunk = Self::call_value_inner(&self.connection, payload).await?;
		thunk.to_value().map_err(|e| OriginSdkError::Decode(e.to_string()))
	}

	pub async fn call_bytes(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<Vec<u8>, OriginSdkError> {
		let payload = self.build_view_payload(pallet, function, raw_args).await?;
		let thunk = Self::call_value_inner(&self.connection, payload).await?;
		Ok(thunk.into_encoded())
	}

	/// For views that return `Result<T, AuthorizationError>`.
	pub async fn call_auth_result<T: Decode>(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<T, OriginSdkError> {
		let res: Result<Result<T, AuthorizationError>, OriginSdkError> =
			self.call(pallet, function, raw_args).await;

		match res {
			Err(e) => Err(e),
			Ok(Ok(v)) => Ok(v),
			Ok(Err(e)) => Err(OriginSdkError::View(format!("{pallet}.{function} err: {e:?}"))),
		}
	}

	async fn call_value_inner(
		connection: &Arc<Connection>,
		payload: subxt::view_functions::DefaultPayload<
			scale_value::Composite<()>,
			subxt::dynamic::DecodedValueThunk,
		>,
	) -> Result<subxt::dynamic::DecodedValueThunk, OriginSdkError> {
		let api = connection
			.online()
			.view_functions()
			.at_latest()
			.await
			.map_err(|e| OriginSdkError::View(e.to_string()))?;

		api.call(payload).await.map_err(|e| OriginSdkError::View(e.to_string()))
	}

	/// For views that return `Result<Option<T>, AuthorizationError>`.
	pub async fn call_auth_option<T: Decode>(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<Option<T>, OriginSdkError> {
		let res: Result<Result<Option<T>, AuthorizationError>, OriginSdkError> =
			self.call(pallet, function, raw_args).await;

		match res {
			Err(e) => Err(e),

			Ok(Ok(Some(v))) => Ok(Some(v)),
			Ok(Ok(None)) => Ok(None),
			Ok(Err(AuthorizationError::NotFound)) => Ok(None),

			Ok(Err(e)) => Err(OriginSdkError::View(format!("{pallet}.{function} err: {e:?}"))),
		}
	}

	// /// Invoke a pallet view function dynamically and decode via metadata.
	// pub async fn call<T: Decode>(
	// 	&self,
	// 	pallet: &str,
	// 	function: &str,
	// 	raw_args: Vec<Vec<u8>>,
	// ) -> Result<T, OriginSdkError> {
	// 	let backoff = RetryPolicy::default();
	// 	let connection = self.connection.clone();
	// 	let auth = self.auth_for(pallet, function).await?;
	// 	backoff
	// 		.retry(|| {
	// 			let connection = connection.clone();
	// 			let raw_args = raw_args.clone();
	// 			let auth = auth.clone();
	// 			async move {
	// 				let metadata = connection.metadata();
	// 				let pallet_meta = metadata.pallet_by_name(pallet).ok_or_else(|| {
	// 					OriginSdkError::View(format!("pallet {pallet} not found"))
	// 				})?;
	// 				let vf =
	// 					pallet_meta.view_functions().find(|vf| vf.name() == function).ok_or_else(
	// 						|| OriginSdkError::View(format!("view {pallet}.{function} not found")),
	// 					)?;
	// 				let query_id = *vf.query_id();
	// 				let inputs: Vec<_> = vf.inputs().collect();
	// 				let mut args_with_auth = vec![auth.encode()];
	// 				args_with_auth.extend(raw_args.clone());
	// 				if inputs.len() != args_with_auth.len() {
	// 					return Err(OriginSdkError::InvalidInput(format!(
	// 						"expected {} args, got {}",
	// 						inputs.len(),
	// 						args_with_auth.len()
	// 					)));
	// 				}
	// 				let mut values = Vec::with_capacity(args_with_auth.len());
	// 				for (bytes, input) in args_with_auth.iter().cloned().zip(inputs) {
	// 					let mut cursor = &bytes[..];
	// 					let val = scale_value::scale::decode_as_type(
	// 						&mut cursor,
	// 						input.ty,
	// 						metadata.types(),
	// 					)
	// 					.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
	// 					values.push(val.remove_context());
	// 				}
	// 				let args = scale_value::Composite::unnamed(values);
	// 				let payload = subxt::dynamic::view_function_call(query_id, args);
	// 				let thunk = Self::call_value_inner(&connection, payload).await?;
	// 				let bytes = thunk.into_encoded();
	// 				T::decode(&mut &bytes[..]).map_err(|e| OriginSdkError::Decode(e.to_string()))
	// 			}
	// 		})
	// 		.await
	// }

	// async fn call_value_inner(
	// 	connection: &Arc<Connection>,
	// 	payload: subxt::view_functions::DefaultPayload<
	// 		scale_value::Composite<()>,
	// 		subxt::dynamic::DecodedValueThunk,
	// 	>,
	// ) -> Result<subxt::dynamic::DecodedValueThunk, OriginSdkError> {
	// 	let api = connection
	// 		.online()
	// 		.view_functions()
	// 		.at_latest()
	// 		.await
	// 		.map_err(|e| OriginSdkError::View(e.to_string()))?;
	// 	api.call(payload).await.map_err(|e| OriginSdkError::View(e.to_string()))
	// }

	// /// Invoke and return raw DecodedValue (no further decode).
	// pub async fn call_value(
	// 	&self,
	// 	pallet: &str,
	// 	function: &str,
	// 	raw_args: Vec<Vec<u8>>,
	// ) -> Result<subxt::dynamic::DecodedValue, OriginSdkError> {
	// 	let metadata = self.connection.metadata();
	// 	let pallet_meta = metadata
	// 		.pallet_by_name(pallet)
	// 		.ok_or_else(|| OriginSdkError::View(format!("pallet {pallet} not found")))?;
	// 	let vf = pallet_meta
	// 		.view_functions()
	// 		.find(|vf| vf.name() == function)
	// 		.ok_or_else(|| OriginSdkError::View(format!("view {pallet}.{function} not found")))?;
	// 	let query_id = *vf.query_id();
	// 	let inputs: Vec<_> = vf.inputs().collect();
	// 	let mut args_with_auth = vec![self.auth_for(pallet, function).await?.encode()];
	// 	args_with_auth.extend(raw_args);
	// 	if inputs.len() != args_with_auth.len() {
	// 		return Err(OriginSdkError::InvalidInput(format!(
	// 			"expected {} args, got {}",
	// 			inputs.len(),
	// 			args_with_auth.len()
	// 		)));
	// 	}
	// 	let mut values = Vec::with_capacity(args_with_auth.len());
	// 	for (bytes, input) in args_with_auth.into_iter().zip(inputs) {
	// 		let mut cursor = &bytes[..];
	// 		let val = scale_value::scale::decode_as_type(&mut cursor, input.ty, metadata.types())
	// 			.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
	// 		values.push(val.remove_context());
	// 	}
	// 	let args = scale_value::Composite::unnamed(values);
	// 	let payload = subxt::dynamic::view_function_call(query_id, args);
	// 	let thunk = Self::call_value_inner(&self.connection, payload).await?;
	// 	thunk.to_value().map_err(|e| OriginSdkError::Decode(e.to_string()))
	// }

	// /// Invoke a pallet view function and return the raw SCALE-encoded bytes.
	// pub async fn call_bytes(
	// 	&self,
	// 	pallet: &str,
	// 	function: &str,
	// 	raw_args: Vec<Vec<u8>>,
	// ) -> Result<Vec<u8>, OriginSdkError> {
	// 	let metadata = self.connection.metadata();
	// 	let pallet_meta = metadata
	// 		.pallet_by_name(pallet)
	// 		.ok_or_else(|| OriginSdkError::View(format!("pallet {pallet} not found")))?;
	// 	let vf = pallet_meta
	// 		.view_functions()
	// 		.find(|vf| vf.name() == function)
	// 		.ok_or_else(|| OriginSdkError::View(format!("view {pallet}.{function} not found")))?;
	// 	let query_id = *vf.query_id();
	// 	let inputs: Vec<_> = vf.inputs().collect();
	// 	let mut args_with_auth = vec![self.auth_for(pallet, function).await?.encode()];
	// 	args_with_auth.extend(raw_args);
	// 	if inputs.len() != args_with_auth.len() {
	// 		return Err(OriginSdkError::InvalidInput(format!(
	// 			"expected {} args, got {}",
	// 			inputs.len(),
	// 			args_with_auth.len()
	// 		)));
	// 	}
	// 	let mut values = Vec::with_capacity(args_with_auth.len());
	// 	for (bytes, input) in args_with_auth.into_iter().zip(inputs) {
	// 		let mut cursor = &bytes[..];
	// 		let val = scale_value::scale::decode_as_type(&mut cursor, input.ty, metadata.types())
	// 			.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
	// 		values.push(val.remove_context());
	// 	}
	// 	let args = scale_value::Composite::unnamed(values);
	// 	let payload = subxt::dynamic::view_function_call(query_id, args);
	// 	let thunk = Self::call_value_inner(&self.connection, payload).await?;
	// 	Ok(thunk.into_encoded())
	// }

	// /// For views that return `Result<T, AuthorizationError>`.
	// pub async fn call_auth_result<T: Decode>(
	// 	&self,
	// 	pallet: &str,
	// 	function: &str,
	// 	raw_args: Vec<Vec<u8>>,
	// ) -> Result<T, OriginSdkError> {
	// 	let res: Result<Result<T, AuthorizationError>, OriginSdkError> =
	// 		self.call(pallet, function, raw_args).await;

	// 	match res {
	// 		Err(e) => Err(e), // transport / decode error

	// 		Ok(Ok(v)) => Ok(v), // happy path

	// 		Ok(Err(e)) => Err(OriginSdkError::View(format!("{pallet}.{function} err: {e:?}"))),
	// 	}
	// }

	// /// For views that return `Result<Option<T>, AuthorizationError>`.
	// pub async fn call_auth_option<T: Decode>(
	// 	&self,
	// 	pallet: &str,
	// 	function: &str,
	// 	raw_args: Vec<Vec<u8>>,
	// ) -> Result<Option<T>, OriginSdkError> {
	// 	let res: Result<Result<Option<T>, AuthorizationError>, OriginSdkError> =
	// 		self.call(pallet, function, raw_args).await;

	// 	match res {
	// 		Err(e) => Err(e),

	// 		Ok(Ok(Some(v))) => Ok(Some(v)),
	// 		Ok(Ok(None)) => Ok(None),

	// 		Ok(Err(AuthorizationError::NotFound)) => Ok(None),

	// 		Ok(Err(e)) => Err(OriginSdkError::View(format!("{pallet}.{function} err: {e:?}"))),
	// 	}
	// }

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
		self.inner
			.call_auth_result(
				"Entity",
				"overview",
				vec![entity_id.encode(), Option::<u32>::None.encode()],
			)
			.await
	}

	// pub async fn overview(
	// 	&self,
	// 	entity_id: origin_primitives::Ss58Identifier,
	// ) -> Result<crate::types::EntityStateView, OriginSdkError> {
	// 	let res: Result<Result<crate::types::EntityStateView, AuthorizationError>, OriginSdkError> =
	// 		self.inner
	// 			.call_auth_result(
	// 				"Entity",
	// 				"overview",
	// 				vec![entity_id.encode(), Option::<u32>::None.encode()],
	// 			)
	// 			.await;
	// self.inner
	// 	.call("Entity", "overview", vec![entity_id.encode(), Option::<u32>::None.encode()])
	// 	.await;
	// match res {
	// 	Ok(Ok(v)) => Ok(v),
	// 	Ok(Err(e)) => Err(OriginSdkError::View(format!("overview err: {e:?}"))),
	// 	Err(OriginSdkError::Decode(_)) => {
	// 		// Fallback to dynamic decoding to tolerate ElementView variant drift.
	// 		let dv = self
	// 			.inner
	// 			.call_value(
	// 				"Entity",
	// 				"overview",
	// 				vec![entity_id.encode(), Option::<u32>::None.encode()],
	// 			)
	// 			.await?;
	// 		decode_overview_dyn(&dv).ok_or_else(|| {
	// 			OriginSdkError::Decode("overview fallback dynamic decode failed".into())
	// 		})
	// 	},
	// 	Err(e) => Err(e),
	// }
	// }

	pub async fn account_token(
		&self,
		account: subxt::utils::AccountId32,
	) -> Result<Option<origin_primitives::Ss58Identifier>, OriginSdkError> {
		// Decode as Result<Vec<u8>, AuthorizationError> then convert to Ss58Identifier.
		let res: Result<Result<Vec<u8>, AuthorizationError>, OriginSdkError> =
			self.inner.call("Entity", "account_token", vec![account.encode()]).await;

		match res {
			Err(e) => Err(e),
			Ok(Ok(raw)) => match origin_primitives::Ss58Identifier::try_from(raw) {
				Ok(id) => Ok(Some(id)),
				Err(e) => Err(OriginSdkError::Decode(format!("{e:?}"))),
			},
			Ok(Err(AuthorizationError::NotFound)) => Ok(None),
			Ok(Err(e)) => Err(OriginSdkError::View(format!("account_token err: {e:?}"))),
		}
	}

	pub async fn details(
		&self,
		entity_id: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::EntityInfoView, OriginSdkError> {
		self.inner.call("Entity", "details", vec![entity_id.encode()]).await
	}

	pub async fn nym(
		&self,
		entity_id: origin_primitives::Ss58Identifier,
	) -> Result<Option<Vec<u8>>, OriginSdkError> {
		let bytes = self.inner.call_bytes("Entity", "entity_nym", vec![entity_id.encode()]).await?;
		let res: Result<Vec<u8>, AuthorizationError> =
			Decode::decode(&mut &bytes[..]).map_err(|e| OriginSdkError::Decode(e.to_string()))?;
		match res {
			Ok(nym) => Ok(Some(nym)),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(e) => Err(OriginSdkError::View(format!("entity_nym err: {e:?}"))),
		}
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
	/// Packet snapshot by token (optionally at a specific version).
	pub async fn state(
		&self,
		packet: origin_primitives::PacketPointer,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		let res: Result<Result<crate::types::PacketStateView, AuthorizationError>, OriginSdkError> =
			self.inner
				.call(
					"Register",
					"packet_snapshot_by_token",
					vec![packet.encode(), version.encode()],
				)
				.await;
		match res {
			Ok(Ok(v)) => Ok(v),
			Ok(Err(e)) => Err(OriginSdkError::View(format!("packet state err: {e:?}"))),
			Err(e) => Err(e),
		}
	}

	/// Resolve a packet snapshot via lookup digest for a registry.
	pub async fn lookup(
		&self,
		registry: origin_primitives::Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		let res: Result<Result<crate::types::PacketStateView, AuthorizationError>, OriginSdkError> =
			self.inner
				.call(
					"Register",
					"lookup_snapshot",
					vec![registry.encode(), digest.encode(), version.encode()],
				)
				.await;
		match res {
			Ok(Ok(v)) => Ok(v),
			Ok(Err(e)) => Err(OriginSdkError::View(format!("packet lookup err: {e:?}"))),
			Err(e) => Err(e),
		}
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
		start: Option<u32>,
		limit: Option<u32>,
	) -> Result<crate::types::TokenTimelineView, OriginSdkError> {
		let res: Result<
			Result<crate::types::TokenTimelineView, AuthorizationError>,
			OriginSdkError,
		> = self
			.inner
			.call("Token", "timeline", vec![token.encode(), start.encode(), limit.encode()])
			.await;
		match res {
			Ok(Ok(v)) => Ok(v),
			Ok(Err(e)) => Err(OriginSdkError::View(format!("timeline err: {e:?}"))),
			Err(e) => Err(e),
		}
	}

	/// Resolve a token into its decoded identifier form.
	pub async fn resolve_identifier(
		&self,
		token: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::TokenLookupView, OriginSdkError> {
		let res: Result<Result<crate::types::TokenLookupView, AuthorizationError>, OriginSdkError> =
			self.inner.call("Token", "resolve_identifier", vec![token.encode()]).await;
		match res {
			Ok(Ok(id)) => Ok(id),
			Ok(Err(e)) => Err(OriginSdkError::View(format!("resolve_identifier err: {e:?}"))),
			Err(e) => Err(e),
		}
	}
}

impl ViewClient {
	async fn auth_for(&self, pallet: &str, function: &str) -> Result<Auth, OriginSdkError> {
		let reference_block = self
			.connection
			.online()
			.blocks()
			.at_latest()
			.await
			.map_err(|e| OriginSdkError::View(e.to_string()))?
			.number()
			.saturated_into::<u32>();
		let account = self.signer.account_id();
		let payload = build_view_payload(&account, pallet, function, reference_block);
		let signature = self.signer.sign_payload(&payload).await;
		let account = origin_primitives::AccountId::from(account.0);
		Ok(origin_primitives::Authorization { account, payload, signature })
	}
}

/// Construct a view-authorization payload that matches the pallet-side expectations:
///   payload = twox_128(nonce || pallet || "::" || function || account || reference_block)
///           || account
///           || reference_block
/// The trailing `reference_block` (u32 LE) is required for TTL checks in the pallets.
fn build_view_payload(
	account: &subxt::utils::AccountId32,
	pallet: &str,
	function: &str,
	reference_block: u32,
) -> Vec<u8> {
	let nonce = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_nanos()
		.to_le_bytes();
	let account_bytes = account.encode();
	let mut preimage = Vec::with_capacity(
		nonce
			.len()
			.saturating_add(pallet.len())
			.saturating_add(function.len())
			.saturating_add(account_bytes.len())
			.saturating_add(core::mem::size_of::<u32>())
			.saturating_add(2),
	);
	preimage.extend_from_slice(&nonce);
	preimage.extend_from_slice(pallet.as_bytes());
	preimage.extend_from_slice(b"::");
	preimage.extend_from_slice(function.as_bytes());
	preimage.extend_from_slice(&account_bytes);
	preimage.extend_from_slice(&reference_block.to_le_bytes());

	let digest = twox_128(&preimage);
	let mut payload = Vec::with_capacity(digest.len() + account_bytes.len() + 4);
	payload.extend_from_slice(&digest);
	payload.extend_from_slice(&account_bytes);
	payload.extend_from_slice(&reference_block.to_le_bytes());
	payload
}

#[allow(dead_code)]
fn decode_result_ss58(
	value: &subxt::dynamic::DecodedValue,
) -> Option<origin_primitives::Ss58Identifier> {
	if let ValueDef::Variant(v) = &value.value {
		if v.name == "Ok" {
			match &v.values {
				SvComposite::Unnamed(vals) => vals.get(0).and_then(decode_ss58),
				SvComposite::Named(vals) => vals.get(0).map(|(_, v)| v).and_then(decode_ss58),
			}
			.map(|id| return id);
		}
	}
	None
}

#[allow(dead_code)]
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

fn decode_overview_dyn(dv: &subxt::dynamic::DecodedValue) -> Option<crate::types::EntityStateView> {
	use crate::types::entity::ElementView;
	use crate::types::{EntityInfoView, EntityStateView};

	let obj = match &dv.value {
		ValueDef::Composite(SvComposite::Named(fields)) => fields,
		_ => return None,
	};
	let info_val = obj.iter().find(|(k, _)| k == "info")?.1.clone();
	let info = decode_info_dyn(&info_val)?;
	let nym = obj.iter().find(|(k, _)| k == "nym").and_then(|(_, v)| match &v.value {
		ValueDef::Primitive(SvPrimitive::String(s)) => Some(s.as_bytes().to_vec()),
		ValueDef::Primitive(SvPrimitive::U128(n)) if *n <= 255 => Some(vec![*n as u8]),
		ValueDef::Composite(SvComposite::Unnamed(vals)) => bytes_from_values(vals),
		_ => None,
	});
	let linked_accounts = Vec::new(); // skip for tolerance
	let history = Vec::new(); // skip for tolerance
	Some(EntityStateView { info, nym, linked_accounts, history })
}

fn decode_info_dyn(v: &subxt::dynamic::DecodedValue) -> Option<crate::types::EntityInfoView> {
	use crate::types::entity::ElementView;
	let fields = match &v.value {
		ValueDef::Composite(SvComposite::Named(fields)) => fields,
		_ => return None,
	};
	let f = |name: &str| {
		fields
			.iter()
			.find(|(k, _)| k == name)
			.and_then(|(_, v)| decode_element_dyn(v))
			.unwrap_or(ElementView::Raw(Vec::new()))
	};
	Some(crate::types::EntityInfoView {
		display: f("display"),
		web: f("web"),
		email: f("email"),
		attributes: None,
	})
}

fn decode_element_dyn(
	v: &subxt::dynamic::DecodedValue,
) -> Option<crate::types::entity::ElementView> {
	use crate::types::entity::ElementView;
	match &v.value {
		ValueDef::Variant(var) => {
			let arg0 = match &var.values {
				SvComposite::Unnamed(vals) => vals.get(0),
				SvComposite::Named(fields) => fields.get(0).map(|(_, v)| v),
			};
			match var.name.as_str() {
				"None" => Some(ElementView::None),
				"Raw" => arg0.and_then(|v| bytes_from_value(v)).map(ElementView::Raw),
				"Bool" => arg0
					.and_then(|v| match &v.value {
						ValueDef::Primitive(SvPrimitive::Bool(b)) => Some(*b),
						ValueDef::Primitive(SvPrimitive::U128(n)) => Some(*n != 0),
						_ => None,
					})
					.map(ElementView::Bool),
				"U64" => arg0
					.and_then(|v| match &v.value {
						ValueDef::Primitive(SvPrimitive::U128(n)) => Some(*n as u64),
						_ => None,
					})
					.map(ElementView::U64),
				"U128" => arg0
					.and_then(|v| match &v.value {
						ValueDef::Primitive(SvPrimitive::U128(n)) => Some(*n),
						_ => None,
					})
					.map(ElementView::U128),
				"Hash" => arg0
					.and_then(bytes_from_value)
					.and_then(|b| b.try_into().ok())
					.map(ElementView::Hash),
				"Token" => arg0.and_then(decode_ss58).map(ElementView::Token),
				"CID" => arg0.and_then(bytes_from_value).map(ElementView::Cid),
				_ => Some(ElementView::Raw(Vec::new())),
			}
		},
		ValueDef::Primitive(SvPrimitive::String(s)) => {
			Some(ElementView::Raw(s.clone().into_bytes()))
		},
		_ => None,
	}
}

fn bytes_from_value(v: &subxt::dynamic::DecodedValue) -> Option<Vec<u8>> {
	match &v.value {
		ValueDef::Primitive(SvPrimitive::String(s)) => Some(s.as_bytes().to_vec()),
		ValueDef::Composite(SvComposite::Unnamed(vals)) => bytes_from_values(vals),
		ValueDef::Primitive(SvPrimitive::U128(n)) if *n <= 255 => Some(vec![*n as u8]),
		_ => None,
	}
}

fn bytes_from_values(vals: &[subxt::dynamic::DecodedValue]) -> Option<Vec<u8>> {
	let mut out = Vec::with_capacity(vals.len());
	for v in vals {
		if let ValueDef::Primitive(SvPrimitive::U128(n)) = &v.value {
			if *n <= 255 {
				out.push(*n as u8);
				continue;
			}
		}
		return None;
	}
	Some(out)
}
