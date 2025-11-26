mod batch;
pub mod config;
pub mod entity;
pub mod handle;
pub mod packet;
pub mod registry;
pub mod token;

pub use batch::BatchBuilder;

use std::sync::Arc;

use crate::{
	client::{
		tx_pipeline::{build_params_with_nonce, submit_with_params, AccountTxQueue, TxPipeline},
		OriginClient, OriginSigner,
	},
	config::OriginConfig,
	tx::config::{TxPipelineConfig, TxSubmitMode},
	tx::handle::TxHandle,
	types::error::OriginSdkError,
};

/// Tx entrypoint bound to a shared pipeline.
#[derive(Clone)]
pub struct TxClient {
	origin: OriginClient,
	client: subxt::OnlineClient<OriginConfig>,
	pipeline: Arc<TxPipeline>,
}

/// Convenience alias matching previous API surface.
pub type Tx<'a> = TxClient;

impl TxClient {
	pub(crate) fn new(origin: OriginClient, pipeline: Arc<TxPipeline>) -> Self {
		let client = origin.online().clone();
		Self { origin, client, pipeline }
	}

	/// Preferred surface: binds a signer to an account-scoped tx queue.
	pub fn using(&self, signer: OriginSigner) -> AccountTx {
		let account_id = signer.account_id();
		let queue =
			futures::executor::block_on(self.pipeline.account_queue(account_id, signer.clone()));
		AccountTx::new(
			self.origin.clone(),
			self.client.clone(),
			queue,
			self.pipeline.config().clone(),
			signer,
		)
	}

	/// Backwards-compatible alias.
	pub fn for_signer(&self, signer: OriginSigner) -> AccountTx {
		self.using(signer)
	}
}

#[derive(Clone)]
pub struct AccountTx {
	origin: OriginClient,
	client: subxt::OnlineClient<OriginConfig>,
	queue: AccountTxQueue,
	cfg: TxPipelineConfig,
	signer: OriginSigner,
}

impl AccountTx {
	pub(crate) fn new(
		origin: OriginClient,
		client: subxt::OnlineClient<OriginConfig>,
		queue: AccountTxQueue,
		cfg: TxPipelineConfig,
		signer: OriginSigner,
	) -> Self {
		Self { origin, client, queue, cfg, signer }
	}

	pub async fn submit(
		&self,
		call: subxt::tx::DynamicPayload,
	) -> Result<TxHandle, OriginSdkError> {
		match self.cfg.submit_mode {
			TxSubmitMode::ManagedQueue | TxSubmitMode::ManagedQueueWithOverride => {
				self.queue.enqueue(call, None).await
			},
			TxSubmitMode::Manual => self.submit_immediate(call).await,
		}
	}

	pub async fn submit_with_nonce(
		&self,
		call: subxt::tx::DynamicPayload,
		nonce: u64,
	) -> Result<TxHandle, OriginSdkError> {
		match self.cfg.submit_mode {
			TxSubmitMode::ManagedQueue => Err(OriginSdkError::Config(
				"submit_with_nonce is disabled in ManagedQueue mode".into(),
			)),
			TxSubmitMode::ManagedQueueWithOverride => self.queue.enqueue(call, Some(nonce)).await,
			TxSubmitMode::Manual => self.submit_immediate_with_nonce(call, nonce).await,
		}
	}

	pub async fn warm_nonce(&self) -> Result<u64, OriginSdkError> {
		self.queue.nonce_manager().refresh(&self.client, self.queue.account_id()).await
	}

	pub async fn allocate_nonce(&self) -> Result<u64, OriginSdkError> {
		self.queue.nonce_manager().allocate(&self.client, self.queue.account_id()).await
	}

	pub async fn refresh_nonce(&self) -> Result<u64, OriginSdkError> {
		self.queue.nonce_manager().refresh(&self.client, self.queue.account_id()).await
	}

	pub async fn submit_immediate(
		&self,
		call: subxt::tx::DynamicPayload,
	) -> Result<TxHandle, OriginSdkError> {
		let nonce = self.allocate_nonce().await?;
		self.submit_immediate_with_nonce(call, nonce).await
	}

	pub async fn submit_immediate_with_nonce(
		&self,
		call: subxt::tx::DynamicPayload,
		nonce: u64,
	) -> Result<TxHandle, OriginSdkError> {
		let params = build_params_with_nonce(&self.client, nonce).await?;
		submit_with_params(&self.client, self.queue.signer(), call, params).await
	}

	pub fn entity(&self) -> entity::EntityTx<'_> {
		entity::EntityTx::new(self)
	}

	pub fn registry(&self) -> registry::RegistryTx<'_> {
		registry::RegistryTx::new(self)
	}

	pub fn packet(&self) -> packet::PacketTx<'_> {
		packet::PacketTx::new(self)
	}

	pub fn token(&self) -> token::TokenTx<'_> {
		token::TokenTx::new(self)
	}

	pub fn batch(&self) -> BatchBuilder {
		BatchBuilder::new(self.clone())
	}

	pub(crate) fn client(&self) -> &subxt::OnlineClient<OriginConfig> {
		&self.client
	}

	pub(crate) fn origin_client(&self) -> &OriginClient {
		&self.origin
	}

	pub fn signer(&self) -> &OriginSigner {
		&self.signer
	}

	pub fn config(&self) -> &TxPipelineConfig {
		&self.cfg
	}
}
