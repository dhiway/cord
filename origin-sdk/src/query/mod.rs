use crate::client::submit::TxOutcome;
use crate::client::OriginClient;
use crate::extrinsic::builder::DynamicCallBuilder;
use crate::types::error::OriginSdkError;
use crate::types::{EntityInfoView, EntityOverview, PacketStateView, RegistryStateView};
use origin_primitives::{PacketPointer, Ss58Identifier};
use scale_value::Value;

/// Unified query facade regrouped by pallet.
pub struct Query<'a> {
	client: &'a OriginClient,
}

impl<'a> Query<'a> {
	pub fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn entity(&self) -> EntityClient<'a> {
		EntityClient { client: self.client }
	}

	pub fn registry(&self) -> RegistryClient<'a> {
		RegistryClient { client: self.client }
	}

	pub fn packet(&self) -> PacketClient<'a> {
		PacketClient { client: self.client }
	}

	pub fn token(&self) -> TokenClient<'a> {
		TokenClient { client: self.client }
	}
}

pub struct EntityClient<'a> {
	client: &'a OriginClient,
}

impl<'a> EntityClient<'a> {
	pub async fn overview(&self, entity: Ss58Identifier) -> Result<EntityOverview, OriginSdkError> {
		self.client.view()?.entity().overview(entity).await.map(EntityOverview::from)
	}

	pub async fn details(&self, entity: Ss58Identifier) -> Result<EntityInfoView, OriginSdkError> {
		self.client.view()?.entity().details(entity).await
	}

	pub fn tx(&self) -> EntityTx<'a> {
		EntityTx { client: self.client }
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
	) -> Result<Vec<origin_primitives::entity::AttributeHistoryEntryView>, OriginSdkError> {
		self.client.view()?.entity().attribute_history(entity).await
	}

	pub async fn attribute_history_for_key(
		&self,
		entity: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<Vec<origin_primitives::entity::AttributeHistoryEntryView>, OriginSdkError> {
		self.client.view()?.entity().attribute_history_for_key(entity, key).await
	}

	pub async fn attribute_history_entry(
		&self,
		entity: Ss58Identifier,
		key: Vec<u8>,
		version: u64,
	) -> Result<origin_primitives::entity::AttributeHistoryEntryView, OriginSdkError> {
		self.client.view()?.entity().attribute_history_entry(entity, key, version).await
	}
}

pub struct EntityTx<'a> {
	client: &'a OriginClient,
}

impl<'a> EntityTx<'a> {
	pub fn set_info(&self, info: Value) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call("Entity", "set_info", vec![info])
	}

	pub fn rotate_attribute(
		&self,
		entity: Ss58Identifier,
		key: impl AsRef<[u8]>,
		value: Value,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"rotate_attribute",
			vec![Value::from_bytes(entity.as_ref()), Value::from_bytes(key.as_ref()), value],
		)
	}

	pub async fn submit_set_info(&self, info: Value) -> Result<TxOutcome, OriginSdkError> {
		let call = self.set_info(info);
		self.client
			.tx()?
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	pub async fn submit_rotate_attribute(
		&self,
		entity: Ss58Identifier,
		key: impl AsRef<[u8]>,
		value: Value,
	) -> Result<TxOutcome, OriginSdkError> {
		let call = self.rotate_attribute(entity, key, value);
		self.client
			.tx()?
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	pub fn set_entity_nym(&self, prefix: &str) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"set_entity_nym",
			vec![Value::from_bytes(prefix.as_bytes())],
		)
	}

	pub async fn submit_set_entity_nym(&self, prefix: &str) -> Result<TxOutcome, OriginSdkError> {
		let call = self.set_entity_nym(prefix);
		self.client
			.tx()?
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	pub fn rotate_attributes(
		&self,
		ops: Vec<(Vec<u8>, Value)>,
	) -> crate::extrinsic::builder::DynamicCall {
		let val = attrs_to_value(&ops);
		DynamicCallBuilder::new().call("Entity", "rotate_attributes", vec![val])
	}

	pub fn add_attributes(
		&self,
		ops: Vec<(Vec<u8>, Value)>,
	) -> crate::extrinsic::builder::DynamicCall {
		let val = attrs_to_value(&ops);
		DynamicCallBuilder::new().call("Entity", "add_attributes", vec![val])
	}

	pub fn remove_attribute(
		&self,
		key: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"remove_attribute",
			vec![Value::from_bytes(key.as_ref())],
		)
	}

	pub fn set_linked_account(
		&self,
		account: subxt::utils::AccountId32,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"set_linked_account",
			vec![Value::from_bytes(account.0)],
		)
	}

	pub fn revoke_linked_account(
		&self,
		account: subxt::utils::AccountId32,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"revoke_linked_account",
			vec![Value::from_bytes(account.0)],
		)
	}

	pub fn revoke_linked_account_for(
		&self,
		token: Ss58Identifier,
		account: subxt::utils::AccountId32,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"revoke_linked_account_for",
			vec![Value::from_bytes(token.as_ref()), Value::from_bytes(account.0)],
		)
	}

	pub fn rotate_controller(
		&self,
		token: Ss58Identifier,
		new_controller: subxt::utils::AccountId32,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"rotate_controller",
			vec![Value::from_bytes(token.as_ref()), Value::from_bytes(new_controller.0)],
		)
	}

	pub fn rotate_controller_for(
		&self,
		token: Ss58Identifier,
		new_controller: subxt::utils::AccountId32,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"rotate_controller_for",
			vec![Value::from_bytes(token.as_ref()), Value::from_bytes(new_controller.0)],
		)
	}

	pub fn clear_everything(
		&self,
		token: Ss58Identifier,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"clear_everything",
			vec![Value::from_bytes(token.as_ref())],
		)
	}

	pub fn clear_everything_for(
		&self,
		token: Ss58Identifier,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"clear_everything_for",
			vec![Value::from_bytes(token.as_ref())],
		)
	}

	pub fn remove_entity_nym(
		&self,
		token: Ss58Identifier,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"remove_entity_nym",
			vec![Value::from_bytes(token.as_ref())],
		)
	}
}

