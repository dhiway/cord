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

/// Consensus-related.
pub mod consensus {
	/// Maximum number of blocks simultaneously accepted by the Runtime, not yet included
	/// into the relay chain.
	pub const UNINCLUDED_SEGMENT_CAPACITY: u32 = 3;
	/// How many parachain blocks are processed by the relay chain per parent. Limits the
	/// number of blocks authored per slot.
	pub const BLOCK_PROCESSING_VELOCITY: u32 = 1;
	/// Relay chain slot duration, in milliseconds.
	pub const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6000;
}

/// Constants relating to KSM.
pub mod currency {
	use polkadot_core_primitives::Balance;

	/// The default existential deposit for system chains. 1/10th of the Relay Chain's existential
	/// deposit. Individual system parachains may modify this in special cases.
	pub const SYSTEM_PARA_EXISTENTIAL_DEPOSIT: Balance =
		cord_loom_runtime_constants::currency::EXISTENTIAL_DEPOSIT / 10;

	/// One "KSM" that a UI would show a user.
	pub const UNITS: Balance = 1_000_000_000_000;
	pub const GIGA: Balance = UNITS * 1_000; // 1_000_000_000_000_000
	pub const MILLI: Balance = UNITS / 100; // 10_000_000_000
	pub const MICRO: Balance = MILLI / 1_000; // 1_000_000

	/// Deposit rate for stored data. 1/100th of the Relay Chain's deposit rate. `items` is the
	/// number of keys in storage and `bytes` is the size of the value.
	pub const fn system_para_deposit(items: u32, bytes: u32) -> Balance {
		cord_loom_runtime_constants::currency::deposit(items, bytes) / 100
	}
}

/// Constants related to Kusama fee payment.
pub mod fee {
	use frame_support::{
		pallet_prelude::Weight,
		weights::{
			constants::ExtrinsicBaseWeight, FeePolynomial, WeightToFeeCoefficient,
			WeightToFeeCoefficients, WeightToFeePolynomial,
		},
	};
	use polkadot_core_primitives::Balance;
	use smallvec::smallvec;
	pub use sp_runtime::Perbill;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	/// Cost of every transaction byte at Kusama system parachains.
	///
	/// It is the Relay Chain (Kusama) `TransactionByteFee` / 10.
	pub const TRANSACTION_BYTE_FEE: Balance = super::currency::MICRO;

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
	impl frame_support::weights::WeightToFee for WeightToFee {
		type Balance = Balance;

		fn weight_to_fee(weight: &Weight) -> Self::Balance {
			let time_poly: FeePolynomial<Balance> = RefTimeToFee::polynomial().into();
			let proof_poly: FeePolynomial<Balance> = ProofSizeToFee::polynomial().into();

			// Take the maximum instead of the sum to charge by the more scarce resource.
			time_poly.eval(weight.ref_time()).max(proof_poly.eval(weight.proof_size()))
		}
	}

	/// Maps the reference time component of `Weight` to a fee.
	pub struct RefTimeToFee;
	impl WeightToFeePolynomial for RefTimeToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// In Kusama, extrinsic base weight (smallest non-zero weight) is mapped to 1/10 CENT:
			// The standard system parachain configuration is 1/10 of that, as in 1/100 CENT.
			let p = super::currency::MILLI;
			let q = 100 * Balance::from(ExtrinsicBaseWeight::get().ref_time());

			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}

	/// Maps the proof size component of `Weight` to a fee.
	pub struct ProofSizeToFee;
	impl WeightToFeePolynomial for ProofSizeToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// Map 10kb proof to 1 CENT.
			let p = super::currency::MILLI;
			let q = 10_000;

			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}

	pub fn calculate_weight_to_fee(weight: &Weight) -> Balance {
		<WeightToFee as frame_support::weights::WeightToFee>::weight_to_fee(weight)
	}
}
