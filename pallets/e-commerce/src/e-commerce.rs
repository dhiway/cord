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

use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain account details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct Account<T: Config> {
	pub account_id: String;
	pub name: String;
	pub trade_no: String;
}

/// An on-chain manufacturer mapper to an Account and product details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct Manufacturer<T: Config> {
	pub account: Account;
	pub product: ProductDetails;
	pub quantity: i64;
}

/// An on-chain seller mapper to an Account and product details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct Seller<T: Config> {
	pub account: Account;
	pub product: ProductDetails;
	pub linked_to: Account;
	pub quantity: i64;
}

/// An on-chain buyer mapper to an Account and product details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct Buyer<T: Config> {
	pub account: Account;
	pub product: ProductDetails;
	pub seller: Seller;
	pub quantity: i64;
}
/// An on-chain transaction mapper linked to buyer and seller Account and product details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct Transaction<T: Config> {
	pub seller: Account;
	pub buyer: Account;
	pub product: ProductDetails;
}
