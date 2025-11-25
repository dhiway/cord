use crate::{
	client::{signer::Signer, OriginClient},
	types::{
		error::OriginSdkError, LookupSpecViewSdk, PacketMetadataViewSdk, PacketStateViewSdk,
		RegistryAttributeViewSdk, RegistryStateViewSdk, RegistryStatus,
	},
};
use origin_primitives::{registry::RegistryPermissions, Ss58Identifier};

type Auth = origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
>;

pub struct RegistryClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> RegistryClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	fn view(&self) -> crate::client::ViewClient {
		self.client.view()
	}

	async fn auth(&self, function: &str) -> Result<Auth, OriginSdkError> {
		self.view().authorization_for(&self.signer, "Register", function).await
	}

	pub async fn details(
		&self,
		registry: Ss58Identifier,
	) -> Result<Option<RegistryStateViewSdk>, OriginSdkError> {
		let auth = self.auth("registry_details").await?;
		self.view().call("Register", "registry_details", (auth, registry)).await
	}

	pub async fn details_nested(
		&self,
		registry: Ss58Identifier,
	) -> Result<Option<crate::schema::registry::RegistryNestedSchema>, OriginSdkError> {
		let flat = self.details(registry).await?;
		Ok(flat.map(|f| crate::schema::registry::expand_registry(&f)))
	}

	pub async fn delegate_permissions(
		&self,
		registry: Ss58Identifier,
		delegate: Ss58Identifier,
	) -> Result<Option<RegistryPermissions>, OriginSdkError> {
		let auth = self.auth("delegate_permissions").await?;
		self.view()
			.call("Register", "delegate_permissions", (auth, registry, delegate))
			.await
	}

	pub async fn query_count(
		&self,
		registry: Ss58Identifier,
		account: subxt::utils::AccountId32,
	) -> Result<Option<u64>, OriginSdkError> {
		let auth = self.auth("query_count").await?;
		self.view().call("Register", "query_count", (auth, registry, account)).await
	}

	pub async fn lookup_specs(
		&self,
		registry: Ss58Identifier,
	) -> Result<Option<Vec<LookupSpecViewSdk>>, OriginSdkError> {
		let auth = self.auth("lookup_specs").await?;
		self.view().call("Register", "lookup_specs", (auth, registry)).await
	}

	pub async fn attribute(
		&self,
		registry: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<Option<(origin_primitives::element::ElementType, bool)>, OriginSdkError> {
		let auth = self.auth("registry_attribute").await?;
		self.view().call("Register", "registry_attribute", (auth, registry, key)).await
	}

	pub async fn attributes(
		&self,
		registry: Ss58Identifier,
	) -> Result<Option<Vec<RegistryAttributeViewSdk>>, OriginSdkError> {
		let auth = self.auth("registry_attributes").await?;
		self.view().call("Register", "registry_attributes", (auth, registry)).await
	}

	pub async fn token_specs(
		&self,
		registry: Ss58Identifier,
	) -> Result<Option<Vec<Vec<u8>>>, OriginSdkError> {
		let auth = self.auth("registry_token_specs").await?;
		self.view().call("Register", "registry_token_specs", (auth, registry)).await
	}

	pub async fn packet_metadata(
		&self,
		registry: Ss58Identifier,
		packet: Ss58Identifier,
	) -> Result<Option<PacketMetadataViewSdk>, OriginSdkError> {
		let auth = self.auth("packet_metadata").await?;
		self.view().call("Register", "packet_metadata", (auth, registry, packet)).await
	}

	pub async fn packet_state(
		&self,
		registry: Ss58Identifier,
		packet: Ss58Identifier,
		version: Option<u32>,
	) -> Result<Option<PacketStateViewSdk>, OriginSdkError> {
		let auth = self.auth("packet_state").await?;
		self.view()
			.call("Register", "packet_state", (auth, registry, packet, version))
			.await
	}

	pub async fn packet_for_token(
		&self,
		token: origin_primitives::Ss58Identifier,
		version: Option<u32>,
	) -> Result<Option<PacketStateViewSdk>, OriginSdkError> {
		let auth = self.auth("packet_for_token").await?;
		self.view().call("Register", "packet_for_token", (auth, token, version)).await
	}

	pub async fn lookup_snapshot(
		&self,
		registry: origin_primitives::Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<Option<PacketStateViewSdk>, OriginSdkError> {
		let auth = self.auth("packet_lookup_snapshot").await?;
		self.view()
			.call("Register", "packet_lookup_snapshot", (auth, registry, digest, version))
			.await
	}

	pub async fn packets_by_digest(
		&self,
		digest: Vec<u8>,
		offset: Option<u32>,
		limit: Option<u32>,
	) -> Result<Option<Vec<origin_primitives::PacketPointer>>, OriginSdkError> {
		let auth = self.auth("packets_by_digest").await?;
		self.view()
			.call("Register", "packets_by_digest", (auth, digest, offset, limit))
			.await
	}

	pub async fn list_by_token(
		&self,
		prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<origin_primitives::Ss58Identifier>,
		limit: Option<u32>,
	) -> Result<
		Option<(Vec<PacketStateViewSdk>, Option<origin_primitives::Ss58Identifier>)>,
		OriginSdkError,
	> {
		let auth = self.auth("list_by_token").await?;
		self.view()
			.call("Register", "list_by_token", (auth, prefix, version, cursor, limit))
			.await
	}

	pub async fn list_by_digest(
		&self,
		digest_prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<Vec<u8>>,
		limit: Option<u32>,
	) -> Result<Option<(Vec<PacketStateViewSdk>, Option<Vec<u8>>)>, OriginSdkError> {
		let auth = self.auth("list_by_digest").await?;
		self.view()
			.call("Register", "list_by_digest", (auth, digest_prefix, version, cursor, limit))
			.await
	}

	pub async fn registry_exists(&self, registry: Ss58Identifier) -> Result<bool, OriginSdkError> {
		let auth = self.auth("registry_exists").await?;
		self.view().call("Register", "registry_exists", (auth, registry)).await
	}

	pub async fn registry_status(
		&self,
		registry: Ss58Identifier,
	) -> Result<Option<RegistryStatus>, OriginSdkError> {
		let auth = self.auth("registry_status").await?;
		self.view().call("Register", "registry_status", (auth, registry)).await
	}

	pub async fn registry_attribute_keys(
		&self,
		registry: Ss58Identifier,
	) -> Result<Option<Vec<Vec<u8>>>, OriginSdkError> {
		let auth = self.auth("registry_attribute_keys").await?;
		self.view().call("Register", "registry_attribute_keys", (auth, registry)).await
	}

	pub async fn registry_is_active(
		&self,
		registry: Ss58Identifier,
	) -> Result<bool, OriginSdkError> {
		let auth = self.auth("registry_is_active").await?;
		self.view().call("Register", "registry_is_active", (auth, registry)).await
	}

	pub async fn registry_is_revoked(
		&self,
		registry: Ss58Identifier,
	) -> Result<bool, OriginSdkError> {
		let auth = self.auth("registry_is_revoked").await?;
		self.view().call("Register", "registry_is_revoked", (auth, registry)).await
	}

	pub async fn registry_is_deleted(
		&self,
		registry: Ss58Identifier,
	) -> Result<bool, OriginSdkError> {
		let auth = self.auth("registry_is_deleted").await?;
		self.view().call("Register", "registry_is_deleted", (auth, registry)).await
	}

	pub async fn registry_delegates(
		&self,
		registry: Ss58Identifier,
	) -> Result<Option<Vec<(Ss58Identifier, RegistryPermissions)>>, OriginSdkError> {
		let auth = self.auth("registry_delegates").await?;
		self.view().call("Register", "registry_delegates", (auth, registry)).await
	}

	pub async fn has_registry_permissions(
		&self,
		registry: Ss58Identifier,
		delegate: Ss58Identifier,
		required: RegistryPermissions,
	) -> Result<bool, OriginSdkError> {
		let auth = self.auth("has_registry_permissions").await?;
		self.view()
			.call("Register", "has_registry_permissions", (auth, registry, delegate, required))
			.await
	}

	pub async fn is_delegate(
		&self,
		registry: Ss58Identifier,
		delegate: Ss58Identifier,
	) -> Result<bool, OriginSdkError> {
		let auth = self.auth("is_delegate").await?;
		self.view().call("Register", "is_delegate", (auth, registry, delegate)).await
	}

	pub async fn registry_maintainer(
		&self,
		registry: Ss58Identifier,
	) -> Result<Option<Ss58Identifier>, OriginSdkError> {
		let auth = self.auth("registry_maintainer").await?;
		self.view().call("Register", "registry_maintainer", (auth, registry)).await
	}

	pub async fn packet_exists(
		&self,
		registry: Ss58Identifier,
		packet: Ss58Identifier,
	) -> Result<bool, OriginSdkError> {
		let auth = self.auth("packet_exists").await?;
		self.view().call("Register", "packet_exists", (auth, registry, packet)).await
	}

	pub async fn packet_status(
		&self,
		packet: Ss58Identifier,
	) -> Result<Option<origin_primitives::packet::PacketStatus>, OriginSdkError> {
		let auth = self.auth("packet_status").await?;
		self.view().call("Register", "packet_status", (auth, packet)).await
	}

	pub async fn packet_controller(
		&self,
		packet: Ss58Identifier,
	) -> Result<Option<Ss58Identifier>, OriginSdkError> {
		let auth = self.auth("packet_controller").await?;
		self.view().call("Register", "packet_controller", (auth, packet)).await
	}
}
