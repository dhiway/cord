use crate::{
	client::{signer::Signer, OriginClient, ViewClient},
	types::{
		error::OriginSdkError, AccountUnbindEntryViewSdk, AttributeHistoryEntryViewSdk,
		EntityInfoViewSdk, EntityNym, EntityStateViewSdk, EntityToken, OriginAccountId,
	},
};

pub struct EntityClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> EntityClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	fn view(&self) -> ViewClient {
		self.client.view_with(self.signer.clone())
	}

	pub async fn overview(
		&self,
		entity: EntityToken,
	) -> Result<EntityStateViewSdk, OriginSdkError> {
		self.view().entity().overview(entity).await
	}

	/// Resolve the entity token linked to an account (Ok(None) if none).
	pub async fn account_token(
		&self,
		account: OriginAccountId,
	) -> Result<Option<EntityToken>, OriginSdkError> {
		self.view().entity().account_token(account).await
	}

	/// Overview decoded then expanded to nested representation (attributes + info).
	pub async fn overview_nested(
		&self,
		entity: EntityToken,
	) -> Result<crate::schema::entity::EntityNestedValue, OriginSdkError> {
		let flat = self.overview(entity).await?;
		let (nested, _, _) = crate::schema::entity::expand_entity_state(&flat);
		Ok(nested)
	}

	pub async fn maybe_overview(
		&self,
		entity: EntityToken,
	) -> Result<Option<EntityStateViewSdk>, OriginSdkError> {
		self.view().entity().maybe_overview(entity).await
	}

	pub async fn details(&self, entity: EntityToken) -> Result<EntityInfoViewSdk, OriginSdkError> {
		self.view().entity().details(entity).await
	}

	pub async fn details_nested(
		&self,
		entity: EntityToken,
	) -> Result<crate::schema::entity::EntityNestedValue, OriginSdkError> {
		let flat = self.details(entity).await?;
		Ok(crate::schema::entity::expand_entity(&flat))
	}

	pub async fn maybe_details(
		&self,
		entity: EntityToken,
	) -> Result<Option<EntityInfoViewSdk>, OriginSdkError> {
		self.view().entity().maybe_details(entity).await
	}

	pub async fn nym(&self, entity: EntityToken) -> Result<Option<EntityNym>, OriginSdkError> {
		self.view().entity().nym(entity).await
	}

	pub async fn linked_accounts(
		&self,
		entity: EntityToken,
	) -> Result<Vec<OriginAccountId>, OriginSdkError> {
		self.view().entity().linked_accounts(entity).await
	}

	pub async fn controller_account(
		&self,
		entity: EntityToken,
	) -> Result<OriginAccountId, OriginSdkError> {
		self.view().entity().controller_account(entity).await
	}

	pub async fn account_history(
		&self,
		entity: EntityToken,
	) -> Result<Vec<AccountUnbindEntryViewSdk>, OriginSdkError> {
		self.view().entity().account_history(entity).await
	}

	pub async fn attribute_version(
		&self,
		entity: EntityToken,
		key: Vec<u8>,
	) -> Result<u64, OriginSdkError> {
		self.view().entity().attribute_version(entity, key).await
	}

	pub async fn attribute_versions(
		&self,
		entity: EntityToken,
	) -> Result<Vec<(Vec<u8>, u64)>, OriginSdkError> {
		self.view().entity().attribute_versions(entity).await
	}

	pub async fn attribute_history(
		&self,
		entity: EntityToken,
	) -> Result<Vec<AttributeHistoryEntryViewSdk>, OriginSdkError> {
		self.view().entity().attribute_history(entity).await
	}

	pub async fn attribute_history_for_key(
		&self,
		entity: EntityToken,
		key: Vec<u8>,
	) -> Result<Vec<AttributeHistoryEntryViewSdk>, OriginSdkError> {
		self.view().entity().attribute_history_for_key(entity, key).await
	}

	pub async fn attribute_history_entry(
		&self,
		entity: EntityToken,
		key: Vec<u8>,
		version: u64,
	) -> Result<AttributeHistoryEntryViewSdk, OriginSdkError> {
		self.view().entity().attribute_history_entry(entity, key, version).await
	}
}
