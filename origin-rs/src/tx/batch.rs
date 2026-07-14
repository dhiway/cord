// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

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
		// Utility has one `calls: Vec<RuntimeCall>` argument. Passing each call as a top-level
		// argument only happens to type-check locally; it cannot encode against runtime metadata.
		Ok(dynamic::tx("Utility", fn_name, vec![Value::unnamed_composite(calls)]))
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
