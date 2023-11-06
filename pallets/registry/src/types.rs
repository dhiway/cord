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
// use crate::*;
use bitflags::bitflags;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

bitflags! {
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct Permissions: u32 {
		const ASSERT = 0b0000_0001;
		const ADMIN = 0b0000_0010;
	}
}

impl Permissions {
	/// Encode permission bitflags into u8 array.
	pub fn as_u8(self) -> [u8; 4] {
		let x: u32 = self.bits;
		let b1: u8 = ((x >> 24) & 0xff) as u8;
		let b2: u8 = ((x >> 16) & 0xff) as u8;
		let b3: u8 = ((x >> 8) & 0xff) as u8;
		let b4: u8 = (x & 0xff) as u8;
		[b4, b3, b2, b1]
	}
}

impl Default for Permissions {
	fn default() -> Self {
		Permissions::ASSERT
	}
}

/// An on-chain space details mapped to an identifier.
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]

pub struct SpaceDetails<SpaceCodeOf, SpaceCreatorOf, StatusOf> {
	// The Space Code
	pub code: SpaceCodeOf,
	/// Space creator.
	pub creator: SpaceCreatorOf,
	/// The maximum capacity that the space is approved for.
	/// Capcity value set to zero allows unlimited transactions
	pub capacity: u64,
	/// The amount of the capacity that is currently being used.
	pub usage: u64,
	/// Approved by governance or root.
	pub approved: StatusOf,
	/// The flag indicating the status of the space.
	pub archive: StatusOf,
}

/// An on-chain registry delegate details mapped to an authoization identifier.
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]

pub struct SpaceAuthorization<SpaceIdOf, SpaceCreatorOf, Permissions> {
	// The Space
	pub space_id: SpaceIdOf,
	/// Space delegate.
	pub delegate: SpaceCreatorOf,
	/// Registry creator.
	pub permissions: Permissions,
	/// Space delegator.
	pub delegator: SpaceCreatorOf,
}
