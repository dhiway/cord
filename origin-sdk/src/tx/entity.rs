use crate::{
	client::{signer::Signer, submit::TxHandle, OriginClient},
	extrinsic::builder::DynamicCallBuilder,
	types::{error::OriginSdkError, EntityInfoInput},
};
use origin_primitives::Ss58Identifier;
use scale_value::Value;

pub struct EntityTx<'a> {
	client: &'a OriginClient,
}

impl<'a> EntityTx<'a> {
	pub(crate) fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using<S>(&self, signer: S) -> EntityTxWithSigner<'a, S>
	where
		S: Signer + Clone + 'static,
	{
		EntityTxWithSigner::new(self.client, signer)
	}

	pub fn set_info_from_input(
		&self,
		info: &EntityInfoInput,
	) -> Result<subxt::tx::DynamicPayload, OriginSdkError> {
		crate::extrinsic::calls::entity::set_info_from_struct(&self.client.metadata(), info)
	}

	pub fn set_info_from_nested(
		&self,
		nested: &crate::schema::entity::EntityNestedValue,
	) -> Result<subxt::tx::DynamicPayload, OriginSdkError> {
		let input = crate::schema::entity::to_entity_input(nested)?;
		self.set_info_from_input(&input)
	}

	pub async fn submit_set_info_from_input(
		&self,
		info: &EntityInfoInput,
	) -> Result<TxHandle, OriginSdkError> {
		self.client
			.tx()?
			.submit_payload(self.set_info_from_input(info)?)
			.await
	}

