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
//! A set of constant values used in runtime.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;

/// Money matters.
pub mod currency {
	use cord_primitives::Balance;

	/// The existential deposit.
	pub const EXISTENTIAL_DEPOSIT: Balance = 100 * MILLI_UNITS;
	pub const UNITS: Balance = 1_000_000_000_000; // 10^12 precision

	pub const MILLI_UNITS: Balance = UNITS / 1_000; // 10^9 precision
	pub const MICRO_UNITS: Balance = UNITS / 1_000_000; // 10^6 precision
	pub const NANO_UNITS: Balance = UNITS / 1_000_000_000; // 10^3 precision

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 100 * UNITS + (bytes as Balance) * 100 * MILLI_UNITS
	}
}

/// Time and blocks.
pub mod time {
	use cord_braid_runtime_common::prod_or_fast;
	use cord_primitives::{BlockNumber, Moment};
	pub const MILLISECS_PER_BLOCK: Moment = 3000;

	// NOTE: Currently it is not possible to change the slot duration after the chain has started.
	//       Attempting to do so will brick block production.
	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
	pub const EPOCH_DURATION_IN_SLOTS: BlockNumber = prod_or_fast!(4 * HOURS, MINUTES);

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;
	pub const WEEKS: BlockNumber = DAYS * 7;
	// Julian year as Substrate handles it
	pub const YEAR: BlockNumber = DAYS * 36525 / 100;

	// 1 in 4 blocks (on average, not counting collisions) will be primary babe
	// blocks. The choice of is done in accordance to the slot duration and expected
	// target block time, for safely resisting network delays of maximum two
	// seconds. <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
	pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
}

/// Fee-related.
pub mod fee {
	use crate::weights::ExtrinsicBaseWeight;
	use cord_primitives::Balance;
	use frame_support::weights::{
		WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use smallvec::smallvec;
	pub use sp_runtime::Perbill;

	// The block saturation level. Fees will be updates based on this value.
	// pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	// Cost of every transaction byte.
	// pub const TRANSACTION_BYTE_FEE: Balance = 10 * super::currency::MILLI_UNITS;

	/// Handles converting a weight scalar to a fee value, based on the scale
	/// and granularity of the node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, `frame_system::MaximumBlockWeight`]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some
	/// examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			let p = super::currency::MICRO_UNITS;
			let q = 100 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
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
	use super::{currency::MILLI_UNITS, fee::WeightToFee};
	use crate::{currency::MICRO_UNITS, weights::ExtrinsicBaseWeight};
	use frame_support::weights::{
		constants::WEIGHT_REF_TIME_PER_SECOND, Weight, WeightToFee as WeightToFeeT,
	};

	pub const MAXIMUM_BLOCK_WEIGHT: Weight =
		Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND, u64::MAX);

	#[test]
	// Test that the fee for `MAXIMUM_BLOCK_WEIGHT` of weight has sane bounds.
	fn full_block_fee_is_correct() {
		// A full block should cost between 1,00 and 1,000 UNITS.
		let full_block = WeightToFee::weight_to_fee(&MAXIMUM_BLOCK_WEIGHT);
		println!("Full Block {}", full_block);
		assert!(full_block >= 1_00 * MICRO_UNITS);
		assert!(full_block <= 2_000 * MICRO_UNITS);
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is
	// correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/10 of a UNIT
		let x = WeightToFee::weight_to_fee(&ExtrinsicBaseWeight::get());
		let y = MILLI_UNITS / 10;
		assert!(x.max(y) - x.min(y) < MILLI_UNITS);
	}
}
