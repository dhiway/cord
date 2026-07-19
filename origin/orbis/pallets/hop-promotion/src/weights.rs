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

//! Weights for `pallet-orbis-hop-promotion`.
//!
//! `authorize_promote` measures the cost of the `#[pallet::authorize]` path for
//! [`crate::Call::promote`]: storage reads for block-fullness / timestamp /
//! account authorization, a `blake2_256` over the data (parameterized by `d`),
//! and an `sr25519` signature verify. The dispatch body itself reuses
//! `pallet_orbis_transaction_storage::WeightInfo::store`, so no `promote`
//! weight is needed here.

use polkadot_sdk_frame::weights_prelude::*;

/// Weight functions needed for `pallet-orbis-hop-promotion`.
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