	pub async fn submit_set_info_from_nested(
		&self,
		nested: &crate::schema::entity::EntityNestedValue,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = self.set_info_from_nested(nested)?;
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_rotate_attribute_from_view(
		&self,
		entity: Ss58Identifier,
		key: impl AsRef<[u8]>,
		value: origin_primitives::element::ElementView,
	) -> Result<TxHandle, OriginSdkError> {
		let elem = crate::schema::entity::element_from_view(&value)?;
		let payload = crate::extrinsic::calls::entity::rotate_attribute_from_element(
			&self.client.metadata(),
			entity,
			key.as_ref(),
			&elem,
		)?;
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_rotate_attributes_from_nested(
		&self,
		ops: &[(Vec<u8>, origin_primitives::element::ElementView)],
	) -> Result<TxHandle, OriginSdkError> {
		let mut pairs = Vec::with_capacity(ops.len());
		for (k, v) in ops {
			let elem = crate::schema::entity::element_from_view(v)?;
			pairs.push((k.clone(), elem));
		}
		let payload = crate::extrinsic::calls::entity::rotate_attributes_from_input(
			&self.client.metadata(),
			&pairs,
		)?;
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_add_attributes_from_nested(
		&self,
		ops: &[(Vec<u8>, origin_primitives::element::ElementView)],
	) -> Result<TxHandle, OriginSdkError> {
		let mut pairs = Vec::with_capacity(ops.len());
		for (k, v) in ops {
			let elem = crate::schema::entity::element_from_view(v)?;
			pairs.push((k.clone(), elem));
		}
		let payload = crate::extrinsic::calls::entity::add_attributes_from_input(
			&self.client.metadata(),
			&pairs,
		)?;
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_remove_attribute(
		&self,
		entity: Ss58Identifier,
		key: &[u8],
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::entity::remove_attribute_call(
			&self.client.metadata(),
			entity,
			key,
		)?;
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_set_linked_account(
		&self,
		entity: Ss58Identifier,
		account: subxt::utils::AccountId32,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::entity::set_linked_account_call(
			&self.client.metadata(),
			entity,
			account,
		)?;
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_revoke_linked_account(
		&self,
		entity: Ss58Identifier,
		account: subxt::utils::AccountId32,
		force: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = if force {
			crate::extrinsic::calls::entity::revoke_linked_account_for_call(
				&self.client.metadata(),
				entity,
				account,
			)?
		} else {
			crate::extrinsic::calls::entity::revoke_linked_account_call(
				&self.client.metadata(),
				entity,
				account,
			)?
		};
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_rotate_controller(
		&self,
		entity: Ss58Identifier,
		new_controller: subxt::utils::AccountId32,
		force: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = if force {
			crate::extrinsic::calls::entity::rotate_controller_for_call(
				&self.client.metadata(),
				entity,
				new_controller,
			)?
		} else {
			crate::extrinsic::calls::entity::rotate_controller_call(
				&self.client.metadata(),
				entity,
				new_controller,
			)?
		};
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_clear_everything(
		&self,
		entity: Ss58Identifier,
		force: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = if force {
			crate::extrinsic::calls::entity::clear_everything_for_call(
				&self.client.metadata(),
				entity,
			)?
		} else {
			crate::extrinsic::calls::entity::clear_everything_call(&self.client.metadata(), entity)?
		};
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_set_entity_nym(&self, prefix: &[u8]) -> Result<TxHandle, OriginSdkError> {
		let payload =
			crate::extrinsic::calls::entity::set_entity_nym_call(&self.client.metadata(), prefix)?;
		self.client.tx()?.submit_payload(payload).await
	}

	pub async fn submit_remove_entity_nym(
		&self,
		entity: Ss58Identifier,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::entity::remove_entity_nym_call(
			&self.client.metadata(),
			entity,
		)?;
		self.client.tx()?.submit_payload(payload).await
	}
}

pub struct EntityTxWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> EntityTxWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	pub fn set_info(&self, info: Value) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call("Entity", "set_info", vec![info])
	}

	/// Build set_info payload from typed input.
	pub fn set_info_from_input(
		&self,
		info: &EntityInfoInput,
	) -> Result<subxt::tx::DynamicPayload, OriginSdkError> {
		crate::extrinsic::calls::entity::set_info_from_struct(&self.client.metadata(), info)
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
) -> Result<TxHandle, OriginSdkError> {
	let call = self.rotate_attribute(entity, key, value);
	self.client
		.tx_with(self.signer.clone())
		.submit(&call.pallet, &call.function, call.args)
		.await
}

	pub async fn submit_set_info(&self, info: Value) -> Result<TxHandle, OriginSdkError> {
		let call = self.set_info(info);
		self.client
			.tx_with(self.signer.clone())
			.submit(&call.pallet, &call.function, call.args)
			.await
	}

	pub async fn submit_set_info_from_input(
		&self,
		info: &EntityInfoInput,
	) -> Result<TxHandle, OriginSdkError> {
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(self.set_info_from_input(info)?)
			.await
	}

	pub async fn submit_set_info_from_nested(
		&self,
		nested: &crate::schema::entity::EntityNestedValue,
	) -> Result<TxHandle, OriginSdkError> {
		let input = crate::schema::entity::to_entity_input(nested)?;
		let payload = self.set_info_from_input(&input)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await
	}

	pub async fn submit_remove_attribute(
		&self,
		entity: Ss58Identifier,
		key: &[u8],
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::entity::remove_attribute_call(
			&self.client.metadata(),
			entity,
			key,
		)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await
	}

	pub async fn submit_set_linked_account(
		&self,
		entity: Ss58Identifier,
		account: subxt::utils::AccountId32,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::entity::set_linked_account_call(
			&self.client.metadata(),
			entity,
			account,
		)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await
	}

	pub async fn submit_revoke_linked_account(
		&self,
		entity: Ss58Identifier,
		account: subxt::utils::AccountId32,
		force: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = if force {
			crate::extrinsic::calls::entity::revoke_linked_account_for_call(
				&self.client.metadata(),
				entity,
				account,
			)?
		} else {
			crate::extrinsic::calls::entity::revoke_linked_account_call(
				&self.client.metadata(),
				entity,
				account,
			)?
		};
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await
	}

	pub async fn submit_rotate_controller(
		&self,
		entity: Ss58Identifier,
		new_controller: subxt::utils::AccountId32,
		force: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = if force {
			crate::extrinsic::calls::entity::rotate_controller_for_call(
				&self.client.metadata(),
				entity,
				new_controller,
			)?
		} else {
			crate::extrinsic::calls::entity::rotate_controller_call(
				&self.client.metadata(),
				entity,
				new_controller,
			)?
		};
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await
	}

	pub async fn submit_clear_everything(
		&self,
		entity: Ss58Identifier,
		force: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = if force {
			crate::extrinsic::calls::entity::clear_everything_for_call(
				&self.client.metadata(),
				entity,
			)?
		} else {
			crate::extrinsic::calls::entity::clear_everything_call(&self.client.metadata(), entity)?
		};
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await
	}

	pub async fn submit_set_entity_nym(&self, prefix: &[u8]) -> Result<TxHandle, OriginSdkError> {
		let payload =
			crate::extrinsic::calls::entity::set_entity_nym_call(&self.client.metadata(), prefix)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await
	}

	pub async fn submit_remove_entity_nym(
		&self,
		entity: Ss58Identifier,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::entity::remove_entity_nym_call(
			&self.client.metadata(),
			entity,
		)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await
	}
}
