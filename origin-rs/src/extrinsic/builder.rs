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

use subxt::{dynamic::Value, tx::Payload, Error, Metadata};

/// Structured call descriptor.
#[derive(Clone, Debug)]
pub struct DynamicCall {
	pub pallet: String,
	pub function: String,
	pub args: Vec<Value>,
}

impl DynamicCall {
	/// Convert into a Subxt dynamic payload (re-usable across helpers).
	pub fn to_payload(&self) -> subxt::tx::DynamicPayload {
		subxt::dynamic::tx(self.pallet.as_str(), self.function.as_str(), self.args.clone())
	}

	/// Encode call bytes using runtime metadata (same as pallet_meta_tx mock).
	pub fn encode_call_data(&self, metadata: &Metadata) -> Result<Vec<u8>, Error> {
		self.to_payload().encode_call_data(metadata).map_err(Into::into)
	}
}

/// Fluent builder for dynamic extrinsics.
pub struct DynamicCallBuilder;

impl DynamicCallBuilder {
	pub fn new() -> Self {
		Self
	}

	pub fn call(&self, pallet: &str, function: &str, args: Vec<Value>) -> DynamicCall {
		DynamicCall { pallet: pallet.into(), function: function.into(), args }
	}
}
