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
use core::str;
use hex::ToHex;
use scale_value::{Composite, Value};
use serde_json::Value as JsonValue;
use subxt::utils::AccountId32;

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
		let encoded = self.call_encoded(pallet, function, args).await?;
		let payload = self.extract_view_ok(encoded)?;
		let bytes: Option<Vec<u8>> =
			Option::decode(&mut &payload[..]).map_err(|e| Error::ViewDecode(e.to_string()))?;
		let data =
			bytes.ok_or_else(|| Error::NotFound(format!("{pallet}.{function} returned none")))?;
		let text = str::from_utf8(&data).map_err(|e| Error::ViewDecode(e.to_string()))?;
		serde_json::from_str(text).map_err(|e| Error::ViewDecode(e.to_string()))
	}

	pub(crate) async fn call_encoded(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<Vec<u8>> {
		let result = dynamic::call_view(self.client, pallet, function, args).await?;
		Ok(result.into_encoded())
	}

	pub(crate) async fn call_typed<T: Decode + 'static>(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<T> {
		let encoded = self.call_encoded(pallet, function, args).await?;
		self.decode_view_result(encoded)
	}

	pub(crate) fn decode_view_result<T: Decode>(&self, bytes: Vec<u8>) -> Result<T> {
		let payload = self.extract_view_ok(bytes)?;
		T::decode(&mut &payload[..]).map_err(|e| Error::ViewDecode(e.to_string()))
	}

	fn extract_view_ok(&self, bytes: Vec<u8>) -> Result<Vec<u8>> {
		let result: core::result::Result<Vec<u8>, Vec<u8>> =
			Decode::decode(&mut &bytes[..]).map_err(|e| Error::ViewDecode(e.to_string()))?;
		match result {
			Ok(inner) => Ok(inner),
			Err(err) => Err(view_error(err)),
		}
	}
}

fn view_error(err_bytes: Vec<u8>) -> Error {
	match String::from_utf8(err_bytes.clone()) {
		Ok(text) if !text.is_empty() => Error::ViewDecode(text),
		_ => Error::ViewDecode(format!("0x{}", err_bytes.encode_hex::<String>())),
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
	let account = types::account_id_value(&authz.account_id);
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

pub(crate) fn u64_value(value: u64) -> Value {
	Value::u128(value as u128)
}

pub(crate) fn u16_value(value: u16) -> Value {
	Value::u128(value as u128)
}

pub(crate) fn hex_arg(raw: &str) -> Result<Value> {
	let bytes = types::hex_to_bytes(raw)?;
	Ok(types::bytes_value(&bytes))
}

pub(crate) fn account_value(account: &AccountId32) -> Value {
	types::account_id_value(account)
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
