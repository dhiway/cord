use super::{ArgBuilder, Query};
use crate::{
	api::runtime::runtime_types,
	error::{Error, Result},
	types::entity::{BlockRef, HistoryEntry},
};
use cord_primitives::{
	dev::ss58_string,
	identifier::Ss58Identifier,
	view_api::{
		AuthorizationError, EntityAccountTokenRequest, EntityAttributeHistoryEntryRequest,
		EntityAttributeHistoryForKeyRequest, EntityAttributeHistoryRequest,
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
	pub async fn attribute_history(
		&self,
		req: &EntityAttributeHistoryRequest,
	) -> Result<Vec<HistoryEntry>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<
			Vec<(Vec<u8>, u64, Vec<u8>, runtime_types::pallet_token::EventBlock)>,
			AuthorizationError,
		> = self.query.call_typed("Entity", "attribute_history", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history", err)).map(|records| {
			records
				.into_iter()
				.map(|(key, version, old, block)| {
					HistoryEntry::from_raw(&key, version, &old, block_ref(block))
				})
				.collect()
		})
	}

	pub async fn attribute_history_for_key(
		&self,
		req: &EntityAttributeHistoryForKeyRequest,
	) -> Result<Vec<HistoryEntry>> {
		let args = self.token_key_args(&req.auth, &req.token, req.key.as_slice())?;
		let raw: core::result::Result<
			Vec<(u64, Vec<u8>, runtime_types::pallet_token::EventBlock)>,
			AuthorizationError,
		> = self.query.call_typed("Entity", "attribute_history_for_key", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history_for_key", err))
			.map(|records| {
				records
					.into_iter()
					.map(|(version, old, block)| {
						HistoryEntry::from_raw(req.key.as_slice(), version, &old, block_ref(block))
					})
					.collect()
			})
	}

	pub async fn attribute_history_entry(
		&self,
		req: &EntityAttributeHistoryEntryRequest,
	) -> Result<HistoryEntry> {
		let args =
			self.token_key_version_args(&req.auth, &req.token, req.key.as_slice(), req.version)?;
		let raw: core::result::Result<
			(Vec<u8>, runtime_types::pallet_token::EventBlock),
			AuthorizationError,
		> = self.query.call_typed("Entity", "attribute_history_entry", args).await?;
		raw.map_err(|err| view_failure("entity.attribute_history_entry", err)).map(
			|(old, block)| {
				HistoryEntry::from_raw(req.key.as_slice(), req.version, &old, block_ref(block))
			},
		)
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

fn block_ref(block: runtime_types::pallet_token::EventBlock) -> BlockRef {
	BlockRef { height: block.height, index: block.index }
}
