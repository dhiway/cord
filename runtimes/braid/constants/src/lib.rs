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
	pub const UNITS: Balance = 1_000_000_000_000; // Base unit (1 trillion)
	pub const CENTI: Balance = UNITS / 100; // 1% of a UNIT (10 billion)
	pub const MILLI: Balance = UNITS / 1_000; // 0.1% of a UNIT (1 billion)
	pub const MICRO: Balance = UNITS / 1_000_000; // 1 millionth of a UNIT (1 million)
	pub const NANO: Balance = UNITS / 1_000_000_000; // 1 billionth of a UNIT (1 thousand)
	pub const GRAND: Balance = UNITS * 1_000; // 1,000 UNITS (1 quadrillion)

	/// The existential deposit.
	pub const EXISTENTIAL_DEPOSIT: Balance = CENTI;

	// Provide a common factor between runtimes
	pub const SUPPLY_FACTOR: Balance = 100;
	pub const STORAGE_BYTE_FEE: Balance = 100 * MICRO * SUPPLY_FACTOR;
	pub const STORAGE_ITEM_FEE: Balance = 100 * MILLI * SUPPLY_FACTOR;

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * STORAGE_ITEM_FEE + (bytes as Balance) * STORAGE_BYTE_FEE
	}
}

/// Time and blocks.
pub mod time {
	use cord_primitives::{BlockNumber, Moment};
	use cord_runtime_common::prod_or_fast;
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
	pub use sp_runtime::{FixedPointNumber, FixedU128, Perbill};

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	/// Cost of every transaction byte.
	pub const TRANSACTION_BYTE_FEE: Balance = 10 * super::currency::MILLI;

	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;

		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			let p = super::currency::CENTI;
			let q = 10 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
			let coeff_integer = p / q;
			let coeff_frac = Perbill::from_rational(p % q, q);
			let multiplier = FixedU128::saturating_from_rational(2, 1);
			let adjusted_coeff_integer =
				(coeff_integer * multiplier.into_inner()) / FixedU128::DIV as Balance;

			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_integer: adjusted_coeff_integer,
				coeff_frac,
			}]
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{
		currency::{MILLI, UNITS},
		fee::WeightToFee,
	};
	use crate::weights::ExtrinsicBaseWeight;
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
		assert!(full_block >= 1_00 * UNITS);
		assert!(full_block <= 2_000 * UNITS);
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is
	// correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/10 of a UNIT
		let x = WeightToFee::weight_to_fee(&ExtrinsicBaseWeight::get());
		let y = UNITS / 10;
		assert!(x.max(y) - x.min(y) < MILLI);
	}
}
