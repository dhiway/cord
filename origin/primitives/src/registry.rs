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
use alloc::vec::Vec;
use bitflags::bitflags;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

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
	Debug,
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
	Debug,
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
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct RegistryAttributeSpec {
	/// Raw key bytes (e.g. "id", "name").
	pub key: Vec<u8>,
	/// Expected element kind for this attribute (Raw/Bool/Token/etc).
	pub kind: ElementType,
	/// Whether this attribute is optional in the schema.
	pub optional: bool,
}

/// How a registry can be looked up (single key or composite keys).
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub enum LookupSpec {
	Single(Vec<u8>),
	Combo(Vec<Vec<u8>>),
}

/// View of a registry’s core metadata and schema,
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct RegistryStateView {
	/// Registry identifier (Ss58Identifier)
	pub registry: Ss58Identifier,
	/// Owner/maintainer (entity token)
	pub maintainer: Ss58Identifier,
	/// Human/application metadata (decoded)
	pub info: ElementView,
	/// Kind of registry (Raw/Token/Hash)
	pub kind: RegistryKind,
	/// Active / Revoked / Deleted
	pub status: RegistryStatus,
	/// Attribute schema
	pub attributes: Vec<RegistryAttributeView>,
	/// Token-spec defining registry ID derivation
	pub token_spec: Vec<Vec<u8>>,
	/// Lookup specifications (list of lists of keys)
	pub lookup_specs: Vec<Vec<Vec<u8>>>,
}

/// Flattened view for a single registry attribute.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct RegistryAttributeView {
	pub key: Vec<u8>,
	pub kind: ElementType,
	pub optional: bool,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::element::ElementView;

	#[test]
	fn permissions_from_list_adds_view_bit_when_elevated() {
		let perms = RegistryPermissions::from_list(&[RegistryPermissions::ADMIN]);
		assert!(perms.has_admin());
		assert!(perms.has_view());
		assert!(perms.has_entry());

		let delegate_only = RegistryPermissions::from_list(&[RegistryPermissions::DELEGATE]);
		assert!(delegate_only.has_delegate());
		assert!(delegate_only.has_view(), "delegate implies view");
	}

	#[test]
	fn default_permissions_include_entry_and_view_only() {
		let perms = RegistryPermissions::default();
		assert!(perms.has_entry());
		assert!(perms.has_view());
		assert!(!perms.has_admin());
		assert!(!perms.has_delegate());
	}

	#[test]
	fn status_helpers_match_variants() {
		assert!(RegistryStatus::Active.is_active());
		assert!(RegistryStatus::Revoked.is_revoked());
		assert!(RegistryStatus::Deleted.is_deleted());
	}

	#[test]
	fn registry_kind_default_is_raw() {
		assert!(matches!(RegistryKind::default(), RegistryKind::Raw));
	}

	#[test]
	fn registry_info_view_round_trip_fields() {
		let info = ElementView::Raw(vec![1, 2, 3]);
		let registry = Ss58Identifier::to_encoded([9u8; 32], 1, 1, 1).expect("identifier ok");
		let maintainer = Ss58Identifier::to_encoded([0u8; 32], 1, 1, 1).expect("identifier ok");
		let attributes = vec![RegistryAttributeView {
			key: b"id".to_vec(),
			kind: ElementType::Raw,
			optional: false,
		}];
		let token_spec = vec![b"id".to_vec()];
		let lookup_specs = vec![vec![b"id".to_vec(), b"name".to_vec()]];
		let view = RegistryStateView {
			registry: registry.clone(),
			maintainer: maintainer.clone(),
			info: info.clone(),
			kind: RegistryKind::Token,
			status: RegistryStatus::Active,
			attributes: attributes.clone(),
			token_spec: token_spec.clone(),
			lookup_specs: lookup_specs.clone(),
		};
		let encoded = view.encode();
		let decoded = RegistryStateView::decode(&mut &encoded[..]).expect("decode view");
		assert_eq!(decoded.registry, registry);
		assert_eq!(decoded.info, info);
		assert_eq!(decoded.maintainer, maintainer);
		assert_eq!(decoded.attributes, attributes);
		assert_eq!(decoded.token_spec, token_spec);
		assert_eq!(decoded.lookup_specs, lookup_specs);
		assert!(decoded.status.is_active());
	}
}
