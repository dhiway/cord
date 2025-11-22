use subxt::dynamic::{self, Value};

use super::builder::DynamicCall;
use crate::client::submit::SubmitClient;
use crate::types::error::OriginSdkError;

/// Collects multiple calls for atomic submission via Utility::batch/batch_all.
pub struct BatchBuilder {
	calls: Vec<DynamicCall>,
	all: bool,
	client: SubmitClient,
}

impl BatchBuilder {
	pub fn new(client: SubmitClient) -> Self {
		Self { calls: Vec::new(), all: true, client }
	}

	pub fn call(mut self, call: DynamicCall) -> Self {
		self.calls.push(call);
		self
	}

	pub fn mode_batch(mut self) -> Self {
		self.all = false;
		self
	}

	pub fn mode_batch_all(mut self) -> Self {
		self.all = true;
		self
	}

	pub fn build(self) -> Result<subxt::tx::DynamicPayload, OriginSdkError> {
		let calls: Vec<Value> = self
			.calls
			.into_iter()
			.map(|c| dynamic::tx(c.pallet, c.function, c.args).into_value())
			.collect();
		let fn_name = if self.all { "batch_all" } else { "batch" };
		Ok(dynamic::tx("Utility", fn_name, calls))
	}

	pub async fn submit_and_wait_finalized(
		self,
	) -> Result<crate::client::submit::TxOutcome, OriginSdkError> {
		let handle = self.client.batch_submit(self.calls, self.all).await?;
		handle.wait_in_block().await
	}
}