pub struct RegistryClient<'a> {
	client: &'a OriginClient,
}

impl<'a> RegistryClient<'a> {
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
	) -> Result<PacketStateView, OriginSdkError> {
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
	) -> Result<Option<PacketStateView>, OriginSdkError> {
		self.client.view()?.registry().packet_snapshot_by_token(token, version).await
	}

	pub async fn lookup_snapshot(
		&self,
		registry: Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<PacketStateView, OriginSdkError> {
		self.client.view()?.registry().lookup_snapshot(registry, digest, version).await
	}

	pub async fn list_by_token(
		&self,
		prefix: Vec<u8>,
		version: Option<u32>,
		cursor: Option<Ss58Identifier>,
		limit: Option<u32>,
	) -> Result<(Vec<PacketStateView>, Option<Ss58Identifier>), OriginSdkError> {
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
	) -> Result<(Vec<PacketStateView>, Option<Vec<u8>>), OriginSdkError> {
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
			"create",
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

pub struct PacketClient<'a> {
	client: &'a OriginClient,
}

impl<'a> PacketClient<'a> {
	pub async fn state(
		&self,
		packet: PacketPointer,
		version: Option<u32>,
	) -> Result<PacketStateView, OriginSdkError> {
		self.client.view()?.packet().state(packet, version).await
	}

	pub fn tx(&self) -> PacketTx<'a> {
		PacketTx { client: self.client }
	}
}

pub struct PacketTx<'a> {
	client: &'a OriginClient,
}

impl<'a> PacketTx<'a> {
	pub fn issue(
		&self,
		registry: impl AsRef<[u8]>,
		body: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Packet",
			"issue",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(body.as_ref())],
		)
	}

	pub async fn submit_issue(
		&self,
		registry: impl AsRef<[u8]>,
		body: impl AsRef<[u8]>,
	) -> Result<TxOutcome, OriginSdkError> {
		let call = self.issue(registry, body);
		self.client
			.tx()?
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	pub fn update(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
		attributes: Value,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"update_packet",
			vec![
				Value::from_bytes(registry.as_ref()),
				Value::from_bytes(packet.as_ref()),
				attributes,
			],
		)
	}

	pub fn revoke(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"revoke_packet",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(packet.as_ref())],
		)
	}

	pub fn restore(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"restore_packet",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(packet.as_ref())],
		)
	}

	pub fn remove(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"remove_packet",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(packet.as_ref())],
		)
	}
}

pub struct TokenClient<'a> {
	client: &'a OriginClient,
}

impl<'a> TokenClient<'a> {
	pub async fn timeline(
		&self,
		token: Ss58Identifier,
		start: Option<u32>,
		limit: Option<u32>,
	) -> Result<crate::types::TokenTimelineView, OriginSdkError> {
		self.client.view()?.token().timeline(token, start, limit).await
	}

	pub async fn resolve_identifier(
		&self,
		token: Ss58Identifier,
	) -> Result<crate::types::TokenLookupView, OriginSdkError> {
		self.client.view()?.token().resolve_identifier(token).await
	}

	pub async fn pallet_index_of(&self, name: Vec<u8>) -> Result<u16, OriginSdkError> {
		self.client.view()?.token().pallet_index_of(name).await
	}

	pub async fn pallet_name(&self, index: u16) -> Result<Vec<u8>, OriginSdkError> {
		self.client.view()?.token().pallet_name(index).await
	}

	pub async fn next_pallet_index(&self) -> Result<u16, OriginSdkError> {
		self.client.view()?.token().next_pallet_index().await
	}

	pub async fn genesis_network_id(&self) -> Result<u32, OriginSdkError> {
		self.client.view()?.token().genesis_network_id().await
	}

	pub async fn state_version(&self, token: Ss58Identifier) -> Result<u32, OriginSdkError> {
		self.client.view()?.token().state_version(token).await
	}

	pub async fn state_event(
		&self,
		token: Ss58Identifier,
		version: u32,
	) -> Result<origin_primitives::token::TokenStateEventView<subxt::utils::H256>, OriginSdkError>
	{
		self.client.view()?.token().state_event(token, version).await
	}

	pub async fn resolve_pallet(&self, index: u16) -> Result<Vec<u8>, OriginSdkError> {
		self.client.view()?.token().resolve_pallet(index).await
	}
}

fn attrs_to_value(entries: &[(Vec<u8>, Value)]) -> Value {
	let pairs: Vec<Value> = entries
		.iter()
		.map(|(k, v)| Value::unnamed_composite(vec![Value::from_bytes(k), v.clone()]))
		.collect();
	Value::from(pairs)
}
