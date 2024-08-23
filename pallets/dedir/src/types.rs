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

//! DeDir pallet types.

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum RegistrySupportedStateOf {
	DRAFT,
	ACTIVE,
	REVOKED,
}

impl RegistrySupportedStateOf {
	pub fn is_valid_state(&self) -> bool {
		matches!(self, Self::DRAFT | Self::ACTIVE | Self::REVOKED)
	}
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Entry<RegistryKeyIdOf, RegistrySupportedTypeOf> {
	/// Type of Registry Key
	pub registry_key: RegistryKeyIdOf,
	/// Type of Registry Key Type
	pub registry_key_type: RegistrySupportedTypeOf,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Registry<RegistryBlobOf, RegistryHashOf, OwnerOf> {
	pub blob: RegistryBlobOf,
	pub owner: OwnerOf,
	pub digest: RegistryHashOf,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct RegistryEntry<
	RegistryBlobOf,
	RegistryEntryHashOf,
	RegistryIdOf,
	RegistrySupportedStateOf,
> {
	/// Type of Entries
	pub blob: RegistryBlobOf,
	/// Type of Digest
	pub digest: RegistryEntryHashOf,
	/// Type of Registry Identifier
	pub registry_id: RegistryIdOf,
	/// Type of Current State of Registry Entry
	pub current_state: RegistrySupportedStateOf,
}

/// The `Permissions` enum defines the levels of access control available for an account within a
/// registry.
///
/// - `DELEGATE`: Grants permission to manage registry entries.
/// - `ADMIN`: Extends `DELEGATE` permissions, allowing the management of delegates in addition to
///   managing registry entries.
/// - `OWNER`: The creator or owner of the registry. This permission level encompasses the full
///   range of management capabilities, including the permissions of both `DELEGATE` and `ADMIN`.
#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum Permissions {
	OWNER,
	ADMIN,
	DELEGATE,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct DelegateInfo<DelegateOf, Permissions> {
	pub delegate: DelegateOf,
	pub permission: Permissions,
	pub delegator: Option<DelegateOf>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Delegates<DelegateEntryOf> {
	pub entries: DelegateEntryOf,
}
