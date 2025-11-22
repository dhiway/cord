use subxt::dynamic::{self, Value};

use crate::types::error::OriginSdkError;
use super::builder::DynamicCall;

/// Collects multiple calls for atomic submission via Utility::batch/batch_all.
#[derive(Default)]
pub struct BatchBuilder {
	calls: Vec<DynamicCall>,
	all: bool,
}

impl BatchBuilder {
	pub fn new() -> Self {
		Self { calls: Vec::new(), all: true }
	}

	pub fn push(mut self, call: DynamicCall) -> Self {
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

	/// Build dynamic payload for Utility::batch or batch_all.
	pub fn build(self) -> Result<dynamic::DefaultPayload<subxt::ext::scale_value::Composite<()>>, OriginSdkError> {
		let calls: Vec<Value> = self
			.calls
			.into_iter()
			.map(|c| dynamic::tx(c.pallet, c.function, c.args).into_value())
			.collect();
		let fn_name = if self.all { "batch_all" } else { "batch" };
		Ok(dynamic::tx("Utility", fn_name, calls))
	}
}
