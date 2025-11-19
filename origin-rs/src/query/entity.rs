use super::{ArgBuilder, Query};

use crate::{
	error::{Error, Result},
	types::entity::{
		AttributeHistoryEntryRecord, AttributeHistoryRecord, AttributeHistoryVersionRecord,
		BlockRef, EntityInfoRecord, EventBlockRecord, HistoryEntry,
	},
};
use origin_primitives::{
	identifier::Ss58Identifier,
	view_api::{
		AuthorizationError, AuthorizationRequest, EntityAccountTokenRequest,
		EntityAttributeHistoryEntryRequest, EntityAttributeHistoryForKeyRequest,
		EntityAttributeHistoryRequest, EntityLinkedAccountsRequest, EntityNymRequest,
		EntityOverviewRequest,
	},
};
use scale_value::Value;
use subxt::utils::AccountId32;

/// Facade for `pallet-entity` query functions.
pub struct EntityQuery<'a> {
	pub(crate) query: &'a Query<'a>,
	pub(crate) supported: bool,
}

impl<'a> EntityQuery<'a> {
	fn ensure_supported(&self) -> Result<()> {
		if self.supported {
			Ok(())
		} else {
			Err(Error::Params(
				"entity queries require origin-hub flavor (not available on origin relay)".into(),
			))
		}
	}

	pub async fn overview(
		&self,
		req: &EntityOverviewRequest,
	) -> Result<Option<origin_primitives::view::EntityOverviewView<AccountId32>>> {
		self.ensure_supported()?;
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(&req.auth)?);
		builder.push("token", super::identifier_struct_value(&req.token));
		builder.push("history_limit", super::option_u32_value(req.history_limit));
		let args = builder.finish();

