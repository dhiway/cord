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

//! Common runtime code

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	parameter_types,
	traits::{
		fungible::{Balanced, Credit},
		OnUnbalanced,
	},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
};
use origin_primitives::{AccountId, Balance, BlockNumber};

use frame_system::limits;
use sp_runtime::{FixedPointNumber, Perbill, Perquintill};
use static_assertions::const_assert;

pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;
use pallet_transaction_payment::{Multiplier, TargetedFeeAdjustment};
pub use sp_runtime::traits::{Bounded, Get};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

/// We assume that an on-initialize consumes 1% of the weight on average, hence a single extrinsic
/// will not be allowed to consume more than `AvailableBlockRatio - 1%`.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(1);
/// We allow `Normal` extrinsics to fill up the block up to 80%, the rest can be used
/// by  Operational  extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(80);
/// We allow for 2 seconds of compute with a 3 second average block time.
/// The storage proof size is not limited so far.
pub const MAXIMUM_BLOCK_WEIGHT: Weight =
	Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2), u64::MAX);

const_assert!(NORMAL_DISPATCH_RATIO.deconstruct() >= AVERAGE_ON_INITIALIZE_RATIO.deconstruct());

// Common constants used in all runtimes.
parameter_types! {
	pub const BlockHashCount: BlockNumber = 4096;
	/// The portion of the `NORMAL_DISPATCH_RATIO` that we adjust the fees with. Blocks filled less
	/// than this will decrease the weight and more will increase.
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	/// The adjustment variable of the runtime. Higher values will cause `TargetBlockFullness` to
	/// change the fees more rapidly.
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(75, 1000_000);
	/// Minimum amount of the multiplier. This value cannot be too low. A test case should ensure
	/// that combined with `AdjustmentVariable`, we can recover from the minimum.
	/// See `multiplier_can_grow_from_zero`.
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 10u128);
	/// The maximum amount of the multiplier.
	pub MaximumMultiplier: Multiplier = Bounded::max_value();
	/// Maximum length of block. Up to 5MB.
	pub BlockLength: limits::BlockLength =
	limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
}

/// Parameterized slow adjusting fee updated based on
/// <https://research.web3.foundation/Polkadot/overview/token-economics#2-slow-adjusting-mechanism>
pub type SlowAdjustingFeeUpdate<R> = TargetedFeeAdjustment<
	R,
	TargetBlockFullness,
	AdjustmentVariable,
	MinimumMultiplier,
	MaximumMultiplier,
>;

/// Logic for the author to get a portion of fees.
pub struct ToAuthor<R>(core::marker::PhantomData<R>);
impl<R> OnUnbalanced<Credit<R::AccountId, pallet_balances::Pallet<R>>> for ToAuthor<R>
where
	R: pallet_balances::Config + pallet_authorship::Config,
	<R as frame_system::Config>::AccountId: From<AccountId>,
	<R as frame_system::Config>::AccountId: Into<AccountId>,
{
	fn on_nonzero_unbalanced(
		amount: Credit<<R as frame_system::Config>::AccountId, pallet_balances::Pallet<R>>,
	) {
		if let Some(author) = <pallet_authorship::Pallet<R>>::author() {
			let _ = <pallet_balances::Pallet<R>>::resolve(&author, amount);
		}
	}
}

/// Implements the weight types for a runtime.
/// It expects the passed runtime constants to contain a `weights` module.
/// The generated weight types were formerly part of the common
/// runtime but are now runtime dependant.
#[macro_export]
macro_rules! impl_runtime_weights {
	($runtime:ident) => {
		use frame_support::{dispatch::DispatchClass, weights::Weight};
		use frame_system::limits;
		use pallet_transaction_payment::{Multiplier, TargetedFeeAdjustment};
		pub use runtime_common::{
			AVERAGE_ON_INITIALIZE_RATIO, MAXIMUM_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO,
		};
		use sp_runtime::{FixedPointNumber, Perquintill};

		// Expose the weight from the runtime constants module.
		pub use $runtime::weights::{
			BlockExecutionWeight, ExtrinsicBaseWeight, ParityDbWeight, RocksDbWeight,
		};

		parameter_types! {
			/// Block weights base values and limits.
			pub BlockWeights: limits::BlockWeights = limits::BlockWeights::builder()
				.base_block($runtime::weights::BlockExecutionWeight::get())
				.for_class(DispatchClass::all(), |weights| {
					weights.base_extrinsic = $runtime::weights::ExtrinsicBaseWeight::get();
				})
				.for_class(DispatchClass::Normal, |weights| {
					weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
				})
				.for_class(DispatchClass::Operational, |weights| {
					weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
					// Operational transactions have an extra reserved space, so that they
					// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
					weights.reserved = Some(
						MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
					);
				})
				.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
				.build_or_panic();
		}
	};
}

static_assertions::assert_eq_size!(origin_primitives::Balance, u128);

/// Convert a balance to an unsigned 256-bit number, use in nomination pools.
pub struct BalanceToU256;
impl sp_runtime::traits::Convert<Balance, sp_core::U256> for BalanceToU256 {
	fn convert(n: Balance) -> sp_core::U256 {
		n.into()
	}
}

/// Convert an unsigned 256-bit number to balance, use in nomination pools.
pub struct U256ToBalance;
impl sp_runtime::traits::Convert<sp_core::U256, Balance> for U256ToBalance {
	fn convert(n: sp_core::U256) -> Balance {
		use frame_support::traits::Defensive;
		n.try_into().defensive_unwrap_or(Balance::MAX)
	}
}

