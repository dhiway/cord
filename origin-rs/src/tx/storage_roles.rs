use crate::{
	extrinsic::builder::DynamicCallBuilder,
	tx::{handle::TxHandle, AccountTx},
	types::error::OriginSdkError,
};
use codec::Encode;
use origin_primitives::{AccountId, Ss58Identifier};
use scale_value::Value;

pub struct StorageRolesTx<'a> {
	account: &'a AccountTx,
}

impl<'a> StorageRolesTx<'a> {
	pub(crate) fn new(account: &'a AccountTx) -> Self {
		Self { account }
	}

	pub fn register_publisher(
		&self,
		entity: Ss58Identifier,
		account_id: &AccountId,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"StorageRoles",
			"register_publisher",
			vec![Value::from_bytes(entity.as_ref()), Value::from_bytes(account_id.encode())],
		)
	}

	pub async fn submit_register_publisher(
		&self,
		entity: Ss58Identifier,
		account_id: &AccountId,
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.register_publisher(entity, account_id);
		self.account.submit(call.to_payload()).await
	}

	pub fn register_aggregator(
		&self,
		entity: Ss58Identifier,
		account_id: &AccountId,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"StorageRoles",
			"register_aggregator",
			vec![Value::from_bytes(entity.as_ref()), Value::from_bytes(account_id.encode())],
		)
	}

	pub async fn submit_register_aggregator(
		&self,
		entity: Ss58Identifier,
		account_id: &AccountId,
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.register_aggregator(entity, account_id);
		self.account.submit(call.to_payload()).await
	}
}
