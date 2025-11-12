use super::{ArgBuilder, Query};
use crate::error::{Error, Result};
use cord_primitives::{
	identifier::Ss58Identifier,
	view::InfoAttributeHistoryEntry,
	view_api::{
		EntityAccountTokenRequest, EntityAttributeHistoryEntryRequest,
		EntityAttributeHistoryForKeyRequest, EntityAttributeHistoryRequest, EntityInfoBytesRequest,
		EntityLinkedAccountsRequest, EntityNymRequest, ViewError,
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
		let raw: core::result::Result<Vec<InfoAttributeHistoryEntry>, ViewError> =
			self.query.call_typed("Entity", "attribute_history_entries", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history_entries", err))
	}

	pub async fn attribute_history_for_key_entries(
		&self,
		req: &EntityAttributeHistoryForKeyRequest,
	) -> Result<Vec<InfoAttributeHistoryEntry>> {
		let args = self.token_key_args(&req.auth, &req.token, req.key.as_slice())?;
		let raw: core::result::Result<Vec<InfoAttributeHistoryEntry>, ViewError> = self
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
		let raw: core::result::Result<InfoAttributeHistoryEntry, ViewError> =
			self.query.call_typed("Entity", "attribute_history_entry_view", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history_entry_view", err))
	}

	pub async fn account_token(&self, req: &EntityAccountTokenRequest) -> Result<Option<String>> {
		let args = self.account_args(req)?;
		let raw: core::result::Result<Ss58Identifier, ViewError> =
			self.query.call_typed("Entity", "account_token", args).await?;
		match raw {
			Ok(id) => Ok(Some(ss58_string(&id))),
			Err(ViewError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.account_token", err)),
		}
	}

	pub async fn entity_info_bytes(&self, req: &EntityInfoBytesRequest) -> Result<Option<Vec<u8>>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<u8>, ViewError> =
			self.query.call_typed("Entity", "entity_info_bytes", args).await?;
		match raw {
			Ok(bytes) => Ok(Some(bytes)),
			Err(ViewError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.entity_info_bytes", err)),
		}
	}

	pub async fn linked_accounts(
		&self,
		req: &EntityLinkedAccountsRequest,
	) -> Result<Vec<AccountId32>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<AccountId32>, ViewError> =
			self.query.call_typed("Entity", "linked_accounts", args).await?;
		raw.map_err(|err| view_failure("entity.linked_accounts", err))
	}

	pub async fn entity_nym(&self, req: &EntityNymRequest) -> Result<Option<String>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<u8>, ViewError> =
			self.query.call_typed("Entity", "entity_nym", args).await?;
		match raw {
			Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
			Err(ViewError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.entity_nym", err)),
		}
	}

	fn token_args(
		&self,
		auth: &cord_primitives::view_api::ViewRequestAuth,
		token: &Ss58Identifier,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(auth)?);
		builder.push("token", super::identifier_struct_value(token));
		Ok(builder.finish())
	}

	fn token_key_args(
		&self,
		auth: &cord_primitives::view_api::ViewRequestAuth,
		token: &Ss58Identifier,
		key: &[u8],
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(auth)?);
		builder.push("token", super::identifier_struct_value(token));
		builder.push("key", super::hex_arg(key));
		Ok(builder.finish())
	}

	fn token_key_version_args(
		&self,
		auth: &cord_primitives::view_api::ViewRequestAuth,
		token: &Ss58Identifier,
		key: &[u8],
		version: u64,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(auth)?);
		builder.push("token", super::identifier_struct_value(token));
		builder.push("key", super::hex_arg(key));
		builder.push("version", super::u64_value(version));
		Ok(builder.finish())
	}

	fn account_args(&self, req: &EntityAccountTokenRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(&req.auth)?);
		builder.push("account", super::account_value(req.account.as_ref()));
		Ok(builder.finish())
	}
}

fn ss58_string(id: &Ss58Identifier) -> String {
	String::from_utf8_lossy(id.as_bytes()).into_owned()
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
