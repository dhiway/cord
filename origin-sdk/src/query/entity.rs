use crate::{
	client::{signer::Signer, OriginClient},
	types::{
		error::OriginSdkError, AccountUnbindEntryViewSdk, AttributeHistoryEntryViewSdk,
		EntityInfoViewSdk, EntityNym, EntityStateViewSdk, EntityToken, OriginAccountId,
	},
};

type Auth = origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
>;

pub struct EntityClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> EntityClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	fn view(&self) -> crate::client::ViewClient {
		self.client.view()
	}

	async fn auth(&self, function: &str) -> Result<Auth, OriginSdkError> {
		self.view().authorization_for(&self.signer, "Entity", function).await
	}

	pub async fn overview(
		&self,
		entity: EntityToken,
	) -> Result<Option<EntityStateViewSdk>, OriginSdkError> {
		let auth = self.auth("overview").await?;
		self.view()
			.call("Entity", "overview", (auth, entity, Option::<u32>::None))
			.await
	}

	/// Resolve the entity token linked to an account (Ok(None) if none).
	pub async fn account_token(
		&self,
		account: OriginAccountId,
	) -> Result<Option<EntityToken>, OriginSdkError> {
		let auth = self.auth("account_token").await?;
		self.view().call("Entity", "account_token", (auth, account)).await
	}

	/// Overview decoded then expanded to nested representation (attributes + info).
	pub async fn overview_nested(
		&self,
		entity: EntityToken,
	) -> Result<Option<crate::schema::entity::EntityNestedValue>, OriginSdkError> {
		let flat = self.overview(entity).await?;
		Ok(flat.map(|f| crate::schema::entity::expand_entity_state(&f).0))
	}

	pub async fn details(
		&self,
		entity: EntityToken,
	) -> Result<Option<EntityInfoViewSdk>, OriginSdkError> {
		let auth = self.auth("details").await?;
		self.view().call("Entity", "details", (auth, entity)).await
	}

	pub async fn details_nested(
		&self,
		entity: EntityToken,
	) -> Result<Option<crate::schema::entity::EntityNestedValue>, OriginSdkError> {
		let flat = self.details(entity).await?;
		Ok(flat.map(|f| crate::schema::entity::expand_entity(&f)))
	}

	pub async fn nym(&self, entity: EntityToken) -> Result<Option<EntityNym>, OriginSdkError> {
		let auth = self.auth("entity_nym").await?;
		self.view().call("Entity", "entity_nym", (auth, entity)).await
	}

	pub async fn linked_accounts(
		&self,
		entity: EntityToken,
	) -> Result<Option<Vec<OriginAccountId>>, OriginSdkError> {
		let auth = self.auth("linked_accounts").await?;
		self.view().call("Entity", "linked_accounts", (auth, entity)).await
	}

	pub async fn linked_account_count(&self, entity: EntityToken) -> Result<u32, OriginSdkError> {
		let auth = self.auth("linked_account_count").await?;
		self.view().call("Entity", "linked_account_count", (auth, entity)).await
	}

	pub async fn controller_account(
		&self,
		entity: EntityToken,
	) -> Result<Option<OriginAccountId>, OriginSdkError> {
		let auth = self.auth("controller_account").await?;
		self.view().call("Entity", "controller_account", (auth, entity)).await
	}

	pub async fn is_controller(
		&self,
		entity: EntityToken,
		account: OriginAccountId,
	) -> Result<bool, OriginSdkError> {
		let auth = self.auth("is_controller").await?;
		self.view().call("Entity", "is_controller", (auth, entity, account)).await
	}

	pub async fn is_linked_account(
		&self,
		entity: EntityToken,
		account: OriginAccountId,
	) -> Result<bool, OriginSdkError> {
		let auth = self.auth("is_linked_account").await?;
		self.view().call("Entity", "is_linked_account", (auth, entity, account)).await
	}

	pub async fn account_history(
		&self,
		entity: EntityToken,
	) -> Result<Option<Vec<AccountUnbindEntryViewSdk>>, OriginSdkError> {
		let auth = self.auth("account_history").await?;
		self.view().call("Entity", "account_history", (auth, entity)).await
	}

	pub async fn has_nym(&self, entity: EntityToken) -> Result<bool, OriginSdkError> {
		let auth = self.auth("has_nym").await?;
		self.view().call("Entity", "has_nym", (auth, entity)).await
	}

	pub async fn token_of_nym(&self, nym: Vec<u8>) -> Result<Option<EntityToken>, OriginSdkError> {
		let auth = self.auth("token_of_nym").await?;
		self.view().call("Entity", "token_of_nym", (auth, nym)).await
	}

	pub async fn attribute_version(
		&self,
		entity: EntityToken,
		key: Vec<u8>,
	) -> Result<Option<u64>, OriginSdkError> {
		let auth = self.auth("attribute_version").await?;
		self.view().call("Entity", "attribute_version", (auth, entity, key)).await
	}

	pub async fn has_attribute(
		&self,
		entity: EntityToken,
		key: Vec<u8>,
	) -> Result<bool, OriginSdkError> {
		let auth = self.auth("has_attribute").await?;
		self.view().call("Entity", "has_attribute", (auth, entity, key)).await
	}

	pub async fn attribute_versions(
		&self,
		entity: EntityToken,
	) -> Result<Option<Vec<(Vec<u8>, u64)>>, OriginSdkError> {
		let auth = self.auth("attribute_versions").await?;
		self.view().call("Entity", "attribute_versions", (auth, entity)).await
	}

	pub async fn attribute_history(
		&self,
		entity: EntityToken,
	) -> Result<Option<Vec<AttributeHistoryEntryViewSdk>>, OriginSdkError> {
		let auth = self.auth("attribute_history").await?;
		self.view().call("Entity", "attribute_history", (auth, entity)).await
	}

	pub async fn attribute_history_for_key(
		&self,
		entity: EntityToken,
		key: Vec<u8>,
	) -> Result<Option<Vec<AttributeHistoryEntryViewSdk>>, OriginSdkError> {
		let auth = self.auth("attribute_history_for_key").await?;
		self.view()
			.call("Entity", "attribute_history_for_key", (auth, entity, key))
			.await
	}

	pub async fn attribute_history_entry(
		&self,
		entity: EntityToken,
		key: Vec<u8>,
		version: u64,
	) -> Result<Option<AttributeHistoryEntryViewSdk>, OriginSdkError> {
		let auth = self.auth("attribute_version_history").await?;
		self.view()
			.call("Entity", "attribute_version_history", (auth, entity, key, version))
			.await
	}

	pub async fn entity_attribute_keys(
		&self,
		entity: EntityToken,
	) -> Result<Option<Vec<Vec<u8>>>, OriginSdkError> {
		let auth = self.auth("entity_attribute_keys").await?;
		self.view().call("Entity", "entity_attribute_keys", (auth, entity)).await
	}
}
