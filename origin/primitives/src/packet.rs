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

use crate::{
	attribute::{Attribute, Attributes, Element},
	element::ElementView,
	identifier::Ss58Identifier,
	registry::RegistryStatus,
};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{traits::Get, CloneNoBound, DebugNoBound, EqNoBound, PartialEqNoBound};
use scale_info::TypeInfo;
use sp_runtime::Debug;

/// Pointer linking an index entry to a specific registry/packet version.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	PartialEq,
	Eq,
	TypeInfo,
	MaxEncodedLen,
	Debug,
)]
pub struct PacketPointer {
	pub registry: Ss58Identifier,
	pub packet: Ss58Identifier,
	pub version: u32,
}

/// Packet lifecycle states.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	PartialEq,
	Eq,
	TypeInfo,
	MaxEncodedLen,
	Debug,
	Default,
)]
pub enum PacketStatus {
	#[default]
	Active,
	Revoked,
	Deleted,
}

/// Metadata tracked per packet token for quick access to the latest state.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	TypeInfo,
	MaxEncodedLen,
	DebugNoBound,
)]
#[scale_info(skip_type_params(Hash))]
pub struct PacketMetadata<Hash>
where
	Hash: Clone + PartialEq + Eq + core::fmt::Debug,
{
	pub registry: Ss58Identifier,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub latest_version: u32,
	pub digest: Hash,
}

/// Packet state persisted for each `(packet token, version)` pair.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	TypeInfo,
	MaxEncodedLen,
	DebugNoBound,
)]
#[scale_info(skip_type_params(MaxRawDataLength, MaxAdditionalAttributes, Hash))]
pub struct PacketState<
	MaxRawDataLength: Get<u32>,
	MaxAdditionalAttributes: Get<u32>,
	Hash: Clone + PartialEq + Eq + core::fmt::Debug,
> {
	pub registry: Ss58Identifier,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub version: u32,
	pub digest: Hash,
	pub attributes: Attributes<MaxRawDataLength, MaxAdditionalAttributes>,
}

impl<
		MaxRawDataLength: Get<u32>,
		MaxAdditionalAttributes: Get<u32>,
		Hash: Clone + PartialEq + Eq + core::fmt::Debug,
	> PacketState<MaxRawDataLength, MaxAdditionalAttributes, Hash>
{
	#[inline]
	pub fn attribute(&self, key: &[u8]) -> Option<&Element<MaxRawDataLength>> {
		self.attributes.get(key)
	}

	#[inline]
	pub fn is_deleted(&self) -> bool {
		matches!(self.status, PacketStatus::Deleted)
	}
}

/// Errors that can occur when applying a single update.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub enum PacketUpdateError {
	AttributeExists,
	TooManyAttributes,
	AttributeNotFound,
	ReservedAttribute,
	InvalidIdentifier,
	InvalidElement,
}

/// Single-operation updates against packet state.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	DebugNoBound,
	MaxEncodedLen,
	TypeInfo,
)]
#[scale_info(skip_type_params(MaxRawDataLength))]
pub enum PacketUpdateOp<MaxRawDataLength: Get<u32>> {
	AddAttribute(Attribute, Element<MaxRawDataLength>),
	RemoveAttribute(Attribute),
	UpdateAttribute(Attribute, Element<MaxRawDataLength>),
}

/// Core trait for “token” info that can be addressed via a packet.
pub trait PacketInformationProvider {
	/// Bitmask type for which fields are set/updated.
	type FieldMask: Encode + Decode + MaxEncodedLen + TypeInfo + Default;

	/// Maximum size for each data field.
	type MaxRawDataLength: Get<u32>;

	/// Maximum number of additional attributes per entity.
	type MaxAdditionalAttributes: Get<u32>;

	/// The enum of update operations.
	type UpdateOp: Encode
		+ Decode
		+ Clone
		+ core::fmt::Debug
		+ Eq
		+ PartialEq
		+ TypeInfo
		+ MaxEncodedLen;

	/// The raw attribute-map (never `None`).
	fn attributes(
		&self,
	) -> Option<&Attributes<Self::MaxRawDataLength, Self::MaxAdditionalAttributes>>;

	/// Return the current value for _any_ key (reserved field or attribute).
	fn get_key(&self, key: &[u8]) -> Element<Self::MaxRawDataLength>;

	/// Return a bitmask of *all* the fields currently set.
	fn present_fields(&self) -> Self::FieldMask;

	/// Check whether *all* bits in `mask` are set in `present_fields()`.
	fn has_info_fields(&self, mask: Self::FieldMask) -> bool;

