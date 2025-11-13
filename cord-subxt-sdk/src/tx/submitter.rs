use crate::{
	client::Client,
	error::Error as SdkError,
	params::config::CordConfig,
	tx::{
		nonce::{NonceMode, NonceTracker},
		TxOptions,
	},
};
use std::fmt;
use subxt::{
	blocks::ExtrinsicEvents,
	tx::{DynamicPayload, TxStatus},
	utils::H256,
};

/// Structured events emitted while tracking extrinsic submission.
#[derive(Clone, Debug)]
pub enum SubmitStage {
	Validated,
	Broadcasted,
	Retracted,
	InBlock { hash: H256, label: Option<String> },
	Finalized { hash: H256, label: Option<String>, description: String },
	Completed { description: String },
}

/// Errors surfaced while submitting transactions through the SDK helper.
#[derive(Debug)]
pub enum SubmitError {
	Invalid(String),
	Dropped(String),
	Node(String),
	Runtime(String),
	StreamEnded,
}

impl SubmitError {
	pub fn message(&self) -> &str {
		match self {
			Self::Invalid(msg) | Self::Dropped(msg) | Self::Node(msg) | Self::Runtime(msg) => msg,
			Self::StreamEnded => "extrinsic stream ended before inclusion",
		}
	}

	pub fn from_subxt_error(err: subxt::Error) -> Self {
		SubmitError::Runtime(err.to_string())
	}

	pub fn from_origin_error(err: SdkError) -> Self {
		SubmitError::Node(err.to_string())
	}
}

impl fmt::Display for SubmitError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Invalid(msg) => write!(f, "invalid transaction: {msg}"),
			Self::Dropped(msg) => write!(f, "dropped transaction: {msg}"),
			Self::Node(msg) => write!(f, "node error: {msg}"),
			Self::Runtime(msg) => write!(f, "runtime error: {msg}"),
			Self::StreamEnded => write!(f, "extrinsic stream ended before inclusion"),
		}
	}
}

impl std::error::Error for SubmitError {}

/// Handles nonce tracking and emits structured progress notifications for extrinsics.
pub struct TxSubmitter<'a, S>
where
	S: subxt::tx::Signer<CordConfig>,
{
	client: &'a Client,
	signer: &'a S,
	nonce_tracker: NonceTracker,
}

impl<'a, S> TxSubmitter<'a, S>
where
	S: subxt::tx::Signer<CordConfig>,
{
	pub fn new(client: &'a Client, signer: &'a S) -> Self {
		let account_id = signer.account_id();
		Self { client, signer, nonce_tracker: NonceTracker::new(account_id.clone()) }
	}

	pub fn client(&self) -> &Client {
		self.client
	}

	pub async fn submit_with_progress<F>(
		&mut self,
		call: DynamicPayload,
		description: impl Into<String>,
		mut handler: F,
	) -> Result<ExtrinsicEvents<crate::params::config::CordConfig>, SubmitError>
	where
		F: FnMut(SubmitStage),
	{
		let desc = description.into();
		let nonce = self
			.nonce_tracker
			.reserve(self.client)
			.await
			.map_err(|e| SubmitError::Node(e.to_string()))?;
		let opts = TxOptions { nonce: Some(NonceMode::Manual(nonce)), tip: None, era: None };
		let mut progress = match self
			.client
			.tx()
			.sign_and_submit_then_watch_with_opts(call, self.signer, opts)
			.await
		{
			Ok(progress) => progress,
			Err(err) => {
				self.nonce_tracker.rollback();
				return Err(SubmitError::Node(err.to_string()));
			},
		};

		while let Some(status) = progress.next().await {
			let status = match status {
				Ok(s) => s,
				Err(err) => {
					self.nonce_tracker.rollback();
					return Err(SubmitError::Node(err.to_string()));
				},
			};
			match status {
				TxStatus::Validated => handler(SubmitStage::Validated),
				TxStatus::Broadcasted => handler(SubmitStage::Broadcasted),
				TxStatus::NoLongerInBestBlock => handler(SubmitStage::Retracted),
				TxStatus::InBestBlock(in_block) => {
					let block_hash: H256 = in_block.block_hash().clone();
					let label = block_label(self.client, block_hash).await;
					handler(SubmitStage::InBlock { hash: block_hash, label });
					let events =
						in_block.wait_for_success().await.map_err(SubmitError::from_subxt_error)?;
					self.nonce_tracker.confirm();
					handler(SubmitStage::Completed { description: desc.clone() });
					return Ok(events);
				},
				TxStatus::InFinalizedBlock(in_block) => {
					let block_hash: H256 = in_block.block_hash().clone();
					let label = block_label(self.client, block_hash).await;
					handler(SubmitStage::Finalized {
						hash: block_hash,
						label,
						description: desc.clone(),
					});
					let events =
						in_block.wait_for_success().await.map_err(SubmitError::from_subxt_error)?;
					self.nonce_tracker.confirm();
					handler(SubmitStage::Completed { description: desc.clone() });
					return Ok(events);
				},
				TxStatus::Error { message } => {
					self.nonce_tracker.rollback();
					return Err(SubmitError::Node(message));
				},
				TxStatus::Invalid { message } => {
					self.nonce_tracker.rollback();
					return Err(SubmitError::Invalid(message));
				},
				TxStatus::Dropped { message } => {
					self.nonce_tracker.rollback();
					return Err(SubmitError::Dropped(message));
				},
			}
		}

		self.nonce_tracker.rollback();
		Err(SubmitError::StreamEnded)
	}
}

async fn block_label(client: &Client, block_hash: H256) -> Option<String> {
	match client.online().blocks().at(block_hash).await {
		Ok(block) => Some(format!("#{} ({block_hash:?})", block.number())),
		Err(_) => None,
	}
}
