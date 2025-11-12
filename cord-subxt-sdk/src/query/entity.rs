use super::{ArgBuilder, Query};
use crate::error::{Error, Result};
use cord_primitives::{
	dev::ss58_string,
	identifier::Ss58Identifier,
	view::InfoAttributeHistoryEntry,
	view_api::{
		AuthorizationError, EntityAccountTokenRequest, EntityAttributeHistoryEntryRequest,
		EntityAttributeHistoryForKeyRequest, EntityAttributeHistoryRequest, EntityInfoBytesRequest,
		EntityLinkedAccountsRequest, EntityNymRequest,
	},
};
use scale_value::Value;
use subxt::utils::AccountId32;

/// Facade for `pallet-entity` view functions.
pub struct EntityQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> EntityQuery<'a> {
	pub async fn attribute_history_entries(
		&self,
		req: &EntityAttributeHistoryRequest,
	) -> Result<Vec<InfoAttributeHistoryEntry>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<InfoAttributeHistoryEntry>, AuthorizationError> =
			self.query.call_typed("Entity", "attribute_history_entries", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history_entries", err))
	}

	pub async fn attribute_history_for_key_entries(
		&self,
		req: &EntityAttributeHistoryForKeyRequest,
	) -> Result<Vec<InfoAttributeHistoryEntry>> {
		let args = self.token_key_args(&req.auth, &req.token, req.key.as_slice())?;
		let raw: core::result::Result<Vec<InfoAttributeHistoryEntry>, AuthorizationError> = self
			.query
			.call_typed("Entity", "attribute_history_for_key_entries", args)
			.await?;
		raw.map_err(|err| view_failure("entity.attribute_history_for_key_entries", err))
	}

	pub async fn attribute_history_entry(
		&self,
		req: &EntityAttributeHistoryEntryRequest,
	) -> Result<InfoAttributeHistoryEntry> {
		let args =
			self.token_key_version_args(&req.auth, &req.token, req.key.as_slice(), req.version)?;
		let raw: core::result::Result<InfoAttributeHistoryEntry, AuthorizationError> =
			self.query.call_typed("Entity", "attribute_history_entry_view", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history_entry_view", err))
	}

	pub async fn account_token(&self, req: &EntityAccountTokenRequest) -> Result<Option<String>> {
		let args = self.account_args(req)?;
		let raw: core::result::Result<Ss58Identifier, AuthorizationError> =
			self.query.call_typed("Entity", "account_token", args).await?;
		match raw {
			Ok(id) => Ok(Some(ss58_string(&id))),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.account_token", err)),
		}
	}

	pub async fn entity_info_bytes(&self, req: &EntityInfoBytesRequest) -> Result<Option<Vec<u8>>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<u8>, AuthorizationError> =
			self.query.call_typed("Entity", "entity_info_bytes", args).await?;
		match raw {
			Ok(bytes) => Ok(Some(bytes)),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.entity_info_bytes", err)),
		}
	}

	pub async fn linked_accounts(
		&self,
		req: &EntityLinkedAccountsRequest,
	) -> Result<Vec<AccountId32>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<AccountId32>, AuthorizationError> =
			self.query.call_typed("Entity", "linked_accounts", args).await?;
		raw.map_err(|err| view_failure("entity.linked_accounts", err))
	}

	pub async fn entity_nym(&self, req: &EntityNymRequest) -> Result<Option<String>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<u8>, AuthorizationError> =
			self.query.call_typed("Entity", "entity_nym", args).await?;
		match raw {
			Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.entity_nym", err)),
		}
	}

	fn token_args(
		&self,
		auth: &cord_primitives::view_api::AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("token", super::identifier_struct_value(token));
		Ok(builder.finish())
	}

	fn token_key_args(
		&self,
		auth: &cord_primitives::view_api::AuthorizationRequest,
		token: &Ss58Identifier,
		key: &[u8],
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("token", super::identifier_struct_value(token));
		builder.push("key", super::hex_arg(key));
		Ok(builder.finish())
	}

	fn token_key_version_args(
		&self,
		auth: &cord_primitives::view_api::AuthorizationRequest,
		token: &Ss58Identifier,
		key: &[u8],
		version: u64,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("token", super::identifier_struct_value(token));
		builder.push("key", super::hex_arg(key));
		builder.push("version", super::u64_value(version));
		Ok(builder.finish())
	}

	fn account_args(&self, req: &EntityAccountTokenRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(&req.auth)?);
		builder.push("account", super::account_value(req.account.as_ref()));
		Ok(builder.finish())
	}
}

fn view_failure(ctx: &str, err: AuthorizationError) -> Error {
	match err {
		AuthorizationError::NotFound => Error::NotFound(format!("{ctx}: not found")),
		AuthorizationError::Unauthorized => Error::Params(format!("{ctx}: unauthorized")),
		AuthorizationError::InvalidInput => Error::Params(format!("{ctx}: invalid input")),
		AuthorizationError::TooLarge => Error::Params(format!("{ctx}: result too large")),
		AuthorizationError::Internal => Error::ViewDecode(format!("{ctx}: internal error")),
	}
}
