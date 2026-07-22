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

//! # Space Management Module Types
//!
//! This module defines types used for managing spaces within the blockchain,
//! including permissions, space details, and space authorizations.

use bitflags::bitflags;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

bitflags! {
	#[derive(Encode, Decode,DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
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

/// Details of an on-chain namespace.
///
/// This structure holds metadata about a namespace, including its identifier,
/// creator, capacity, and current usage. It also tracks the approval and
/// archival status of the namespace.
///
/// ## Fields
///
/// - `code`: The unique code or identifier for the namespace.
/// - `creator`: The account or entity that created the namespace.
/// - `archive`: Indicates whether the namespace is currently archived.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	MaxEncodedLen,
	Debug,
	PartialEq,
	Eq,
	TypeInfo,
)]
pub struct NameSpaceDetails<NameSpaceHashOf, NameSpaceCreatorOf, StatusOf, RegistryIdOf> {
	pub digest: NameSpaceHashOf,
	pub creator: NameSpaceCreatorOf,
	pub archive: StatusOf,
	pub registry_ids: Option<RegistryIdOf>,
}

/// Authorization details for a namespace delegate.
///
/// This structure defines the permissions granted to a delegate within a namespace,
/// as well as the delegator who granted these permissions. It is used to manage
/// and verify the actions that delegates are allowed to perform within a namespace.
///
/// ## Fields
///
/// - `namespace_id`: The identifier of the namespace to which the authorization applies.
/// - `delegate`: The entity that has been granted permissions within the namespace.
/// - `permissions`: The specific permissions granted to the delegate.
/// - `delegator`: The entity that granted the permissions to the delegates
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	MaxEncodedLen,
	Debug,
	PartialEq,
	Eq,
	TypeInfo,
)]
pub struct NameSpaceAuthorization<NameSpaceIdOf, NameSpaceCreatorOf, Permissions> {
	pub namespace_id: NameSpaceIdOf,
	pub delegate: NameSpaceCreatorOf,
	pub permissions: Permissions,
	pub delegator: NameSpaceCreatorOf,
}
