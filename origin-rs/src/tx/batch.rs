use subxt::dynamic::{self, Value};

use crate::{
	extrinsic::builder::DynamicCall,
	tx::{handle::TxHandle, AccountTx},
	types::error::OriginSdkError,
};

/// Collects multiple calls for atomic submission via Utility::batch/batch_all.
pub struct BatchBuilder {
	calls: Vec<DynamicCall>,
	all: bool,
	account: AccountTx,
}

impl BatchBuilder {
	pub fn new(account: AccountTx) -> Self {
		Self { calls: Vec::new(), all: true, account }
	}

	pub fn call(mut self, call: DynamicCall) -> Self {
		self.calls.push(call);
		self
	}

	pub fn call_many<I>(mut self, calls: I) -> Self
	where
		I: IntoIterator<Item = DynamicCall>,
	{
		self.calls.extend(calls);
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

	fn build(&self) -> Result<subxt::tx::DynamicPayload, OriginSdkError> {
		let calls: Vec<Value> = self
			.calls
			.iter()
			.cloned()
			.map(|c| dynamic::tx(c.pallet, c.function, c.args).into_value())
			.collect();
		let fn_name = if self.all { "batch_all" } else { "batch" };
		Ok(dynamic::tx("Utility", fn_name, calls))
	}

	pub async fn submit_and_wait_finalized(
		self,
	) -> Result<crate::tx::handle::TxOutcome, OriginSdkError> {
		let handle = self.submit().await?;
		handle.wait_finalized().await
	}

	pub async fn submit(self) -> Result<TxHandle, OriginSdkError> {
		let payload = self.build()?;
		self.account.submit(payload).await
	}
}
