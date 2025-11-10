use super::{auth, ArgBuilder, Query};
use crate::{error::Result, types::identifier_value};
use serde_json::Value as JsonValue;

/// Facade for `pallet-token` view functions.
pub struct TokenQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> TokenQuery<'a> {
	fn base_args(
		&self,
		auth: &auth::ViewAuthorization,
		builder_fn: impl FnOnce(&mut ArgBuilder) -> Result<()>,
	) -> Result<scale_value::Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(auth)?);
		builder_fn(&mut builder)?;
		Ok(builder.finish())
	}

	pub async fn state_version(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
	) -> Result<Option<u32>> {
		let args = self.base_args(auth, |builder| {
			builder.push("token", identifier_value(token_ss58)?);
			Ok(())
		})?;
		self.query.call_typed("Token", "state_version", args).await
	}

	pub async fn resolve_identifier_json(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
	) -> Result<JsonValue> {
		let args = self.base_args(auth, |builder| {
			builder.push("token", identifier_value(token_ss58)?);
			Ok(())
		})?;
		self.query.call_json_raw("Token", "resolve_identifier", args).await
	}

	pub async fn timeline_json(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
		start: Option<u32>,
		limit: Option<u32>,
	) -> Result<JsonValue> {
		let args = self.base_args(auth, |builder| {
			builder.push("token", identifier_value(token_ss58)?);
			builder.push("start", super::option_u32_value(start));
			builder.push("limit", super::option_u32_value(limit));
			Ok(())
		})?;
		self.query.call_json_raw("Token", "timeline", args).await
	}

	pub async fn resolve_pallet_json(
		&self,
		auth: &auth::ViewAuthorization,
		index: u16,
	) -> Result<JsonValue> {
		let args = self.base_args(auth, |builder| {
			builder.push("index", super::u16_value(index));
			Ok(())
		})?;
		self.query.call_json_raw("Token", "resolve_pallet", args).await
	}
}
