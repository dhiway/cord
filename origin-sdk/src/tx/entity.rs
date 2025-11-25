use crate::{
	client::{signer::Signer, submit::TxHandle, OriginClient},
	extrinsic::builder::DynamicCallBuilder,
	types::{error::OriginSdkError, EntityInfoInput},
};
use origin_primitives::Ss58Identifier;
use scale_value::Value;

pub struct EntityTx<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> EntityTx<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
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
		let payload = self.set_info_from_input(info)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub async fn submit_set_info_from_nested(
		&self,
		nested: &crate::schema::entity::EntityNestedValue,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = self.set_info_from_nested(nested)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub async fn submit_rotate_attribute_from_view(
		&self,
		key: impl AsRef<[u8]>,
		value: origin_primitives::element::ElementView,
	) -> Result<TxHandle, OriginSdkError> {
		let elem = crate::schema::entity::element_from_view(&value)?;
		let payload = crate::extrinsic::calls::entity::rotate_attribute_from_element(
			&self.client.metadata(),
			key.as_ref(),
			&elem,
		)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
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
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
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
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub async fn submit_remove_attribute(&self, key: &[u8]) -> Result<TxHandle, OriginSdkError> {
		let payload =
			crate::extrinsic::calls::entity::remove_attribute_call(&self.client.metadata(), key)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub async fn submit_set_linked_account(
		&self,
		account: subxt::utils::AccountId32,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::entity::set_linked_account_call(
			&self.client.metadata(),
			account,
		)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub async fn submit_revoke_linked_account(
		&self,
		token_for_force: Option<Ss58Identifier>,
		account: subxt::utils::AccountId32,
		force: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = if force {
			crate::extrinsic::calls::entity::revoke_linked_account_for_call(
				&self.client.metadata(),
				token_for_force.ok_or_else(|| {
					OriginSdkError::InvalidInput("token required for force revoke".into())
				})?,
				account,
			)?
		} else {
			crate::extrinsic::calls::entity::revoke_linked_account_call(
				&self.client.metadata(),
				account,
			)?
		};
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub async fn submit_rotate_controller(
		&self,
		token_for_force: Option<Ss58Identifier>,
		new_controller: subxt::utils::AccountId32,
		force: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = if force {
			crate::extrinsic::calls::entity::rotate_controller_for_call(
				&self.client.metadata(),
				token_for_force.ok_or_else(|| {
					OriginSdkError::InvalidInput(
						"token required for force controller rotate".into(),
					)
				})?,
				new_controller,
			)?
		} else {
			crate::extrinsic::calls::entity::rotate_controller_call(
				&self.client.metadata(),
				new_controller,
			)?
		};
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub async fn submit_clear_everything(
		&self,
		token_for_force: Option<Ss58Identifier>,
		force: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = if force {
			crate::extrinsic::calls::entity::clear_everything_for_call(
				&self.client.metadata(),
				token_for_force.ok_or_else(|| {
					OriginSdkError::InvalidInput("token required for force clear_everything".into())
				})?,
			)?
		} else {
			crate::extrinsic::calls::entity::clear_everything_call(&self.client.metadata())?
		};
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub async fn submit_set_entity_nym(&self, prefix: &[u8]) -> Result<TxHandle, OriginSdkError> {
		let payload =
			crate::extrinsic::calls::entity::set_entity_nym_call(&self.client.metadata(), prefix)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub async fn submit_remove_entity_nym(
		&self,
		entity: Ss58Identifier,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = crate::extrinsic::calls::entity::remove_entity_nym_call(
			&self.client.metadata(),
			entity,
		)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	/// Dynamic builder escape hatch.
	pub fn set_info(&self, info: Value) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call("Entity", "set_info", vec![info])
	}

	pub fn rotate_attribute(
		&self,
		key: impl AsRef<[u8]>,
		value: Value,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Entity",
			"rotate_attribute",
			vec![Value::from_bytes(key.as_ref()), value],
		)
	}
}
