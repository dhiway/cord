// CORD Chain Node – https://cord.network
// Copyright (C) 2019-2022 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.
//! A set of constant values used in runtime.

/// Money matters.
pub mod currency {
	use cord_primitives::Balance;

	pub const WAY: Balance = 10u128.pow(12);
	pub const MILLI_WAY: Balance = 10u128.pow(9); // mWAY
	pub const MICRO_WAY: Balance = 10u128.pow(6); // uWAY
	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 2_000 * MILLI_WAY + (bytes as Balance) * 100 * MICRO_WAY
	}
}

/// Time and blocks.
pub mod time {
	use cord_primitives::{BlockNumber, Moment};
	pub const MILLISECS_PER_BLOCK: Moment = 1000;
	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
	pub const EPOCH_DURATION_IN_SLOTS: BlockNumber = 1 * HOURS;

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;
	pub const WEEKS: BlockNumber = DAYS * 7;

	// 1 in 4 blocks (on average, not counting collisions) will be primary BABE
	// blocks.
	pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
}

/// Fee-related.
pub mod fee {
	use cord_primitives::Balance;
	use frame_support::weights::{
		constants::ExtrinsicBaseWeight, WeightToFeeCoefficient, WeightToFeeCoefficients,
		WeightToFeePolynomial,
	};

	use smallvec::smallvec;
	pub use sp_runtime::Perbill;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// extrinsic base weight (smallest non-zero weight) is mapped to 1/2000 MILLI_WAY units.
			let p = super::currency::MILLI_WAY;
			let q = 2000 * Balance::from(ExtrinsicBaseWeight::get());
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{
		currency::{MICRO_WAY, MILLI_WAY},
		fee::WeightToFee,
	};
	use frame_support::weights::{
		constants::{ExtrinsicBaseWeight, WEIGHT_PER_MILLIS},
		Weight, WeightToFeePolynomial,
	};
	const MAX_BLOCK_WEIGHT: Weight = 333 * WEIGHT_PER_MILLIS;

	#[test]
	// This function tests that the fee for `MAXIMUM_BLOCK_WEIGHT` of weight is correct
	fn full_block_fee_is_correct() {
		// A full block should cost 2 MILLI_WAY
		println!("Base: {}", ExtrinsicBaseWeight::get());
		let x = WeightToFee::calc(&MAX_BLOCK_WEIGHT);
		let y = 2 * MILLI_WAY;
		println!(
			"Y: {} X: {} {} {} {} {}",
			y,
			x,
			x.max(y),
			x.min(y),
			x.max(y) - x.min(y),
			MICRO_WAY
		);
		assert!(x.max(y) - x.min(y) < MILLI_WAY);
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/2000 of a MILLI_WAY
		println!("Base: {}", ExtrinsicBaseWeight::get());
		let x = WeightToFee::calc(&ExtrinsicBaseWeight::get());
		let y = MILLI_WAY / 10;
		assert!(x.max(y) - x.min(y) < MILLI_WAY);
	}
}
