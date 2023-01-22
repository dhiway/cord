// This file is part of CORD – https://cord.network
// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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

///! This module contains utilities for testing.
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::sr25519;
use sp_runtime::AccountId32;

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct CreatorId(pub AccountId32);

impl From<AccountId32> for CreatorId {
	fn from(acc: AccountId32) -> Self {
		CreatorId(acc)
	}
}

impl From<sr25519::Public> for CreatorId {
	fn from(acc: sr25519::Public) -> Self {
		CreatorId(acc.into())
	}
}
