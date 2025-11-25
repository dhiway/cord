use std::sync::Arc;

use super::{
	connection::Connection,
	nonce::{NonceManager, NonceStrategy},
};
use crate::{
	client::{
		signer::{Signer, SubxtSignerAdapter},
		OriginConfig,
	},
	types::error::OriginSdkError,
};
use std::{collections::HashMap, sync::Arc as StdArc};
use subxt::dynamic;
use tokio::sync::{oneshot, Mutex};

/// Handle returned by submit operations.
#[derive(Debug, Clone)]
pub struct TxHandle {
	pub hash: subxt::utils::H256,
	in_block: Arc<Mutex<Option<oneshot::Receiver<Result<TxOutcome, OriginSdkError>>>>>,
	finalized: Arc<Mutex<Option<oneshot::Receiver<Result<TxOutcome, OriginSdkError>>>>>,
}

impl TxHandle {
	pub async fn wait_in_block(self) -> Result<TxOutcome, OriginSdkError> {
		let mut rx = self.in_block.lock().await;
		let recv = rx.take().ok_or_else(|| OriginSdkError::Tx("tx channel dropped".into()))?;
		recv.await.map_err(|_| OriginSdkError::Tx("tx watcher dropped".into()))?
	}

	pub async fn wait_finalized(self) -> Result<TxOutcome, OriginSdkError> {
		let mut rx = self.finalized.lock().await;
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

async fn collect_outcome<C>(
	hash: subxt::utils::H256,
	status: &subxt::tx::TxInBlock<OriginConfig, C>,
) -> Result<TxOutcome, OriginSdkError>
where
	C: subxt::client::OnlineClientT<OriginConfig>,
{
	let block = status.block_hash();
	let events = status.wait_for_success().await.map_err(|e| OriginSdkError::Tx(e.to_string()))?;
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

/// Submission client with nonce coordination.
#[derive(Clone)]
pub struct SubmitClient {
	connection: Arc<Connection>,
	signer: Arc<dyn Signer>,
	nonce: Arc<NonceManager>,
	locks: StdArc<Mutex<HashMap<origin_primitives::AccountId, StdArc<Mutex<()>>>>>,
}

const DEFAULT_TIP: u128 = 10u128;

impl SubmitClient {
	pub(crate) fn new(connection: Arc<Connection>, signer: Arc<dyn Signer>) -> Self {
		Self {
			connection,
			signer,
			nonce: Arc::new(NonceManager::new(
				NonceStrategy::LocalCache,
				std::time::Duration::from_secs(10),
			)),
			locks: StdArc::new(Mutex::new(HashMap::new())),
		}
	}

	/// Submit a raw dynamic payload.
	pub async fn submit(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
	) -> Result<TxHandle, OriginSdkError> {
		self.submit_with_tip(pallet, call, args, DEFAULT_TIP).await
	}

	pub async fn submit_with_tip(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
		tip: u128,
	) -> Result<TxHandle, OriginSdkError> {
		let call = dynamic::tx(pallet, call, args);
		self.submit_payload_with_tip(call, tip).await
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
		self.submit_payload_with_tip(call, DEFAULT_TIP).await
	}

	pub(crate) async fn submit_payload_with_tip(
		&self,
		call: subxt::tx::DynamicPayload,
		tip: u128,
	) -> Result<TxHandle, OriginSdkError> {
		let signer = self.signer.clone();
		let account = signer.account_id();
		let lock = {
			let mut guard = self.locks.lock().await;
			guard
				.entry(account.clone())
				.or_insert_with(|| StdArc::new(Mutex::new(())))
				.clone()
		};
		let _acct_guard = lock.lock().await;
		let (tx_in_block, rx_in_block) = oneshot::channel();
		let (tx_finalized, rx_finalized) = oneshot::channel();
		let connection = self.connection.clone();
		let adapter = SubxtSignerAdapter::new(signer.clone());

		let mut attempt = 0;
		let progress = loop {
			let nonce = self.nonce.allocate(connection.online(), &account).await?;
			let params = subxt::config::DefaultExtrinsicParamsBuilder::<OriginConfig>::new()
				.nonce(nonce)
				.tip(tip)
				.build();
			match connection
				.online()
				.tx()
				.sign_and_submit_then_watch(&call, &adapter, params)
				.await
			{
				Ok(p) => break p,
				Err(e) if attempt == 0 && e.to_string().contains("Future") => {
					// Refresh nonce and retry once on future nonce errors.
					attempt += 1;
					continue;
				},
				Err(e) => return Err(OriginSdkError::Tx(e.to_string())),
			}
		};

		let hash = progress.extrinsic_hash();
		tokio::spawn(async move {
			let mut send_in_block = Some(tx_in_block);
			let mut send_finalized = Some(tx_finalized);
			let result: Result<TxOutcome, OriginSdkError> = async {
				let mut prog = progress;
				loop {
					match prog.next().await {
						Some(Ok(subxt::tx::TxStatus::InBestBlock(inb))) => {
							let outcome = collect_outcome(hash, &inb).await?;
							if let Some(sender) = send_in_block.take() {
								let _ = sender.send(Ok(outcome));
							}
						},
						Some(Ok(subxt::tx::TxStatus::InFinalizedBlock(inb))) => {
							let outcome = collect_outcome(hash, &inb).await?;
							if let Some(sender) = send_finalized.take() {
								let _ = sender.send(Ok(outcome.clone()));
							}
							if let Some(sender) = send_in_block.take() {
								let _ = sender.send(Ok(outcome.clone()));
							}
							return Ok(outcome);
						},
						Some(Ok(_)) => continue,
						Some(Err(e)) => return Err(OriginSdkError::Tx(e.to_string())),
						None => return Err(OriginSdkError::Tx("transaction stream ended".into())),
					}
				}
			}
			.await;
			if let Err(e) = result {
				if let Some(sender) = send_in_block.take() {
					let _ = sender.send(Err(e.clone()));
				}
				if let Some(sender) = send_finalized.take() {
					let _ = sender.send(Err(e));
				}
			}
		});

		Ok(TxHandle {
			hash,
			in_block: Arc::new(Mutex::new(Some(rx_in_block))),
			finalized: Arc::new(Mutex::new(Some(rx_finalized))),
		})
	}
}
