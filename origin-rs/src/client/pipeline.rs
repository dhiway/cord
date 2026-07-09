use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::{
	client::{
		nonce::NonceManager,
		signer::{Signer, SubxtSignerAdapter},
	},
	config::{build_origin_params, OriginConfig},
	tx::{config::TxPipelineConfig, handle::TxHandle},
	types::error::OriginSdkError,
};
use subxt::config::{DefaultExtrinsicParamsBuilder, ExtrinsicParams};

pub(crate) struct TxPipeline {
	client: subxt::OnlineClient<OriginConfig>,
	cfg: TxPipelineConfig,
	nonce_mgr: Arc<NonceManager>,
	accounts: Arc<Mutex<HashMap<origin_primitives::AccountId, AccountTxQueue>>>,
}

#[derive(Clone)]
pub struct AccountTxQueue {
	account_id: origin_primitives::AccountId,
	signer: Arc<dyn Signer>,
	sender: mpsc::Sender<TxJob>,
	nonce_mgr: Arc<NonceManager>,
}

struct TxJob {
	call: subxt::tx::DynamicPayload,
	override_nonce: Option<u64>,
	handle_tx: oneshot::Sender<Result<TxHandle, OriginSdkError>>,
}

struct AccountTxWorker {
	client: subxt::OnlineClient<OriginConfig>,
	account_id: origin_primitives::AccountId,
	signer: Arc<dyn Signer>,
	nonce_mgr: Arc<NonceManager>,
	cfg: TxPipelineConfig,
	receiver: mpsc::Receiver<TxJob>,
}

impl TxPipeline {
	pub fn new(client: subxt::OnlineClient<OriginConfig>, cfg: TxPipelineConfig) -> Self {
		let nonce_mgr = Arc::new(NonceManager::new(cfg.nonce_mode, cfg.nonce_refresh_after));
		Self { client, cfg, nonce_mgr, accounts: Arc::new(Mutex::new(HashMap::new())) }
	}

	pub fn config(&self) -> &TxPipelineConfig {
		&self.cfg
	}

	pub async fn account_queue<S>(
		&self,
		account_id: origin_primitives::AccountId,
		signer: S,
	) -> AccountTxQueue
	where
		S: Signer + Clone + 'static,
	{
		let mut guard = self.accounts.lock().await;
		if let Some(queue) = guard.get(&account_id) {
			return queue.clone();
		}

		let (tx, rx) = mpsc::channel(self.cfg.queue_capacity);
		let signer: Arc<dyn Signer> = Arc::new(signer);
		let queue = AccountTxQueue {
			account_id: account_id.clone(),
			signer: signer.clone(),
			sender: tx,
			nonce_mgr: self.nonce_mgr.clone(),
		};

		AccountTxWorker {
			client: self.client.clone(),
			account_id: account_id.clone(),
			signer,
			nonce_mgr: self.nonce_mgr.clone(),
			cfg: self.cfg.clone(),
			receiver: rx,
		}
		.spawn();

		if self.cfg.prewarm_on_first_use {
			let nonce_mgr = self.nonce_mgr.clone();
			let client = self.client.clone();
			let acct = account_id.clone();
			tokio::spawn(async move {
				let _ = nonce_mgr.refresh(&client, &acct).await;
			});
		}

		guard.insert(account_id.clone(), queue.clone());
		queue
	}
}

impl AccountTxQueue {
	pub async fn enqueue(
		&self,
		call: subxt::tx::DynamicPayload,
		override_nonce: Option<u64>,
	) -> Result<TxHandle, OriginSdkError> {
		let (tx, rx) = oneshot::channel();
		let job = TxJob { call, override_nonce, handle_tx: tx };
		self.sender.send(job).await.map_err(|e| OriginSdkError::Tx(e.to_string()))?;
		rx.await.map_err(|_| OriginSdkError::Tx("tx worker dropped".into()))?
	}

	pub fn account_id(&self) -> &origin_primitives::AccountId {
		&self.account_id
	}

	pub fn signer(&self) -> Arc<dyn Signer> {
		self.signer.clone()
	}

	pub fn nonce_manager(&self) -> &NonceManager {
		self.nonce_mgr.as_ref()
	}
}

impl AccountTxWorker {
	pub fn spawn(self) {
		tokio::spawn(async move {
			self.run().await;
		});
	}

	async fn run(mut self) {
		while let Some(job) = self.receiver.recv().await {
			self.handle_job(job).await;
		}
	}

	async fn handle_job(&self, job: TxJob) {
		let result = self.submit_with_retry(&job).await;
		let _ = job.handle_tx.send(result);
	}

	async fn submit_with_retry(&self, job: &TxJob) -> Result<TxHandle, OriginSdkError> {
		let mut attempts: u8 = 0;
		loop {
			attempts = attempts.saturating_add(1);
			match self.submit_once(job).await {
				Ok(h) => return Ok(h),
				Err(e)
					if !self.cfg.retry_on_stale_nonce ||
						!self.is_stale_nonce_error(&e) ||
						attempts >= self.cfg.max_retry_attempts =>
				{
					return Err(e);
				},
				Err(_) => {
					let _ = self.nonce_mgr.refresh(&self.client, &self.account_id).await;
					continue;
				},
			}
		}
	}

	async fn submit_once(&self, job: &TxJob) -> Result<TxHandle, OriginSdkError> {
		let nonce = match job.override_nonce {
			Some(n) => n,
			None => self.nonce_mgr.allocate(&self.client, &self.account_id).await?,
		};

		let params = build_params_with_nonce(&self.client, nonce).await?;
		let call = job.call.clone();
		submit_with_params(&self.client, self.signer.clone(), call, params).await
	}

	fn is_stale_nonce_error(&self, err: &OriginSdkError) -> bool {
		match err {
			OriginSdkError::Tx(msg) | OriginSdkError::Nonce(msg) =>
				msg.contains("Invalid Transaction") ||
					msg.contains("Future") ||
					msg.contains("Priority is too low"),
			_ => false,
		}
	}
}

pub(crate) async fn build_params_with_nonce(
	client: &subxt::OnlineClient<OriginConfig>,
	nonce: u64,
) -> Result<
	<crate::config::OriginExtrinsicParams<OriginConfig> as ExtrinsicParams<OriginConfig>>::Params,
	OriginSdkError,
> {
	let _ = client; // reserved for mortal era computation if needed
	let params =
		build_origin_params(DefaultExtrinsicParamsBuilder::<OriginConfig>::new().nonce(nonce));
	Ok(params)
}

pub(crate) async fn submit_with_params(
	client: &subxt::OnlineClient<OriginConfig>,
	signer: Arc<dyn Signer>,
	call: subxt::tx::DynamicPayload,
	params: <crate::config::OriginExtrinsicParams<OriginConfig> as ExtrinsicParams<OriginConfig>>::Params,
) -> Result<TxHandle, OriginSdkError> {
	let adapter = SubxtSignerAdapter::new(signer);
	let signed = client
		.tx()
		.create_signed(&call, &adapter, params)
		.await
		.map_err(|e| OriginSdkError::Tx(e.to_string()))?;

	let progress =
		signed.submit_and_watch().await.map_err(|e| OriginSdkError::Tx(e.to_string()))?;
	Ok(TxHandle::from_progress(progress))
}
