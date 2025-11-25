use crate::{
	client::{signer::Signer, OriginClient, ViewClient},
	types::{error::OriginSdkError, RegistryStateView},
};
use origin_primitives::Ss58Identifier;

pub struct RegistryClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> RegistryClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	fn view(&self) -> ViewClient {
		self.client.view_with(self.signer.clone())
	}

	pub async fn details(
		&self,
		registry: Ss58Identifier,
	) -> Result<RegistryStateView, OriginSdkError> {
		self.view().registry().details(registry).await
	}

	pub async fn details_nested(
		&self,
		registry: Ss58Identifier,
	) -> Result<crate::schema::registry::RegistryNestedSchema, OriginSdkError> {
		let flat = self.details(registry).await?;
		Ok(crate::schema::registry::expand_registry(&flat))
	}

	pub async fn delegate_permissions(
		&self,
		registry: Ss58Identifier,
		delegate: Ss58Identifier,
	) -> Result<origin_primitives::registry::RegistryPermissions, OriginSdkError> {
		self.view().registry().delegate_permissions(registry, delegate).await
	}

	pub async fn query_count(
		&self,
		registry: Ss58Identifier,
		account: subxt::utils::AccountId32,
	) -> Result<u32, OriginSdkError> {
		self.view().registry().query_count(registry, account).await
	}

	pub async fn lookup_specs(
		&self,
		registry: Ss58Identifier,
	) -> Result<Vec<origin_primitives::registry::LookupSpec>, OriginSdkError> {
		self.view().registry().lookup_specs(registry).await
	}

	pub async fn attribute(
		&self,
		registry: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<(origin_primitives::element::ElementType, bool), OriginSdkError> {
		self.view().registry().attribute(registry, key).await
	}

	pub async fn attributes(
		&self,
		registry: Ss58Identifier,
	) -> Result<Vec<origin_primitives::registry::RegistryAttributeView>, OriginSdkError> {
		self.view().registry().attributes(registry).await
	}

	pub async fn token_specs(
		&self,
		registry: Ss58Identifier,
	) -> Result<Vec<Vec<u8>>, OriginSdkError> {
		self.view().registry().token_specs(registry).await
	}

	pub async fn packet_metadata(
		&self,
		registry: Ss58Identifier,
		packet: Ss58Identifier,
	) -> Result<origin_primitives::packet::PacketMetadataView, OriginSdkError> {
		self.view().registry().packet_metadata(registry, packet).await
	}

	pub async fn packet_snapshot(
		&self,
		registry: Ss58Identifier,
		packet: Ss58Identifier,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.view().registry().packet_snapshot(registry, packet, version).await
	}

	pub async fn overview(
		&self,
		registry: Ss58Identifier,
	) -> Result<crate::types::RegistryStateView, OriginSdkError> {
		self.view().registry().overview(registry).await
	}

	pub async fn packet_snapshot_by_token(
		&self,
		token: origin_primitives::Ss58Identifier,
		version: Option<u32>,
	) -> Result<Option<crate::types::PacketStateView>, OriginSdkError> {
		self.view().registry().packet_snapshot_by_token(token, version).await
	}

	pub async fn lookup_snapshot(
		&self,
		registry: origin_primitives::Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.view().registry().lookup_snapshot(registry, digest, version).await
	}

	pub async fn list_by_token(
		&self,
		prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<origin_primitives::Ss58Identifier>,
		limit: Option<u32>,
	) -> Result<
		(
			Vec<crate::types::PacketStateView>,
			Option<origin_primitives::Ss58Identifier>,
		),
		OriginSdkError,
	> {
		self.view().registry().list_by_token(prefix, version, cursor, limit).await
	}

	pub async fn list_by_digest(
		&self,
		digest_prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<Vec<u8>>,
		limit: Option<u32>,
	) -> Result<(Vec<crate::types::PacketStateView>, Option<Vec<u8>>), OriginSdkError>
	{
		self.view().registry().list_by_digest(digest_prefix, version, cursor, limit).await
	}
}
