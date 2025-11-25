use crate::{
	client::{signer::Signer, OriginClient},
	types::{error::OriginSdkError, RegistryStateView},
};
use origin_primitives::Ss58Identifier;

pub struct RegistryClient<'a> {
	client: &'a OriginClient,
}

impl<'a> RegistryClient<'a> {
	pub(crate) fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using<S>(&self, signer: S) -> RegistryClientWithSigner<'a, S>
	where
		S: Signer + Clone + 'static,
	{
		RegistryClientWithSigner::new(self.client, signer)
	}

	pub async fn details(
		&self,
		registry: Ss58Identifier,
	) -> Result<RegistryStateView, OriginSdkError> {
		self.client.view()?.registry().details(registry).await
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
		self.client.view()?.registry().delegate_permissions(registry, delegate).await
	}

	pub async fn query_count(
		&self,
		registry: Ss58Identifier,
		account: subxt::utils::AccountId32,
	) -> Result<u32, OriginSdkError> {
		self.client.view()?.registry().query_count(registry, account).await
	}

	pub async fn lookup_specs(
		&self,
		registry: Ss58Identifier,
	) -> Result<Vec<origin_primitives::registry::LookupSpec>, OriginSdkError> {
		self.client.view()?.registry().lookup_specs(registry).await
	}

	pub async fn attribute(
		&self,
		registry: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<(origin_primitives::element::ElementType, bool), OriginSdkError> {
		self.client.view()?.registry().attribute(registry, key).await
	}

	pub async fn attributes(
		&self,
		registry: Ss58Identifier,
	) -> Result<Vec<(Vec<u8>, origin_primitives::element::ElementType, bool)>, OriginSdkError> {
		self.client.view()?.registry().attributes(registry).await
	}

	pub async fn token_specs(
		&self,
		registry: Ss58Identifier,
	) -> Result<Vec<Vec<u8>>, OriginSdkError> {
		self.client.view()?.registry().token_specs(registry).await
	}

	pub async fn packet_metadata(
		&self,
		registry: Ss58Identifier,
		packet: Ss58Identifier,
	) -> Result<origin_primitives::packet::PacketMetadataView, OriginSdkError> {
		self.client.view()?.registry().packet_metadata(registry, packet).await
	}

	pub async fn packet_snapshot(
		&self,
		registry: Ss58Identifier,
		packet: Ss58Identifier,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.client.view()?.registry().packet_snapshot(registry, packet, version).await
	}

	pub async fn overview(
		&self,
		registry: Ss58Identifier,
	) -> Result<RegistryStateView, OriginSdkError> {
		self.client.view()?.registry().overview(registry).await
	}

	pub async fn overview_nested(
		&self,
		registry: Ss58Identifier,
	) -> Result<crate::schema::registry::RegistryNestedSchema, OriginSdkError> {
		let flat = self.overview(registry).await?;
		Ok(crate::schema::registry::expand_registry(&flat))
	}

	pub async fn packet_snapshot_by_token(
		&self,
		token: Ss58Identifier,
		version: Option<u32>,
	) -> Result<Option<crate::types::PacketStateView>, OriginSdkError> {
		self.client.view()?.registry().packet_snapshot_by_token(token, version).await
	}

	pub async fn lookup_snapshot(
		&self,
		registry: Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.client.view()?.registry().lookup_snapshot(registry, digest, version).await
	}

	pub async fn list_by_token(
		&self,
		prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<Ss58Identifier>,
		limit: Option<u32>,
	) -> Result<(Vec<crate::types::PacketSnapshot>, Option<Ss58Identifier>), OriginSdkError> {
		self.client
			.view()?
			.registry()
			.list_by_token(prefix, version, cursor, limit)
			.await
	}

	pub async fn list_by_digest(
		&self,
		digest_prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<Vec<u8>>,
		limit: Option<u32>,
	) -> Result<(Vec<crate::types::PacketSnapshot>, Option<Vec<u8>>), OriginSdkError> {
		self.client
			.view()?
			.registry()
			.list_by_digest(digest_prefix, version, cursor, limit)
			.await
	}
}

pub struct RegistryClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> RegistryClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	pub async fn details(
		&self,
		registry: Ss58Identifier,
	) -> Result<RegistryStateView, OriginSdkError> {
		self.client.view_with(self.signer.clone()).registry().details(registry).await
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
		self.client
			.view_with(self.signer.clone())
			.registry()
			.delegate_permissions(registry, delegate)
			.await
	}

	pub async fn query_count(
		&self,
		registry: Ss58Identifier,
		account: subxt::utils::AccountId32,
	) -> Result<u32, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.query_count(registry, account)
			.await
	}

	pub async fn lookup_specs(
		&self,
		registry: Ss58Identifier,
	) -> Result<Vec<origin_primitives::registry::LookupSpec>, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.lookup_specs(registry)
			.await
	}

	pub async fn attribute(
		&self,
		registry: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<(origin_primitives::element::ElementType, bool), OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.attribute(registry, key)
			.await
	}

	pub async fn attributes(
		&self,
		registry: Ss58Identifier,
	) -> Result<Vec<(Vec<u8>, origin_primitives::element::ElementType, bool)>, OriginSdkError> {
		self.client.view_with(self.signer.clone()).registry().attributes(registry).await
	}

	pub async fn token_specs(
		&self,
		registry: Ss58Identifier,
	) -> Result<Vec<Vec<u8>>, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.token_specs(registry)
			.await
	}

	pub async fn packet_metadata(
		&self,
		registry: Ss58Identifier,
		packet: Ss58Identifier,
	) -> Result<origin_primitives::packet::PacketMetadataView, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.packet_metadata(registry, packet)
			.await
	}

	pub async fn packet_snapshot(
		&self,
		registry: Ss58Identifier,
		packet: Ss58Identifier,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.packet_snapshot(registry, packet, version)
			.await
	}

	pub async fn overview(
		&self,
		registry: Ss58Identifier,
	) -> Result<RegistryStateView, OriginSdkError> {
		self.client.view_with(self.signer.clone()).registry().overview(registry).await
	}

	pub async fn overview_nested(
		&self,
		registry: Ss58Identifier,
	) -> Result<crate::schema::registry::RegistryNestedSchema, OriginSdkError> {
		let flat = self.overview(registry).await?;
		Ok(crate::schema::registry::expand_registry(&flat))
	}

	pub async fn packet_snapshot_by_token(
		&self,
		token: Ss58Identifier,
		version: Option<u32>,
	) -> Result<Option<crate::types::PacketStateView>, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.packet_snapshot_by_token(token, version)
			.await
	}

	pub async fn lookup_snapshot(
		&self,
		registry: Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<crate::types::PacketStateView, OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.lookup_snapshot(registry, digest, version)
			.await
	}

	pub async fn list_by_token(
		&self,
		prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<Ss58Identifier>,
		limit: Option<u32>,
	) -> Result<(Vec<crate::types::PacketSnapshot>, Option<Ss58Identifier>), OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.list_by_token(prefix, version, cursor, limit)
			.await
	}

	pub async fn list_by_digest(
		&self,
		digest_prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<Vec<u8>>,
		limit: Option<u32>,
	) -> Result<(Vec<crate::types::PacketSnapshot>, Option<Vec<u8>>), OriginSdkError> {
		self.client
			.view_with(self.signer.clone())
			.registry()
			.list_by_digest(digest_prefix, version, cursor, limit)
			.await
	}
}