/// Macro to set a value (e.g. when using the `parameter_types` macro) to either a production value
/// or to an environment variable or testing value (in case the `fast-runtime` feature is selected)
/// or one of two testing values depending on feature.
/// Note that the environment variable is evaluated _at compile time_.
///
/// Usage:
/// ```Rust
/// parameter_types! {
/// 	// Note that the env variable version parameter cannot be const.
/// 	pub LaunchPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1, "KSM_LAUNCH_PERIOD");
/// 	pub const VotingPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1 * MINUTES);
/// 	pub const EpochDuration: BlockNumber =
/// 		prod_or_fast!(1 * HOURS, "fast-runtime", 1 * MINUTES, "fast-runtime-10m", 10 * MINUTES);
/// }
/// ```
#[macro_export]
macro_rules! prod_or_fast {
	($prod:expr, $test:expr) => {
		if cfg!(feature = "fast-runtime") {
			$test
		} else {
			$prod
		}
	};
	($prod:expr, $test:expr, $env:expr) => {
		if cfg!(feature = "fast-runtime") {
			core::option_env!($env).map(|s| s.parse().ok()).flatten().unwrap_or($test)
		} else {
			$prod
		}
	};
}

/// Relay-chain specific helpers shared across runtimes.
pub mod relay {
	//! Utilities adopted from the upstream Polkadot runtime for managing relay economics.
	#![allow(clippy::needless_question_mark)]

	use codec::{Decode, Encode, MaxEncodedLen};
	use pallet_staking_reward_fn;
	use polkadot_primitives::Balance;
	use scale_info::TypeInfo;
	use sp_runtime::{Perquintill, Saturating};

	/// Extra runtime APIs for runtimes that expose inflation info downstream.
	pub mod apis {
		use super::*;
		use sp_api::decl_runtime_apis;

		/// Information about the current inflation rate of the system.
		///
		/// Both fields should be treated as best-effort, given that the inflation rate might not be
		/// fully predict-able.
		#[derive(TypeInfo, Encode, Decode, MaxEncodedLen)]
		#[cfg_attr(feature = "std", derive(Debug, Clone, PartialEq))]
		pub struct InflationInfo {
			/// The rate of inflation estimated per annum.
			pub inflation: sp_runtime::Perquintill,
			/// Next amount that we anticipate to mint.
			///
			/// First item is the amount that goes to stakers, second is the leftover that is usually
			/// forwarded to the treasury.
			pub next_mint: (Balance, Balance),
		}

		decl_runtime_apis! {
			pub trait Inflation {
				/// Return the current estimates of the inflation amount.
				///
				/// This is marked as experimental in light of RFC#89. Nonetheless, its usage is highly
				/// recommended over trying to read-storage, or re-create the onchain logic.
				fn experimental_inflation_prediction_info() -> InflationInfo;
			}
		}
	}

	#[derive(Debug, Clone)]
	/// Parameters passed into [`relay_era_payout`] function.
	pub struct EraPayoutParams {
		/// Total staked amount.
		pub total_staked: Balance,
		/// Total stakable amount.
		///
		/// Usually, this is equal to the total issuance, except if a large part of the issuance is
		/// locked in another sub-system.
		pub total_stakable: Balance,
		/// Ideal stake ratio, which is reduced by `legacy_auction_proportion` if not `None`.
		pub ideal_stake: Perquintill,
		/// Maximum inflation rate.
		pub max_annual_inflation: Perquintill,
		/// Minimum inflation rate.
		pub min_annual_inflation: Perquintill,
		/// Falloff used to calculate era payouts.
		pub falloff: Perquintill,
		/// Fraction of the era period used to calculate era payouts.
		pub period_fraction: Perquintill,
		/// Legacy auction proportion, which, if not `None`, is subtracted from `ideal_stake`.
		pub legacy_auction_proportion: Option<Perquintill>,
	}

	/// A specialized function to compute the inflation of the staking system, tailored for relay-style
	/// chains (e.g. Polkadot, Kusama, and Origin relay configurations).
	pub fn relay_era_payout(params: EraPayoutParams) -> (Balance, Balance) {
		let EraPayoutParams {
			total_staked,
			total_stakable,
			ideal_stake,
			max_annual_inflation,
			min_annual_inflation,
			falloff,
			period_fraction,
			legacy_auction_proportion,
		} = params;

		let delta_annual_inflation = max_annual_inflation.saturating_sub(min_annual_inflation);

		let ideal_stake = ideal_stake.saturating_sub(legacy_auction_proportion.unwrap_or_default());

		let stake = Perquintill::from_rational(total_staked, total_stakable);
		let adjustment = pallet_staking_reward_fn::compute_inflation(stake, ideal_stake, falloff);
		let staking_inflation =
			min_annual_inflation.saturating_add(delta_annual_inflation * adjustment);

		let max_payout = period_fraction * max_annual_inflation * total_stakable;
		let staking_payout = (period_fraction * staking_inflation) * total_stakable;
		let rest = max_payout.saturating_sub(staking_payout);

		let other_issuance = total_stakable.saturating_sub(total_staked);
		if total_staked > other_issuance {
			let _cap_rest =
				Perquintill::from_rational(other_issuance, total_staked) * staking_payout;
			// We don't do anything with this, but if we wanted to, we could introduce a cap on the
			// treasury amount with: `rest = rest.min(cap_rest);`
		}
		(staking_payout, rest)
	}
}

pub use relay::{apis, relay_era_payout, EraPayoutParams};
