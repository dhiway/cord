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

use crate::element::Elum;
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{
	traits::{ConstU32, Get},
	BoundedVec, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;

/// The raw‐data type used throughout the entity pallet.
pub type Element<MaxRawDataLength> = Elum<MaxRawDataLength>;

/// Maximum length for an additional‐field key.
pub type Attribute = BoundedVec<u8, ConstU32<64>>;

/// Errors returned when normalising attribute collections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributesError {
	DuplicateKey,
	TooManyAttributes,
	InvalidElement,
}

impl AttributesError {
	fn into_codec_error(self) -> codec::Error {
		match self {
			AttributesError::DuplicateKey => "Duplicate attribute keys found".into(),
			AttributesError::TooManyAttributes => "Attribute count exceeds limit".into(),
			AttributesError::InvalidElement => "Attribute contains invalid element".into(),
		}
	}
}

/// Deterministic set of `(Attribute, Element)` pairs kept in lexicographic key order.
#[derive(
	Encode, CloneNoBound, PartialEqNoBound, EqNoBound, RuntimeDebugNoBound, MaxEncodedLen, TypeInfo,
)]
#[scale_info(skip_type_params(MaxRawDataLength, MaxAdditionalAttributes))]
pub struct Attributes<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>(
	BoundedVec<(Attribute, Element<MaxRawDataLength>), MaxAdditionalAttributes>,
);

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>
	Attributes<MaxRawDataLength, MaxAdditionalAttributes>
{
	pub fn new() -> Self {
		Self(BoundedVec::new())
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn iter(&self) -> core::slice::Iter<'_, (Attribute, Element<MaxRawDataLength>)> {
		self.0.iter()
	}

	pub fn get(&self, key: &[u8]) -> Option<&Element<MaxRawDataLength>> {
		self.position(key).ok().map(|idx| &self.0[idx].1)
	}

	pub fn get_mut(&mut self, key: &[u8]) -> Option<&mut Element<MaxRawDataLength>> {
		self.position(key).ok().map(move |idx| &mut self.0[idx].1)
	}

	pub fn contains_key(&self, key: &[u8]) -> bool {
		self.position(key).is_ok()
	}

	pub fn try_insert(
		&mut self,
		key: Attribute,
		value: Element<MaxRawDataLength>,
	) -> Result<(), AttributesError> {
		value.validate().map_err(|_| AttributesError::InvalidElement)?;
		match self.position(key.as_slice()) {
			Ok(_) => Err(AttributesError::DuplicateKey),
			Err(pos) => self
				.0
				.try_insert(pos, (key, value))
				.map_err(|_| AttributesError::TooManyAttributes),
		}
	}

	pub fn remove(&mut self, key: &[u8]) -> Option<(Attribute, Element<MaxRawDataLength>)> {
		self.position(key).ok().map(|idx| self.0.remove(idx))
	}

	pub fn validate(&self) -> Result<(), AttributesError> {
		let mut prev: Option<&[u8]> = None;
		for (key, value) in self.0.iter() {
			value.validate().map_err(|_| AttributesError::InvalidElement)?;
			let key_slice = key.as_slice();
			if let Some(prev_key) = prev {
				if prev_key >= key_slice {
					return Err(AttributesError::DuplicateKey);
				}
			}
			prev = Some(key_slice);
		}
		Ok(())
	}

	fn position(&self, key: &[u8]) -> Result<usize, usize> {
		self.0.binary_search_by(|(existing, _)| existing.as_slice().cmp(key))
	}

	fn canonicalize(
		mut inner: BoundedVec<(Attribute, Element<MaxRawDataLength>), MaxAdditionalAttributes>,
	) -> Result<Self, AttributesError> {
		inner.sort_by(|(a, _), (b, _)| a.as_slice().cmp(b.as_slice()));
		let attrs = Self(inner);
		attrs.validate().map(|_| attrs)
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> Default
	for Attributes<MaxRawDataLength, MaxAdditionalAttributes>
{
	fn default() -> Self {
		Self::new()
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> core::ops::Deref
	for Attributes<MaxRawDataLength, MaxAdditionalAttributes>
{
	type Target = [(Attribute, Element<MaxRawDataLength>)];

	fn deref(&self) -> &Self::Target {
		self.0.as_ref()
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>
	From<Attributes<MaxRawDataLength, MaxAdditionalAttributes>>
	for Vec<(Attribute, Element<MaxRawDataLength>)>
{
	fn from(value: Attributes<MaxRawDataLength, MaxAdditionalAttributes>) -> Self {
		value.0.into()
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>
	TryFrom<Vec<(Attribute, Element<MaxRawDataLength>)>>
	for Attributes<MaxRawDataLength, MaxAdditionalAttributes>
{
	type Error = AttributesError;

	fn try_from(value: Vec<(Attribute, Element<MaxRawDataLength>)>) -> Result<Self, Self::Error> {
		let bounded = BoundedVec::<_, MaxAdditionalAttributes>::try_from(value)
			.map_err(|_| AttributesError::TooManyAttributes)?;
		Self::canonicalize(bounded)
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>
	TryFrom<BoundedVec<(Attribute, Element<MaxRawDataLength>), MaxAdditionalAttributes>>
	for Attributes<MaxRawDataLength, MaxAdditionalAttributes>
{
	type Error = AttributesError;

	fn try_from(
		value: BoundedVec<(Attribute, Element<MaxRawDataLength>), MaxAdditionalAttributes>,
	) -> Result<Self, Self::Error> {
		Self::canonicalize(value)
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> Decode
	for Attributes<MaxRawDataLength, MaxAdditionalAttributes>
{
	fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
		let raw =
			BoundedVec::<(Attribute, Element<MaxRawDataLength>), MaxAdditionalAttributes>::decode(
				input,
			)?;
		Self::canonicalize(raw).map_err(AttributesError::into_codec_error)
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> DecodeWithMemTracking
	for Attributes<MaxRawDataLength, MaxAdditionalAttributes>
{}

/// Errors that can occur when applying a single update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketUpdateError {
	AttributeExists,
	TooManyAttributes,
	AttributeNotFound,
	ReservedAttribute,
	InvalidIdentifier,
	InvalidElement,
}

/// Single‐op‐for‐any‐key update operations.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	RuntimeDebugNoBound,
	MaxEncodedLen,
	TypeInfo,
)]
#[scale_info(skip_type_params(MaxRawDataLength))]
pub enum PacketUpdateOp<MaxRawDataLength: Get<u32>> {
	AddAttribute(Attribute, Element<MaxRawDataLength>),
	RemoveAttribute(Attribute),
	UpdateAttribute(Attribute, Element<MaxRawDataLength>),
}

/// Core trait for “token” info.
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

	/// The raw attribute‐map (never `None`).
	fn attributes(
		&self,
	) -> Option<&Attributes<Self::MaxRawDataLength, Self::MaxAdditionalAttributes>>;

	/// Return the current value for _any_ key (reserved field or attribute).
	fn get_key(&self, key: &[u8]) -> Element<Self::MaxRawDataLength>;

	/// Return a bitmask of *all* the identity‐fields currently set.
	fn present_fields(&self) -> Self::FieldMask;

	/// Check whether *all* bits in `mask` are set in `present_fields()`.
	fn has_info_fields(&self, mask: Self::FieldMask) -> bool;

	/// Apply one operation.
	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), PacketUpdateError>;

	/// Helper function - Benchmarking and tests
	fn create_info() -> Self
	where
		Self: Sized;
	/// Helper function - Benchmarking and tests
	fn all_fields() -> Self::FieldMask;
}

#[cfg(test)]
mod tests {
	use super::{Attribute, Attributes, AttributesError, Element};
	use alloc::vec;
	use frame_support::traits::ConstU32;

	type MaxRaw = ConstU32<32>;
	type MaxAttrs = ConstU32<8>;

	#[test]
	fn attributes_try_from_sorts_and_validates() {
		let key_a: Attribute = b"b".to_vec().try_into().unwrap();
		let key_b: Attribute = b"a".to_vec().try_into().unwrap();
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
		let key: Attribute = b"dup".to_vec().try_into().unwrap();
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
		let key: Attribute = b"flag".to_vec().try_into().unwrap();
		let mut invalid = Element::<MaxRaw>::from_bool(true);
		if let Element::Bool(ref mut flag) = invalid {
			*flag = 2;
		}
		assert!(matches!(attrs.try_insert(key, invalid), Err(AttributesError::InvalidElement)));
	}
}
