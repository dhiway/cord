use super::{ArgBuilder, Query};
use crate::error::{Error, Result};
use cord_primitives::{
	identifier::DecodedIdentifier,
	view::InfoTokenHistoryEntry,
	view_api::{
		TokenResolveIdentifierRequest, TokenResolvePalletRequest, TokenStateVersionRequest,
		TokenTimelineRequest,
	},
};
use scale_value::Value;

/// Facade for `pallet-token` view functions.
pub struct TokenQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> TokenQuery<'a> {
	pub async fn state_version(&self, req: &TokenStateVersionRequest) -> Result<Option<u32>> {
		let args = self.token_args(&req.auth, &req.token)?;
		self.query.call_optional("Token", "state_version", args).await
	}

	pub async fn resolve_identifier(
		&self,
		req: &TokenResolveIdentifierRequest,
	) -> Result<DecodedIdentifier> {
		let args = self.token_args(&req.auth, &req.token)?;
		let value = self
			.query
			.call_optional::<DecodedIdentifier>("Token", "resolve_identifier", args)
			.await?;
		value.ok_or_else(|| Error::NotFound("token.resolve_identifier returned none".into()))
	}

	pub async fn timeline(&self, req: &TokenTimelineRequest) -> Result<Vec<InfoTokenHistoryEntry>> {
		let args = self.timeline_args(req)?;
		let value = self
			.query
			.call_optional::<Vec<InfoTokenHistoryEntry>>("Token", "timeline", args)
			.await?;
		value.ok_or_else(|| Error::NotFound("token.timeline returned none".into()))
	}

	pub async fn resolve_pallet(&self, req: &TokenResolvePalletRequest) -> Result<String> {
		let args = self.resolve_pallet_args(req)?;
		let value = self.query.call_optional::<String>("Token", "resolve_pallet", args).await?;
		value.ok_or_else(|| Error::NotFound("token.resolve_pallet returned none".into()))
	}

	fn token_args(
		&self,
		auth: &cord_primitives::view_api::ViewRequestAuth,
		token: &cord_primitives::identifier::Ss58Identifier,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(auth)?);
		builder.push("token", super::identifier_struct_value(token));
		Ok(builder.finish())
	}

	fn timeline_args(&self, req: &TokenTimelineRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(&req.auth)?);
		builder.push("token", super::identifier_struct_value(&req.token));
		builder.push("start", super::option_u32_value(req.start));
		builder.push("limit", super::option_u32_value(req.limit));
		Ok(builder.finish())
	}

	fn resolve_pallet_args(&self, req: &TokenResolvePalletRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(&req.auth)?);
		builder.push("index", super::u16_value(req.index));
		Ok(builder.finish())
	}
}
