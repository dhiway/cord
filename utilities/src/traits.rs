// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

use frame_support::traits::{Currency, ReservableCurrency};
use sp_runtime::DispatchError;

use crate::{deposit::Deposit, free_deposit};

/// The sources of a call struct.
///
/// This trait allows to differentiate between the sender of a call and the
/// subject of the call. The sender account submitted the call to the chain and
/// might pay all fees and deposits that are required by the call.
pub trait CallSources<S, P> {
	/// The sender of the call who will pay for all deposits and fees.
	fn sender(&self) -> S;

	/// The subject of the call.
	fn subject(&self) -> P;
}

impl<S: Clone> CallSources<S, S> for S {
	fn sender(&self) -> S {
		self.clone()
	}

	fn subject(&self) -> S {
		self.clone()
	}
}

impl<S: Clone, P: Clone> CallSources<S, P> for (S, P) {
	fn sender(&self) -> S {
		self.0.clone()
	}

	fn subject(&self) -> P {
		self.1.clone()
	}
}

/// A trait that allows version migrators to access the underlying pallet's
/// context, e.g., its Config trait.
///
/// In this way, the migrator can access the pallet's storage and the pallet's
/// types directly.
pub trait VersionMigratorTrait<T>: Sized {
	#[cfg(feature = "try-runtime")]
	fn pre_migrate(&self) -> Result<(), &'static str>;
	fn migrate(&self) -> frame_support::weights::Weight;
	#[cfg(feature = "try-runtime")]
	fn post_migrate(&self) -> Result<(), &'static str>;
}

/// Trait to simulate an origin with different sender and subject.
/// This origin is only used on benchmarks and testing.
#[cfg(feature = "runtime-benchmarks")]
pub trait GenerateBenchmarkOrigin<OuterOrigin, AccountId, SubjectId> {
	fn generate_origin(sender: AccountId, subject: SubjectId) -> OuterOrigin;
}

/// Trait that allows types to implement a worst case value for a type,
/// only when running benchmarks.
#[cfg(feature = "runtime-benchmarks")]
pub trait GetWorstCase {
	fn worst_case() -> Self;
}

/// Generic filter.
pub trait ItemFilter<Item> {
	fn should_include(&self, credential: &Item) -> bool;
}

pub trait StorageDepositCollector<AccountId, Key> {
	type Currency: ReservableCurrency<AccountId>;

	/// Returns the deposit of the storage entry that is stored behind the key.
	fn deposit(
		key: &Key,
	) -> Result<Deposit<AccountId, <Self::Currency as Currency<AccountId>>::Balance>, DispatchError>;

	/// Returns the deposit amount that should be reserved for the storage entry
	/// behind the key.
	///
	/// This value can differ from the actual deposit that is reserved at the
	/// time, since the deposit can be changed.
	fn deposit_amount(key: &Key) -> <Self::Currency as Currency<AccountId>>::Balance;

	/// Store the new deposit information in the storage entry behind the key.
	fn store_deposit(
		key: &Key,
		deposit: Deposit<AccountId, <Self::Currency as Currency<AccountId>>::Balance>,
	) -> Result<(), DispatchError>;

	/// Change the deposit owner.
	///
	/// The deposit balance of the current owner will be freed, while the
	/// deposit balance of the new owner will get reserved. The deposit amount
	/// will not change even if the required byte and item fees were updated.
	fn change_deposit_owner(key: &Key, new_owner: AccountId) -> Result<(), DispatchError> {
		let deposit = Self::deposit(key)?;

		free_deposit::<AccountId, Self::Currency>(&deposit);

		let deposit = Deposit { owner: new_owner, ..deposit };
		Self::Currency::reserve(&deposit.owner, deposit.amount)?;

		Self::store_deposit(key, deposit)?;

		Ok(())
	}

	/// Update the deposit amount.
	///
	/// In case the required deposit per item and byte changed, this function
	/// updates the deposit amount. It either frees parts of the reserved
	/// balance in case the deposit was lowered or reserves more balance when
	/// the deposit was raised.
	fn update_deposit(key: &Key) -> Result<(), DispatchError> {
		let deposit = Self::deposit(key)?;

		free_deposit::<AccountId, Self::Currency>(&deposit);

		let deposit = Deposit { amount: Self::deposit_amount(key), ..deposit };
		Self::Currency::reserve(&deposit.owner, deposit.amount)?;

		Self::store_deposit(key, deposit)?;

		Ok(())
	}
}
