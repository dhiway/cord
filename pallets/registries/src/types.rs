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
//
//! # Registry Management Module Types
//!
//! This module defines types used for managing registries within the blockchain,
//! including permissions, registry details, and registry authorizations.

use bitflags::bitflags;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

bitflags! {
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct Permissions: u32 {
		const ASSERT = 0b0000_0001;
		const DELEGATE = 0b0000_0010;
		const ADMIN = 0b0000_0100;
	}
}

impl Permissions {
	// Encodes the permission bitflags into a 4-byte array.
	///
	/// This method is useful for serialization and storage purposes, as it
	/// converts the internal representation of the permissions into a format
	/// that can be easily stored and transmitted.
	///
	/// Returns a `[u8; 4]` representing the encoded permissions.
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
	/// Provides a default for the Permissions struct.
	///
	/// By default, the `ASSERT` permission is granted
	fn default() -> Self {
		Permissions::ASSERT
	}
}

/// Details of an on-chain registry.
///
/// This structure holds metadata about a registry, including its identifier,
/// creator, capacity, and current usage. It also tracks the revoke and
/// archival status of the registry.
///
/// ## Fields
///
/// - `code`: The unique code or identifier for the registry.
/// - `creator`: The account or entity that created the registry.
/// - `txn_capacity`: The maximum allowed transactions within the registry. A value of zero denotes
///   unlimited capacity.
/// - `txn_count`: The current usage of the registry's capacity.
/// - `approved`: Indicates whether the registry has been approved by the appropriate governance
///   body.
/// - `archive`: Indicates whether the registry is currently archived.
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub struct RegistryDetails<RegistryCreatorOf, StatusOf, RegistryHashOf> {
	pub creator: RegistryCreatorOf,
	pub revoked: StatusOf,
	pub archived: StatusOf, /* Only at the registry level. (If it is archived, revoked dont
	                         * allow entry creation) */
	pub digest: RegistryHashOf,
}

/// Authorization details for a registry delegate.
///
/// This structure defines the permissions granted to a delegate within a registry,
/// as well as the delegator who granted these permissions. It is used to manage
/// and verify the actions that delegates are allowed to perform within a registry.
///
/// ## Fields
///
/// - `registry_id`: The identifier of the registry to which the authorization applies.
/// - `delegate`: The entity that has been granted permissions within the registry.
/// - `permissions`: The specific permissions granted to the delegate.
/// - `delegator`: The entity that granted the permissions to the delegates
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub struct RegistryAuthorization<RegistryIdOf, RegistryCreatorOf, Permissions> {
	pub registry_id: RegistryIdOf,
	pub delegate: RegistryCreatorOf,
	pub permissions: Permissions,
	pub delegator: RegistryCreatorOf,
}
