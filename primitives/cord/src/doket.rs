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

/// The bounded list of `(Attribute, Data)` pairs.
pub type Attributes<MaxRawDataLength, MaxAdditionalAttributes> =
	BoundedVec<(Attribute, Element<MaxRawDataLength>), MaxAdditionalAttributes>;

/// Errors that can occur when applying a single update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoketUpdateError {
	AttributeExists,
	TooManyAttributes,
	AttributeNotFound,
	Invalididentifier,
	ReservedAttribute,
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
pub enum DoketUpdateOp<MaxRawDataLength: Get<u32>> {
	AddAttribute(Attribute, Element<MaxRawDataLength>),
	RemoveAttribute(Attribute),
	UpdateAttribute(Attribute, Element<MaxRawDataLength>),
}

/// Core trait for “doken” info.
pub trait DoketInformationProvider {
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
	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), DoketUpdateError>;

	/// Helper function - Benchmarking and tests
	fn create_info() -> Self
	where
		Self: Sized;
	/// Helper function - Benchmarking and tests
	fn all_fields() -> Self::FieldMask;
}
