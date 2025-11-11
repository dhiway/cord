use super::{ArgBuilder, Query};
use crate::error::{Error, Result};
use cord_primitives::{
	identifier::Ss58Identifier,
	view::InfoAttributeHistoryEntry,
	view_api::{
		EntityAccountTokenRequest, EntityAttributeHistoryEntryRequest,
		EntityAttributeHistoryForKeyRequest, EntityAttributeHistoryRequest, EntityInfoBytesRequest,
		EntitySubAccountsRequest,
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
		let value = self
			.query
			.call_optional::<Vec<InfoAttributeHistoryEntry>>(
				"Entity",
				"attribute_history_entries",
				args,
			)
			.await?;
		value
			.ok_or_else(|| Error::NotFound("entity.attribute_history_entries returned none".into()))
	}

	pub async fn attribute_history_for_key_entries(
		&self,
		req: &EntityAttributeHistoryForKeyRequest,
	) -> Result<Vec<InfoAttributeHistoryEntry>> {
		let args = self.token_key_args(&req.auth, &req.token, req.key.as_slice())?;
		let value = self
			.query
			.call_optional::<Vec<InfoAttributeHistoryEntry>>(
				"Entity",
				"attribute_history_for_key_entries",
				args,
			)
			.await?;
		value.ok_or_else(|| {
			Error::NotFound("entity.attribute_history_for_key_entries returned none".into())
		})
	}

	pub async fn attribute_history_entry(
		&self,
		req: &EntityAttributeHistoryEntryRequest,
	) -> Result<InfoAttributeHistoryEntry> {
		let args =
			self.token_key_version_args(&req.auth, &req.token, req.key.as_slice(), req.version)?;
		let value = self
			.query
			.call_optional::<InfoAttributeHistoryEntry>(
				"Entity",
				"attribute_history_entry_view",
				args,
			)
			.await?;
		value.ok_or_else(|| {
			Error::NotFound("entity.attribute_history_entry_view returned none".into())
		})
	}

	pub async fn account_token(&self, req: &EntityAccountTokenRequest) -> Result<Option<String>> {
		let args = self.account_args(req)?;
		let value = self
			.query
			.call_optional::<Ss58Identifier>("Entity", "account_token", args)
			.await?;
		Ok(value.map(|id| ss58_string(&id)))
	}

	pub async fn entity_info_bytes(&self, req: &EntityInfoBytesRequest) -> Result<Option<Vec<u8>>> {
		let args = self.token_args(&req.auth, &req.token)?;
		self.query.call_optional::<Vec<u8>>("Entity", "entity_info_bytes", args).await
	}

	pub async fn sub_accounts(&self, req: &EntitySubAccountsRequest) -> Result<Vec<AccountId32>> {
		let args = self.token_args(&req.auth, &req.token)?;
		let value = self
			.query
			.call_optional::<Vec<AccountId32>>("Entity", "sub_accounts", args)
			.await?;
		Ok(value.unwrap_or_default())
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
