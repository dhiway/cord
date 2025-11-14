pub mod auth;
pub mod dynamic;
pub mod entity;
pub mod register;
pub mod token;

use crate::{
	client::Client,
	error::{Error, Result},
	types,
};
use codec::Decode;
use cord_primitives::{identifier::Ss58Identifier, view_api::{AuthorizationError, AuthorizationRequest}};
use hex::ToHex;
use scale_decode::DecodeAsType;
use scale_value::{Composite, Value};
use sp_runtime::MultiSignature;
use subxt::dynamic::DecodedValueThunk;

/// Facade for runtime query functions.
pub struct Query<'a> {
	pub(crate) client: &'a Client,
}

impl<'a> Query<'a> {
	pub fn register(&self) -> register::RegisterQuery<'_> {
		register::RegisterQuery { query: self }
	}

	pub fn entity(&self) -> entity::EntityQuery<'_> {
		entity::EntityQuery { query: self }
	}

	pub fn token(&self) -> token::TokenQuery<'_> {
		token::TokenQuery { query: self }
	}

	pub(crate) async fn call_view_as<T: DecodeAsType + 'static>(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<T> {
		let view_bytes = self.call_view_bytes(pallet, function, args).await?;
		crate::scale::decode::decode_with_metadata(
			&view_bytes.metadata,
			view_bytes.type_id,
			&view_bytes.data,
		)
	}

	pub(crate) async fn call_result<T, E>(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<core::result::Result<T, E>>
	where
		T: DecodeAsType + 'static,
		E: DecodeAsType + 'static,
	{
		self.call_view_as::<core::result::Result<T, E>>(pallet, function, args).await
	}
	pub(crate) async fn call_view_bytes(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<ViewBytes> {
		let dispatch = dynamic::call_view(self.client, pallet, function, args).await?;
		let data = extract_view_payload(dispatch.thunk)?;
		Ok(ViewBytes { data, type_id: dispatch.output_ty, metadata: dispatch.metadata })
	}
}

pub(crate) struct ViewBytes {
	data: Vec<u8>,
	type_id: u32,
	metadata: subxt::Metadata,
}

#[derive(Default)]
pub struct ArgBuilder {
	entries: Vec<(&'static str, Value)>,
}

impl ArgBuilder {
	pub fn push(&mut self, name: &'static str, value: Value) -> &mut Self {
		self.entries.push((name, value));
		self
	}

	pub fn finish(self) -> Value {
		Value::named_composite(self.entries)
	}
}

pub(crate) fn authorization_value(authz: &AuthorizationRequest) -> Result<Value> {
	let account = account_value(authz.account.as_ref());
	let payload = types::bytes_value(authz.payload.as_slice());
	let signature = signature_value(authz);
	Ok(Value::named_composite([
		("account", account),
		("payload", payload),
		("signature", signature),
	]))
}

pub(crate) fn identifier_struct_value(id: &Ss58Identifier) -> Value {
	types::identifier_struct(id)
}

pub(crate) fn option_u32_value(value: Option<u32>) -> Value {
	option_value(value.map(|v| Value::u128(v as u128)))
}

pub(crate) fn u64_value(value: u64) -> Value {
	Value::u128(value as u128)
}

pub(crate) fn u16_value(value: u16) -> Value {
	Value::u128(value as u128)
}

pub(crate) fn hex_arg(raw: &[u8]) -> Value {
	types::bytes_value(raw)
}

pub(crate) fn account_value(account: &[u8]) -> Value {
	Value::from_bytes(account.to_vec())
}

pub(crate) fn option_value(inner: Option<Value>) -> Value {
	match inner {
		Some(value) => Value::variant("Some", Composite::unnamed(vec![value])),
		None => Value::variant("None", Composite::unnamed(Vec::new())),
	}
}

fn signature_value(authz: &AuthorizationRequest) -> Value {
	let (scheme, bytes) = match &authz.signature {
		MultiSignature::Sr25519(sig) => ("Sr25519", sig.as_ref()),
		MultiSignature::Ed25519(sig) => ("Ed25519", sig.as_ref()),
		MultiSignature::Ecdsa(sig) => ("Ecdsa", sig.as_ref()),
	};
	let inner = types::bytes_value(bytes);
	Value::unnamed_variant(scheme, [inner])
}

pub(crate) fn view_failure(ctx: &str, err: AuthorizationError) -> Error {
	match err {
		AuthorizationError::NotFound => Error::NotFound(format!("{ctx}: not found")),
		AuthorizationError::Unauthorized => Error::Params(format!("{ctx}: unauthorized")),
		AuthorizationError::InvalidInput => Error::Params(format!("{ctx}: invalid input")),
		AuthorizationError::TooLarge => Error::Params(format!("{ctx}: result too large")),
		AuthorizationError::Expired => Error::Params(format!("{ctx}: authorization expired")),
		AuthorizationError::Internal => Error::ViewDecode(format!("{ctx}: internal error")),
	}
}

fn extract_view_payload(thunk: DecodedValueThunk) -> Result<Vec<u8>> {
	type Envelope = core::result::Result<Vec<u8>, Vec<u8>>;
	let bytes = thunk.into_encoded();
	let result: Envelope =
		Decode::decode(&mut &bytes[..]).map_err(|e| Error::ViewDecode(e.to_string()))?;
	match result {
		Ok(inner) => Ok(inner),
		Err(err) => Err(view_error(err)),
	}
}

fn view_error(err_bytes: Vec<u8>) -> Error {
	match String::from_utf8(err_bytes.clone()) {
		Ok(text) if !text.is_empty() => Error::ViewDecode(text),
		_ => Error::ViewDecode(format!("0x{}", err_bytes.encode_hex::<String>())),
	}
}
