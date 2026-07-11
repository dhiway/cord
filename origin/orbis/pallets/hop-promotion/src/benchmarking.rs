// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Benchmarks for `pallet-bulletin-hop-promotion`.

use super::{signing_payload, Call, Config, Pallet};
use alloc::vec;
use frame_support::traits::Authorize;
use pallet_bulletin_transaction_storage::Config as TxStorageConfig;
use polkadot_sdk_frame::benchmarking::prelude::*;
use sp_io::{
	crypto::{sr25519_generate, sr25519_sign},
	hashing::blake2_256,
};
use sp_runtime::{
	traits::IdentifyAccount, transaction_validity::TransactionSource, MultiSignature, MultiSigner,
};

#[benchmarks(where T: Send + Sync)]
mod benchmarks {
	use super::*;

	/// Worst-case authorize path: all checks pass through to the sr25519 verify and
	/// blake2_256 over `data` of length `d`.
	#[benchmark]
	fn authorize_promote(
		d: Linear<1, { <T as TxStorageConfig>::MaxTransactionSize::get() }>,
	) -> Result<(), BenchmarkError> {
		// Pin a non-zero `now` so the freshness check passes. Write `Now` directly
		// to avoid `OnTimestampSet`, which would route into Aura and panic because
		// `CurrentSlot` is unset in the benchmark environment.
		let ts: u64 = 1_700_000_000_000;
		pallet_timestamp::Now::<T>::put(ts);

		// Sr25519 key in the bench keystore; the matching public seeds the signer.
		let public = sr25519_generate(0.into(), None);
		let signer = MultiSigner::Sr25519(public);
		let account_id = signer.clone().into_account();

		// Authorize the account so `account_has_active_authorization` returns true.
		let auth_origin = <T as TxStorageConfig>::Authorizer::try_successful_origin()
			.map_err(|_| BenchmarkError::Stop("unable to compute authorizer origin"))?;
		// Allowance does not gate `can_account_promote` (it only requires an active
		// authorization entry), so a 1-byte allowance suffices.
		pallet_bulletin_transaction_storage::Pallet::<T>::authorize_account(
			auth_origin,
			account_id.clone(),
			1,
			1,
		)
		.map_err(|_| BenchmarkError::Stop("unable to authorize account"))?;

		// Sign the canonical payload.
		let data = vec![0u8; d as usize];
		let payload = signing_payload(&blake2_256(&data), ts);
		let sig = sr25519_sign(0.into(), &public, &payload[..])
			.ok_or(BenchmarkError::Stop("unable to sign"))?;
		let signature = MultiSignature::Sr25519(sig);

		let call = Call::<T>::promote { data, signer, signature, submit_timestamp: ts };

		#[block]
		{
			call.authorize(TransactionSource::InBlock)
				.expect("call has an authorize hook")
				.expect("authorize closure returns Ok");
		}

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
