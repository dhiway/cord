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

use crate::*;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchClass;
use scale_info::TypeInfo;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;

/// Information related to a dispatchable's class and weight that can be
/// queried from the runtime.
#[derive(Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RuntimeDispatchWeightInfo<Weight = frame_support::weights::Weight> {
	/// Weight of this dispatch.
	pub weight: Weight,
	/// Class of this dispatch.
	pub class: DispatchClass,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct MemberData<BlockNumber: Decode + Encode + TypeInfo> {
	pub expire_on: BlockNumber,
}
