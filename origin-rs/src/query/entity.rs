use super::{ArgBuilder, Query};

use crate::{
	error::{Error, Result},
	types::entity::{BlockRef, HistoryEntry},
};
use codec::Decode;
use core::convert::TryFrom;
use hex;
use log::warn;
use origin_primitives::{
	identifier::Ss58Identifier,
	view::AccountId32 as ViewAccount32,
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
	) -> Result<Option<origin_primitives::view::EntityOverview>> {
		self.ensure_supported()?;
		let mut builder = ArgBuilder::default();
		builder.push("auth_bytes", super::authorization_bytes_value(&req.auth)?);
		builder.push("token", Value::from_bytes(req.token.clone()));
		builder.push("history_limit", super::option_u32_value(req.history_limit));
		let args = builder.finish();
		match self
			.query
			.call_result::<origin_primitives::view::EntityOverview, AuthorizationError>(
				"Entity",
				"overview",
				args,
			)
			.await?
		{
			Ok(view) => Ok(Some(view)),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(super::view_failure("entity.overview", err)),
		}
	}

	pub async fn details(
		&self,
		auth: &AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<Option<Vec<u8>>> {
		self.ensure_supported()?;
		let args = self.token_args(auth, token.as_ref())?;
		match self.query.call_result("Entity", "details", args.clone()).await {
			Ok(Ok(bytes)) => Ok(Some(bytes)),
			Ok(Err(AuthorizationError::NotFound)) => Ok(None),
			Ok(Err(err)) => Err(super::view_failure("entity.details", err)),
			Err(err) => Err(err),
		}
	}

	pub async fn attribute_history(
		&self,
		req: &EntityAttributeHistoryRequest,
	) -> Result<Vec<HistoryEntry>> {
		self.ensure_supported()?;
		let args = self.token_args(&req.auth, req.token.as_slice())?;
		let raw: core::result::Result<
			Vec<(Vec<u8>, u64, Vec<u8>, origin_primitives::view::DevEventBlockView)>,
			AuthorizationError,
		> = self.query.call_result("Entity", "attribute_history", args).await?;
		raw.map_err(|err| super::view_failure("entity.attribute_history", err))
			.map(|records| {
				records
					.into_iter()
					.map(|(key, version, old, block)| {
						HistoryEntry::from_raw(&key, version, &old, block)
					})
					.collect()
			})
	}

	pub async fn attribute_history_for_key(
		&self,
		req: &EntityAttributeHistoryForKeyRequest,
	) -> Result<Vec<HistoryEntry>> {
		self.ensure_supported()?;
		let args = self.token_key_args(&req.auth, req.token.as_slice(), req.key.as_slice())?;
		let raw: core::result::Result<
			Vec<(u64, Vec<u8>, origin_primitives::view::DevEventBlockView)>,
			AuthorizationError,
		> = self.query.call_result("Entity", "attribute_history_for_key", args).await?;
		raw.map_err(|err| super::view_failure("entity.attribute_history_for_key", err))
			.map(|records| {
				records
					.into_iter()
					.map(|(version, old, block)| {
						HistoryEntry::from_raw(req.key.as_slice(), version, &old, block)
					})
					.collect()
			})
	}

	pub async fn attribute_history_entry(
		&self,
		req: &EntityAttributeHistoryEntryRequest,
	) -> Result<HistoryEntry> {
		self.ensure_supported()?;
		let args = self.token_key_version_args(
			&req.auth,
			req.token.as_slice(),
			req.key.as_slice(),
			req.version,
		)?;
		let raw: core::result::Result<
			(Vec<u8>, origin_primitives::view::DevEventBlockView),
			AuthorizationError,
		> = self.query.call_result("Entity", "attribute_history_entry", args).await?;
		raw.map_err(|err| super::view_failure("entity.attribute_history_entry", err))
			.map(|(old, block)| {
				HistoryEntry::from_raw(req.key.as_slice(), req.version, &old, block)
			})
	}

	pub async fn account_token(
		&self,
		req: &EntityAccountTokenRequest,
	) -> Result<Option<Ss58Identifier>> {
		self.ensure_supported()?;
		let args = self.account_args(req)?;
		let bytes = self.query.call_view_bytes("Entity", "account_token", args).await?;
		let mut cursor = &bytes.data[..];
		let decoded: core::result::Result<Vec<u8>, AuthorizationError> =
			Decode::decode(&mut cursor).map_err(|e| Error::Codec(e.to_string()))?;
			match decoded {
				Ok(raw) => Ok(decode_identifier_bytes(&raw)),
				Err(AuthorizationError::NotFound) => Ok(None),
				Err(err) => Err(super::view_failure("entity.account_token", err)),
			}
	}

	pub async fn linked_accounts(
		&self,
		req: &EntityLinkedAccountsRequest,
	) -> Result<Vec<AccountId32>> {
		self.ensure_supported()?;
		let args = self.token_args(&req.auth, req.token.as_slice())?;
		match self
			.query
			.call_result::<Vec<ViewAccount32>, AuthorizationError>(
				"Entity",
				"linked_accounts",
				args.clone(),
			)
			.await
		{
			Ok(Ok(accounts)) => Ok(accounts.into_iter().map(to_subxt_account).collect()),
			Ok(Err(AuthorizationError::NotFound)) => Ok(Vec::new()),
			Ok(Err(err)) => Err(super::view_failure("entity.linked_accounts", err)),
			Err(err) => Err(err),
		}
	}

	pub async fn entity_nym(&self, req: &EntityNymRequest) -> Result<Option<String>> {
		self.ensure_supported()?;
		let args = self.token_args(&req.auth, req.token.as_slice())?;
		let bytes = self.query.call_view_bytes("Entity", "entity_nym", args).await?;
		let mut cursor = &bytes.data[..];
		let decoded: core::result::Result<Vec<u8>, AuthorizationError> =
			Decode::decode(&mut cursor).map_err(|e| Error::Codec(e.to_string()))?;
		match decoded {
			Ok(raw) => Ok(Some(String::from_utf8_lossy(&raw).into_owned())),
			Err(AuthorizationError::NotFound) => Ok(None),
			Err(err) => Err(super::view_failure("entity.entity_nym", err)),
		}
	}

	pub async fn controller_account(
		&self,
		auth: &AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<AccountId32> {
		let args = self.token_args(auth, token.as_ref())?;
		let raw: core::result::Result<ViewAccount32, AuthorizationError> =
			self.query.call_result("Entity", "controller_account", args).await?;
		raw.map(|account| to_subxt_account(account))
			.map_err(|err| super::view_failure("entity.controller_account", err))
	}

	pub async fn account_history(
		&self,
		auth: &AuthorizationRequest,
		token: &Ss58Identifier,
	) -> Result<Vec<(AccountId32, BlockRef)>> {
		let args = self.token_args(auth, token.as_ref())?;
		let bytes = self.query.call_view_bytes("Entity", "account_history", args).await?;
		let mut cursor = &bytes.data[..];
		let decoded: core::result::Result<
			Vec<origin_primitives::view::EntityEventBlock>,
			AuthorizationError,
		> = Decode::decode(&mut cursor).map_err(|e| Error::Codec(e.to_string()))?;
		decoded
			.map_err(|err| super::view_failure("entity.account_history", err))
			.map(|records| {
				records
					.into_iter()
					.map(|entry| {
						(
							to_subxt_account(entry.account),
							BlockRef { height: entry.height, index: entry.index },
						)
					})
					.collect()
			})
	}

	fn token_args(
		&self,
		auth: &origin_primitives::view_api::AuthorizationRequest,
		token: &[u8],
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth_bytes", super::authorization_bytes_value(auth)?);
		builder.push("token", Value::from_bytes(token.to_vec()));
		Ok(builder.finish())
	}

	fn token_key_args(
		&self,
		auth: &origin_primitives::view_api::AuthorizationRequest,
		token: &[u8],
		key: &[u8],
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth_bytes", super::authorization_bytes_value(auth)?);
		builder.push("token", Value::from_bytes(token.to_vec()));
		builder.push("key", super::hex_arg(key));
		Ok(builder.finish())
	}

	fn token_key_version_args(
		&self,
		auth: &origin_primitives::view_api::AuthorizationRequest,
		token: &[u8],
		key: &[u8],
		version: u64,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth_bytes", super::authorization_bytes_value(auth)?);
		builder.push("token", Value::from_bytes(token.to_vec()));
		builder.push("key", super::hex_arg(key));
		builder.push("version", super::u64_value(version));
		Ok(builder.finish())
	}

	fn account_args(&self, req: &EntityAccountTokenRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth_bytes", super::authorization_bytes_value(&req.auth)?);
		builder.push("account", super::account_value(req.account.as_ref()));
		Ok(builder.finish())
	}
}

fn to_subxt_account(account: ViewAccount32) -> AccountId32 {
	let raw: [u8; 32] = account.into_inner().into();
	AccountId32::from(raw)
}

fn decode_identifier_bytes(bytes: &[u8]) -> Option<Ss58Identifier> {
	if bytes.iter().all(|b| *b <= 1) {
		return None;
	}
	if let Ok(id) = Ss58Identifier::try_from(bytes.to_vec()) {
		return Some(id);
	}
	if let Some(pos) = bytes.iter().position(|b| b.is_ascii_graphic()) {
		let ascii = &bytes[pos..];
		if let Ok(text) = String::from_utf8(ascii.to_vec()) {
			if !text.trim().is_empty() {
				if let Ok(id) = Ss58Identifier::try_from(text) {
					return Some(id);
				}
			}
		}
	}
	let raw_hex = hex::encode(bytes);
	warn!(
		target: "sdk::entity::account_token",
		"runtime returned non-SS58 token bytes; treating as None (0x{raw_hex})"
	);
	None
}