		match self.query.call_result("Entity", "overview", args.clone()).await {
			Ok(Ok(view)) => Ok(Some(view)),
			Ok(Err(AuthorizationError::NotFound)) => Ok(None),
			Ok(Err(err)) => Err(super::view_failure("entity.overview", err)),
			Err(Error::Codec(_)) | Err(Error::ViewDecode(_)) => {
				let dynamic =
					self.query.client.origin().call_view("Entity", "overview", args).await?;
				match scale_value::serde::from_value::<
					u32,
					origin_primitives::view::EntityOverviewView<AccountId32>,
				>(dynamic)
				{
					Ok(view) => Ok(Some(view)),
					Err(err) => Err(Error::ViewDecode(err.to_string())),
				}
			},
			Err(err) => Err(err),
		}
	}

	pub async fn details(
		&self,
		auth: &AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<Option<EntityInfoRecord>> {
		self.ensure_supported()?;
		let args = self.token_args(auth, token)?;
		match self.query.call_result("Entity", "details", args.clone()).await {
			Ok(Ok(info)) => Ok(Some(info)),
			Ok(Err(AuthorizationError::NotFound)) => Ok(None),
			Ok(Err(err)) => Err(super::view_failure("entity.details", err)),
			Err(Error::Codec(_)) | Err(Error::ViewDecode(_)) => {
				// Fallback: dynamic decode tolerant of new variants.
				if let Ok(dynamic) =
					self.query.client.origin().call_view("Entity", "details", args).await
				{
					if let Ok(info) = scale_value::serde::from_value(dynamic) {
						return Ok(Some(info));
					}
				}
				Ok(None)
			},
			Err(err) => Err(err),
		}
	}

	pub async fn attribute_history(
		&self,
		req: &EntityAttributeHistoryRequest,
	) -> Result<Vec<HistoryEntry>> {
		self.ensure_supported()?;
		let args = self.token_args(&req.auth, &req.token)?;
		let raw: core::result::Result<Vec<AttributeHistoryRecord>, AuthorizationError> =
			self.query.call_result("Entity", "attribute_history", args).await?;
		raw.map_err(|err| super::view_failure("entity.attribute_history", err))
			.map(|records| records.into_iter().map(HistoryEntry::from).collect())
	}

	pub async fn attribute_history_for_key(
		&self,
		req: &EntityAttributeHistoryForKeyRequest,
	) -> Result<Vec<HistoryEntry>> {
		self.ensure_supported()?;
		let args = self.token_key_args(&req.auth, &req.token, req.key.as_slice())?;
		let raw: core::result::Result<Vec<AttributeHistoryVersionRecord>, AuthorizationError> =
			self.query.call_result("Entity", "attribute_history_for_key", args).await?;
		raw.map_err(|err| super::view_failure("entity.attribute_history_for_key", err))
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
		self.ensure_supported()?;
		let args =
			self.token_key_version_args(&req.auth, &req.token, req.key.as_slice(), req.version)?;
		let raw: core::result::Result<AttributeHistoryEntryRecord, AuthorizationError> =
			self.query.call_result("Entity", "attribute_history_entry", args).await?;
		raw.map_err(|err| super::view_failure("entity.attribute_history_entry", err))
			.map(|record| record.into_entry(req.key.as_slice(), req.version))
	}

	pub async fn account_token(
		&self,
		req: &EntityAccountTokenRequest,
	) -> Result<Option<Ss58Identifier>> {
		self.ensure_supported()?;
		let args = self.account_args(req)?;
		match self.query.call_result("Entity", "account_token", args).await {
			Ok(Ok(id)) => Ok(Some(id)),
			Ok(Err(AuthorizationError::NotFound)) => Ok(None),
			Ok(Err(err)) => Err(super::view_failure("entity.account_token", err)),
			Err(err) => Err(err),
		}
	}

	pub async fn linked_accounts(
		&self,
		req: &EntityLinkedAccountsRequest,
	) -> Result<Vec<AccountId32>> {
		self.ensure_supported()?;
		let args = self.token_args(&req.auth, &req.token)?;
		match self.query.call_result("Entity", "linked_accounts", args.clone()).await {
			Ok(Ok(accounts)) => Ok(accounts),
			Ok(Err(AuthorizationError::NotFound)) => Ok(Vec::new()),
			Ok(Err(err)) => Err(super::view_failure("entity.linked_accounts", err)),
			Err(Error::Codec(_)) | Err(Error::ViewDecode(_)) => {
				// Fallback: dynamic decode tolerant of type/variant changes.
				let dynamic =
					self.query.client.origin().call_view("Entity", "linked_accounts", args).await?;
				let decoded: Result<Vec<AccountId32>, _> =
					scale_value::serde::from_value::<u32, Vec<AccountId32>>(dynamic);
				decoded.map_err(|e| Error::ViewDecode(e.to_string()))
			},
			Err(err) => Err(err),
		}
	}

	pub async fn entity_nym(&self, req: &EntityNymRequest) -> Result<Option<String>> {
		self.ensure_supported()?;
		let args = self.token_args(&req.auth, &req.token)?;
		match self
			.query
			.call_view_as::<core::result::Result<Vec<u8>, AuthorizationError>>(
				"Entity",
				"entity_nym",
				args.clone(),
			)
			.await
		{
			Ok(Ok(bytes)) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
			Ok(Err(AuthorizationError::NotFound)) => Ok(None),
			Ok(Err(err)) => Err(super::view_failure("entity.entity_nym", err)),
			Err(Error::Codec(_)) | Err(Error::ViewDecode(_)) => {
				let dynamic =
					self.query.client.origin().call_view("Entity", "entity_nym", args).await?;
				if let Ok(bytes) = scale_value::serde::from_value::<u32, Vec<u8>>(dynamic) {
					return Ok(Some(String::from_utf8_lossy(&bytes).into_owned()));
				}
				Err(Error::ViewDecode("entity_nym decode failed".into()))
			},
			Err(err) => Err(err),
		}
	}

	pub async fn entity_nym_lookup(
		&self,
		auth: &AuthorizationRequest,
		nym: Vec<u8>,
	) -> Result<Option<Ss58Identifier>> {
		self.ensure_supported()?;
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("nym", super::hex_arg(&nym));
		let args = builder.finish();
		let raw: core::result::Result<Ss58Identifier, AuthorizationError> =
			self.query.call_result("Entity", "entity_nym_lookup", args).await?;
		match raw {
			Ok(id) => Ok(Some(id)),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(super::view_failure("entity.entity_nym_lookup", err)),
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
		raw.map_err(|err| super::view_failure("entity.controller_account", err))
	}

	pub async fn account_history(
		&self,
		auth: &AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<Vec<(AccountId32, BlockRef)>> {
		let args = self.token_args(auth, token)?;
		let raw: core::result::Result<Vec<(AccountId32, EventBlockRecord)>, AuthorizationError> =
			self.query.call_result("Entity", "account_history", args).await?;
		raw.map_err(|err| super::view_failure("entity.account_history", err))
			.map(|records| {
				records.into_iter().map(|(account, block)| (account, block.into())).collect()
			})
	}

	fn token_args(
		&self,
		auth: &origin_primitives::view_api::AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("token", super::identifier_struct_value(token));
		Ok(builder.finish())
	}

	fn token_key_args(
		&self,
		auth: &origin_primitives::view_api::AuthorizationRequest,
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
		auth: &origin_primitives::view_api::AuthorizationRequest,
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
