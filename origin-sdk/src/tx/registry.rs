use crate::{
	client::{signer::Signer, submit::TxOutcome, OriginClient},
	extrinsic::{builder::DynamicCallBuilder, calls::packet as packet_calls},
	types::error::OriginSdkError,
};
use origin_primitives::Ss58Identifier;
use scale_value::Value;

pub struct RegistryTx<'a> {
	client: &'a OriginClient,
}

impl<'a> RegistryTx<'a> {
	pub(crate) fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using<S>(&self, signer: S) -> RegistryTxWithSigner<'a, S>
	where
		S: Signer + Clone + 'static,
	{
		RegistryTxWithSigner::new(self.client, signer)
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

	/// Typed update_registry_info using ElementView via schema helper.
	pub async fn submit_update_registry_info_from_view(
		&self,
		registry: Ss58Identifier,
		info: origin_primitives::element::ElementView,
	) -> Result<TxOutcome, OriginSdkError> {
		let elem = crate::schema::registry::element_from_view(&info)?;
		let payload = crate::extrinsic::calls::registry::update_info_from_input(
			&self.client.metadata(),
			registry.as_ref(),
			&elem,
		)?;
		self.client.tx()?.submit_payload(payload).await?.wait_in_block().await
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

pub struct RegistryTxWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> RegistryTxWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
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
	) -> Result<TxOutcome, OriginSdkError> {
		let call = self.create(registry_id, info);
		self.client
			.tx_with(self.signer.clone())
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	/// Create a registry from nested schema using SDK mirrors.
	pub async fn submit_create_from_nested(
		&self,
		registry_id: &[u8],
		nested: &crate::schema::registry::RegistryNestedSchema,
	) -> Result<TxOutcome, OriginSdkError> {
		let input = crate::schema::registry::to_create_input(nested)?;
		let payload = crate::extrinsic::calls::registry::create_from_input(
			&self.client.metadata(),
			registry_id,
			&input,
		)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await?
			.wait_in_block()
			.await
	}

	/// Issue a packet for a registry using nested packet values (validated against schema).
	pub async fn submit_packet_from_nested(
		&self,
		registry: Ss58Identifier,
		nested: &crate::schema::packet::PacketNestedValue,
	) -> Result<TxOutcome, OriginSdkError> {
		// fetch schema via view
		let view = self
			.client
			.view_with(self.signer.clone())
			.registry()
			.details(registry.clone())
			.await?;
		let attrs = crate::schema::packet::validate_and_flatten(nested, &view.attributes)?;
		let payload = packet_calls::issue_from_input(&self.client.metadata(), registry, &attrs)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await?
			.wait_in_block()
			.await
	}

	pub async fn submit_update_registry_info_from_view(
		&self,
		registry: Ss58Identifier,
		info: origin_primitives::element::ElementView,
	) -> Result<TxOutcome, OriginSdkError> {
		let elem = crate::schema::registry::element_from_view(&info)?;
		let payload = crate::extrinsic::calls::registry::update_info_from_input(
			&self.client.metadata(),
			registry.as_ref(),
			&elem,
		)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await?
			.wait_in_block()
			.await
	}
}
