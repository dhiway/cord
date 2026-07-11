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

//! Weights for `pallet-bulletin-hop-promotion`.
//!
//! `authorize_promote` measures the cost of the `#[pallet::authorize]` path for
//! [`crate::Call::promote`]: storage reads for block-fullness / timestamp /
//! account authorization, a `blake2_256` over the data (parameterized by `d`),
//! and an `sr25519` signature verify. The dispatch body itself reuses
//! `pallet_bulletin_transaction_storage::WeightInfo::store`, so no `promote`
//! weight is needed here.

use polkadot_sdk_frame::weights_prelude::*;

/// Weight functions needed for `pallet-bulletin-hop-promotion`.
pub trait WeightInfo {
	/// Worst-case weight of the `#[pallet::authorize]` closure for
	/// [`crate::Call::promote`], parameterized by `d` = data length in bytes.
	fn authorize_promote(d: u32) -> Weight;
}

impl WeightInfo for () {
	fn authorize_promote(_d: u32) -> Weight {
		Weight::zero()
	}
}
