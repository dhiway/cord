// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.
//
// Registry primitives

use crate::{
	element::{ElementType, ElementView},
	identifier::Ss58Identifier,
};
use alloc::{string::String, vec::Vec};
use bitflags::bitflags;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

bitflags! {
	#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
	pub struct RegistryPermissions: u16 {
		const VIEW     = 1 << 0;
		const ENTRY    = 1 << 1;
		const DELEGATE = 1 << 2;
		const ADMIN    = 1 << 3;
	}
}

impl Default for RegistryPermissions {
	fn default() -> Self {
		RegistryPermissions::ENTRY | RegistryPermissions::VIEW
	}
}

impl RegistryPermissions {
	pub fn from_list(list: &[RegistryPermissions]) -> Self {
		let mut mask = list.iter().copied().fold(Self::empty(), |acc, p| acc | p);
		if mask.intersects(
			RegistryPermissions::ADMIN | RegistryPermissions::ENTRY | RegistryPermissions::DELEGATE,
		) {
			mask |= RegistryPermissions::VIEW;
		}
		mask
	}

	pub fn has_entry(self) -> bool {
		self.contains(RegistryPermissions::ADMIN) || self.contains(RegistryPermissions::ENTRY)
	}

	pub fn has_delegate(self) -> bool {
		self.contains(RegistryPermissions::ADMIN) || self.contains(RegistryPermissions::DELEGATE)
	}

	pub fn has_admin(self) -> bool {
		self.contains(RegistryPermissions::ADMIN)
	}

	pub fn has_view(self) -> bool {
		self.contains(RegistryPermissions::VIEW)
			|| self.contains(RegistryPermissions::ENTRY)
			|| self.contains(RegistryPermissions::ADMIN)
	}
}

/// High-level kind of registry (what its primary value encodes).
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
	Default,
)]
pub enum RegistryKind {
	#[default]
	Raw,
	Token,
	Hash,
}

/// Lifecycle state of a registry.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	Copy,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
	Default,
)]
pub enum RegistryStatus {
	#[default]
	Active,
	Revoked,
	Deleted,
}

impl RegistryStatus {
	pub fn is_active(self) -> bool {
		matches!(self, RegistryStatus::Active)
	}

	pub fn is_revoked(self) -> bool {
		matches!(self, RegistryStatus::Revoked)
	}

	pub fn is_deleted(self) -> bool {
		matches!(self, RegistryStatus::Deleted)
	}
}
/// Specification for a single attribute in a registry schema.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, RuntimeDebug)]
pub struct RegistryAttributeSpec {
	/// Raw key bytes (e.g. "id", "name").
	pub key: Vec<u8>,
	/// Expected element kind for this attribute (Raw/Bool/Token/etc).
	pub kind: ElementType,
	/// Whether this attribute is optional in the schema.
	pub optional: bool,
}

/// How a registry can be looked up (single key or composite keys).
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, RuntimeDebug)]
pub enum LookupSpec {
	Single(Vec<u8>),
	Combo(Vec<Vec<u8>>),
}

/// View of a registry’s core metadata and schema,
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, RuntimeDebug)]
pub struct RegistryInfoView {
	pub info: ElementView,
	pub maintainer: Ss58Identifier,
	pub attributes: Vec<RegistryAttributeSpec>,
	pub token_spec: LookupSpec,
	pub lookup_specs: Vec<LookupSpec>,
	pub kind: RegistryKind,
	pub status: RegistryStatus,
}
