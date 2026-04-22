use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};

use crate::{client::EventEnvelope, config::OriginConfig, types::error::OriginSdkError};

/// Outcome bundle for tx inclusion/finalization.
#[derive(Debug, Clone)]
pub struct TxOutcome {
	pub hash: subxt::utils::H256,
	pub block: Option<subxt::utils::H256>,
	pub events: Vec<EventEnvelope>,
}

/// Non-blocking handle produced by submissions.
#[derive(Debug, Clone)]
pub struct TxHandle {
	hash: subxt::utils::H256,
	in_block: Arc<Mutex<Option<oneshot::Receiver<Result<TxOutcome, OriginSdkError>>>>>,
	finalized: Arc<Mutex<Option<oneshot::Receiver<Result<TxOutcome, OriginSdkError>>>>>,
}

impl TxHandle {
	pub(crate) fn new(
		hash: subxt::utils::H256,
		in_block: oneshot::Receiver<Result<TxOutcome, OriginSdkError>>,
		finalized: oneshot::Receiver<Result<TxOutcome, OriginSdkError>>,
	) -> Self {
		Self {
			hash,
			in_block: Arc::new(Mutex::new(Some(in_block))),
			finalized: Arc::new(Mutex::new(Some(finalized))),
		}
	}

	pub fn hash(&self) -> &subxt::utils::H256 {
		&self.hash
	}

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

	pub(crate) fn from_progress(
		progress: subxt::transactions::TransactionProgress<
			OriginConfig,
			subxt::client::OnlineClientAtBlockImpl<OriginConfig>,
		>,
	) -> Self {
		let hash = progress.extrinsic_hash();
		let (tx_in_block, rx_in_block) = oneshot::channel();
		let (tx_finalized, rx_finalized) = oneshot::channel();

		tokio::spawn(async move {
			let mut send_in_block = Some(tx_in_block);
			let mut send_finalized = Some(tx_finalized);
			let result: Result<TxOutcome, OriginSdkError> = async {
				let mut prog = progress;
				loop {
					match prog.next().await {
						Some(Ok(subxt::transactions::TransactionStatus::InBestBlock(inb))) => {
							let outcome = collect_outcome(hash, &inb).await?;
							if let Some(sender) = send_in_block.take() {
								let _ = sender.send(Ok(outcome));
							}
						},
						Some(Ok(subxt::transactions::TransactionStatus::InFinalizedBlock(inb))) => {
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

		Self::new(hash, rx_in_block, rx_finalized)
	}
}

async fn collect_outcome(
	hash: subxt::utils::H256,
	status: &subxt::transactions::TransactionInBlock<
		OriginConfig,
		subxt::client::OnlineClientAtBlockImpl<OriginConfig>,
	>,
) -> Result<TxOutcome, OriginSdkError> {
	let block = status.block_hash();
	let events = status.wait_for_success().await.map_err(|e| OriginSdkError::Tx(e.to_string()))?;

	let at = status.at().await.map_err(|e| OriginSdkError::Tx(e.to_string()))?;
	let metadata = at.metadata_ref();

	let mut envelopes = Vec::new();
	for ev in events.iter().flatten() {
		let fields = crate::client::events::decode_event_fields(
			metadata,
			ev.pallet_name(),
			ev.event_name(),
			ev.field_bytes(),
		)
		.unwrap_or_default();
		envelopes.push(EventEnvelope {
			block,
			pallet: ev.pallet_name().to_string(),
			variant: ev.event_name().to_string(),
			fields,
		});
	}
	Ok(TxOutcome { hash, block: Some(block), events: envelopes })
}
