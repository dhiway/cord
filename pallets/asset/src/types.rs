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

use codec::{Decode, Encode, MaxEncodedLen};
use cord_primitives::BlockNumber;
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct AssetInputEntry<AssetDescription, AssetTypeOf, AccountId, AssetTag, AssetMeta> {
	/// type of the asset
	pub asset_type: AssetTypeOf,
	/// asset description
	pub asset_desc: AssetDescription,
	/// asset quantity
	pub asset_qty: u32,
	/// asset value
	pub asset_value: u32,
	/// asset owner
	pub asset_owner: AccountId,
	/// open structure - 1024 bytes max
	pub asset_tag: AssetTag,
	/// open structure - 1024 bytes max
	pub asset_meta: AssetMeta,
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum AssetTypeOf {
	ART,
	BOND,
	MF,
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum AssetStatusOf {
	ACTIVE,
	INACTIVE,
	EXPIRED,
}

impl AssetTypeOf {
	pub fn is_valid_asset_type(&self) -> bool {
		matches!(self, Self::ART | Self::BOND | Self::MF)
	}
}

impl AssetStatusOf {
	pub fn is_valid_status_type(&self) -> bool {
		matches!(self, Self::ACTIVE | Self::INACTIVE | Self::EXPIRED)
	}
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct AssetEntry<
	AssetDescription,
	AssetTypeOf,
	AssetStatusOf,
	AccountId,
	AssetTag,
	AssetMeta,
	BlockNumber,
> {
	pub asset_entry: AssetInputEntry<AssetDescription, AssetTypeOf, AccountId, AssetTag, AssetMeta>,
	/// status of the asset
	pub asset_status: AssetStatusOf,
	/// asset issuer
	pub asset_issuer: AccountId,
	/// asset inlclusion block
	pub created_at: BlockNumber,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct AssetTransferEntry<AssetIdOf, AccountId> {
	/// type of the asset
	pub asset_id: AssetIdOf,
	/// asset quantity
	pub transfer_qty: u32,
	/// asset owner
	pub new_asset_owner: AccountId,
}
