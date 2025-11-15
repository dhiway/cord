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
//! Runtime API definition for assets.

use alloc::vec::Vec;
use codec::Codec;

sp_api::decl_runtime_apis! {
	pub trait AssetsApi<AccountId, AssetBalance, AssetId>
	where
		AccountId: Codec,
		AssetBalance: Codec,
		AssetId: Codec,
	{
		/// Returns the list of `AssetId`s and corresponding balance that an `AccountId` has.
		fn account_balances(account: AccountId) -> Vec<(AssetId, AssetBalance)>;
	}
}
