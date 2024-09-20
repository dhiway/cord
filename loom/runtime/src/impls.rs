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

use frame_support::traits::{fungible::Credit, tokens::imbalance::ResolveTo, OnUnbalanced};
use pallet_treasury::TreasuryAccountId;

pub struct DealWithFees<R>(core::marker::PhantomData<R>);
impl<R> OnUnbalanced<Credit<R::AccountId, pallet_balances::Pallet<R>>> for DealWithFees<R>
where
	R: pallet_balances::Config + pallet_authorship::Config + pallet_treasury::Config,
	<R as frame_system::Config>::AccountId: From<polkadot_primitives::AccountId>,
	<R as frame_system::Config>::AccountId: Into<polkadot_primitives::AccountId>,
{
	fn on_unbalanceds<B>(
		fees_then_tips: impl Iterator<Item = Credit<R::AccountId, pallet_balances::Pallet<R>>>,
	) {
		for credit in fees_then_tips {
			// Directly send each credit (fees or tips) to the treasury
			ResolveTo::<TreasuryAccountId<R>, pallet_balances::Pallet<R>>::on_unbalanced(credit);
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarks {
	use crate::{xcm_config::CheckAccount, Balances, ExistentialDeposit};
	use frame_support::{
		dispatch::RawOrigin,
		traits::{Currency, EnsureOrigin},
	};

	pub struct InitializeReaperForBenchmarking<A, E>(core::marker::PhantomData<(A, E)>);
	impl<A, O: Into<Result<RawOrigin<A>, O>> + From<RawOrigin<A>>, E: EnsureOrigin<O>>
		EnsureOrigin<O> for InitializeReaperForBenchmarking<A, E>
	{
		type Success = E::Success;

		fn try_origin(o: O) -> Result<E::Success, O> {
			E::try_origin(o)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin() -> Result<O, ()> {
			// initialize the XCM Check Account with the existential deposit
			Balances::make_free_balance_be(&CheckAccount::get(), ExistentialDeposit::get());

			// call the real implementation
			E::try_successful_origin()
		}
	}
}
