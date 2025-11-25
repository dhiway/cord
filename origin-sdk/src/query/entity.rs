use crate::{
	client::{signer::Signer, OriginClient},
	types::{error::OriginSdkError, EntityInfoView, EntityStateView},
};
use origin_primitives::Ss58Identifier;

pub struct EntityClient<'a> {
	client: &'a OriginClient,
}

impl<'a> EntityClient<'a> {
	pub(crate) fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using<S>(&self, signer: S) -> EntityClientWithSigner<'a, S>
	where
		S: Signer + Clone + 'static,
	{
		EntityClientWithSigner::new(self.client, signer)
	}

	pub async fn overview(
		&self,
		entity: Ss58Identifier,
	) -> Result<EntityStateView, OriginSdkError> {
		self.client.view()?.entity().overview(entity).await
	}

	/// Overview decoded then expanded to nested representation (attributes + info).
	pub async fn overview_nested(
		&self,
		entity: Ss58Identifier,
	) -> Result<crate::schema::entity::EntityNestedValue, OriginSdkError> {
		let flat = self.overview(entity).await?;
		let (nested, _, _) = crate::schema::entity::expand_entity_state(&flat);
		Ok(nested)
	}

	pub async fn maybe_overview(
		&self,
		entity: Ss58Identifier,
	) -> Result<Option<EntityStateView>, OriginSdkError> {
		self.client.view()?.entity().maybe_overview(entity).await
	}

	pub async fn details(&self, entity: Ss58Identifier) -> Result<EntityInfoView, OriginSdkError> {
		self.client.view()?.entity().details(entity).await
	}

	pub async fn details_nested(
		&self,
		entity: Ss58Identifier,
	) -> Result<crate::schema::entity::EntityNestedValue, OriginSdkError> {
		let flat = self.details(entity).await?;
		Ok(crate::schema::entity::expand_entity(&flat))
	}

	pub async fn maybe_details(
		&self,
		entity: Ss58Identifier,
	) -> Result<Option<EntityInfoView>, OriginSdkError> {
		self.client.view()?.entity().maybe_details(entity).await
	}

	pub async fn nym(&self, entity: Ss58Identifier) -> Result<Option<Vec<u8>>, OriginSdkError> {
		self.client.view()?.entity().nym(entity).await
	}

	pub async fn linked_accounts(
		&self,
		entity: Ss58Identifier,
	) -> Result<Vec<subxt::utils::AccountId32>, OriginSdkError> {
		self.client.view()?.entity().linked_accounts(entity).await
	}

	pub async fn controller_account(
		&self,
		entity: Ss58Identifier,
	) -> Result<subxt::utils::AccountId32, OriginSdkError> {
		self.client.view()?.entity().controller_account(entity).await
	}

	pub async fn account_history(
		&self,
		entity: Ss58Identifier,
	) -> Result<
		Vec<origin_primitives::entity::AccountUnbindEntryView<subxt::utils::AccountId32>>,
		OriginSdkError,
	> {
		self.client.view()?.entity().account_history(entity).await
	}

	pub async fn attribute_version(
		&self,
		entity: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<u64, OriginSdkError> {
		self.client.view()?.entity().attribute_version(entity, key).await
	}

	pub async fn attribute_versions(
		&self,
		entity: Ss58Identifier,
	) -> Result<Vec<(Vec<u8>, u64)>, OriginSdkError> {
		self.client.view()?.entity().attribute_versions(entity).await
	}

	pub async fn attribute_history(
		&self,
		entity: Ss58Identifier,
	) -> Result<Vec<origin_primitives::AttributeHistoryEntryView>, OriginSdkError> {
		self.client.view()?.entity().attribute_history(entity).await
	}

	pub async fn attribute_history_for_key(
		&self,
		entity: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<Vec<origin_primitives::AttributeHistoryEntryView>, OriginSdkError> {
		self.client.view()?.entity().attribute_history_for_key(entity, key).await
	}

	pub async fn attribute_history_entry(
		&self,
		entity: Ss58Identifier,
		key: Vec<u8>,
		version: u64,
	) -> Result<origin_primitives::AttributeHistoryEntryView, OriginSdkError> {
		self.client.view()?.entity().attribute_history_entry(entity, key, version).await
	}
}

pub struct EntityClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> EntityClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	pub async fn overview(
		&self,
		entity: Ss58Identifier,
	) -> Result<EntityStateView, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().overview(entity).await
	}

	pub async fn overview_nested(
		&self,
		entity: Ss58Identifier,
	) -> Result<crate::schema::entity::EntityNestedValue, OriginSdkError> {
		let flat = self.overview(entity).await?;
		let (nested, _, _) = crate::schema::entity::expand_entity_state(&flat);
		Ok(nested)
	}

	pub async fn maybe_overview(
		&self,
		entity: Ss58Identifier,
	) -> Result<Option<EntityStateView>, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().maybe_overview(entity).await
	}

	pub async fn details(&self, entity: Ss58Identifier) -> Result<EntityInfoView, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().details(entity).await
	}

	pub async fn details_nested(
		&self,
		entity: Ss58Identifier,
	) -> Result<crate::schema::entity::EntityNestedValue, OriginSdkError> {
		let flat = self.details(entity).await?;
		Ok(crate::schema::entity::expand_entity(&flat))
	}

	pub async fn maybe_details(
		&self,
		entity: Ss58Identifier,
	) -> Result<Option<EntityInfoView>, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().maybe_details(entity).await
	}

	pub async fn nym(&self, entity: Ss58Identifier) -> Result<Option<Vec<u8>>, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().nym(entity).await
	}

	pub async fn linked_accounts(
		&self,
		entity: Ss58Identifier,
	) -> Result<Vec<subxt::utils::AccountId32>, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.entity()
			.linked_accounts(entity)
			.await
	}

	pub async fn controller_account(
		&self,
		entity: Ss58Identifier,
	) -> Result<subxt::utils::AccountId32, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.entity()
			.controller_account(entity)
			.await
	}

	pub async fn account_history(
		&self,
		entity: Ss58Identifier,
	) -> Result<
		Vec<origin_primitives::entity::AccountUnbindEntryView<subxt::utils::AccountId32>>,
		OriginSdkError,
	> {
		self.client
			.view_with(self.signer.clone())
			.entity()
			.account_history(entity)
			.await
	}

	pub async fn attribute_version(
		&self,
		entity: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<u64, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.entity()
			.attribute_version(entity, key)
			.await
	}

	pub async fn attribute_versions(
		&self,
		entity: Ss58Identifier,
	) -> Result<Vec<(Vec<u8>, u64)>, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.entity()
			.attribute_versions(entity)
			.await
	}

	pub async fn attribute_history(
		&self,
		entity: Ss58Identifier,
	) -> Result<Vec<origin_primitives::AttributeHistoryEntryView>, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.entity()
			.attribute_history(entity)
			.await
	}

	pub async fn attribute_history_for_key(
		&self,
		entity: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<Vec<origin_primitives::AttributeHistoryEntryView>, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.entity()
			.attribute_history_for_key(entity, key)
			.await
	}

	pub async fn attribute_history_entry(
		&self,
		entity: Ss58Identifier,
		key: Vec<u8>,
		version: u64,
	) -> Result<origin_primitives::AttributeHistoryEntryView, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.entity()
			.attribute_history_entry(entity, key, version)
			.await
	}
}
