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

pub mod elections;

use core::marker;

use cord_primitives::{AccountId, Balance, BlockNumber};
use frame_support::{
	dispatch::DispatchClass,
	parameter_types,
	traits::{
		fungible::{Balanced, Credit},
		tokens::imbalance::ResolveTo,
		Imbalance, OnUnbalanced,
	},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
};
use pallet_treasury::TreasuryAccountId;

use frame_system::limits;
use sp_runtime::{FixedPointNumber, Perbill, Perquintill};
use static_assertions::const_assert;

pub use pallet_balances::Call as BalancesCall;
use pallet_balances::Pallet as PalletBalance;
#[cfg(feature = "std")]
pub use pallet_staking::StakerStatus;
pub use pallet_timestamp::Call as TimestampCall;
use pallet_transaction_payment::{Multiplier, TargetedFeeAdjustment};
use sp_core::{crypto::Ss58AddressFormat, TypedGet};
pub use sp_runtime::traits::{Bounded, Get};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

#[derive(Debug, Clone, Copy)]
pub enum Ss58AddressFormatPrefix {
	/// Default for Braid
	Braid = 3893,
	/// Default for Loom
	Loom = 4926,
	/// Default for Weave
	Weave = 29,
	/// Default for unknown chains
	Default = 42,
}

impl From<Ss58AddressFormatPrefix> for Ss58AddressFormat {
	fn from(prefix: Ss58AddressFormatPrefix) -> Self {
		Ss58AddressFormat::custom(prefix as u16)
	}
}

pub(crate) type CreditOf<T> = Credit<<T as frame_system::Config>::AccountId, PalletBalance<T, ()>>;

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
	pub BlockLength: limits::BlockLength = limits::BlockLength::builder()
		.max_length(5 * 1024 * 1024)
		.modify_max_length_for_class(DispatchClass::Normal, |m| {
			*m = NORMAL_DISPATCH_RATIO * *m
		})
		.build();
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

pub struct DealWithFees<R>(core::marker::PhantomData<R>);
impl<R> OnUnbalanced<Credit<R::AccountId, pallet_balances::Pallet<R>>> for DealWithFees<R>
where
	R: pallet_balances::Config + pallet_authorship::Config + pallet_treasury::Config,
	<R as frame_system::Config>::AccountId: From<AccountId>,
	<R as frame_system::Config>::AccountId: Into<AccountId>,
{
	fn on_unbalanceds(
		mut fees_then_tips: impl Iterator<Item = Credit<R::AccountId, pallet_balances::Pallet<R>>>,
	) {
		// Merge all unbalanced amounts (fees and tips) into a single Credit
		if let Some(mut total_unbalanced) = fees_then_tips.next() {
			for additional_unbalanced in fees_then_tips {
				additional_unbalanced.merge_into(&mut total_unbalanced);
			}
			// Send the merged amount to the treasury
			ResolveTo::<TreasuryAccountId<R>, pallet_balances::Pallet<R>>::on_unbalanced(
				total_unbalanced,
			);
		}
	}
}

pub struct SendFeesToTreasury<R>(marker::PhantomData<R>);

impl<R> OnUnbalanced<CreditOf<R>> for SendFeesToTreasury<R>
where
	R: pallet_balances::Config + pallet_treasury::Config,
{
	fn on_nonzero_unbalanced(amount: CreditOf<R>) {
		// let treasury_account_id = pallet_treasury::Pallet::<T>::account_id()
		let treasury_account = TreasuryAccountId::<R>::get();

		let result = pallet_balances::Pallet::<R>::resolve(&treasury_account, amount);
		debug_assert!(result.is_ok(), "The whole credit cannot be countered");
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
			impl_elections_weights, AVERAGE_ON_INITIALIZE_RATIO, MAXIMUM_BLOCK_WEIGHT,
			NORMAL_DISPATCH_RATIO,
		};
		use sp_runtime::{FixedPointNumber, Perquintill};

		// Implement the weight types of the elections module.
		impl_elections_weights!($runtime);

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

/// The type used for currency conversion.
///
/// This must only be used as long as the balance type is `u128`.
pub type CurrencyToVote = sp_staking::currency_to_vote::U128CurrencyToVote;
static_assertions::assert_eq_size!(cord_primitives::Balance, u128);

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