	/// Apply one operation.
	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), PacketUpdateError>;

	/// Helper function - benchmarking and tests.
	fn create_info() -> Self
	where
		Self: Sized;

	/// Helper function - benchmarking and tests.
	fn all_fields() -> Self::FieldMask;
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
#[scale_info(skip_type_params(MaxRaw, MaxAttrs, Hash))]
pub struct PacketSnapshot<MaxRaw: Get<u32>, MaxAttrs: Get<u32>, Hash>
where
	Hash: Clone + PartialEq + Eq + core::fmt::Debug + Encode,
{
	pub state: PacketState<MaxRaw, MaxAttrs, Hash>,
	pub registry_status: RegistryStatus,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct PacketAttributeView {
	pub key: Vec<u8>,
	pub value: ElementView,
}

/// View-friendly representation of a packet’s state.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct PacketStateView {
	pub registry: Ss58Identifier,
	pub packet: Ss58Identifier,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub version: u32,
	pub registry_status: RegistryStatus,
	pub digest: Vec<u8>,
	pub attributes: Vec<PacketAttributeView>,
}

impl PacketStateView {
	/// Generic constructor converting runtime snapshot → view
	pub fn from_snapshot<
		MaxRaw: Get<u32>,
		MaxAttrs: Get<u32>,
		Hash: Clone + PartialEq + Eq + core::fmt::Debug + Encode,
	>(
		packet: &Ss58Identifier,
		snap: &PacketSnapshot<MaxRaw, MaxAttrs, Hash>,
	) -> Self {
		Self {
			registry: snap.state.registry.clone(),
			packet: packet.clone(),
			controller: snap.state.controller.clone(),
			status: snap.state.status.clone(),
			version: snap.state.version,
			registry_status: snap.registry_status.clone(),
			digest: snap.state.digest.encode(),
			attributes: snap
				.state
				.attributes
				.iter()
				.map(|(k, v)| PacketAttributeView { key: k.to_vec(), value: ElementView::from(v) })
				.collect(),
		}
	}

	#[inline]
	pub fn attribute(&self, key: &[u8]) -> Option<&ElementView> {
		self.attributes
			.iter()
			.find(|attr| attr.key.as_slice() == key)
			.map(|attr| &attr.value)
	}
}

/// View-friendly representation of packet metadata.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct PacketMetadataView {
	pub registry: Ss58Identifier,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub latest_version: u32,
	pub digest: Vec<u8>,
}

