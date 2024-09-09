// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
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

//! Registries pallet types.

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

use bitflags::bitflags;

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum RegistrySupportedStateOf {
	ACTIVE,
	REVOKED,
}

impl RegistrySupportedStateOf {
	pub fn is_valid_state(&self) -> bool {
		matches!(self, Self::ACTIVE | Self::REVOKED)
	}
}

/* ROLES:
 * - DELEGATE - Can add a new Entry, update and change state of the Entry which the DELEGATE
 *   created.
 * - ADMIN - Can manage addition and removal of Delegate. Can update the state of all Entries
 *   within a Registry. Can update the state of Registries.
 * - OWNER - Can manage addition and removal of ADMINS, Addition of new OWNERS & everything
 *   above.
 */
bitflags! {
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct Permissions: u32 {
		const DELEGATE = 0b0000_0001;
		const ADMIN = 0b0000_0010;
		const OWNER = 0b0000_0100;
	}
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct DelegateInfo<DelegateOf, Permissions> {
	/// Type of Delegate.
	pub delegate: DelegateOf,
	/// Bitflag to hold composite permissions of the delegate.
	pub permissions: Permissions,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct RegistryDetails<RegistryHashOf, RegistrySupportedStateOf, DelegateEntryOf> {
	/// Type of the digest of Registry.
	pub digest: RegistryHashOf,
	/// Type of the Registry State.
	pub state: RegistrySupportedStateOf,
	/// Type to store list of Delegates.
	pub delegates: DelegateEntryOf,
}
