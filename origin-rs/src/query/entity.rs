use super::{ArgBuilder, Query};
use crate::{
	api::runtime,
	error::{Error, Result},
	types::entity::{
		AttributeHistoryEntryRecord, AttributeHistoryRecord, AttributeHistoryVersionRecord,
		BlockRef, EntityInfoRecord, HistoryEntry,
	},
};
use cord_primitives::{
	identifier::Ss58Identifier,
	view_api::{
		AuthorizationError, AuthorizationRequest, EntityAccountTokenRequest,
		EntityAttributeHistoryEntryRequest, EntityAttributeHistoryForKeyRequest,
		EntityAttributeHistoryRequest, EntityLinkedAccountsRequest, EntityNymRequest,
	},
};
use scale_value::Value;
use subxt::utils::AccountId32;

pub type RuntimeEventBlock = runtime::runtime_types::pallet_token::EventBlock;

/// Facade for `pallet-entity` query functions.
pub struct EntityQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> EntityQuery<'a> {
	pub async fn details(
		&self,
		auth: &AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<Option<EntityInfoRecord>> {
		let args = self.token_args(auth, token)?;
		let raw: core::result::Result<EntityInfoRecord, AuthorizationError> =
			self.query.call_result("Entity", "details", args).await?;
		match raw {
			Ok(info) => Ok(Some(info)),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.details", err)),
		}
	}

	pub async fn attribute_history(
		&self,
		req: &EntityAttributeHistoryRequest,
	) -> Result<Vec<HistoryEntry>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<AttributeHistoryRecord>, AuthorizationError> =
			self.query.call_result("Entity", "attribute_history", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history", err))
			.map(|records| records.into_iter().map(HistoryEntry::from).collect())
	}

	pub async fn attribute_history_for_key(
		&self,
		req: &EntityAttributeHistoryForKeyRequest,
	) -> Result<Vec<HistoryEntry>> {
		let args = self.token_key_args(&req.auth, &req.token, req.key.as_slice())?;
		let raw: core::result::Result<Vec<AttributeHistoryVersionRecord>, AuthorizationError> =
			self.query.call_result("Entity", "attribute_history_for_key", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history_for_key", err))
			.map(|records| {
				records
					.into_iter()
					.map(|record| record.into_entry(req.key.as_slice()))
					.collect()
			})
	}

	pub async fn attribute_history_entry(
		&self,
		req: &EntityAttributeHistoryEntryRequest,
	) -> Result<HistoryEntry> {
		let args =
			self.token_key_version_args(&req.auth, &req.token, req.key.as_slice(), req.version)?;
		let raw: core::result::Result<AttributeHistoryEntryRecord, AuthorizationError> =
			self.query.call_result("Entity", "attribute_history_entry", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history_entry", err))
			.map(|record| record.into_entry(req.key.as_slice(), req.version))
	}

	pub async fn account_token(
		&self,
		req: &EntityAccountTokenRequest,
	) -> Result<Option<Ss58Identifier>> {
		let args = self.account_args(req)?;
		let raw: core::result::Result<Ss58Identifier, AuthorizationError> =
			self.query.call_result("Entity", "account_token", args).await?;
		match raw {
			Ok(id) => Ok(Some(id)),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.account_token", err)),
		}
	}

	pub async fn linked_accounts(
		&self,
		req: &EntityLinkedAccountsRequest,
	) -> Result<Vec<AccountId32>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<AccountId32>, AuthorizationError> =
			self.query.call_result("Entity", "linked_accounts", args).await?;
		raw.map_err(|err| view_failure("entity.linked_accounts", err))
	}

	pub async fn entity_nym(&self, req: &EntityNymRequest) -> Result<Option<String>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<u8>, AuthorizationError> =
			self.query.call_result("Entity", "entity_nym", args).await?;
		match raw {
			Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.entity_nym", err)),
		}
	}

	pub async fn entity_nym_lookup(
		&self,
		auth: &AuthorizationRequest,
		nym: Vec<u8>,
	) -> Result<Option<Ss58Identifier>> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("nym", super::hex_arg(&nym));
		let args = builder.finish();
		let raw: core::result::Result<Ss58Identifier, AuthorizationError> =
			self.query.call_result("Entity", "entity_nym_lookup", args).await?;
		match raw {
			Ok(id) => Ok(Some(id)),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(view_failure("entity.entity_nym_lookup", err)),
		}
	}

	pub async fn controller_account(
		&self,
		auth: &AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<AccountId32> {
		let args = self.token_args(auth, token)?;
		let raw: core::result::Result<AccountId32, AuthorizationError> =
			self.query.call_result("Entity", "controller_account", args).await?;
		raw.map_err(|err| view_failure("entity.controller_account", err))
	}

	pub async fn account_history(
		&self,
		auth: &AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<Vec<(AccountId32, BlockRef)>> {
		let args = self.token_args(auth, token)?;
		let raw: core::result::Result<Vec<(AccountId32, RuntimeEventBlock)>, AuthorizationError> =
			self.query.call_result("Entity", "account_history", args).await?;
		raw.map_err(|err| view_failure("entity.account_history", err)).map(|records| {
			records
				.into_iter()
				.map(|(account, block)| (account, block_ref(block)))
				.collect()
		})
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

fn block_ref(block: RuntimeEventBlock) -> BlockRef {
	BlockRef { height: block.height, index: block.index }
}