impl<Hash: Clone + PartialEq + Eq + core::fmt::Debug + Encode> From<&PacketMetadata<Hash>>
	for PacketMetadataView
{
	fn from(meta: &PacketMetadata<Hash>) -> Self {
		Self {
			registry: meta.registry.clone(),
			controller: meta.controller.clone(),
			status: meta.status.clone(),
			latest_version: meta.latest_version,
			digest: meta.digest.encode(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{
		Attribute, Attributes, Element, PacketMetadata, PacketMetadataView, PacketSnapshot,
		PacketState, PacketStateView, PacketStatus,
	};
	use crate::{attribute::AttributesError, identifier::Ss58Identifier, RegistryStatus};
	use alloc::{vec, vec::Vec};
	use codec::{Decode, Encode};
	use frame_support::{traits::ConstU32, BoundedVec};

	type MaxRaw = ConstU32<32>;
	type MaxAttrs = ConstU32<8>;
	type TinyAttrs = ConstU32<1>;

	fn key(name: &[u8]) -> Attribute {
		name.to_vec().try_into().expect("within attribute bounds")
	}

	#[test]
	fn attributes_try_from_sorts_and_validates() {
		let key_a = key(b"b");
		let key_b = key(b"a");
		let value = Element::<MaxRaw>::from_bool(true);
		let attrs = Attributes::<MaxRaw, MaxAttrs>::try_from(vec![
			(key_a.clone(), value.clone()),
			(key_b.clone(), value.clone()),
		])
		.expect("within bounds");
		let mut iter = attrs.iter();
		assert_eq!(iter.next().unwrap().0.as_slice(), key_b.as_slice());
		assert_eq!(iter.next().unwrap().0.as_slice(), key_a.as_slice());
	}

	#[test]
	fn attributes_reject_duplicate_keys() {
		let key = key(b"dup");
		let value = Element::<MaxRaw>::from_bool(false);
		let err = Attributes::<MaxRaw, MaxAttrs>::try_from(vec![
			(key.clone(), value.clone()),
			(key, value),
		]);
		assert!(matches!(err, Err(AttributesError::DuplicateKey)));
	}

	#[test]
	fn attributes_try_insert_rejects_invalid_element() {
		let mut attrs = Attributes::<MaxRaw, MaxAttrs>::default();
		let key = key(b"flag");
		let mut invalid = Element::<MaxRaw>::from_bool(true);
		if let Element::Bool(ref mut flag) = invalid {
			*flag = 2;
		}
		assert!(matches!(attrs.try_insert(key, invalid), Err(AttributesError::InvalidElement)));
	}

	#[test]
	fn try_collect_orders_entries() {
		let key_a = key(b"z");
		let key_b = key(b"a");
		let elem = Element::<MaxRaw>::from_bool(true);
		let attrs = Attributes::<MaxRaw, MaxAttrs>::try_collect(vec![
			(key_a.clone(), elem.clone()),
			(key_b.clone(), elem.clone()),
		])
		.expect("within bounds");
		let mut iter = attrs.iter();
		assert_eq!(iter.next().unwrap().0.as_slice(), key_b.as_slice());
		assert_eq!(iter.next().unwrap().0.as_slice(), key_a.as_slice());
	}

	#[test]
	fn merge_updates_and_inserts() {
		let mut base = Attributes::<MaxRaw, MaxAttrs>::default();
		let key_existing = key(b"foo");
		let key_new = key(b"bar");
		base.try_insert(key_existing.clone(), Element::<MaxRaw>::from_bool(false))
			.unwrap();
		let mut updates = Attributes::<MaxRaw, MaxAttrs>::default();
		updates
			.try_insert(key_existing.clone(), Element::<MaxRaw>::from_bool(true))
			.unwrap();
		updates.try_insert(key_new.clone(), Element::<MaxRaw>::from_bool(true)).unwrap();
		base.merge(&updates).unwrap();
		assert_eq!(base.get(key_existing.as_slice()).unwrap().as_bool(), Some(true));
		assert!(base.contains_key(key_new.as_slice()));
	}

	#[test]
	fn encoded_pairs_are_sorted() {
		let attrs = Attributes::<MaxRaw, MaxAttrs>::try_collect(vec![
			(b"b".to_vec().try_into().unwrap(), Element::<MaxRaw>::from_bool(true)),
			(b"a".to_vec().try_into().unwrap(), Element::<MaxRaw>::from_bool(false)),
		])
		.expect("collect ok");
		let pairs = attrs.encoded_pairs();
		assert_eq!(pairs[0].0, b"a".to_vec());
		assert_eq!(pairs[1].0, b"b".to_vec());
	}

	#[test]
	fn get_and_get_mut_follow_sorted_keys() {
		let mut attrs = Attributes::<MaxRaw, MaxAttrs>::default();
		let key = key(b"flip");
		attrs.try_insert(key.clone(), Element::<MaxRaw>::from_bool(false)).unwrap();
		let flag = attrs.get_mut(key.as_slice()).expect("entry present");
		if let Element::Bool(ref mut inner) = flag {
			*inner = 1;
		} else {
			panic!("expected bool element");
		}
		assert_eq!(attrs.get(key.as_slice()).unwrap().as_bool(), Some(true));
	}

	#[test]
	fn remove_clears_entries_and_shifts_remaining() {
		let mut attrs = Attributes::<MaxRaw, MaxAttrs>::try_from(vec![
			(key(b"a"), Element::<MaxRaw>::from_bool(true)),
			(key(b"b"), Element::<MaxRaw>::from_bool(false)),
		])
		.expect("valid attributes");
		let removed = attrs.remove(b"a").expect("entry removed");
		assert_eq!(removed.0.as_slice(), b"a");
		assert!(!attrs.contains_key(b"a"));
		assert_eq!(attrs.len(), 1);
		assert_eq!(attrs.iter().next().unwrap().0.as_slice(), b"b");
	}

	#[test]
	fn upsert_replaces_existing_and_preserves_order() {
		let mut attrs = Attributes::<MaxRaw, MaxAttrs>::try_from(vec![
			(key(b"a"), Element::<MaxRaw>::from_bool(false)),
			(key(b"c"), Element::<MaxRaw>::from_bool(true)),
		])
		.expect("valid attributes");
		attrs.upsert(key(b"b"), Element::<MaxRaw>::from_bool(true)).unwrap();
		attrs.upsert(key(b"a"), Element::<MaxRaw>::from_bool(true)).unwrap();
		let keys: Vec<Vec<u8>> = attrs.iter().map(|(k, _)| k.to_vec()).collect();
		assert_eq!(keys, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
		assert_eq!(attrs.get(b"a").unwrap().as_bool(), Some(true));
	}

	#[test]
	fn upsert_rejects_invalid_elements() {
		let mut attrs = Attributes::<MaxRaw, MaxAttrs>::default();
		let key = key(b"flag");
		let mut invalid = Element::<MaxRaw>::from_bool(true);
		if let Element::Bool(ref mut flag) = invalid {
			*flag = 3;
		}
		assert_eq!(attrs.upsert(key, invalid), Err(AttributesError::InvalidElement));
	}

	#[test]
	fn try_insert_enforces_capacity_limits() {
		let mut attrs = Attributes::<MaxRaw, TinyAttrs>::default();
		attrs.try_insert(key(b"a"), Element::<MaxRaw>::from_bool(true)).unwrap();
		let err = attrs.try_insert(key(b"b"), Element::<MaxRaw>::from_bool(false));
		assert_eq!(err, Err(AttributesError::TooManyAttributes));
	}

	#[test]
	fn merge_propagates_capacity_errors() {
		let mut base = Attributes::<MaxRaw, TinyAttrs>::default();
		base.try_insert(key(b"a"), Element::<MaxRaw>::from_bool(false)).unwrap();
		let mut updates = Attributes::<MaxRaw, TinyAttrs>::default();
		updates.try_insert(key(b"b"), Element::<MaxRaw>::from_bool(true)).unwrap();
		assert_eq!(base.merge(&updates), Err(AttributesError::TooManyAttributes));
	}

	#[test]
	fn try_collect_rejects_invalid_elements() {
		let mut invalid = Element::<MaxRaw>::from_bool(true);
		if let Element::Bool(ref mut flag) = invalid {
			*flag = 9;
		}
		let err = Attributes::<MaxRaw, MaxAttrs>::try_collect(vec![(key(b"x"), invalid)]);
		assert_eq!(err, Err(AttributesError::InvalidElement));
	}

	#[test]
	fn decode_rejects_duplicate_keys() {
		let dup_key = key(b"d");
		let elem = Element::<MaxRaw>::from_bool(true);
		let raw = BoundedVec::<_, MaxAttrs>::try_from(vec![
			(dup_key.clone(), elem.clone()),
			(dup_key, elem),
		])
		.expect("bounded vec allows duplicates");
		let encoded = raw.encode();
		let mut cursor = &encoded[..];
		let err = Attributes::<MaxRaw, MaxAttrs>::decode(&mut cursor).unwrap_err();
		assert_eq!(err.to_string(), "Duplicate attribute keys found");
	}

	#[test]
	fn packet_state_view_exposes_attributes_and_metadata() {
		let registry = Ss58Identifier::to_encoded([1u8; 32], 1, 1, 1).expect("registry id ok");
		let controller = Ss58Identifier::to_encoded([2u8; 32], 2, 2, 1).expect("controller id ok");

		let attrs = Attributes::<MaxRaw, MaxAttrs>::try_from(vec![(
			key(b"foo"),
			Element::<MaxRaw>::from_bool(true),
		)])
		.expect("valid attrs");

		let state = PacketState::<MaxRaw, MaxAttrs, [u8; 32]> {
			registry: registry.clone(),
			controller: controller.clone(),
			status: PacketStatus::Active,
			version: 3,
			digest: [9u8; 32],
			attributes: attrs,
		};

		let packet = Ss58Identifier::to_encoded([8u8; 32], 8, 8, 1).expect("packet id ok");
		let snapshot =
			PacketSnapshot { state: state.clone(), registry_status: RegistryStatus::Active };
		let view = PacketStateView::from_snapshot(&packet, &snapshot);
		assert_eq!(view.registry, registry);
		assert_eq!(view.packet, packet);
		assert_eq!(view.controller, controller);
		assert_eq!(view.version, 3);
		assert_eq!(view.status, PacketStatus::Active);
		assert_eq!(view.attribute(b"foo").and_then(|v| v.as_bool()), Some(true));
		assert!(view.attribute(b"missing").is_none());
	}

	#[test]
	fn packet_metadata_view_round_trips_fields() {
		let registry = Ss58Identifier::to_encoded([3u8; 32], 5, 5, 1).expect("registry id ok");
		let controller = Ss58Identifier::to_encoded([4u8; 32], 6, 6, 1).expect("controller id ok");
		let meta = PacketMetadata::<[u8; 32]> {
			registry: registry.clone(),
			controller: controller.clone(),
			status: PacketStatus::Revoked,
			latest_version: 7,
			digest: [0xAA; 32],
		};

		let view = PacketMetadataView::from(&meta);
		assert_eq!(view.registry, registry);
		assert_eq!(view.controller, controller);
		assert_eq!(view.status, PacketStatus::Revoked);
		assert_eq!(view.latest_version, 7);
		assert_eq!(view.digest, meta.digest.encode());
	}
}
