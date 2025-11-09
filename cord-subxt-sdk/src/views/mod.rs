pub mod auth;
pub mod dynamic;

use crate::{
	client::Client,
	error::{Error, Result},
};

/// Facade for runtime view functions.
pub struct Views<'a> {
	pub(crate) client: &'a Client,
}

impl<'a> Views<'a> {
	pub async fn call_json(
		&self,
		pallet: &str,
		function: &str,
		json_args: serde_json::Value,
	) -> Result<serde_json::Value> {
		dynamic::state_call_json(self.client, pallet, function, json_args).await
	}

	pub async fn entity_attribute_history_json(
		&self,
		_auth: &auth::ViewAuthorization,
		_token_ss58: &str,
	) -> Result<serde_json::Value> {
		Err(Error::NotFound("entity views not implemented yet".into()))
	}

	pub async fn entity_attribute_history_for_key_json(
		&self,
		_auth: &auth::ViewAuthorization,
		_token_ss58: &str,
		_key_hex: &str,
	) -> Result<serde_json::Value> {
		Err(Error::NotFound("entity views not implemented yet".into()))
	}

	pub async fn entity_attribute_history_for_key_utf8_json(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
		key_utf8: &str,
	) -> Result<serde_json::Value> {
		let key_hex = crate::types::to_key_hex_from_utf8(key_utf8);
		self.entity_attribute_history_for_key_json(auth, token_ss58, &key_hex).await
	}

	pub async fn entity_attribute_history_entry_json(
		&self,
		_auth: &auth::ViewAuthorization,
		_token_ss58: &str,
		_key_hex: &str,
		_version: u64,
	) -> Result<serde_json::Value> {
		Err(Error::NotFound("entity views not implemented yet".into()))
	}
}
