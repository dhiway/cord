/*
 * This file is part of the CORD
 * Copyright (C) 2020-21  Dhiway
 *
 */

//! A set of constant values used in runtime.

/// Money matters.
pub mod currency {
	use primitives::Balance;

	pub const DCU: Balance = 1_000_000_000_000_000;
	pub const DOLLARS: Balance = DCU;
	pub const CENTS: Balance = DOLLARS / 100;     
	pub const MILLICENTS: Balance = CENTS / 1_000; 

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 1 * DOLLARS + (bytes as Balance) * 5 * MILLICENTS
	}
}

/// Time and blocks.
pub mod time {
	use primitives::{Moment, BlockNumber};
	// use primitives::{Moment};
	
	pub const SECS_PER_BLOCK: Moment = 4;
	pub const MILLISECS_PER_BLOCK: Moment = SECS_PER_BLOCK * 1000;

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60 / (SECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;

	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
}

/// Fee-related
pub mod fee {
	pub use super::currency::CENTS;
	use frame_support::weights::{
		constants::ExtrinsicBaseWeight, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use primitives::Balance;
	use smallvec::smallvec;
	use sp_runtime::Perbill;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	/// Handles converting a weight scalar to a fee value, based on the scale
	/// and granularity of the node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, system::MaximumBlockWeight]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some
	/// examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to
	///     be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// in cord, extrinsic base weight (smallest non-zero weight) is mapped to 1/10
			// CENT:
			let p = super::currency::CENTS;
			let q = 10 * Balance::from(ExtrinsicBaseWeight::get()); 
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational_approximation(p % q, q), // zero
				coeff_integer: p / q,                                       // 8_000_000
			}]
		}
	}
}

#[cfg(test)]
mod tests {
	use frame_support::weights::{WeightToFeePolynomial, DispatchClass};
	use runtime_common::BlockWeights;
	use super::fee::WeightToFee;
	use super::currency::{CENTS, DOLLARS, MILLICENTS};

	#[test]
	// This function tests that the fee for `MaximumBlockWeight` of weight is correct
	fn full_block_fee_is_correct() {
		// A full block should cost 16 DOLLARS
		println!("Base: {}", BlockWeights::get().get(DispatchClass::Normal).base_extrinsic);
		let x = WeightToFee::calc(&BlockWeights::get().max_block);
		let y = 16 * DOLLARS;
		assert!(x.max(y) - x.min(y) < MILLICENTS);
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/10 of a CENT
		let base_weight = BlockWeights::get().get(DispatchClass::Normal).base_extrinsic;
		println!("Base: {}", base_weight);
		let x = WeightToFee::calc(&base_weight);
		let y = CENTS / 10;
		assert!(x.max(y) - x.min(y) < MILLICENTS);
	}
}
