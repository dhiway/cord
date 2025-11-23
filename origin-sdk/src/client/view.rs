use std::{
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

use super::connection::Connection;
use super::Signer;
use crate::types::error::OriginSdkError;
use crate::util::retry::RetryPolicy;
use codec::{Decode, Encode};
use origin_primitives::authorization::AuthorizationError;
use scale_value::{Composite as SvComposite, Primitive as SvPrimitive, ValueDef};
use sp_core::hashing::twox_128;
use sp_runtime::traits::SaturatedConversion;
use subxt::utils::AccountId32;

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
		(
			u32,
			subxt::view_functions::DefaultPayload<
				scale_value::Composite<()>,
				subxt::dynamic::DecodedValueThunk,
			>,
		),
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
		let output_ty = vf.output_ty();
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
		Ok((output_ty, subxt::dynamic::view_function_call(query_id, args)))
	}

	/// Call a view and decode it as `Result<Vec<u8>, AuthorizationError>`, returning the inner bytes.
	pub async fn call_auth_raw(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<Vec<u8>, OriginSdkError> {
		let (_output_ty, payload) = self.build_view_payload(pallet, function, raw_args).await?;
		let thunk = Self::call_value_inner(&self.connection, payload).await?;
		let bytes = thunk.into_encoded();

		let res: Result<Vec<u8>, AuthorizationError> =
			Decode::decode(&mut &bytes[..]).map_err(|e| OriginSdkError::Decode(e.to_string()))?;

		match res {
			Ok(raw) => Ok(raw),
			Err(e) => Err(OriginSdkError::View(format!("{pallet}.{function} err: {e:?}"))),
		}
	}

	pub async fn call<T: Decode>(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<T, OriginSdkError> {
		let backoff = RetryPolicy::default();
		let connection = self.connection.clone();
		let (_output_ty, payload) = self.build_view_payload(pallet, function, raw_args).await?;

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
		let (_output_ty, payload) = self.build_view_payload(pallet, function, raw_args).await?;
		let thunk = Self::call_value_inner(&self.connection, payload).await?;
		thunk.to_value().map_err(|e| OriginSdkError::Decode(e.to_string()))
	}

	pub async fn call_bytes(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<Vec<u8>, OriginSdkError> {
		let (_output_ty, payload) = self.build_view_payload(pallet, function, raw_args).await?;
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
		let raw = self.call_auth_raw(pallet, function, raw_args).await?;
		T::decode(&mut &raw[..]).map_err(|e| OriginSdkError::Decode(e.to_string()))
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
		let raw = self
			.inner
			.call_auth_raw(
				"Entity",
				"overview",
				vec![entity_id.encode(), Option::<u32>::None.encode()],
			)
			.await?;

		match crate::types::EntityStateView::decode(&mut &raw[..]) {
			Ok(v) => Ok(v),
			Err(_) => lenient_decode_entity_state(&raw)
				.ok_or_else(|| OriginSdkError::Decode("overview decode failed".into())),
		}
	}

	// pub async fn overview(
	// 	&self,
	// 	entity_id: origin_primitives::Ss58Identifier,
	// ) -> Result<crate::types::EntityStateView, OriginSdkError> {
	// 	match self
	// 		.inner
	// 		.call_auth_result(
	// 			"Entity",
	// 			"overview",
	// 			vec![entity_id.encode(), Option::<u32>::None.encode()],
	// 		)
	// 		.await
	// 	{
	// 		Ok(v) => Ok(v),
	// 		Err(OriginSdkError::Decode(_)) => {
	// 			// Try dynamic decoding first.
	// 			if let Ok(dv) = self
	// 				.inner
	// 				.call_value(
	// 					"Entity",
	// 					"overview",
	// 					vec![entity_id.encode(), Option::<u32>::None.encode()],
	// 				)
	// 				.await
	// 			{
	// 				if let Some(v) = decode_overview_dyn(&dv) {
	// 					return Ok(v);
	// 				}
	// 			}

	// 			// As a final fallback, leniently decode legacy layouts that used raw bytes.
	// 			let bytes = self
	// 				.inner
	// 				.call_bytes(
	// 					"Entity",
	// 					"overview",
	// 					vec![entity_id.encode(), Option::<u32>::None.encode()],
	// 				)
	// 				.await?;
	// 			lenient_decode_entity_state(bytes.as_slice())
	// 				.ok_or_else(|| OriginSdkError::Decode("overview legacy decode failed".into()))
	// 		},
	// 		Err(e) => Err(e),
	// 	}
	// }

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
		let raw = self
			.inner
			.call_auth_raw("Entity", "details", vec![entity_id.encode()])
			.await?;

		match crate::types::EntityInfoView::decode(&mut &raw[..]) {
			Ok(v) => Ok(v),
			Err(_) => lenient_decode_entity_info(&raw)
				.ok_or_else(|| OriginSdkError::Decode("details decode failed".into())),
		}
	}

	// pub async fn details(
	// 	&self,
	// 	entity_id: origin_primitives::Ss58Identifier,
	// ) -> Result<crate::types::EntityInfoView, OriginSdkError> {
	// 	match self
	// 		.inner
	// 		.call_auth_result("Entity", "details", vec![entity_id.encode()])
	// 		.await
	// 	{
	// 		Ok(v) => Ok(v),
	// 		Err(OriginSdkError::Decode(_)) => {
	// 			if let Ok(dv) =
	// 				self.inner.call_value("Entity", "details", vec![entity_id.encode()]).await
	// 			{
	// 				if let Some(v) = decode_info_dyn(&dv) {
	// 					return Ok(v);
	// 				}
	// 			}
	// 			let bytes =
	// 				self.inner.call_bytes("Entity", "details", vec![entity_id.encode()]).await?;
	// 			lenient_decode_entity_info(bytes.as_slice())
	// 				.ok_or_else(|| OriginSdkError::Decode("details legacy decode failed".into()))
	// 		},
	// 		Err(e) => Err(e),
	// 	}
	// }

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
		self.inner
			.call_auth_result("Register", "details", vec![registry.encode()])
			.await
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
		self.inner
			.call_auth_result(
				"Register",
				"packet_snapshot_by_token",
				vec![packet.encode(), version.encode()],
			)
			.await
	}

	/// Resolve a packet snapshot via lookup digest for a registry.
	pub async fn lookup(
		&self,
		registry: origin_primitives::Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.inner
			.call_auth_result(
				"Register",
				"lookup_snapshot",
				vec![registry.encode(), digest.encode(), version.encode()],
			)
			.await
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
		self.inner
			.call_auth_result(
				"Token",
				"timeline",
				vec![token.encode(), start.encode(), limit.encode()],
			)
			.await
	}

	/// Resolve a token into its decoded identifier form.
	pub async fn resolve_identifier(
		&self,
		token: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::TokenLookupView, OriginSdkError> {
		self.inner
			.call_auth_result("Token", "resolve_identifier", vec![token.encode()])
			.await
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
	use crate::types::EntityStateView;

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

#[derive(codec::Decode)]
struct LegacyInfo {
	display: Vec<u8>,
	web: Vec<u8>,
	email: Vec<u8>,
	attributes: Option<Vec<origin_primitives::AttributeValueView>>,
}

#[derive(codec::Decode)]
struct LegacyState {
	info: LegacyInfo,
	nym: Option<Vec<u8>>,
	linked_accounts: Vec<AccountId32>,
	history: Vec<origin_primitives::AttributeHistoryEntryView>,
}

fn lenient_decode_entity_info(input: &[u8]) -> Option<crate::types::EntityInfoView> {
	// Preferred: decode as Result<EntityInfoView, AuthorizationError>.
	let mut cursor = &input[..];
	if let Ok(Ok(v)) =
		Result::<crate::types::EntityInfoView, AuthorizationError>::decode(&mut cursor)
	{
		return Some(v);
	}

	// Legacy path 1: Result<Vec<u8>> wrapping an encoded EntityInfoView.
	if let Ok(Result::<Vec<u8>, AuthorizationError>::Ok(raw)) =
		codec::Decode::decode(&mut &input[..])
	{
		// If the vec is length-wrapped, strip and retry decode.
		if let Some(inner) = strip_length_wrapped(&raw) {
			let mut cur = inner;
			if let Ok(v) = crate::types::EntityInfoView::decode(&mut cur) {
				return Some(v);
			}
			if let Some(mut v) = parse_legacy_info(inner) {
				normalize_info(&mut v);
				return Some(v);
			}
		}

		let mut cursor = &raw[..];
		if let Ok(decoded) = crate::types::EntityInfoView::decode(&mut cursor) {
			return Some(decoded);
		}

		// Heuristic parse: extract ASCII slices between nulls / control bytes.
		let fields = ascii_segments(&raw);
		if !fields.is_empty() {
			let display = decode_loose_element(fields.get(0));
			let web = decode_loose_element(fields.get(1));
			let email_bytes = fields.get(2).cloned().unwrap_or_default();
			return Some(crate::types::EntityInfoView {
				display,
				web,
				email: if email_bytes.is_empty() {
					crate::types::entity::ElementView::None
				} else {
					decode_loose_element(Some(&email_bytes))
				},
				attributes: None,
			});
		}
	}

	// Legacy path 2: old struct with raw bytes.
	let inner: Result<Result<LegacyInfo, AuthorizationError>, codec::Error> =
		codec::Decode::decode(&mut &input[..]);
	let info = inner.ok()?.ok()?;
	Some(crate::types::EntityInfoView {
		display: crate::types::entity::ElementView::Raw(info.display),
		web: crate::types::entity::ElementView::Raw(info.web),
		email: crate::types::entity::ElementView::Raw(info.email),
		attributes: info.attributes,
	})
}

fn lenient_decode_entity_state(input: &[u8]) -> Option<crate::types::EntityStateView> {
	// Preferred: decode as Result<EntityStateView, AuthorizationError>.
	let mut cursor = &input[..];
	if let Ok(Ok(v)) =
		Result::<crate::types::EntityStateView, AuthorizationError>::decode(&mut cursor)
	{
		return Some(v);
	}

	// Legacy path 1: Result<Vec<u8>> wrapping an encoded EntityStateView.
	if let Ok(Result::<Vec<u8>, AuthorizationError>::Ok(raw)) =
		codec::Decode::decode(&mut &input[..])
	{
		if let Some(inner) = strip_length_wrapped(&raw) {
			let mut cur = inner;
			if let Ok(v) = crate::types::EntityStateView::decode(&mut cur) {
				return Some(v);
			}
			if let Some(mut info) = parse_legacy_info(inner) {
				normalize_info(&mut info);
				return Some(crate::types::EntityStateView {
					info,
					nym: None,
					linked_accounts: Vec::<AccountId32>::new(),
					history: Vec::new(),
				});
			}
		}

		let mut cursor = &raw[..];
		if let Ok(decoded) = crate::types::EntityStateView::decode(&mut cursor) {
			return Some(decoded);
		}

		// Heuristic fallback: split on 0x00 separators into display / web segments.
		let parts: Vec<Vec<u8>> = raw.split(|b| *b == 0u8).map(|s| s.to_vec()).collect();
		if parts.len() >= 2 {
			let strip_flag = |mut v: Vec<u8>| {
				if let Some(first) = v.first() {
					if *first <= 1 {
						v.remove(0);
					}
				}
				v
			};
			let display = strip_flag(parts[0].clone());
			let web = strip_flag(parts[1].clone());
			let info = crate::types::EntityInfoView {
				display: crate::types::entity::ElementView::Raw(display),
				web: crate::types::entity::ElementView::Raw(web),
				email: crate::types::entity::ElementView::None,
				attributes: None,
			};
			return Some(crate::types::EntityStateView {
				info,
				nym: None,
				linked_accounts: Vec::<AccountId32>::new(),
				history: Vec::new(),
			});
		}
	}

	// Legacy path 2: old struct with raw bytes.
	let inner: Result<Result<LegacyState, AuthorizationError>, codec::Error> =
		codec::Decode::decode(&mut &input[..]);
	let state = inner.ok()?.ok()?;
	let info = crate::types::EntityInfoView {
		display: crate::types::entity::ElementView::Raw(state.info.display),
		web: crate::types::entity::ElementView::Raw(state.info.web),
		email: crate::types::entity::ElementView::Raw(state.info.email),
		attributes: state.info.attributes,
	};
	Some(crate::types::EntityStateView {
		info,
		nym: state.nym,
		linked_accounts: state.linked_accounts,
		history: state.history,
	})
}

fn ascii_segments(raw: &[u8]) -> Vec<Vec<u8>> {
	raw.split(|b| *b == 0)
		.filter_map(|s| {
			let cleaned: Vec<u8> =
				s.iter().copied().filter(|b| b.is_ascii_graphic() || *b == b' ').collect();
			if cleaned.is_empty() {
				None
			} else {
				Some(cleaned)
			}
		})
		.collect()
}

fn decode_loose_element(bytes: Option<&Vec<u8>>) -> crate::types::entity::ElementView {
	let Some(buf) = bytes else {
		return crate::types::entity::ElementView::None;
	};
	if buf.is_empty() {
		return crate::types::entity::ElementView::None;
	}
	// Try strict decode first (if the buffer is an encoded ElementView).
	let mut cursor = &buf[..];
	if let Ok(ev) = crate::types::entity::ElementView::decode(&mut cursor) {
		return ev;
	}
	// If it looks like a compact length followed by data (starts with 0x41 or other),
	// treat the whole buffer as raw bytes payload.
	let raw = buf.clone();
	// Try utf8 display for convenience
	if let Ok(s) = core::str::from_utf8(&raw) {
		return crate::types::entity::ElementView::Raw(s.as_bytes().to_vec());
	}
	crate::types::entity::ElementView::Raw(raw)
}

fn strip_length_wrapped(raw: &[u8]) -> Option<&[u8]> {
	let mut cur = &raw[..];
	if let Ok(len) = codec::Compact::<u32>::decode(&mut cur) {
		if (cur.len() as u32) == len.0 {
			return Some(cur);
		}
	}
	None
}

fn parse_legacy_info(buf: &[u8]) -> Option<crate::types::EntityInfoView> {
	// The payload we see is: length-wrapped body with fields separated by 0x00, each prefixed by small tags.
	// We split on 0x00, drop empties, and take first = display, second = email, web = None.
	let parts: Vec<Vec<u8>> =
		buf.split(|b| *b == 0).filter(|s| !s.is_empty()).map(|s| s.to_vec()).collect();
	if parts.is_empty() {
		return None;
	}
	let clean = |mut v: Vec<u8>| -> Vec<u8> {
		while let Some(b) = v.first() {
			if *b <= 1 {
				v.remove(0);
				continue;
			}
			if !b.is_ascii_alphabetic() {
				v.remove(0);
				continue;
			}
			break;
		}
		v
	};
	let display = decode_loose_element(Some(&clean(parts.get(0).cloned().unwrap_or_default())));
	let email_bytes = clean(parts.get(1).cloned().unwrap_or_default());
	let email = if email_bytes.is_empty() {
		crate::types::entity::ElementView::None
	} else {
		decode_loose_element(Some(&email_bytes))
	};
	let web = if parts.len() > 2 {
		decode_loose_element(Some(&clean(parts[2].clone())))
	} else {
		crate::types::entity::ElementView::None
	};
	Some(crate::types::EntityInfoView { display, web, email, attributes: None })
}

fn normalize_info(info: &mut crate::types::EntityInfoView) {
	// If display came back None but web carried the actual value, shift it.
	if matches!(info.display, crate::types::entity::ElementView::None)
		&& !matches!(info.web, crate::types::entity::ElementView::None)
	{
		info.display = core::mem::replace(&mut info.web, crate::types::entity::ElementView::None);
	}
	// If email is missing but web looks like an email address, move it.
	if matches!(info.email, crate::types::entity::ElementView::None) {
		if let crate::types::entity::ElementView::Raw(ref bytes) = info.web {
			if bytes.contains(&b'@') {
				info.email =
					core::mem::replace(&mut info.web, crate::types::entity::ElementView::None);
			}
		}
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
