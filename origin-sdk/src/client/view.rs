use std::sync::Arc;

use super::{connection::Connection, Signer};
use crate::types::{self, auth, error::OriginSdkError};
use codec::{Decode, Encode};
use origin_primitives::authorization::AuthorizationError;
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

	/// Call a view and return the raw encoded outer result bytes.
	async fn call_view_raw_bytes(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<Vec<u8>, OriginSdkError> {
		let (_output_ty, payload) = self.build_view_payload(pallet, function, raw_args).await?;
		let thunk = Self::call_value_inner(&self.connection, payload).await?;
		Ok(thunk.into_encoded())
	}

	#[allow(dead_code)]
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

	#[allow(dead_code)]
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

	/// For views returning `Result<Vec<u8>, AuthorizationError>`; decode inner as `T`.
	pub async fn call_auth_result<T: Decode>(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<T, OriginSdkError> {
		let bytes = self.call_view_raw_bytes(pallet, function, raw_args).await?;
		let outer: Result<Vec<u8>, AuthorizationError> =
			Decode::decode(&mut &bytes[..]).map_err(|e| {
				OriginSdkError::Decode(format!("{pallet}.{function} outer decode: {e}"))
			})?;
		match outer {
			Ok(inner) => T::decode(&mut &inner[..]).map_err(|e| {
				OriginSdkError::Decode(format!("{pallet}.{function} inner decode: {e}"))
			}),
			Err(e) => Err(OriginSdkError::ViewAuth(e)),
		}
	}

	/// Same as call_auth_result but maps NotFound -> Ok(None).
	pub async fn call_auth_result_maybe<T: Decode>(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<Option<T>, OriginSdkError> {
		let bytes = self.call_view_raw_bytes(pallet, function, raw_args).await?;
		let outer: Result<Vec<u8>, AuthorizationError> =
			Decode::decode(&mut &bytes[..]).map_err(|e| {
				OriginSdkError::Decode(format!("{pallet}.{function} outer decode: {e}"))
			})?;

		println!("outer result is {:?}", outer);
		match outer {
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(e) => Err(OriginSdkError::ViewAuth(e)),
			Ok(inner) => T::decode(&mut &inner[..]).map(Some).map_err(|e| {
				OriginSdkError::Decode(format!("{pallet}.{function} inner decode: {e}"))
			}),
		}
	}

	/// For views returning `Result<Vec<u8>, AuthorizationError>` where inner is *raw bytes*.
	pub async fn call_auth_result_maybe_raw(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<Option<Vec<u8>>, OriginSdkError> {
		let bytes = self.call_view_raw_bytes(pallet, function, raw_args).await?;
		let outer: Result<Vec<u8>, AuthorizationError> =
			Decode::decode(&mut &bytes[..]).map_err(|e| {
				OriginSdkError::Decode(format!("{pallet}.{function} outer decode: {e}"))
			})?;

		println!("{pallet}.{function} outer result is {:?}", outer);

		match outer {
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(e) => Err(OriginSdkError::ViewAuth(e)),
			Ok(inner) => Ok(Some(inner)), // no inner T::decode here
		}
	}

	/// For views returning `Result<Option<T>, AuthorizationError>` where T is already decoded.
	pub async fn call_auth_option_decoded<T: Decode>(
		&self,
		pallet: &str,
		function: &str,
		raw_args: Vec<Vec<u8>>,
	) -> Result<Option<T>, OriginSdkError> {
		let bytes = self.call_view_raw_bytes(pallet, function, raw_args).await?;
		let outer: Result<Option<T>, AuthorizationError> = Decode::decode(&mut &bytes[..])
			.map_err(|e| {
				OriginSdkError::Decode(format!("{pallet}.{function} outer decode: {e}"))
			})?;
		match outer {
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(e) => Err(OriginSdkError::ViewAuth(e)),
			Ok(opt) => Ok(opt),
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

	/// Token view helpers.
	pub fn token(&self) -> TokenViews {
		TokenViews { inner: self.clone() }
	}
}

#[derive(Clone)]
pub struct EntityViews {
	pub(crate) inner: ViewClient,
}

impl EntityViews {
	/// Return `Ok(None)` when the entity is not found.
	pub async fn maybe_overview(
		&self,
		entity_id: types::EntityToken,
	) -> Result<Option<types::EntityStateViewSdk>, OriginSdkError> {
		self.inner
			.call_auth_result_maybe::<types::EntityStateViewSdk>(
				"Entity",
				"overview",
				vec![entity_id.encode(), Option::<u32>::None.encode()],
			)
			.await
	}

	pub async fn overview(
		&self,
		entity_id: types::EntityToken,
	) -> Result<types::EntityStateViewSdk, OriginSdkError> {
		self.inner
			.call_auth_result::<types::EntityStateViewSdk>(
				"Entity",
				"overview",
				vec![entity_id.encode(), Option::<u32>::None.encode()],
			)
			.await
	}

	/// Resolve the entity token linked to an account.
	/// - Ok(None)    if no mapping exists (AuthorizationError::NotFound or sentinel [1,1])
	/// - Ok(Some(..)) for a valid EntityToken
	pub async fn account_token(
		&self,
		account: types::OriginAccountId,
	) -> Result<Option<types::EntityToken>, OriginSdkError> {
		// 1. Get the raw SCALE bytes from the view
		let bytes = self
			.inner
			.call_view_raw_bytes("Entity", "account_token", vec![account.encode()])
			.await?;

		// 2. Decode outer Result<Vec<u8>, AuthorizationError>
		let outer: Result<Vec<u8>, AuthorizationError> =
			Decode::decode(&mut &bytes[..]).map_err(|e| {
				OriginSdkError::Decode(format!("Entity.account_token outer decode: {e}"))
			})?;

		println!("Entity.account_token outer result is {:?}", outer);

		match outer {
			// Standard "no mapping" case from the pallet
			Err(AuthorizationError::NotFound) => Ok(None),

			// For other auth errors, bubble up
			Err(e) => Err(OriginSdkError::ViewAuth(e)),

			Ok(inner) => {
				// IMPORTANT: the inner Vec<u8> = [1,1] is a sentinel meaning "NotFound"
				// in this older/quirky view design. Treat it as None, not as an identifier.
				if inner == [1, 1] {
					return Ok(None);
				}

				// Otherwise, interpret the inner bytes as a SS58 string and build EntityToken
				match types::EntityToken::try_from(inner) {
					Ok(id) => Ok(Some(id)),
					Err(e) => Err(OriginSdkError::Decode(format!(
						"Entity.account_token: invalid Ss58Identifier bytes: {e:?}"
					))),
				}
			},
		}
	}

	// pub async fn account_token(
	// 	&self,
	// 	account: types::OriginAccountId,
	// ) -> Result<Option<types::EntityToken>, OriginSdkError> {
	// 	self.inner
	// 		.call_auth_result_maybe::<types::EntityToken>(
	// 			"Entity",
	// 			"account_token",
	// 			vec![account.encode()],
	// 		)
	// 		.await
	// }

	pub async fn details(
		&self,
		entity_id: types::EntityToken,
	) -> Result<types::EntityInfoViewSdk, OriginSdkError> {
		self.inner
			.call_auth_result::<types::EntityInfoViewSdk>(
				"Entity",
				"details",
				vec![entity_id.encode()],
			)
			.await
	}

	pub async fn maybe_details(
		&self,
		entity_id: types::EntityToken,
	) -> Result<Option<types::EntityInfoViewSdk>, OriginSdkError> {
		self.inner
			.call_auth_result_maybe::<types::EntityInfoViewSdk>(
				"Entity",
				"details",
				vec![entity_id.encode()],
			)
			.await
	}

	pub async fn nym(
		&self,
		entity_id: types::EntityToken,
	) -> Result<Option<types::EntityNym>, OriginSdkError> {
		self.inner
			.call_auth_result_maybe::<types::EntityNym>(
				"Entity",
				"entity_nym",
				vec![entity_id.encode()],
			)
			.await
	}

	pub async fn linked_accounts(
		&self,
		entity_id: types::EntityToken,
	) -> Result<Vec<types::OriginAccountId>, OriginSdkError> {
		self.inner
			.call_auth_result("Entity", "linked_accounts", vec![entity_id.encode()])
			.await
	}

	pub async fn controller_account(
		&self,
		entity_id: types::EntityToken,
	) -> Result<types::OriginAccountId, OriginSdkError> {
		self.inner
			.call_auth_result("Entity", "controller_account", vec![entity_id.encode()])
			.await
	}

	pub async fn account_history(
		&self,
		entity_id: types::EntityToken,
	) -> Result<Vec<types::AccountUnbindEntryViewSdk>, OriginSdkError> {
		self.inner
			.call_auth_result("Entity", "account_history", vec![entity_id.encode()])
			.await
	}

	pub async fn attribute_version(
		&self,
		entity_id: types::EntityToken,
		key: Vec<u8>,
	) -> Result<u64, OriginSdkError> {
		self.inner
			.call_auth_result("Entity", "attribute_version", vec![entity_id.encode(), key.encode()])
			.await
	}

	pub async fn attribute_versions(
		&self,
		entity_id: types::EntityToken,
	) -> Result<Vec<(Vec<u8>, u64)>, OriginSdkError> {
		self.inner
			.call_auth_result("Entity", "attribute_versions", vec![entity_id.encode()])
			.await
	}

	pub async fn attribute_history(
		&self,
		entity_id: types::EntityToken,
	) -> Result<Vec<types::AttributeHistoryEntryViewSdk>, OriginSdkError> {
		self.inner
			.call_auth_result("Entity", "attribute_history", vec![entity_id.encode()])
			.await
	}

	pub async fn attribute_history_for_key(
		&self,
		entity_id: types::EntityToken,
		key: Vec<u8>,
	) -> Result<Vec<types::AttributeHistoryEntryViewSdk>, OriginSdkError> {
		self.inner
			.call_auth_result(
				"Entity",
				"attribute_history_for_key",
				vec![entity_id.encode(), key.encode()],
			)
			.await
	}

	pub async fn attribute_history_entry(
		&self,
		entity_id: types::EntityToken,
		key: Vec<u8>,
		version: u64,
	) -> Result<types::AttributeHistoryEntryViewSdk, OriginSdkError> {
		self.inner
			.call_auth_result(
				"Entity",
				"attribute_history_entry",
				vec![entity_id.encode(), key.encode(), version.encode()],
			)
			.await
	}
}

#[derive(Clone)]
pub struct RegistryViews {
	pub(crate) inner: ViewClient,
}

impl RegistryViews {
	/// Return `Ok(None)` when the registry is not found.
	#[allow(dead_code)]
	pub async fn maybe_details(
		&self,
		registry: origin_primitives::Ss58Identifier,
	) -> Result<Option<crate::types::RegistryStateView>, OriginSdkError> {
		self.inner
			.call_auth_result_maybe::<crate::types::RegistryStateView>(
				"Register",
				"details",
				vec![registry.encode()],
			)
			.await
	}

	#[allow(dead_code)]
	pub async fn maybe_overview(
		&self,
		registry: origin_primitives::Ss58Identifier,
	) -> Result<Option<crate::types::RegistryStateView>, OriginSdkError> {
		self.inner
			.call_auth_result_maybe::<crate::types::RegistryStateView>(
				"Register",
				"overview",
				vec![registry.encode()],
			)
			.await
	}

	#[allow(dead_code)]
	pub async fn maybe_attribute(
		&self,
		registry: origin_primitives::Ss58Identifier,
		key: Vec<u8>,
	) -> Result<Option<(origin_primitives::element::ElementType, bool)>, OriginSdkError> {
		self.inner
			.call_auth_result_maybe::<(origin_primitives::element::ElementType, bool)>(
				"Register",
				"attribute",
				vec![registry.encode(), key.encode()],
			)
			.await
	}

	#[allow(dead_code)]
	pub async fn maybe_packet_metadata(
		&self,
		registry: origin_primitives::Ss58Identifier,
		packet: origin_primitives::Ss58Identifier,
	) -> Result<Option<origin_primitives::packet::PacketMetadataView>, OriginSdkError> {
		self.inner
			.call_auth_option_decoded::<origin_primitives::packet::PacketMetadataView>(
				"Register",
				"packet_metadata",
				vec![registry.encode(), packet.encode()],
			)
			.await
	}

	#[allow(dead_code)]
	pub async fn maybe_packet_snapshot(
		&self,
		registry: origin_primitives::Ss58Identifier,
		packet: origin_primitives::Ss58Identifier,
		version: Option<u32>,
	) -> Result<Option<crate::types::PacketStateView>, OriginSdkError> {
		self.inner
			.call_auth_option_decoded::<crate::types::PacketStateView>(
				"Register",
				"packet_snapshot",
				vec![registry.encode(), packet.encode(), version.encode()],
			)
			.await
	}

	pub async fn maybe_packet_snapshot_by_token(
		&self,
		token: origin_primitives::Ss58Identifier,
		version: Option<u32>,
	) -> Result<Option<crate::types::PacketStateView>, OriginSdkError> {
		self.inner
			.call_auth_option_decoded::<crate::types::PacketStateView>(
				"Register",
				"packet_snapshot_by_token",
				vec![token.encode(), version.encode()],
			)
			.await
	}

	#[allow(dead_code)]
	pub async fn maybe_lookup_snapshot(
		&self,
		registry: origin_primitives::Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<Option<crate::types::PacketStateView>, OriginSdkError> {
		self.inner
			.call_auth_option_decoded::<crate::types::PacketStateView>(
				"Register",
				"lookup_snapshot",
				vec![registry.encode(), digest.encode(), version.encode()],
			)
			.await
	}

	pub async fn details(
		&self,
		registry: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::RegistryStateView, OriginSdkError> {
		self.inner
			.call_auth_result("Register", "details", vec![registry.encode()])
			.await
	}

	pub async fn delegate_permissions(
		&self,
		registry: origin_primitives::Ss58Identifier,
		delegate: origin_primitives::Ss58Identifier,
	) -> Result<origin_primitives::registry::RegistryPermissions, OriginSdkError> {
		self.inner
			.call_auth_result(
				"Register",
				"delegate_permissions",
				vec![registry.encode(), delegate.encode()],
			)
			.await
	}

	pub async fn query_count(
		&self,
		registry: origin_primitives::Ss58Identifier,
		account: subxt::utils::AccountId32,
	) -> Result<u32, OriginSdkError> {
		self.inner
			.call_auth_result("Register", "query_count", vec![registry.encode(), account.encode()])
			.await
	}

	pub async fn lookup_specs(
		&self,
		registry: origin_primitives::Ss58Identifier,
	) -> Result<Vec<origin_primitives::registry::LookupSpec>, OriginSdkError> {
		self.inner
			.call_auth_result("Register", "lookup_specs", vec![registry.encode()])
			.await
	}

	pub async fn attribute(
		&self,
		registry: origin_primitives::Ss58Identifier,
		key: Vec<u8>,
	) -> Result<(origin_primitives::element::ElementType, bool), OriginSdkError> {
		self.inner
			.call_auth_result("Register", "attribute", vec![registry.encode(), key.encode()])
			.await
	}

	pub async fn attributes(
		&self,
		registry: origin_primitives::Ss58Identifier,
	) -> Result<Vec<origin_primitives::registry::RegistryAttributeView>, OriginSdkError> {
		self.inner
			.call_auth_result("Register", "attributes", vec![registry.encode()])
			.await
	}

	pub async fn token_specs(
		&self,
		registry: origin_primitives::Ss58Identifier,
	) -> Result<Vec<Vec<u8>>, OriginSdkError> {
		self.inner
			.call_auth_result("Register", "token_specs", vec![registry.encode()])
			.await
	}

	pub async fn packet_snapshot(
		&self,
		registry: origin_primitives::Ss58Identifier,
		packet: origin_primitives::Ss58Identifier,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.inner
			.call_auth_result(
				"Register",
				"packet_snapshot",
				vec![registry.encode(), packet.encode(), version.encode()],
			)
			.await
	}

	pub async fn packet_metadata(
		&self,
		registry: origin_primitives::Ss58Identifier,
		packet: origin_primitives::Ss58Identifier,
	) -> Result<origin_primitives::packet::PacketMetadataView, OriginSdkError> {
		self.inner
			.call_auth_result(
				"Register",
				"packet_metadata",
				vec![registry.encode(), packet.encode()],
			)
			.await
	}

	pub async fn overview(
		&self,
		registry: origin_primitives::Ss58Identifier,
	) -> Result<crate::types::RegistryStateView, OriginSdkError> {
		self.inner
			.call_auth_result("Register", "overview", vec![registry.encode()])
			.await
	}

	pub async fn packet_snapshot_by_token(
		&self,
		token: origin_primitives::Ss58Identifier,
		version: Option<u32>,
	) -> Result<Option<crate::types::PacketStateView>, OriginSdkError> {
		self.maybe_packet_snapshot_by_token(token, version).await
	}

	pub async fn lookup_snapshot(
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

	pub async fn list_by_token(
		&self,
		prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<origin_primitives::Ss58Identifier>,
		limit: Option<u32>,
	) -> Result<
		(Vec<crate::types::PacketStateView>, Option<origin_primitives::Ss58Identifier>),
		OriginSdkError,
	> {
		self.inner
			.call_auth_result(
				"Register",
				"list_by_token",
				vec![prefix.encode(), version.encode(), cursor.encode(), limit.encode()],
			)
			.await
	}

	pub async fn list_by_digest(
		&self,
		digest_prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<Vec<u8>>,
		limit: Option<u32>,
	) -> Result<(Vec<crate::types::PacketStateView>, Option<Vec<u8>>), OriginSdkError> {
		self.inner
			.call_auth_result(
				"Register",
				"list_by_digest",
				vec![digest_prefix.encode(), version.encode(), cursor.encode(), limit.encode()],
			)
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
			.call_auth_result::<Option<crate::types::PacketStateView>>(
				"Register",
				"packet_snapshot_by_token",
				vec![packet.encode(), version.encode()],
			)
			.await?
			.ok_or_else(|| OriginSdkError::View("packet snapshot not found".into()))
	}

	/// Resolve a packet snapshot via lookup digest for a registry.
	#[allow(dead_code)]
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
	pub async fn maybe_state_event(
		&self,
		token: origin_primitives::Ss58Identifier,
		version: u32,
	) -> Result<
		Option<origin_primitives::token::TokenStateEventView<subxt::utils::H256>>,
		OriginSdkError,
	> {
		self.inner
			.call_auth_option_decoded::<origin_primitives::token::TokenStateEventView<subxt::utils::H256>>(
				"Token",
				"state_event",
				vec![token.encode(), version.encode()],
			)
			.await
	}

	pub async fn maybe_resolve_identifier(
		&self,
		token: origin_primitives::Ss58Identifier,
	) -> Result<Option<crate::types::TokenLookupView>, OriginSdkError> {
		self.inner
			.call_auth_result_maybe::<Vec<u8>>("Token", "resolve_identifier", vec![token.encode()])
			.await
			.and_then(|opt| {
				opt.map(|raw| {
					crate::types::TokenLookupView::decode(&mut &raw[..])
						.map_err(|e| OriginSdkError::Decode(e.to_string()))
				})
				.transpose()
			})
	}

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

	pub async fn pallet_index_of(&self, name: Vec<u8>) -> Result<u16, OriginSdkError> {
		self.inner
			.call_auth_result("Token", "pallet_index_of", vec![name.encode()])
			.await
	}

	pub async fn pallet_name(&self, index: u16) -> Result<String, OriginSdkError> {
		self.inner.call_auth_result("Token", "pallet_name", vec![index.encode()]).await
	}

	pub async fn next_pallet_index(&self) -> Result<u16, OriginSdkError> {
		self.inner.call_auth_result("Token", "next_pallet_index", vec![]).await
	}

	pub async fn genesis_network_id(&self) -> Result<u32, OriginSdkError> {
		self.inner.call_auth_result("Token", "genesis_network_id", vec![]).await
	}

	pub async fn state_version(
		&self,
		token: origin_primitives::Ss58Identifier,
	) -> Result<u32, OriginSdkError> {
		self.inner
			.call_auth_result("Token", "state_version", vec![token.encode()])
			.await
	}

	pub async fn state_event(
		&self,
		token: origin_primitives::Ss58Identifier,
		version: u32,
	) -> Result<origin_primitives::token::TokenStateEventView<subxt::utils::H256>, OriginSdkError>
	{
		self.inner
			.call_auth_result("Token", "state_event", vec![token.encode(), version.encode()])
			.await
	}

	pub async fn resolve_pallet(&self, index: u16) -> Result<Vec<u8>, OriginSdkError> {
		self.inner
			.call_auth_result("Token", "resolve_pallet", vec![index.encode()])
			.await
	}
}

impl ViewClient {
	async fn auth_for(&self, pallet: &str, function: &str) -> Result<Auth, OriginSdkError> {
		let signer = self.signer.clone();
		let reference_block = self
			.connection
			.online()
			.blocks()
			.at_latest()
			.await
			.map_err(|e| OriginSdkError::View(e.to_string()))?
			.number()
			.saturated_into::<u32>();
		Ok(auth::build_authorization(signer.as_ref(), pallet, function, reference_block).await)
	}
}

// Legacy decoding helpers removed; SDK expects current pallet encodings only.
