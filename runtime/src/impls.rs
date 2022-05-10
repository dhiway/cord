// This file is part of Cord – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// Cord is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cord is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cord. If not, see <https://www.gnu.org/licenses/>.

//! Some configurable implementations as associated type for the CORD runtime.

pub use cord_primitives::{AccountId, Balance};
use frame_support::traits::{Currency, Imbalance, OnUnbalanced};

pub type NegativeImbalance<T> = <pallet_balances::Pallet<T> as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

/// Logic for the block author to get a portion of fees.
pub struct ToAuthor<R>(sp_std::marker::PhantomData<R>);

impl<R> OnUnbalanced<NegativeImbalance<R>> for ToAuthor<R>
where
	R: pallet_balances::Config + pallet_authorship::Config,
	<R as frame_system::Config>::AccountId: From<cord_primitives::AccountId>,
	<R as frame_system::Config>::AccountId: Into<cord_primitives::AccountId>,
	<R as frame_system::Config>::Event: From<pallet_balances::Event<R>>,
{
	fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
		if let Some(author) = <pallet_authorship::Pallet<R>>::author() {
			<pallet_balances::Pallet<R>>::resolve_creating(&author, amount);
		}
	}
}

pub struct DealWithCredits<R>(sp_std::marker::PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for DealWithCredits<R>
where
	R: pallet_balances::Config
		+ pallet_builder::Config
		+ pallet_foundation::Config
		+ pallet_authorship::Config,
	pallet_builder::Pallet<R>: OnUnbalanced<NegativeImbalance<R>>,
	pallet_foundation::Pallet<R>: OnUnbalanced<NegativeImbalance<R>>,
	<R as frame_system::Config>::AccountId: From<cord_primitives::AccountId>,
	<R as frame_system::Config>::AccountId: Into<cord_primitives::AccountId>,
	<R as frame_system::Config>::Event: From<pallet_balances::Event<R>>,
{
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<R>>) {
		if let Some(fees) = fees_then_tips.next() {
			// for fees, 30% to builder, 70% to foundation
			let mut split = fees.ration(30, 70);
			if let Some(tips) = fees_then_tips.next() {
				// for tips, if any, 100% to foundation
				tips.merge_into(&mut split.1);
			}

			use pallet_builder::Pallet as Builder;
			use pallet_foundation::Pallet as Foundation;
			<Builder<R> as OnUnbalanced<_>>::on_unbalanced(split.0);
			<Foundation<R> as OnUnbalanced<_>>::on_unbalanced(split.1);
		}
	}
}
