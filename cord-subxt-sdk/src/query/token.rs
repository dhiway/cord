use super::{auth, ArgBuilder, Query};
use crate::{
	error::{Error, Result},
	types::identifier_value,
};
use cord_primitives::{identifier::DecodedIdentifier, view::InfoTokenHistoryEntry};

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

	pub async fn resolve_identifier(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
	) -> Result<DecodedIdentifier> {
		let args = self.base_args(auth, |builder| {
			builder.push("token", identifier_value(token_ss58)?);
			Ok(())
		})?;
		let value: Option<DecodedIdentifier> =
			self.query.call_typed("Token", "resolve_identifier", args).await?;
		value.ok_or_else(|| Error::NotFound("token.resolve_identifier returned none".into()))
	}

	pub async fn timeline(
		&self,
		auth: &auth::ViewAuthorization,
		token_ss58: &str,
		start: Option<u32>,
		limit: Option<u32>,
	) -> Result<Vec<InfoTokenHistoryEntry>> {
		let args = self.base_args(auth, |builder| {
			builder.push("token", identifier_value(token_ss58)?);
			builder.push("start", super::option_u32_value(start));
			builder.push("limit", super::option_u32_value(limit));
			Ok(())
		})?;
		let value: Option<Vec<InfoTokenHistoryEntry>> =
			self.query.call_typed("Token", "timeline", args).await?;
		value.ok_or_else(|| Error::NotFound("token.timeline returned none".into()))
	}

	pub async fn resolve_pallet_name(
		&self,
		auth: &auth::ViewAuthorization,
		index: u16,
	) -> Result<String> {
		let args = self.base_args(auth, |builder| {
			builder.push("index", super::u16_value(index));
			Ok(())
		})?;
		let value: Option<String> = self.query.call_typed("Token", "resolve_pallet", args).await?;
		value.ok_or_else(|| Error::NotFound("token.resolve_pallet returned none".into()))
	}
}
