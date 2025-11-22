use std::sync::Arc;

use subxt::{config::DefaultExtrinsicParamsBuilder, dynamic};
use subxt::tx::Signer as SubxtSigner;
use crate::client::signer::Signer;
use crate::client::{nonce::NonceManager, OriginConfig};
use crate::types::error::OriginSdkError;
use super::connection::Connection;

/// Handle returned by submit operations.
#[derive(Debug, Clone)]
pub struct TxHandle {
	pub hash: subxt::utils::H256,
}

/// Submission client with nonce coordination.
#[derive(Clone)]
pub struct SubmitClient {
	connection: Arc<Connection>,
	nonce: Arc<NonceManager>,
}

impl SubmitClient {
	pub(crate) fn new(connection: Arc<Connection>, nonce: Arc<NonceManager>) -> Self {
		Self { connection, nonce }
	}

	/// Submit a raw dynamic payload.
	pub async fn submit(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
		signer: &dyn Signer,
	) -> Result<TxHandle, OriginSdkError> {
		let adapter = SubxtSignerAdapter::new(signer);
		let call = dynamic::tx(pallet, call, args);
		let account = adapter.account_id();
		let nonce = self.nonce.allocate(self.connection.online(), &account.0).await?;
		let params = DefaultExtrinsicParamsBuilder::<OriginConfig>::new().nonce(nonce).build();
		let progress = self
			.connection
			.online()
			.tx()
			.sign_and_submit_then_watch(&call, &adapter, params)
			.await?;
		let hash = progress.extrinsic_hash();
		Ok(TxHandle { hash })
	}

	pub async fn submit_and_watch(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
		signer: &dyn Signer,
	) -> Result<TxHandle, OriginSdkError> {
		let adapter = SubxtSignerAdapter::new(signer);
		let call = dynamic::tx(pallet, call, args);
		let account = adapter.account_id();
		let nonce = self.nonce.allocate(self.connection.online(), &account.0).await?;
		let params = DefaultExtrinsicParamsBuilder::<OriginConfig>::new().nonce(nonce).build();
		let progress = self
			.connection
			.online()
			.tx()
			.sign_and_submit_then_watch(&call, &adapter, params)
			.await?;
		let hash = progress.extrinsic_hash();
		let _ = progress.wait_for_finalized_success().await?;
		Ok(TxHandle { hash })
	}

	pub async fn batch_submit(
		&self,
		calls: Vec<super::super::extrinsic::builder::DynamicCall>,
		signer: &dyn Signer,
	) -> Result<TxHandle, OriginSdkError> {
		let adapter = SubxtSignerAdapter::new(signer);
		let account = adapter.account_id();
		let nonce = self.nonce.allocate(self.connection.online(), &account.0).await?;
		let params = DefaultExtrinsicParamsBuilder::<OriginConfig>::new().nonce(nonce).build();
		let payloads: Vec<_> = calls
			.into_iter()
			.map(|c| dynamic::tx(c.pallet, c.function, c.args).into_value())
			.collect();
		let batch_call = dynamic::tx("Utility", "batch_all", payloads);
		let progress = self
			.connection
			.online()
			.tx()
			.sign_and_submit_then_watch(&batch_call, &adapter, params)
			.await?;
		let hash = progress.extrinsic_hash();
		let _ = progress.wait_for_finalized_success().await?;
		Ok(TxHandle { hash })
	}
}

/// Adapter to plug the SDK `Signer` into Subxt transaction flows.
#[derive(Clone)]
struct SubxtSignerAdapter<'a> {
	inner: &'a dyn Signer,
}

impl<'a> SubxtSignerAdapter<'a> {
	fn new(inner: &'a dyn Signer) -> Self {
		Self { inner }
	}
}

impl<'a> subxt::tx::Signer<OriginConfig> for SubxtSignerAdapter<'a> {
	fn account_id(&self) -> subxt::utils::AccountId32 {
		self.inner.account_id()
	}

	fn sign(&self, payload: &[u8]) -> subxt::utils::MultiSignature {
		self.inner.sign(payload)
	}
}
