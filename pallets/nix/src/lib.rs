// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # Pallet Nix Accounts Filter
//!
//! The Nix Pallet provides functionality to restrict a set of accounts from
//! extrinsic submission. The filtering of accounts is done during the
//! transaction queue validation.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::weights::DispatchInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, Dispatchable, SignedExtension},
	transaction_validity::{
		InvalidTransaction, TransactionLongevity, TransactionPriority, TransactionValidity,
		TransactionValidityError, ValidTransaction,
	},
};
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The origin which may add or remove accounts. Root can always do
		/// this.
		type AccountOrigin: EnsureOrigin<Self::Origin>;
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// The pallet's runtime storage items.
	#[pallet::storage]
	#[pallet::getter(fn nixaccounts)]
	pub type NixAccounts<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ()>;

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// When a new account is added to the allow-list.
		TxAdd(T::AccountId),
		// When an account is removed from the allow-list.
		TxRemove(T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no registrar with the given ID.
		AccountNotFound,
		/// The registrar already exists.
		AccountAlreadyExists,
		/// The registrar has already been revoked.
		AccountAlreadyRemoved,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub nix_accounts: Vec<(T::AccountId, ())>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { nix_accounts: Vec::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let nix_accounts = &self.nix_accounts;
			if !nix_accounts.is_empty() {
				for (account, extrinsics) in nix_accounts.iter() {
					<NixAccounts<T>>::insert(account, extrinsics);
				}
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a new account to the nix-list.
		#[pallet::weight(0)]
		pub fn add(origin: OriginFor<T>, add_account: T::AccountId) -> DispatchResult {
			T::AccountOrigin::ensure_origin(origin)?;
			ensure!(
				!<NixAccounts<T>>::contains_key(&add_account),
				Error::<T>::AccountAlreadyExists
			);

			<NixAccounts<T>>::insert(&add_account, ());

			Self::deposit_event(Event::TxAdd(add_account));

			Ok(())
		}

		/// Remove an account from the nix-list.
		#[pallet::weight(0)]
		pub fn remove(origin: OriginFor<T>, account_to_remove: T::AccountId) -> DispatchResult {
			T::AccountOrigin::ensure_origin(origin)?;
			ensure!(
				<NixAccounts<T>>::contains_key(&account_to_remove),
				Error::<T>::AccountNotFound
			);
			<NixAccounts<T>>::remove(&account_to_remove);

			Self::deposit_event(Event::TxRemove(account_to_remove));

			Ok(())
		}
	}
}

/// The following section implements the `SignedExtension` trait
/// for the `NixAccount` type.
/// `SignedExtension` is being used here to filter out the denied accounts
/// when they try to send extrinsics to the runtime.
/// Inside the `validate` function of the `SignedExtension` trait,
/// we check if the sender (origin) of the extrinsic is part of the
/// nix-list or not.
/// The extrinsic will be rejected as invalid if the origin is part
/// of the nix-list.
/// The validation happens at the transaction queue level,
///  and the extrinsics are filtered out before they hit the pallet logic.

/// The `NixAccount` struct.
#[derive(Encode, Decode, Clone, Eq, PartialEq, Default)]
pub struct NixAccount<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> NixAccount<T> {
	/// utility constructor. Used only in client/factory code.
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

/// Debug impl for the `NixAccount` struct.
impl<T: Config + Send + Sync> Debug for NixAccount<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "NixAccount")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

/// Implementation of the `SignedExtension` trait for the `NixAccount` struct.
impl<T: Config + Send + Sync> SignedExtension for NixAccount<T>
where
	T::Call: Dispatchable<Info = DispatchInfo>,
{
	type AccountId = T::AccountId;
	type Call = T::Call;
	type AdditionalSigned = ();
	type Pre = ();
	const IDENTIFIER: &'static str = "NixAccount";

	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	// Filter out the nix keys.
	// If the key is in the nix-list, return an invalid transaction,
	// else return a valid transaction.
	fn validate(
		&self,
		who: &Self::AccountId,
		_call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		if !<NixAccounts<T>>::contains_key(who) {
			Ok(ValidTransaction {
				priority: info.weight as TransactionPriority,
				longevity: TransactionLongevity::max_value(),
				propagate: true,
				..Default::default()
			})
		} else {
			Err(InvalidTransaction::Call.into())
		}
	}
}
