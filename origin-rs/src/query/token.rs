use super::{ArgBuilder, Query};
use crate::{error::Result, types::token::StateEventRecord};
use origin_primitives::{
	identifier::DecodedIdentifier,
	view_api::{
		AuthorizationError, TokenResolveIdentifierRequest, TokenResolvePalletRequest,
		TokenStateVersionRequest, TokenTimelineRequest,
	},
};
use scale_value::Value;

/// Facade for `pallet-token` view functions.
pub struct TokenQuery<'a> {
	pub(crate) query: &'a Query<'a>,
	pub(crate) supported: bool,
}

impl<'a> TokenQuery<'a> {
	fn ensure_supported(&self) -> Result<()> {
		if self.supported {
			Ok(())
		} else {
			Err(crate::error::Error::Params(
				"token queries require origin-hub flavor (not available on origin relay)".into(),
			))
		}
	}

	pub async fn state_version(&self, req: &TokenStateVersionRequest) -> Result<u32> {
		self.ensure_supported()?;
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<u32, AuthorizationError> =
			self.query.call_result("Token", "state_version", args).await?;
		raw.map_err(|err| super::view_failure("token.state_version", err))
	}

	pub async fn resolve_identifier(
		&self,
		req: &TokenResolveIdentifierRequest,
	) -> Result<DecodedIdentifier> {
		self.ensure_supported()?;
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<DecodedIdentifier, AuthorizationError> =
			self.query.call_result("Token", "resolve_identifier", args).await?;
		raw.map_err(|err| super::view_failure("token.resolve_identifier", err))
	}

	pub async fn timeline(
		&self,
		req: &TokenTimelineRequest,
	) -> Result<(Vec<StateEventRecord>, Option<u32>)> {
		self.ensure_supported()?;
		let args = self.timeline_args(req)?;
		let raw: core::result::Result<(Vec<StateEventRecord>, Option<u32>), AuthorizationError> =
			self.query.call_result("Token", "timeline", args).await?;
		raw.map_err(|err| super::view_failure("token.timeline", err))
	}

	pub async fn resolve_pallet(&self, req: &TokenResolvePalletRequest) -> Result<String> {
		self.ensure_supported()?;
		let args = self.resolve_pallet_args(req)?;
		let raw: core::result::Result<String, AuthorizationError> =
			self.query.call_result("Token", "resolve_pallet", args).await?;
		raw.map_err(|err| super::view_failure("token.resolve_pallet", err))
	}

	fn token_args(
		&self,
		auth: &origin_primitives::view_api::AuthorizationRequest,
		token: &origin_primitives::identifier::Ss58Identifier,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("token", super::identifier_struct_value(token));
		Ok(builder.finish())
	}

	fn timeline_args(&self, req: &TokenTimelineRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(&req.auth)?);
		builder.push("token", super::identifier_struct_value(&req.token));
		builder.push("start", super::option_u32_value(req.start));
		builder.push("limit", super::option_u32_value(req.limit));
		Ok(builder.finish())
	}

	fn resolve_pallet_args(&self, req: &TokenResolvePalletRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(&req.auth)?);
		builder.push("index", super::u16_value(req.index));
		Ok(builder.finish())
	}
}
