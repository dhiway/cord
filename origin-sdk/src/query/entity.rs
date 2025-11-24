use crate::{
	client::{signer::Signer, submit::TxOutcome, OriginClient},
	extrinsic::builder::DynamicCallBuilder,
	types::{error::OriginSdkError, EntityInfoView, EntityStateView},
};
use origin_primitives::Ss58Identifier;
use scale_value::Value;
use serde_json::Value as JsonValue;

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

	pub async fn details(&self, entity: Ss58Identifier) -> Result<EntityInfoView, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().details(entity).await
	}

	pub fn tx(&self) -> EntityTxWithSigner<'a, S> {
		EntityTxWithSigner { client: self.client, signer: self.signer.clone() }
	}

	pub async fn nym(&self, entity: Ss58Identifier) -> Result<Option<Vec<u8>>, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().nym(entity).await
	}

	pub async fn linked_accounts(
		&self,
		entity: Ss58Identifier,
	) -> Result<Vec<subxt::utils::AccountId32>, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().linked_accounts(entity).await
	}

	pub async fn controller_account(
		&self,
		entity: Ss58Identifier,
	) -> Result<subxt::utils::AccountId32, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().controller_account(entity).await
	}

	pub async fn account_history(
		&self,
		entity: Ss58Identifier,
	) -> Result<
		Vec<origin_primitives::entity::AccountUnbindEntryView<subxt::utils::AccountId32>>,
		OriginSdkError,
	> {
		self.client.view_with(self.signer.clone()).entity().account_history(entity).await
	}

	pub async fn attribute_version(
		&self,
		entity: Ss58Identifier,
		key: Vec<u8>,
	) -> Result<u64, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().attribute_version(entity, key).await
	}

	pub async fn attribute_versions(
		&self,
		entity: Ss58Identifier,
	) -> Result<Vec<(Vec<u8>, u64)>, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().attribute_versions(entity).await
	}

	pub async fn attribute_history(
		&self,
		entity: Ss58Identifier,
	) -> Result<Vec<origin_primitives::AttributeHistoryEntryView>, OriginSdkError> {
		self.client.view_with(self.signer.clone()).entity().attribute_history(entity).await
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

	/// Accept raw JSON attribute value, map to raw bytes for now (placeholder).
	pub async fn submit_rotate_attribute_json(
		&self,
		entity: Ss58Identifier,
		key: &str,
		value: &JsonValue,
	) -> Result<TxOutcome, OriginSdkError> {
		let elem = crate::util::codec::element_value_from_json(
			origin_primitives::element::ElementType::Raw,
			value,
		)?;
		let call = DynamicCallBuilder::new().call(
			"Entity",
			"rotate_attribute",
			vec![Value::from_bytes(entity.as_ref()), Value::from_bytes(key.as_bytes()), elem],
		);
		self.client
			.tx()?
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	/// Bulk rotate based on provided schema (key, type, optional) from views.
	pub async fn submit_rotate_from_object(
		&self,
		entity: Ss58Identifier,
		schema: &[(Vec<u8>, origin_primitives::element::ElementType, bool)],
		obj: &JsonValue,
	) -> Result<TxOutcome, OriginSdkError> {
		let calls = crate::extrinsic::calls::entity::rotate_attributes_from_json(
			&self.client.metadata(),
			entity,
			schema,
			obj,
		)?;
		// Convert DynamicPayloads into DynamicCalls to reuse batch builder.
		let dyn_calls: Vec<_> = calls
			.into_iter()
			.map(|p| crate::extrinsic::builder::DynamicCall {
				pallet: "Utility".into(), // actual pallet name embedded in payload, but batch expects Value; we wrap directly.
				function: "batch".into(),
				args: vec![p.into_value()],
			})
			.collect();
		let handle = self.client.tx()?.batch().call_many(dyn_calls).submit_and_wait_finalized().await?;
		Ok(handle)
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

pub struct EntityTxWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> EntityTxWithSigner<'a, S> {
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

	pub async fn submit_rotate_attribute(
		&self,
		entity: Ss58Identifier,
		key: impl AsRef<[u8]>,
		value: Value,
	) -> Result<TxOutcome, OriginSdkError> {
		let call = self.rotate_attribute(entity, key, value);
		self.client
			.tx_with(self.signer.clone())
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	pub async fn submit_set_info(&self, info: Value) -> Result<TxOutcome, OriginSdkError> {
		let call = self.set_info(info);
		self.client
			.tx_with(self.signer.clone())
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}
}

fn attrs_to_value(entries: &[(Vec<u8>, Value)]) -> Value {
	let pairs: Vec<Value> = entries
		.iter()
		.map(|(k, v)| Value::unnamed_composite(vec![Value::from_bytes(k), v.clone()]))
		.collect();
	Value::from(pairs)
}
