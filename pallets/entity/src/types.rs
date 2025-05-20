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

// crate/types.rs

// use crate::entity::EntityField;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use cord_primitives::element::Element;
use core::fmt::Debug;
use frame_support::{
	traits::{ConstU32, Get},
	BoundedVec, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;

/// The raw‐data type used throughout the entity pallet.
pub type Data<MaxRawDataLength> = Element<MaxRawDataLength>;

/// Maximum length for an additional‐field key.
pub type Attribute = BoundedVec<u8, ConstU32<64>>;

/// The bounded list of `(Attribute, Data)` pairs.
pub type Attributes<MaxRawDataLength> =
	BoundedVec<(Attribute, Data<MaxRawDataLength>), ConstU32<32>>;

/// Errors that can occur when applying a single `EntityUpdateOp`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityUpdateError {
	AttributeExists,
	TooManyAttributes,
	AttributeNotFound,
}

/// Core trait for “entity” info.
pub trait EntityInformationProvider:
	Encode + Decode + MaxEncodedLen + Clone + Debug + Eq + PartialEq + TypeInfo + Default
{
	/// Bitmask type for which fields are set/updated.
	type FieldsIdentifier: Encode + Decode + MaxEncodedLen + TypeInfo + Default;

	/// Limit on the raw‐data size.
	type MaxRawDataLength: Get<u32>;

	/// The enum of update operations.
	type UpdateOp: Encode + Decode + Clone + Debug + PartialEq + TypeInfo + MaxEncodedLen;

	/// The raw attribute‐map (never `None`).
	fn attributes(&self) -> Option<&Attributes<Self::MaxRawDataLength>>;

	/// Return the current value for _any_ key (reserved field or attribute).
	fn get_key(&self, key: &[u8]) -> Data<Self::MaxRawDataLength>;

	/// Return a bitmask of *all* the identity‐fields currently set.
	fn present_fields(&self) -> Self::FieldsIdentifier;

	/// Check whether *all* bits in `mask` are set in `present_fields()`.
	fn has_info_fields(&self, fields: Self::FieldsIdentifier) -> bool;

	/// Apply one operation.
	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), EntityUpdateError>;

	/// For benchmarking only.
	#[cfg(feature = "runtime-benchmarks")]
	fn create_entity_info() -> Self;

	/// For benchmarking only.
	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier;
}

/// Single‐op‐for‐any‐key
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
pub enum EntityUpdateOp<MaxRawDataLength: Get<u32>> {
	/// Add a new attribute key → Data (fails if key exists or reserved)
	AddAttribute(Attribute, Data<MaxRawDataLength>),
	/// Remove any key (reserved fields set to None, attributes dropped)
	RemoveAttribute(Attribute),
	/// Update value of any existing key (reserved or attribute)
	UpdateAttribute(Attribute, Data<MaxRawDataLength>),
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_data_roundtrip_and_as_ref() {
		// None variant
		type TestData = Data<ConstU32<128>>;
		let d_none = TestData::None;
		let enc_none = d_none.encode();
		let dec_none = TestData::decode(&mut &enc_none[..]).expect("Decode None");
		assert_eq!(dec_none, d_none);
		assert_eq!(d_none.as_ref(), &[] as &[u8]);

		// Raw variant with various lengths
		for &n in &[0usize, 1, 5, 32, 64, 128] {
			let vec: Vec<u8> = vec![0xAB; n];
			let bounded: BoundedVec<u8, ConstU32<128>> = vec.clone().try_into().unwrap();
			let d_raw = TestData::Raw(bounded.clone());
			let enc = d_raw.encode();
			let dec = TestData::decode(&mut &enc[..]).expect("Decode Raw");
			assert_eq!(dec, d_raw);
			assert_eq!(d_raw.as_ref(), vec.as_slice());
		}

		// Digest variant
		let hash = [0x11u8; 32];
		let d_digest = TestData::Digest(hash);
		let enc_digest = d_digest.encode();
		let dec_digest = TestData::decode(&mut &enc_digest[..]).expect("Decode Digest");
		assert_eq!(dec_digest, d_digest);
		assert_eq!(d_digest.as_ref(), &hash[..]);

		// CID variant
		let cid = [0x22u8; 64];
		let d_cid = TestData::CID(cid);
		let enc_cid = d_cid.encode();
		let dec_cid = TestData::decode(&mut &enc_cid[..]).expect("Decode CID");
		assert_eq!(dec_cid, d_cid);
		assert_eq!(d_cid.as_ref(), &cid[..]);
	}
}
