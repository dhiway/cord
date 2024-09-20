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

//! Module with configuration which reflects AssetHub Loom runtime setup.

#![cfg_attr(not(feature = "std"), no_std)]

use cord_weave_system_parachains_constants::loom::currency::system_para_deposit;

frame_support::parameter_types! {
	/// Should match the `AssetDeposit` of the `ForeignAssets` pallet on Asset Hub.
	pub const CreateForeignAssetDeposit: u128 = system_para_deposit(1, 190);
}
