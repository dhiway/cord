pub mod auth;
pub mod dynamic;
pub mod register;

use crate::{
	client::Client,
	error::{Error, Result},
	types,
};
use codec::Decode;
use core::str;
use scale_value::{Composite, Value};
use serde_json::Value as JsonValue;

/// Facade for runtime query functions.
pub struct Query<'a> {
	pub(crate) client: &'a Client,
}

impl<'a> Query<'a> {
	pub fn register(&self) -> register::RegisterQuery<'_> {
		register::RegisterQuery { query: self }
	}

	pub async fn call_json(
		&self,
		pallet: &str,
		function: &str,
		json_args: serde_json::Value,
	) -> Result<JsonValue> {
		let metadata = self.client.api.metadata();
		let args = dynamic::encode_args_from_json(pallet, function, &json_args, &metadata)?;
		self.call_json_raw(pallet, function, args).await
	}

	pub(crate) async fn call_json_raw(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<JsonValue> {
		let result = dynamic::call_view(self.client, pallet, function, args).await?;
		let encoded = result.into_encoded();
		let mut cursor = &encoded[..];
		let bytes: Option<Vec<u8>> =
			Option::<Vec<u8>>::decode(&mut cursor).map_err(|e| Error::ViewDecode(e.to_string()))?;
		let data =
			bytes.ok_or_else(|| Error::NotFound(format!("{pallet}.{function} returned none")))?;
		let text = str::from_utf8(&data).map_err(|e| Error::ViewDecode(e.to_string()))?;
		serde_json::from_str(text).map_err(|e| Error::ViewDecode(e.to_string()))
	}

	pub(crate) async fn call_typed<T: Decode + 'static>(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<T> {
		let result = dynamic::call_view(self.client, pallet, function, args).await?;
		let bytes = result.into_encoded();
		T::decode(&mut &bytes[..]).map_err(|e| Error::ViewDecode(e.to_string()))
	}
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

pub(crate) fn view_auth_value(authz: &auth::ViewAuthorization) -> Result<Value> {
	let account = types::bytes_value(authz.account_id.as_ref());
	let payload = types::bytes_value(&authz.message);
	let signature = signature_value(authz)?;
	Ok(Value::named_composite([
		("account", account),
		("payload", payload),
		("signature", signature),
	]))
}

pub(crate) fn option_u32_value(value: Option<u32>) -> Value {
	option_value(value.map(|v| Value::u128(v as u128)))
}

fn option_value(inner: Option<Value>) -> Value {
	match inner {
		Some(value) => Value::variant("Some", Composite::unnamed(vec![value])),
		None => Value::variant("None", Composite::unnamed(Vec::new())),
	}
}

fn signature_value(authz: &auth::ViewAuthorization) -> Result<Value> {
	use auth::SignatureScheme;
	let (name, expected_len) = match authz.scheme {
		SignatureScheme::Sr25519 => ("Sr25519", 64usize),
		SignatureScheme::Ed25519 => ("Ed25519", 64usize),
		SignatureScheme::Ecdsa => ("Ecdsa", 65usize),
	};
	if authz.signature.len() != expected_len {
		return Err(Error::Params(format!("signature for {name} must be {expected_len} bytes")));
	}
	let inner = types::bytes_value(&authz.signature);
	Ok(Value::unnamed_variant(name, [inner]))
}
