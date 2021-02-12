// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! A set of constant values used in runtime.

/// Money matters.
pub mod currency {
	use cord_primitives::Balance;

	pub const DCU: Balance = 1;
	pub const RUPEES: Balance = DCU;
	pub const PAISE: Balance = RUPEES / 100;     
	pub const MILLIPAISE: Balance = PAISE / 100; 

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 10 * RUPEES + (bytes as Balance) * 100 * MILLIPAISE
	}
}

/// Time and blocks.
pub mod time {
	use cord_primitives::{Moment, BlockNumber};
	pub const MILLISECS_PER_BLOCK: Moment = 4000;
	pub const SECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK / 1000;

	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
	pub const EPOCH_DURATION_IN_BLOCKS: BlockNumber = 4 * HOURS;

	pub const EPOCH_DURATION_IN_SLOTS: u64 = {
		const SLOT_FILL_RATE: f64 = MILLISECS_PER_BLOCK as f64 / SLOT_DURATION as f64;

		(EPOCH_DURATION_IN_BLOCKS as f64 * SLOT_FILL_RATE) as u64
	};

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60 / (SECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;

}

/// Fee-related.
pub mod fee {
	pub use sp_runtime::Perbill;
	use cord_primitives::Balance;
	use frame_support::weights::constants::ExtrinsicBaseWeight;
	use frame_support::weights::{
		WeightToFeePolynomial, WeightToFeeCoefficient, WeightToFeeCoefficients,
	};
	use smallvec::smallvec;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, MAXIMUM_BLOCK_WEIGHT]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// in CORD, extrinsic base weight (smallest non-zero weight) is mapped to 1/10 PAISE:
			let p = super::currency::PAISE;
			let q = 10 * Balance::from(ExtrinsicBaseWeight::get());
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational_approximation(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}
}
