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

use codec::{Decode, DecodeWithMemTracking, Encode};
use core::marker::PhantomData;
use frame_support::{
	dispatch::{CheckIfFeeless, DispatchResult},
	ensure,
	pallet_prelude::TransactionSource,
	traits::{CallerTrait, Get, OriginTrait, StorageVersion},
	weights::Weight,
};
use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
use scale_info::{StaticTypeInfo, TypeInfo};
use sp_runtime::{
	traits::{
		DispatchInfoOf, DispatchOriginOf, Implication, PostDispatchInfoOf, TransactionExtension,
		ValidateResult,
	},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
};
use sp_std::vec::Vec;

pub use weights::WeightInfo;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

/// Trait that allows other pallets to query whether an account is whitelisted for feeless
/// transactions.
pub trait FeelessAccounts<AccountId> {
	/// Returns `true` if the provided account should be treated as feeless.
	fn is_feeless(account: &AccountId) -> bool;
}

/// Whether the wrapped payment extension is applied or one fee-free quota unit is consumed.
pub enum PaymentIntermediate<Applied, AccountId> {
	Apply(Applied),
	Skip(AccountId),
}

/// Payment extension that only skips fees after atomically consuming an account quota unit.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq)]
pub struct ChargeOrSkipFeeless<T, S>(pub S, PhantomData<T>);

impl<T, S: StaticTypeInfo> TypeInfo for ChargeOrSkipFeeless<T, S> {
	type Identity = S;
	fn type_info() -> scale_info::Type {
		S::type_info()
	}
}

impl<T, S: Encode> core::fmt::Debug for ChargeOrSkipFeeless<T, S> {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		write!(f, "ChargeOrSkipFeeless<{:?}>", self.0.encode())
	}
}

impl<T, S> From<S> for ChargeOrSkipFeeless<T, S> {
	fn from(extension: S) -> Self {
		Self(extension, PhantomData)
	}
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

		/// Maximum number of allowlisted fee-free transactions an account may consume in one block.
		#[pallet::constant]
		type MaxFeelessTransactionsPerBlock: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::storage]
	#[pallet::getter(fn accounts)]
	pub type FeelessAccountStore<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

	/// Per-account fee-free usage in the current block.
	#[pallet::storage]
	pub type FeelessUsage<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, (BlockNumberFor<T>, u32), OptionQuery>;

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
		/// A fee was skipped after consuming one quota unit.
		FeeSkipped { account: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Account is already marked as feeless.
		AccountAlreadyFeeless,
		/// Account is not marked as feeless.
		AccountNotFeeless,
		/// Account exhausted its fee-free allowance for the current block.
		QuotaExhausted,
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
			FeelessUsage::<T>::remove(&account);

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
		match origin.caller().as_system_ref() {
			Some(frame_system::RawOrigin::Signed(ref who)) => Self::is_feeless_account(who),
			_ => false,
		}
	}

	/// Returns true when an allowlisted account still has fee-free capacity this block.
	pub fn has_feeless_quota(account: &T::AccountId) -> bool {
		if !Self::is_feeless_account(account) {
			return false;
		}
		let now = frame_system::Pallet::<T>::block_number();
		let used = FeelessUsage::<T>::get(account)
			.filter(|(block, _)| *block == now)
			.map(|(_, used)| used)
			.unwrap_or_default();
		used < T::MaxFeelessTransactionsPerBlock::get()
	}

	/// Atomically consumes one fee-free unit for an allowlisted account.
	pub fn consume_feeless_quota(account: &T::AccountId) -> DispatchResult {
		ensure!(Self::is_feeless_account(account), Error::<T>::AccountNotFeeless);
		let now = frame_system::Pallet::<T>::block_number();
		FeelessUsage::<T>::try_mutate(account, |usage| {
			let used = usage
				.as_ref()
				.filter(|(block, _)| *block == now)
				.map(|(_, used)| *used)
				.unwrap_or_default();
			ensure!(used < T::MaxFeelessTransactionsPerBlock::get(), Error::<T>::QuotaExhausted);
			*usage = Some((now, used.saturating_add(1)));
			Ok(())
		})
	}
}

impl<T: Config> FeelessAccounts<T::AccountId> for Pallet<T> {
	fn is_feeless(account: &T::AccountId) -> bool {
		Self::has_feeless_quota(account)
	}
}

use PaymentIntermediate::{Apply, Skip};

impl<T: Config + Send + Sync, S: TransactionExtension<T::RuntimeCall>>
	TransactionExtension<T::RuntimeCall> for ChargeOrSkipFeeless<T, S>
where
	T::RuntimeCall: CheckIfFeeless<Origin = OriginFor<T>>,
{
	// Preserve the wrapped payment extension's public metadata contract.
	const IDENTIFIER: &'static str = S::IDENTIFIER;
	type Implicit = S::Implicit;
	type Val = PaymentIntermediate<S::Val, T::AccountId>;
	type Pre = PaymentIntermediate<S::Pre, T::AccountId>;

	fn metadata() -> Vec<sp_runtime::traits::TransactionExtensionMetadata> {
		S::metadata()
	}

	fn implicit(&self) -> Result<Self::Implicit, TransactionValidityError> {
		self.0.implicit()
	}

	fn weight(&self, call: &T::RuntimeCall) -> Weight {
		self.0.weight(call)
	}

	fn validate(
		&self,
		origin: DispatchOriginOf<T::RuntimeCall>,
		call: &T::RuntimeCall,
		info: &DispatchInfoOf<T::RuntimeCall>,
		len: usize,
		self_implicit: S::Implicit,
		inherited_implication: &impl Implication,
		source: TransactionSource,
	) -> ValidateResult<Self::Val, T::RuntimeCall> {
		if call.is_feeless(&origin) {
			if let Some(frame_system::RawOrigin::Signed(who)) = origin.caller().as_system_ref() {
				return Ok((Default::default(), Skip(who.clone()), origin));
			}
		}

		let (validity, val, origin) = self.0.validate(
			origin,
			call,
			info,
			len,
			self_implicit,
			inherited_implication,
			source,
		)?;
		Ok((validity, Apply(val), origin))
	}

	fn prepare(
		self,
		val: Self::Val,
		origin: &DispatchOriginOf<T::RuntimeCall>,
		call: &T::RuntimeCall,
		info: &DispatchInfoOf<T::RuntimeCall>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		match val {
			Apply(val) => self.0.prepare(val, origin, call, info, len).map(Apply),
			Skip(who) => {
				Pallet::<T>::consume_feeless_quota(&who)
					.map_err(|_| InvalidTransaction::ExhaustsResources)?;
				Ok(Skip(who))
			},
		}
	}

	fn post_dispatch_details(
		pre: Self::Pre,
		info: &DispatchInfoOf<T::RuntimeCall>,
		post_info: &PostDispatchInfoOf<T::RuntimeCall>,
		len: usize,
		result: &DispatchResult,
	) -> Result<Weight, TransactionValidityError> {
		match pre {
			Apply(pre) => S::post_dispatch_details(pre, info, post_info, len, result),
			Skip(account) => {
				Pallet::<T>::deposit_event(Event::<T>::FeeSkipped { account });
				Ok(Weight::zero())
			},
		}
	}
}
