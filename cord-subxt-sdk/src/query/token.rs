use super::{ArgBuilder, Query};
use crate::error::{Error, Result};
use cord_primitives::{
	identifier::DecodedIdentifier,
	view::InfoTokenHistoryEntry,
	view_api::{
		TokenResolveIdentifierRequest, TokenResolvePalletRequest, TokenStateVersionRequest,
		TokenTimelineRequest, ViewError,
	},
};
use scale_value::Value;

/// Facade for `pallet-token` view functions.
pub struct TokenQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> TokenQuery<'a> {
	pub async fn state_version(&self, req: &TokenStateVersionRequest) -> Result<u32> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<u32, ViewError> =
			self.query.call_typed("Token", "state_version", args).await?;
		raw.map_err(|err| view_failure("token.state_version", err))
	}

	pub async fn resolve_identifier(
		&self,
		req: &TokenResolveIdentifierRequest,
	) -> Result<DecodedIdentifier> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<DecodedIdentifier, ViewError> =
			self.query.call_typed("Token", "resolve_identifier", args).await?;
		raw.map_err(|err| view_failure("token.resolve_identifier", err))
	}

	pub async fn timeline(&self, req: &TokenTimelineRequest) -> Result<Vec<InfoTokenHistoryEntry>> {
		let args = self.timeline_args(req)?;
		let raw: core::result::Result<Vec<InfoTokenHistoryEntry>, ViewError> =
			self.query.call_typed("Token", "timeline", args).await?;
		raw.map_err(|err| view_failure("token.timeline", err))
	}

	pub async fn resolve_pallet(&self, req: &TokenResolvePalletRequest) -> Result<String> {
		let args = self.resolve_pallet_args(req)?;
		let raw: core::result::Result<String, ViewError> =
			self.query.call_typed("Token", "resolve_pallet", args).await?;
		raw.map_err(|err| view_failure("token.resolve_pallet", err))
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

fn view_failure(ctx: &str, err: ViewError) -> Error {
	match err {
		ViewError::NotFound => Error::NotFound(format!("{ctx}: not found")),
		ViewError::AuthFailed | ViewError::PermissionDenied => {
			Error::Params(format!("{ctx}: {err:?}"))
		},
		ViewError::InvalidRequest | ViewError::InvalidContext | ViewError::Replay => {
			Error::Params(format!("{ctx}: {err:?}"))
		},
		ViewError::Expired => Error::Params(format!("{ctx}: authorization expired")),
	}
}
