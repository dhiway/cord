// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
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

// Credit Treasury Pallet

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

pub mod weights;

use sp_runtime::traits::{AccountIdConversion, Saturating, StaticLookup};

use frame_support::{
	traits::{Currency, ExistenceRequirement, Get, Imbalance, OnUnbalanced},
	PalletId,
};

pub use pallet::*;
pub use weights::WeightInfo;

pub type BalanceOf<T, I = ()> =
	<<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type NegativeImbalanceOf<T, I = ()> = <<T as Config<I>>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// Origin from which approvals must come.
		type ApproveOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// The pallet id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type PalletId: Get<PalletId>;
		/// The balance.
		type Currency: Currency<Self::AccountId>;
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// The weight for this pallet's extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig;

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self
		}
	}

	#[cfg(feature = "std")]
	impl GenesisConfig {
		pub fn assimilate_storage<T: Config<I>, I: 'static>(
			&self,
			storage: &mut sp_runtime::Storage,
		) -> Result<(), String> {
			<Self as GenesisBuild<T, I>>::assimilate_storage(self, storage)
		}
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig {
		fn build(&self) {
			let account_id = <Pallet<T, I>>::account_id();
			let min = T::Currency::minimum_balance();
			if T::Currency::free_balance(&account_id) < min {
				let _ = T::Currency::make_free_balance_be(&account_id, min);
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// Some credits have been transfered  \[recipient, credits\].
		Transfer { beneficiary: T::AccountId, credits: BalanceOf<T, I> },
		/// Some infrastructure utilization credits, have been paid by the
		/// author.
		UtilityCreditPaid { credits: BalanceOf<T, I> },
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// treasury balance is too low.
		InsufficientBalance,
	}

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			beneficiary: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: BalanceOf<T, I>,
		) -> DispatchResult {
			T::ApproveOrigin::ensure_origin(origin)?;
			let beneficiary = T::Lookup::lookup(beneficiary)?;
			let balance = T::Currency::free_balance(&Self::account_id());
			ensure!(balance >= amount, Error::<T, I>::InsufficientBalance);

			let _ = T::Currency::transfer(
				&Self::account_id(),
				&beneficiary,
				amount,
				ExistenceRequirement::KeepAlive,
			);

			Self::deposit_event(Event::Transfer { beneficiary, credits: amount });

			Ok(())
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}
		/// Return the amount of money in the treasury.
		pub fn balance() -> BalanceOf<T, I> {
			T::Currency::free_balance(&Self::account_id())
				// Must never be less than 0 but better be safe.
				.saturating_sub(T::Currency::minimum_balance())
		}
	}

	impl<T: Config<I>, I: 'static> OnUnbalanced<NegativeImbalanceOf<T, I>> for Pallet<T, I> {
		fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<T, I>) {
			let numeric_amount = amount.peek();

			// Must resolve into existing but better to be safe.
			let _ = T::Currency::resolve_creating(&Self::account_id(), amount);

			Self::deposit_event(Event::UtilityCreditPaid { credits: numeric_amount });
		}
	}
}
