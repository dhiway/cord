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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::ReservableCurrency;
use scale_info::TypeInfo;
use sp_runtime::{traits::Zero, DispatchError};

/// An amount of balance reserved by the specified address.
#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, Ord, PartialOrd, TypeInfo, MaxEncodedLen)]
pub struct Deposit<Account, Balance> {
	pub owner: Account,
	pub amount: Balance,
}

pub fn reserve_deposit<Account, Currency: ReservableCurrency<Account>>(
	account: Account,
	deposit_amount: Currency::Balance,
) -> Result<Deposit<Account, Currency::Balance>, DispatchError> {
	Currency::reserve(&account, deposit_amount)?;
	Ok(Deposit { owner: account, amount: deposit_amount })
}

pub fn free_deposit<Account, Currency: ReservableCurrency<Account>>(
	deposit: &Deposit<Account, Currency::Balance>,
) {
	let err_amount = Currency::unreserve(&deposit.owner, deposit.amount);
	debug_assert!(err_amount.is_zero());
}
