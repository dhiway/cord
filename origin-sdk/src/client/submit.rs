use std::collections::HashMap;
use std::sync::Arc;

use super::connection::Connection;
use crate::client::signer::Signer;
use crate::client::OriginConfig;
use crate::types::error::OriginSdkError;
use subxt::dynamic;
use tokio::sync::{oneshot, Mutex};

/// Handle returned by submit operations.
#[derive(Debug, Clone)]
pub struct TxHandle {
	pub hash: subxt::utils::H256,
	receiver: Arc<Mutex<Option<oneshot::Receiver<Result<TxOutcome, OriginSdkError>>>>>,
}

impl TxHandle {
	pub async fn wait_in_block(self) -> Result<TxOutcome, OriginSdkError> {
		let mut rx = self.receiver.lock().await;
		let recv = rx.take().ok_or_else(|| OriginSdkError::Tx("tx channel dropped".into()))?;
		recv.await.map_err(|_| OriginSdkError::Tx("tx watcher dropped".into()))?
	}

	pub async fn wait_finalized(self) -> Result<TxOutcome, OriginSdkError> {
		let mut rx = self.receiver.lock().await;
		let recv = rx.take().ok_or_else(|| OriginSdkError::Tx("tx channel dropped".into()))?;
		recv.await.map_err(|_| OriginSdkError::Tx("tx watcher dropped".into()))?
	}
}

#[derive(Debug, Clone)]
pub struct TxOutcome {
	pub hash: subxt::utils::H256,
	pub block: Option<subxt::utils::H256>,
	pub events: Vec<crate::client::events::EventEnvelope>,
}

/// Submission client with nonce coordination.
#[derive(Clone)]
pub struct SubmitClient {
	connection: Arc<Connection>,
	signer: Arc<dyn Signer>,
	locks: Arc<Mutex<HashMap<[u8; 32], Arc<Mutex<()>>>>>,
}

impl SubmitClient {
	pub(crate) fn new(connection: Arc<Connection>, signer: Arc<dyn Signer>) -> Self {
		Self { connection, signer, locks: Arc::new(Mutex::new(HashMap::new())) }
	}

	/// Submit a raw dynamic payload.
	pub async fn submit(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
	) -> Result<TxHandle, OriginSdkError> {
		let call = dynamic::tx(pallet, call, args);
		self.submit_payload(call).await
	}

	pub async fn submit_and_watch(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
	) -> Result<TxHandle, OriginSdkError> {
		let call = dynamic::tx(pallet, call, args);
		self.submit_payload(call).await
	}

	pub async fn batch_submit(
		&self,
		calls: Vec<super::super::extrinsic::builder::DynamicCall>,
		all: bool,
	) -> Result<TxHandle, OriginSdkError> {
		let payloads: Vec<_> = calls
			.into_iter()
			.map(|c| dynamic::tx(c.pallet, c.function, c.args).into_value())
			.collect();
		let fn_name = if all { "batch_all" } else { "batch" };
		let batch_call = dynamic::tx("Utility", fn_name, payloads);
		self.submit_payload(batch_call).await
	}

	pub fn batch(&self) -> crate::extrinsic::batch::BatchBuilder {
		crate::extrinsic::batch::BatchBuilder::new(self.clone())
	}

	pub(crate) async fn submit_payload(
		&self,
		call: subxt::tx::DynamicPayload,
	) -> Result<TxHandle, OriginSdkError> {
		let account = self.signer.account_id();
		let lock = self.account_lock(account.0).await;
		let (tx, rx) = oneshot::channel();
		let connection = self.connection.clone();
		let signer = self.signer.clone();
		let adapter = SubxtSignerAdapter::new(signer.clone());

		let progress = {
			let _guard = lock.lock().await;
			connection
				.online()
				.tx()
				.sign_and_submit_then_watch_default(&call, &adapter)
				.await
				.map_err(|e| OriginSdkError::Tx(e.to_string()))?
		};

		let hash = progress.extrinsic_hash();
		tokio::spawn(async move {
			let result: Result<TxOutcome, OriginSdkError> = async {
				let mut prog = progress;
				let in_block = loop {
					match prog.next().await {
						Some(Ok(subxt::tx::TxStatus::InBestBlock(inb))) => break inb,
						Some(Ok(subxt::tx::TxStatus::InFinalizedBlock(inb))) => break inb,
						Some(Ok(_)) => continue,
						Some(Err(e)) => return Err(OriginSdkError::Tx(e.to_string())),
						None => return Err(OriginSdkError::Tx("transaction stream ended".into())),
					}
				};
				let block = in_block.block_hash();
				let events = in_block
					.wait_for_success()
					.await
					.map_err(|e| OriginSdkError::Tx(e.to_string()))?;
				let mut envelopes = Vec::new();
				for ev in events.iter() {
					if let Ok(ev) = ev {
						let fields = ev.field_values().map_or(Vec::new(), |comp| match comp {
							scale_value::Composite::Named(v) => {
								v.into_iter().map(|(_, val)| val.remove_context()).collect()
							},
							scale_value::Composite::Unnamed(v) => {
								v.into_iter().map(|val| val.remove_context()).collect()
							},
						});
						envelopes.push(crate::client::events::EventEnvelope {
							block,
							pallet: ev.pallet_name().to_string(),
							variant: ev.variant_name().to_string(),
							fields,
						});
					}
				}
				Ok(TxOutcome { hash, block: Some(block), events: envelopes })
			}
			.await;
			let _ = tx.send(result);
		});

		Ok(TxHandle { hash, receiver: Arc::new(Mutex::new(Some(rx))) })
	}

	async fn account_lock(&self, account: [u8; 32]) -> Arc<Mutex<()>> {
		let mut guard = self.locks.lock().await;
		guard.entry(account).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
	}
}

/// Adapter to plug the SDK `Signer` into Subxt transaction flows.
#[derive(Clone)]
pub struct SubxtSignerAdapter {
	inner: Arc<dyn Signer>,
}

impl SubxtSignerAdapter {
	pub fn new(inner: Arc<dyn Signer>) -> Self {
		Self { inner }
	}
}

impl subxt::tx::Signer<OriginConfig> for SubxtSignerAdapter {
	fn account_id(&self) -> subxt::utils::AccountId32 {
		self.inner.account_id()
	}

	fn sign(&self, payload: &[u8]) -> subxt::utils::MultiSignature {
		self.inner.sign(payload)
	}
}
