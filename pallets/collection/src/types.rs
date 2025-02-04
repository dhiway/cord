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

use bitflags::bitflags;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

bitflags! {
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct Permissions: u32 {
		/// Permission to add registries.
		const REGISTRY_MANAGEMENT = 0b0000_0001;
		/// Permission to add delegates.
		const DELEGATE_MANAGEMENT = 0b0000_0010;
		/// Admin has all rights.
		const ADMIN = 0b0000_0100;
	}
}

impl Permissions {
	/// Encodes the permission bitflags into a 4-byte array.
	pub fn as_u8(self) -> [u8; 4] {
		let x: u32 = self.bits;
		[
			(x & 0xff) as u8,
			((x >> 8) & 0xff) as u8,
			((x >> 16) & 0xff) as u8,
			((x >> 24) & 0xff) as u8,
		]
	}
}

impl Default for Permissions {
	fn default() -> Self {
		Permissions::REGISTRY_MANAGEMENT
	}
}

/// A simple status enum.
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum Status {
	Active,
	Archived,
}

/// Details for a catalog.
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub struct CollectionDetails<Account, Status> {
	pub creator: Account,
	pub status: Status,
}
