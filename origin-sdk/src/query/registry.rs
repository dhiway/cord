use crate::{
	client::{submit::TxOutcome, OriginClient},
	extrinsic::builder::DynamicCallBuilder,
	types::{error::OriginSdkError, RegistryStateView},
};
use origin_primitives::Ss58Identifier;
use scale_value::Value;

pub struct RegistryClient<'a> {
	client: &'a OriginClient,
}

impl<'a> RegistryClient<'a> {
	pub(crate) fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub async fn details(
		&self,
		registry: Ss58Identifier,
	) -> Result<RegistryStateView, OriginSdkError> {
		self.client.view()?.registry().details(registry).await
	}

	pub fn tx(&self) -> RegistryTx<'a> {
		RegistryTx { client: self.client }
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
	) -> Result<(Vec<crate::types::PacketStateView>, Option<Ss58Identifier>), OriginSdkError> {
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
	) -> Result<(Vec<crate::types::PacketStateView>, Option<Vec<u8>>), OriginSdkError> {
		self.client
			.view()?
			.registry()
			.list_by_digest(digest_prefix, version, cursor, limit)
			.await
	}
}

pub struct RegistryTx<'a> {
	client: &'a OriginClient,
}

impl<'a> RegistryTx<'a> {
	pub fn create(
		&self,
		registry_id: &[u8],
		info: &[u8],
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"create_registry",
			vec![Value::from_bytes(registry_id), Value::from_bytes(info)],
		)
	}

	pub async fn submit_create(
		&self,
		registry_id: &[u8],
		info: &[u8],
	) -> Result<TxOutcome, OriginSdkError> {
		let call = self.create(registry_id, info);
		self.client
			.tx()?
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	pub fn set_delegate_permissions(
		&self,
		registry: Ss58Identifier,
		delegate_account: subxt::utils::AccountId32,
		roles: Vec<origin_primitives::registry::RegistryPermissions>,
	) -> crate::extrinsic::builder::DynamicCall {
		let delegate = Value::from_bytes(delegate_account.0);
		let roles_val = Value::from(
			roles.into_iter().map(|r| Value::u128(r.bits() as u128)).collect::<Vec<_>>(),
		);
		DynamicCallBuilder::new().call(
			"Register",
			"set_delegate_permissions",
			vec![Value::from_bytes(registry.as_ref()), delegate, roles_val],
		)
	}

	pub fn remove_delegate_permissions(
		&self,
		registry: Ss58Identifier,
		delegate: Ss58Identifier,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"remove_delegate_permissions",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(delegate.as_ref())],
		)
	}

	pub fn update_registry_info(
		&self,
		registry: Ss58Identifier,
		info: &[u8],
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"update_registry_info",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(info)],
		)
	}

	pub fn revoke_registry(
		&self,
		registry: Ss58Identifier,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"revoke_registry",
			vec![Value::from_bytes(registry.as_ref())],
		)
	}

	pub fn restore_registry(
		&self,
		registry: Ss58Identifier,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"restore_registry",
			vec![Value::from_bytes(registry.as_ref())],
		)
	}

	pub fn delete_registry(
		&self,
		registry: Ss58Identifier,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"delete_registry",
			vec![Value::from_bytes(registry.as_ref())],
		)
	}
}
