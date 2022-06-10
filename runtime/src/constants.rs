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
//! A set of constant values used in runtime.

/// Money matters.
pub mod currency {
	use cord_primitives::Balance;

	//Royal Blue
	pub const DCU: Balance = 10u128.pow(18);

	pub const WAY: Balance = 10u128.pow(12);
	pub const MILLI_WAY: Balance = 10u128.pow(9); // mWAY
	pub const MICRO_WAY: Balance = 10u128.pow(6); // uWAY
	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 200 * MILLI_WAY + (bytes as Balance) * 100 * MICRO_WAY
	}
}

/// Time and blocks.
pub mod time {
	use cord_primitives::{BlockNumber, Moment};
	/// This determines the average expected block time that we are targetting.
	/// Blocks will be produced at a minimum duration defined by
	/// `SLOT_DURATION`. `SLOT_DURATION` is picked up by `pallet_timestamp`
	/// which is in turn picked up by `pallet_aura` to implement `fn
	/// slot_duration()`.
	///
	/// Change this to adjust the block time.
	pub const MILLISECS_PER_BLOCK: Moment = 4000;

	// NOTE: Currently it is not possible to change the slot duration after the
	// chain has started. Attempting to do so will brick block production.
	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;
}

/// Fee-related.
pub mod fee {
	use cord_primitives::Balance;

	use frame_support::{
		parameter_types,
		weights::{
			constants::WEIGHT_PER_MILLIS, Weight, WeightToFeeCoefficient, WeightToFeeCoefficients,
			WeightToFeePolynomial,
		},
	};

	use smallvec::smallvec;
	pub use sp_runtime::Perbill;

	parameter_types! {
		/// 20 ms to process an empty extrinsic.
		pub const ExtrinsicBaseWeight: Weight = 20 * WEIGHT_PER_MILLIS;
		/// We want the no-op transaction to cost 0.01 WAY
		pub const ExtrinsicBaseFee: Balance = 10 * super::currency::MILLI_WAY;
	}
	/// Converts Weight to Fee
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		/// We want a 0.01 WAY fee per ExtrinsicBaseWeight.
		/// 20_000_000_000 weight = 10_000_000_000 fee => 2 weight = 1 fee.
		/// Hence, 1 fee = 0 + 1/2 weight.
		/// This implies, coeff_integer = 0 and coeff_frac = 1/2.
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				coeff_frac: Perbill::from_rational(
					ExtrinsicBaseFee::get().into(),
					ExtrinsicBaseWeight::get() as u128
				),
				coeff_integer: 0u128, // Coefficient is zero.
				negative: false,
			}]
		}
	}
}
