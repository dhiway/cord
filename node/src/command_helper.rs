// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
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

//! Contains code to setup the command invocations in [`super::command`] which would
//! otherwise bloat that module.

use crate::service::FullClient;

// use cord_runtime as runtime;
use cord_runtime::SystemCall;
use sc_cli::Result;
use sc_client_api::BlockBackend;
use sp_core::{Encode, Pair};
use sp_inherents::{InherentData, InherentDataProvider};
use sp_keyring::Sr25519Keyring;
use sp_runtime::{OpaqueExtrinsic, SaturatedConversion};

use std::{sync::Arc, time::Duration};

/// Generates extrinsics for the `benchmark overhead` command.
///
/// Note: Should only be used for benchmarking.
pub struct BenchmarkExtrinsicBuilder {
	client: Arc<FullClient>,
}

impl BenchmarkExtrinsicBuilder {
	/// Creates a new [`Self`] from the given client.
	pub fn new(client: Arc<FullClient>) -> Self {
		Self { client }
	}
}

impl frame_benchmarking_cli::ExtrinsicBuilder for BenchmarkExtrinsicBuilder {
	fn remark(&self, nonce: u32) -> std::result::Result<OpaqueExtrinsic, &'static str> {
		let acc = Sr25519Keyring::Bob.pair();
		let extrinsic: OpaqueExtrinsic = create_benchmark_extrinsic(
			self.client.as_ref(),
			acc,
			SystemCall::remark { remark: vec![] },
			nonce,
		)
		.into();

		Ok(extrinsic)
	}
}

/// Create a transaction using the given `call`.
///
/// Note: Should only be used for benchmarking.
pub fn create_benchmark_extrinsic(
	client: &FullClient,
	sender: sp_core::sr25519::Pair,
	function: impl Into<cord_runtime::Call>,
	nonce: u32,
) -> cord_runtime::UncheckedExtrinsic {
	let function = function.into();
	let genesis_hash = client.block_hash(0).ok().flatten().expect("Genesis block exists; qed");
	let best_hash = client.chain_info().best_hash;
	let best_block = client.chain_info().best_number;
	// let nonce = nonce.unwrap_or_else(|| fetch_nonce(client, sender.clone()));

	let period = cord_runtime::BlockHashCount::get()
		.checked_next_power_of_two()
		.map(|c| c / 2)
		.unwrap_or(2) as u64;
	let extra: cord_runtime::SignedExtra = (
		frame_system::CheckNonZeroSender::<cord_runtime::Runtime>::new(),
		frame_system::CheckSpecVersion::<cord_runtime::Runtime>::new(),
		frame_system::CheckTxVersion::<cord_runtime::Runtime>::new(),
		frame_system::CheckGenesis::<cord_runtime::Runtime>::new(),
		frame_system::CheckEra::<cord_runtime::Runtime>::from(sp_runtime::generic::Era::mortal(
			period,
			best_block.saturated_into(),
		)),
		frame_system::CheckNonce::<cord_runtime::Runtime>::from(nonce),
		frame_system::CheckWeight::<cord_runtime::Runtime>::new(),
		pallet_transaction_payment::ChargeTransactionPayment::<cord_runtime::Runtime>::from(0),
	);

	let raw_payload = cord_runtime::SignedPayload::from_raw(
		function.clone(),
		extra.clone(),
		(
			(),
			cord_runtime::VERSION.spec_version,
			cord_runtime::VERSION.transaction_version,
			genesis_hash,
			best_hash,
			(),
			(),
			(),
		),
	);
	let signature = raw_payload.using_encoded(|e| sender.sign(e));

	cord_runtime::UncheckedExtrinsic::new_signed(
		function.clone(),
		sp_runtime::AccountId32::from(sender.public()).into(),
		cord_runtime::Signature::Sr25519(signature.clone()),
		extra.clone(),
	)
}

/// Generates inherent data for the `benchmark overhead` command.
///
/// Note: Should only be used for benchmarking.
pub fn inherent_benchmark_data() -> Result<InherentData> {
	let mut inherent_data = InherentData::new();
	let d = Duration::from_millis(0);
	let timestamp = sp_timestamp::InherentDataProvider::new(d.into());

	timestamp
		.provide_inherent_data(&mut inherent_data)
		.map_err(|e| format!("creating inherent data: {:?}", e))?;
	Ok(inherent_data)
}
