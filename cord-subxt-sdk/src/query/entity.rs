use super::{auth, ArgBuilder, Query};
use crate::{
	error::Result,
	types::{identifier_value, to_key_hex_from_utf8},
};
use cord_primitives::identifier::Ss58Identifier;
use serde_json::Value as JsonValue;
use subxt::utils::AccountId32;

/// Facade for `pallet-entity` view functions.
pub struct EntityQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> EntityQuery<'a> {
	fn build_args<F>(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
		f: F,
	) -> Result<scale_value::Value>
	where
		F: FnOnce(&mut ArgBuilder) -> Result<()>,
	{
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(auth)?);
		builder.push("token", identifier_value(token_ss58)?);
		f(&mut builder)?;
		Ok(builder.finish())
	}

	async fn call_json(
		&self,
		function: &str,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
		f: impl FnOnce(&mut ArgBuilder) -> Result<()>,
	) -> Result<JsonValue> {
		let args = self.build_args(auth, token_ss58, f)?;
		self.query.call_json_raw("Entity", function, args).await
	}

	pub async fn attribute_history_json(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
	) -> Result<JsonValue> {
		self.call_json("get_attribute_history_json", auth, token_ss58, |_| Ok(())).await
	}

	pub async fn attribute_history_for_key_json(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
		key_hex: &str,
	) -> Result<JsonValue> {
		self.call_json("get_attribute_history_for_key_json", auth, token_ss58, |builder| {
			builder.push("key", super::hex_arg(key_hex)?);
			Ok(())
		})
		.await
	}

	pub async fn attribute_history_for_key_utf8_json(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
		key_utf8: &str,
	) -> Result<JsonValue> {
		let key_hex = to_key_hex_from_utf8(key_utf8);
		self.attribute_history_for_key_json(auth, token_ss58, &key_hex).await
	}

	pub async fn attribute_history_entry_json(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
		key_hex: &str,
		version: u64,
	) -> Result<JsonValue> {
		self.call_json("get_attribute_history_entry_json", auth, token_ss58, |builder| {
			builder.push("key", super::hex_arg(key_hex)?);
			builder.push("version", super::u64_value(version));
			Ok(())
		})
		.await
	}

	pub async fn account_token(
		&self,
		auth: &auth::ViewAuthorization,
		account: &AccountId32,
	) -> Result<Option<String>> {
		let args = self.build_account_args(auth, account)?;
		let bytes = self.query.call_encoded("Entity", "account_token", args).await?;
		let token: Option<Ss58Identifier> = self.query.decode_view_result(bytes)?;
		Ok(token.map(|id| ss58_string(&id)))
	}

	pub async fn entity_info_bytes(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
	) -> Result<Option<Vec<u8>>> {
		let args = self.build_args(auth, token_ss58, |_| Ok(()))?;
		self.query.call_typed("Entity", "entity_info_bytes", args).await
	}

	fn build_account_args(
		&self,
		auth: &auth::ViewAuthorization,
		account: &AccountId32,
	) -> Result<scale_value::Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(auth)?);
		builder.push("account", super::account_value(account));
		Ok(builder.finish())
	}
}

fn ss58_string(id: &Ss58Identifier) -> String {
	String::from_utf8_lossy(id.as_bytes()).into_owned()
}
