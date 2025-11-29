use crate::{
	extrinsic::{builder::DynamicCallBuilder, calls::packet as packet_calls},
	tx::{handle::TxHandle, AccountTx},
	types::error::OriginSdkError,
};
use origin_primitives::Ss58Identifier;
use scale_value::Value;

pub struct RegistryTx<'a> {
	account: &'a AccountTx,
}

impl<'a> RegistryTx<'a> {
	pub(crate) fn new(account: &'a AccountTx) -> Self {
		Self { account }
	}

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
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.create(registry_id, info);
		let payload = subxt::dynamic::tx(call.pallet, call.function, call.args);
		self.account.submit(payload).await
	}

	pub fn set_delegate_permissions(
		&self,
		input: &crate::types::registry::DelegatePermissionsInput,
	) -> Result<subxt::tx::DynamicPayload, OriginSdkError> {
		crate::extrinsic::calls::registry::set_delegate_permissions_from_input(
			&self.account.client().metadata(),
			input,
		)
	}

	pub fn remove_delegate_permissions(
		&self,
		input: &crate::types::registry::RemoveDelegatePermissionsInput,
	) -> Result<subxt::tx::DynamicPayload, OriginSdkError> {
		crate::extrinsic::calls::registry::remove_delegate_permissions_from_input(
			&self.account.client().metadata(),
			input,
		)
	}

	pub async fn submit_update_registry_info_from_view(
		&self,
		registry: Ss58Identifier,
		info: origin_primitives::element::ElementView,
	) -> Result<TxHandle, OriginSdkError> {
		let elem = crate::schema::registry::element_from_view(&info)?;
		let payload = crate::extrinsic::calls::registry::update_info_from_input(
			&self.account.client().metadata(),
			registry.as_ref(),
			&elem,
		)?;
		self.account.submit(payload).await
	}

	pub async fn submit_set_delegate_permissions(
		&self,
		input: &crate::types::registry::DelegatePermissionsInput,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::registry::set_delegate_permissions_from_input(
			&self.account.client().metadata(),
			input,
		)?;
		self.account.submit(payload).await
	}

	pub async fn submit_remove_delegate_permissions(
		&self,
		input: &crate::types::registry::RemoveDelegatePermissionsInput,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::registry::remove_delegate_permissions_from_input(
			&self.account.client().metadata(),
			input,
		)?;
		self.account.submit(payload).await
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

	pub async fn submit_create_from_nested(
		&self,
		registry_id: &[u8],
		nested: &crate::schema::registry::RegistryNestedSchema,
	) -> Result<TxHandle, OriginSdkError> {
		let input = crate::schema::registry::to_create_input(nested)?;
		let payload = crate::extrinsic::calls::registry::create_from_input(
			&self.account.client().metadata(),
			registry_id,
			&input,
		)?;
		self.account.submit(payload).await
	}

	pub async fn submit_packet_from_nested(
		&self,
		registry: Ss58Identifier,
		nested: &crate::schema::packet::PacketNestedValue,
	) -> Result<TxHandle, OriginSdkError> {
		let view = self
			.account
			.origin_client()
			.query()
			.using(self.account.signer().clone())
			.registry()
			.attributes(registry.clone())
			.await?
			.ok_or_else(|| OriginSdkError::View("registry schema not found".into()))?;
		let attrs = crate::schema::packet::validate_and_flatten(nested, &view)?;
		let payload =
			packet_calls::issue_from_input(&self.account.client().metadata(), registry, &attrs)?;
		self.account.submit(payload).await
	}

	pub async fn submit_revoke_registry(
		&self,
		registry: Ss58Identifier,
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.revoke_registry(registry);
		let payload = subxt::dynamic::tx(call.pallet, call.function, call.args);
		self.account.submit(payload).await
	}

	pub async fn submit_restore_registry(
		&self,
		registry: Ss58Identifier,
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.restore_registry(registry);
		let payload = subxt::dynamic::tx(call.pallet, call.function, call.args);
		self.account.submit(payload).await
	}

	pub async fn submit_delete_registry(
		&self,
		registry: Ss58Identifier,
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.delete_registry(registry);
		let payload = subxt::dynamic::tx(call.pallet, call.function, call.args);
		self.account.submit(payload).await
	}
}
