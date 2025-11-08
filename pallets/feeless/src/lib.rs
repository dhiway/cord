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

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

use core::marker::PhantomData;
use frame_support::traits::{CallerTrait, OriginTrait, StorageVersion};
use frame_system::pallet_prelude::OriginFor;
use sp_std::vec::Vec;

pub use weights::WeightInfo;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

/// Trait that allows other pallets to query whether an account is whitelisted for feeless
/// transactions.
pub trait FeelessAccounts<AccountId> {
	/// Returns `true` if the provided account should be treated as feeless.
	fn is_feeless(account: &AccountId) -> bool;
}

impl<AccountId> FeelessAccounts<AccountId> for () {
	fn is_feeless(_: &AccountId) -> bool {
		false
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::ensure_root;
	use sp_std::collections::btree_set::BTreeSet;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::storage]
	#[pallet::getter(fn accounts)]
	pub type FeelessAccountStore<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub feeless_accounts: Vec<T::AccountId>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { feeless_accounts: Vec::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			let mut unique = BTreeSet::new();
			for account in &self.feeless_accounts {
				if unique.insert(account.clone()) {
					FeelessAccountStore::<T>::insert(account, ());
				}
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Account was added to the feeless allow list.
		FeelessAccountAdded { account: T::AccountId },
		/// Account was removed from the feeless allow list.
		FeelessAccountRemoved { account: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Account is already marked as feeless.
		AccountAlreadyFeeless,
		/// Account is not marked as feeless.
		AccountNotFeeless,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds an account to the feeless allow list. Root only.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::add_feeless_account())]
		pub fn add_feeless_account(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(
				!FeelessAccountStore::<T>::contains_key(&account),
				Error::<T>::AccountAlreadyFeeless
			);

			FeelessAccountStore::<T>::insert(&account, ());
			Self::deposit_event(Event::FeelessAccountAdded { account });
			Ok(())
		}

		/// Removes an account from the feeless allow list. Root only.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::remove_feeless_account())]
		pub fn remove_feeless_account(
			origin: OriginFor<T>,
			account: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(
				FeelessAccountStore::<T>::take(&account).is_some(),
				Error::<T>::AccountNotFeeless
			);

			Self::deposit_event(Event::FeelessAccountRemoved { account });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Returns true if `account` exists in the feeless store.
	pub fn is_feeless_account(account: &T::AccountId) -> bool {
		FeelessAccountStore::<T>::contains_key(account)
	}

	/// Convenience helper to check the origin of a call.
	pub fn is_feeless_origin(origin: &OriginFor<T>) -> bool {
		origin.caller().as_signed().map(Self::is_feeless_account).unwrap_or(false)
	}
}

impl<T: Config> FeelessAccounts<T::AccountId> for Pallet<T> {
	fn is_feeless(account: &T::AccountId) -> bool {
		Self::is_feeless_account(account)
	}
}
