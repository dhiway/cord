// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;

/// Money matters.
pub mod currency {
	use polkadot_primitives::Balance;

	/// The existential deposit.
	pub const EXISTENTIAL_DEPOSIT: Balance = MILLI;

	pub const UNITS: Balance = 1_000_000_000_000;
	pub const GIGA: Balance = UNITS * 1_000; // 1_000_000_000_000_000
	pub const MILLI: Balance = UNITS / 100; // 10_000_000_000
	pub const MICRO: Balance = MILLI / 1_000; // 1_000_000

	pub const SUPPLY_FACTOR: Balance = 100;
	pub const STORAGE_BYTE_FEE: Balance = 100 * MICRO * SUPPLY_FACTOR;
	pub const STORAGE_ITEM_FEE: Balance = 100 * MILLI * SUPPLY_FACTOR;

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * STORAGE_ITEM_FEE + (bytes as Balance) * STORAGE_BYTE_FEE
	}
}

/// Time and blocks.
pub mod time {
	use polkadot_primitives::{BlockNumber, Moment};
	use polkadot_runtime_common::prod_or_fast;

	pub const MILLISECS_PER_BLOCK: Moment = 6000;
	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
	// pub const EPOCH_DURATION_IN_SLOTS: BlockNumber = prod_or_fast!(4 * HOURS, MINUTES);
	pub const EPOCH_DURATION_IN_SLOTS: BlockNumber = prod_or_fast!(2 * MINUTES, MINUTES);

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;
	pub const WEEKS: BlockNumber = DAYS * 7;

	// 1 in 4 blocks (on average, not counting collisions) will be primary babe blocks.
	// The choice of is done in accordance to the slot duration and expected target
	// block time, for safely resisting network delays of maximum two seconds.
	// <https://research.web3.foundation/Polkadot/protocols/block-production/Babe#6-practical-results>
	pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
}

/// Fee-related.
pub mod fee {
	use crate::weights::ExtrinsicBaseWeight;
	use frame_support::weights::{
		WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use polkadot_primitives::Balance;
	use smallvec::smallvec;
	pub use sp_runtime::Perbill;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	/// Cost of every transaction byte at the relay chain.
	pub const TRANSACTION_BYTE_FEE: Balance = 10 * super::currency::MICRO;

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0,` MAXIMUM_BLOCK_WEIGHT`]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// in Loom, extrinsic base weight (smallest non-zero weight) is mapped to 1/10 CENT:
			let p = super::currency::MILLI;
			let q = 10 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}
}

/// System Parachains.
pub mod system_parachain {
	use polkadot_primitives::Id;
	use xcm_builder::IsChildSystemParachain;

	/// Network's Asset Hub parachain ID.
	pub const ASSET_HUB_ID: u32 = 1000;
	/// Collectives parachain ID.
	pub const COLLECTIVES_ID: u32 = 1001;
	/// People Chain parachain ID.
	pub const PEOPLE_ID: u32 = 1002;
	/// Brokerage parachain ID.
	pub const BROKER_ID: u32 = 1003;

	/// All system parachains of Loom.
	pub type SystemParachains = IsChildSystemParachain<Id>;

	/// Coretime constants
	pub mod coretime {
		/// Coretime timeslice period in blocks
		/// WARNING: This constant is used accross chains, so additional care should be taken
		/// when changing it.
		#[cfg(feature = "fast-runtime")]
		pub const TIMESLICE_PERIOD: u32 = 20;
		#[cfg(not(feature = "fast-runtime"))]
		pub const TIMESLICE_PERIOD: u32 = 80;
	}
}

/// Westend Treasury pallet instance.
pub const TREASURY_PALLET_ID: u8 = 18;

#[cfg(test)]
mod tests {
	use super::{
		currency::{MICRO, MILLI, UNITS},
		fee::WeightToFee,
	};
	use crate::weights::ExtrinsicBaseWeight;
	use frame_support::weights::WeightToFee as WeightToFeeT;
	use polkadot_runtime_common::MAXIMUM_BLOCK_WEIGHT;

	#[test]
	// Test that the fee for `MAXIMUM_BLOCK_WEIGHT` of weight has sane bounds.
	fn full_block_fee_is_correct() {
		// A full block should cost between 10 and 100 UNITS.
		let full_block = WeightToFee::weight_to_fee(&MAXIMUM_BLOCK_WEIGHT);
		assert!(full_block >= 10 * UNITS);
		assert!(full_block <= 100 * UNITS);
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/10 of a MILLI
		println!("Base: {}", ExtrinsicBaseWeight::get());
		let x = WeightToFee::weight_to_fee(&ExtrinsicBaseWeight::get());
		let y = MILLI / 10;
		assert!(x.max(y) - x.min(y) < MICRO);
	}
}
