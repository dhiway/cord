use crate::{
	extrinsic::builder::DynamicCallBuilder,
	tx::{handle::TxHandle, AccountTx},
	types::{blob_store::RegisterBlobAuthorization, error::OriginSdkError},
};
use codec::Encode;
use origin_primitives::{authorization::Authorization, AccountId, Signature};
use scale_value::Value;
use sp_core::H256;

pub struct BlobStoreTx<'a> {
	account: &'a AccountTx,
}

impl<'a> BlobStoreTx<'a> {
	pub(crate) fn new(account: &'a AccountTx) -> Self {
		Self { account }
	}

	pub fn set_price_per_byte_per_period(
		&self,
		price: u128,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"BlobStore",
			"set_price_per_byte_per_period",
			vec![Value::u128(price)],
		)
	}

	pub async fn submit_set_price_per_byte_per_period(
		&self,
		price: u128,
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.set_price_per_byte_per_period(price);
		self.account.submit(call.to_payload()).await
	}

	pub fn register_blob_by_publisher(
		&self,
		owner: &AccountId,
		blob_id: H256,
		root_hash: H256,
		size_bytes: u64,
		encoding: u8,
		auth: &Authorization<AccountId, Vec<u8>, Signature>,
	) -> crate::extrinsic::builder::DynamicCall {
		let auth_bytes = auth.encode();
		DynamicCallBuilder::new().call(
			"BlobStore",
			"register_blob_by_publisher",
			vec![
				Value::from_bytes(owner.encode()),
				Value::from_bytes(blob_id.encode()),
				Value::from_bytes(root_hash.encode()),
				Value::from_bytes(size_bytes.encode()),
				Value::from_bytes(encoding.encode()),
				Value::from_bytes(auth_bytes),
			],
		)
	}

	pub async fn submit_register_blob_by_publisher(
		&self,
		owner: &AccountId,
		blob_id: H256,
		root_hash: H256,
		size_bytes: u64,
		encoding: u8,
		auth: &Authorization<AccountId, Vec<u8>, Signature>,
	) -> Result<TxHandle, OriginSdkError> {
		let call =
			self.register_blob_by_publisher(owner, blob_id, root_hash, size_bytes, encoding, auth);
		self.account.submit(call.to_payload()).await
	}

	pub fn confirm_stored(
		&self,
		blob_id: H256,
		checksum: H256,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"BlobStore",
			"confirm_stored",
			vec![Value::from_bytes(blob_id.encode()), Value::from_bytes(checksum.encode())],
		)
	}

	pub async fn submit_confirm_stored(
		&self,
		blob_id: H256,
		checksum: H256,
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.confirm_stored(blob_id, checksum);
		self.account.submit(call.to_payload()).await
	}

	pub fn authorize_archive(&self, blob_id: H256) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"BlobStore",
			"authorize_archive",
			vec![Value::from_bytes(blob_id.encode())],
		)
	}

	pub async fn submit_authorize_archive(
		&self,
		blob_id: H256,
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.authorize_archive(blob_id);
		self.account.submit(call.to_payload()).await
	}

	pub fn mark_archived(&self, blob_id: H256) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"BlobStore",
			"mark_archived",
			vec![Value::from_bytes(blob_id.encode())],
		)
	}

	pub async fn submit_mark_archived(&self, blob_id: H256) -> Result<TxHandle, OriginSdkError> {
		let call = self.mark_archived(blob_id);
		self.account.submit(call.to_payload()).await
	}
}

/// Helper: build the on-chain payload bytes that the owner must sign (mirrors pallet).
pub fn encode_register_blob_payload(payload: &RegisterBlobAuthorization) -> Vec<u8> {
	payload.encode()
}
