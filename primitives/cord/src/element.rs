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

// # CORD Element (MultiData Format)

// Note: This module is part of cord-primitives and should be imported by all higher-level
// modules that need to interact with elements.

use crate::identifier::Ss58Identifier;
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{
	traits::Get, BoundedVec, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;

#[cfg(test)]
use alloc::vec;

const fn compact_len_u32(len: u32) -> usize {
	if len <= 0b0011_1111 {
		1
	} else if len <= 0b0011_1111_1111_1111 {
		2
	} else if len <= 0x3FFF_FFFF {
		4
	} else {
		5
	}
}

/// The `Elum` enum supports a set of typed payloads used across CORD.
/// * `None`: Represents an unset/empty value.
/// * `Raw`: Arbitrary bounded bytes. Use this for free-form payloads that do not fit a dedicated
///   type (e.g. JSON, CBOR, signatures). Any type **except** hashes or SS58 identifiers can be
///   encoded here.
/// * `Bool`, `U64`, `U128`: Fixed-width scalar encodings using little-endian byte order. The
///   dedicated variants avoid per-call decoding for common primitives.
/// * `Hash`: BlakeTwo compatible 32-byte digests.
/// * `Token`: Embedded [`Ss58Identifier`], typically used for entity or registry ownership.
/// * `CID`: Content identifier (64-byte multihash or equivalent).
#[derive(
	CloneNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	PartialEqNoBound,
	EqNoBound,
	RuntimeDebugNoBound,
	MaxEncodedLen,
	TypeInfo,
)]
#[scale_info(skip_type_params(MaxCap))]
pub enum Elum<MaxCap: Get<u32>> {
	/// No data provided.
	#[codec(index = 0)]
	None,
	/// Raw data stored directly.
	#[codec(index = 1)]
	Raw(BoundedVec<u8, MaxCap>),
	/// Boolean value stored as `0`/`1`.
	#[codec(index = 2)]
	Bool(u8),
	/// Unsigned 64-bit value in little-endian byte order.
	#[codec(index = 3)]
	U64([u8; 8]),
	/// Unsigned 128-bit value in little-endian byte order.
	#[codec(index = 4)]
	U128([u8; 16]),
	/// A 32-byte BlakeTwo256 digest.
	#[codec(index = 5)]
	Hash([u8; 32]),
	/// An embedded token. The underlying bytes are the canonical binary encoding of the
	/// identifier.
	#[codec(index = 6)]
	Token(Ss58Identifier),
	/// Content identifier stored as bounded bytes.
	#[codec(index = 7)]
	CID(BoundedVec<u8, MaxCap>),
}

/// Declarative descriptor of the variants supported by [`Elum`].
#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
)]
pub enum ElementType {
	None,
	Raw,
	Bool,
	U64,
	U128,
	Hash,
	Token,
	Cid,
}

impl<MaxCap: Get<u32>> From<&Elum<MaxCap>> for ElementType {
	fn from(value: &Elum<MaxCap>) -> Self {
		match value {
			Elum::None => ElementType::None,
			Elum::Raw(_) => ElementType::Raw,
			Elum::Bool(_) => ElementType::Bool,
			Elum::U64(_) => ElementType::U64,
			Elum::U128(_) => ElementType::U128,
			Elum::Hash(_) => ElementType::Hash,
			Elum::Token(_) => ElementType::Token,
			Elum::CID(_) => ElementType::Cid,
		}
	}
}

impl ElementType {
	/// Returns `true` if this schema variant expects an embedded Ss58Identifier.
	pub fn is_token(self) -> bool {
		matches!(self, ElementType::Token)
	}
}

// Provide a unified AsRef<[u8]> implementation to obtain a view of the inner bytes.
impl<MaxCap: Get<u32>> AsRef<[u8]> for Elum<MaxCap> {
	fn as_ref(&self) -> &[u8] {
		match self {
			Elum::None => &[],
			Elum::Raw(raw) => raw.as_slice(),
			Elum::Bool(flag) => core::slice::from_ref(flag),
			Elum::U64(bytes) => bytes,
			Elum::U128(bytes) => bytes,
			Elum::Hash(digest) => digest,
			Elum::Token(id) => id.as_bytes(),
			Elum::CID(cid) => cid.as_slice(),
		}
	}
}

// Explicit accessor methods for each variant.
impl<MaxCap: Get<u32>> Elum<MaxCap> {
	/// Returns `true` if the Elum is `None`.
	pub fn is_none(&self) -> bool {
		matches!(self, Elum::None)
	}

	/// Validate internal invariants (e.g., boolean payloads).
	pub fn validate(&self) -> Result<(), codec::Error> {
		match self {
			Elum::Bool(flag) if *flag > 1 => {
				Err("Invalid boolean discriminant for Elum::Bool".into())
			},
			_ => Ok(()),
		}
	}

	/// Returns `true` if the Elum contains raw bytes.
	pub fn is_raw(&self) -> bool {
		matches!(self, Elum::Raw(_))
	}

	/// Returns `true` if the Elum contains a boolean.
	pub fn is_bool(&self) -> bool {
		matches!(self, Elum::Bool(_))
	}

	/// Returns `true` if the Elum contains an unsigned 64-bit integer.
	pub fn is_u64(&self) -> bool {
		matches!(self, Elum::U64(_))
	}

	/// Returns `true` if the Elum contains an unsigned 128-bit integer.
	pub fn is_u128(&self) -> bool {
		matches!(self, Elum::U128(_))
	}

	/// Returns `true` if the Elum contains a hash digest.
	pub fn is_hash(&self) -> bool {
		matches!(self, Elum::Hash(_))
	}

	/// Returns `true` if the Elum contains an SS58 identifier.
	pub fn is_token(&self) -> bool {
		matches!(self, Elum::Token(_))
	}

	/// Returns `true` if the Elum contains a CID.
	pub fn is_cid(&self) -> bool {
		matches!(self, Elum::CID(_))
	}

	/// If the Elum is `Raw`, returns a reference to its contents; otherwise, `None`.
	pub fn as_raw(&self) -> Option<&[u8]> {
		if let Elum::Raw(raw) = self {
			Some(raw.as_slice())
		} else {
			None
		}
	}

	/// If the Elum is `Bool`, returns the boolean value; otherwise, `None`.
	pub fn as_bool(&self) -> Option<bool> {
		if let Elum::Bool(flag) = self {
			Some(*flag != 0)
		} else {
			None
		}
	}
	/// If the Elum is `U64`, returns the 64-bit integer; otherwise, `None`.
	pub fn as_u64(&self) -> Option<u64> {
		if let Elum::U64(bytes) = self {
			Some(u64::from_le_bytes(*bytes))
		} else {
			None
		}
	}

	/// If the Elum is `U128`, returns the 128-bit integer; otherwise, `None`.
	pub fn as_u128(&self) -> Option<u128> {
		if let Elum::U128(bytes) = self {
			Some(u128::from_le_bytes(*bytes))
		} else {
			None
		}
	}

	/// If the Elum is `Hash`, returns the 32-byte digest; otherwise, `None`.
	pub fn as_hash(&self) -> Option<&[u8; 32]> {
		if let Elum::Hash(digest) = self {
			Some(digest)
		} else {
			None
		}
	}

	/// Returns the embedded Ss58Identifier if the element is `Identifier`.
	pub fn as_token(&self) -> Option<&Ss58Identifier> {
		if let Elum::Token(id) = self {
			Some(id)
		} else {
			None
		}
	}

	/// If the Elum is `CID`, returns the 64-byte CID; otherwise, `None`.
	pub fn as_cid(&self) -> Option<&[u8]> {
		if let Elum::CID(cid) = self {
			Some(cid.as_slice())
		} else {
			None
		}
	}

	/// Payload byte length (excludes the discriminant and any length prefix).
	pub fn byte_len(&self) -> usize {
		match self {
			Elum::None => 0,
			Elum::Raw(raw) => raw.len(),
			Elum::Bool(_) => 1,
			Elum::U64(bytes) => bytes.len(),
			Elum::U128(bytes) => bytes.len(),
			Elum::Hash(bytes) => bytes.len(),
			Elum::Token(id) => id.as_bytes().len(),
			Elum::CID(cid) => cid.len(),
		}
	}

	/// Full SCALE-encoded length including discriminant and any prefixes.
	pub fn encoded_len(&self) -> usize {
		match self {
			Elum::None => 1,
			Elum::Raw(raw) => 1 + compact_len_u32(raw.len() as u32) + raw.len(),
			Elum::Bool(_) => 1 + 1,
			Elum::U64(_) => 1 + 8,
			Elum::U128(_) => 1 + 16,
			Elum::Hash(_) => 1 + 32,
			Elum::Token(id) => 1 + id.encoded_size(),
			Elum::CID(cid) => 1 + compact_len_u32(cid.len() as u32) + cid.len(),
		}
	}

	/// Construct a boolean element.
	pub fn from_bool(value: bool) -> Self {
		Self::Bool(value as u8)
	}

	/// Construct a `U64` element using little-endian encoding.
	pub fn from_u64(value: u64) -> Self {
		Self::U64(value.to_le_bytes())
	}

	/// Construct a `U128` element using little-endian encoding.
	pub fn from_u128(value: u128) -> Self {
		Self::U128(value.to_le_bytes())
	}
}

impl<MaxCap: Get<u32>> Default for Elum<MaxCap> {
	fn default() -> Self {
		Elum::None
	}
}

impl<MaxCap: Get<u32>> From<bool> for Elum<MaxCap> {
	fn from(value: bool) -> Self {
		Self::from_bool(value)
	}
}

impl<MaxCap: Get<u32>> From<u64> for Elum<MaxCap> {
	fn from(value: u64) -> Self {
		Self::from_u64(value)
	}
}

impl<MaxCap: Get<u32>> From<u128> for Elum<MaxCap> {
	fn from(value: u128) -> Self {
		Self::from_u128(value)
	}
}

impl<MaxCap: Get<u32>> TryFrom<Vec<u8>> for Elum<MaxCap> {
	type Error = ();

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		BoundedVec::<u8, MaxCap>::try_from(value).map(Elum::Raw).map_err(|_| ())
	}
}

impl<'a, MaxCap: Get<u32>> TryFrom<&'a [u8]> for Elum<MaxCap> {
	type Error = ();

	fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
		BoundedVec::<u8, MaxCap>::try_from(value.to_vec())
			.map(Elum::Raw)
			.map_err(|_| ())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use alloc::vec::Vec;
	use codec::{Decode, Encode};
	use core::convert::TryInto;
	use frame_support::{traits::ConstU32, BoundedVec};

	// Use a default Elum type with MAX_CAP = 1024
	pub type DefaultElement = Elum<ConstU32<1024>>;

	#[test]
	fn test_element_none_encode_decode() {
		let element: DefaultElement = DefaultElement::None;
		let encoded = element.encode();
		assert_eq!(encoded, vec![0]);
		let decoded = DefaultElement::decode(&mut &encoded[..]).expect("Decode None");
		assert_eq!(decoded, element);
		assert_eq!(element.as_ref(), &[] as &[u8]);
		assert!(element.is_none());
		assert_eq!(element.encoded_len(), encoded.len());
		assert!(element.validate().is_ok());
		assert!(element.as_raw().is_none());
		assert!(element.as_bool().is_none());
		assert!(element.as_u64().is_none());
		assert!(element.as_u128().is_none());
		assert!(element.as_hash().is_none());
		assert!(element.as_cid().is_none());
		assert!(element.as_token().is_none());
	}

	#[test]
	fn test_element_raw_encode_decode() {
		let raw_data: Vec<u8> = vec![1, 2, 3, 4, 5];
		let bounded: BoundedVec<u8, ConstU32<1024>> = raw_data.clone().try_into().unwrap();
		let element: DefaultElement = DefaultElement::Raw(bounded.clone());
		let encoded = element.encode();
		let decoded = DefaultElement::decode(&mut &encoded[..]).expect("Decode Raw");
		assert_eq!(decoded, element);
		assert_eq!(element.as_raw(), Some(&raw_data[..]));
		assert_eq!(element.as_ref(), &raw_data[..]);
		assert!(element.is_raw());
		assert_eq!(element.encoded_len(), encoded.len());
		assert!(element.validate().is_ok());
	}

	#[test]
	fn test_element_bool_encode_decode() {
		let element: DefaultElement = DefaultElement::from_bool(true);
		let encoded = element.encode();
		let decoded = DefaultElement::decode(&mut &encoded[..]).expect("Decode Bool");
		assert_eq!(decoded, element);
		assert_eq!(element.as_bool(), Some(true));
		assert_eq!(element.as_ref(), &[1u8][..]);
		assert!(element.is_bool());
		assert_eq!(element.encoded_len(), encoded.len());
		assert!(element.validate().is_ok());
	}

	#[test]
	fn test_numeric_is_helpers() {
		let u64_elem = DefaultElement::from_u64(9);
		assert!(u64_elem.is_u64());
		assert_eq!(u64_elem.as_u64(), Some(9));
		assert!(u64_elem.validate().is_ok());

		let u128_elem = DefaultElement::from_u128(11);
		assert!(u128_elem.is_u128());
		assert_eq!(u128_elem.as_u128(), Some(11));
		assert!(u128_elem.validate().is_ok());
	}

	#[test]
	fn test_element_identifier_encode_decode() {
		let digest: Vec<u8> = vec![0xAB; 32];
		let ss58_id =
			Ss58Identifier::to_encoded(digest.clone(), 100, 5, 1).expect("Ss58Identifier created");
		let element: DefaultElement = DefaultElement::Token(ss58_id.clone());
		let encoded = element.encode();
		let decoded = DefaultElement::decode(&mut &encoded[..]).expect("Decode Identifier");
		assert_eq!(decoded, element);
		assert_eq!(element.as_token(), Some(&ss58_id));
		assert_eq!(element.as_ref(), ss58_id.as_bytes());
		assert!(element.is_token());
		assert_eq!(element.encoded_len(), encoded.len());
	}

	#[test]
	fn test_element_hash_encode_decode() {
		let digest: [u8; 32] = [0xAA; 32];
		let element: DefaultElement = DefaultElement::Hash(digest);
		let encoded = element.encode();
		assert_eq!(encoded.len(), 33);
		let decoded = DefaultElement::decode(&mut &encoded[..]).expect("Decode Hash");
		assert_eq!(decoded, element);
		assert_eq!(element.as_hash(), Some(&digest));
		assert_eq!(element.as_ref(), &digest[..]);
		assert!(element.is_hash());
		assert_eq!(element.encoded_len(), encoded.len());
		assert!(element.validate().is_ok());
	}

	#[test]
	fn test_element_cid_encode_decode() {
		let cid_vec = vec![0x55; 64];
		let cid: BoundedVec<u8, ConstU32<1024>> = cid_vec.clone().try_into().unwrap();
		let element: DefaultElement = DefaultElement::CID(cid.clone());
		let encoded = element.encode();
		let decoded = DefaultElement::decode(&mut &encoded[..]).expect("Decode CID");
		assert_eq!(decoded, element);
		assert_eq!(element.as_cid(), Some(&cid_vec[..]));
		assert_eq!(element.as_ref(), &cid_vec[..]);
		assert!(element.is_cid());
		assert_eq!(element.encoded_len(), encoded.len());
		assert!(element.validate().is_ok());
	}

	#[test]
	fn test_as_ref_for_all_variants() {
		let none_elem: DefaultElement = DefaultElement::None;
		assert_eq!(none_elem.as_ref(), &[] as &[u8]);

		let raw_data: Vec<u8> = vec![10, 20, 30];
		let bounded: BoundedVec<u8, ConstU32<1024>> = raw_data.clone().try_into().unwrap();
		let raw_elem: DefaultElement = DefaultElement::Raw(bounded);
		assert_eq!(raw_elem.as_ref(), &raw_data[..]);

		let hash_elem: DefaultElement = DefaultElement::Hash([1; 32]);
		assert_eq!(hash_elem.as_ref(), &[1; 32][..]);

		let cid_bytes = vec![2; 64];
		let cid_bounded: BoundedVec<u8, ConstU32<1024>> = cid_bytes.clone().try_into().unwrap();
		let cid_elem: DefaultElement = DefaultElement::CID(cid_bounded);
		assert_eq!(cid_elem.as_ref(), &cid_bytes[..]);

		let ss58_id = Ss58Identifier::to_encoded(vec![0xAB; 32], 100, 5, 1).unwrap();
		let id_elem: DefaultElement = DefaultElement::Token(ss58_id.clone());
		assert_eq!(id_elem.as_token(), Some(&ss58_id));
		assert_eq!(id_elem.as_ref(), ss58_id.as_bytes());
	}

	#[test]
	fn test_decode_unknown_variant() {
		let invalid_encoded: Vec<u8> = vec![255];
		let result = DefaultElement::decode(&mut &invalid_encoded[..]);
		assert!(result.is_err());
	}

	#[test]
	fn test_decode_incomplete_raw() {
		let raw_data: Vec<u8> = vec![1, 2, 3, 4, 5];
		let bounded: BoundedVec<u8, ConstU32<1024>> = raw_data.clone().try_into().unwrap();
		let mut encoded = DefaultElement::Raw(bounded.clone()).encode();
		encoded.pop();
		assert!(DefaultElement::decode(&mut &encoded[..]).is_err());
	}

	#[test]
	fn test_decode_incomplete_hash() {
		let mut encoded = DefaultElement::Hash([0xAA; 32]).encode();
		encoded.truncate(encoded.len() - 5);
		assert!(DefaultElement::decode(&mut &encoded[..]).is_err());
	}

	#[test]
	fn test_decode_incomplete_cid() {
		let cid: BoundedVec<u8, ConstU32<1024>> = vec![0x55; 64].try_into().unwrap();
		let mut encoded = DefaultElement::CID(cid).encode();
		encoded.truncate(encoded.len() - 10);
		assert!(DefaultElement::decode(&mut &encoded[..]).is_err());
	}

	#[test]
	fn test_boundedvec_too_long() {
		let long_vec: Vec<u8> = vec![0u8; 1025];
		let bounded: Result<BoundedVec<u8, ConstU32<1024>>, _> = long_vec.try_into();
		assert!(bounded.is_err());
	}

	#[test]
	fn test_integration_with_container() {
		#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
		struct Container {
			pub id: u32,
			pub element: DefaultElement,
		}

		let element = DefaultElement::Raw(vec![42, 43].try_into().unwrap());
		let container = Container { id: 7, element };
		let decoded = Container::decode(&mut &container.encode()[..]).unwrap();
		assert_eq!(decoded, container);
	}

	#[test]
	fn test_default_is_none() {
		let d: DefaultElement = Default::default();
		assert_eq!(d, DefaultElement::None);
	}

	#[test]
	fn test_discriminant_bytes() {
		assert_eq!(DefaultElement::None.encode()[0], 0);
		assert_eq!(DefaultElement::Raw(vec![].try_into().unwrap()).encode()[0], 1);
		assert_eq!(DefaultElement::from_bool(true).encode()[0], 2);
		assert_eq!(DefaultElement::from_u64(7).encode()[0], 3);
		assert_eq!(DefaultElement::from_u128(7).encode()[0], 4);
		assert_eq!(DefaultElement::Hash([0; 32]).encode()[0], 5);
		let ss58 = Ss58Identifier::to_encoded(vec![0; 32], 0, 0, 0).unwrap();
		assert_eq!(DefaultElement::Token(ss58).encode()[0], 6);
		let cid: BoundedVec<u8, ConstU32<1024>> = vec![0; 64].try_into().unwrap();
		assert_eq!(DefaultElement::CID(cid).encode()[0], 7);
	}

	#[test]
	fn test_max_encoded_len_runtime() {
		// With MAX_CAP=1024, the Raw variant is worst-case:
		// 1 byte tag + 2-byte compact length for 1024 + 1024-byte payload = 1027
		assert_eq!(DefaultElement::max_encoded_len() as usize, 1 + 2 + 1024);
	}

	#[test]
	fn test_compact_length_encoding() {
		// 63 bytes => one-byte compact (mode 0)
		let small = vec![0u8; 63].try_into().unwrap();
		let enc = DefaultElement::Raw(small).encode();
		assert_eq!(enc[1] & 0b11, 0b00);

		// 64 bytes => two-byte compact (mode 1)
		let big = vec![0u8; 64].try_into().unwrap();
		let enc = DefaultElement::Raw(big).encode();
		assert_eq!(enc[1] & 0b11, 0b01);
	}

	#[test]
	fn test_decode_empty_input() {
		let empty: [u8; 0] = [];
		assert!(DefaultElement::decode(&mut &empty[..]).is_err());
	}

	#[test]
	fn test_vec_of_elements_encode_decode() {
		let cid: BoundedVec<u8, ConstU32<1024>> = vec![2; 64].try_into().unwrap();
		let elems =
			vec![DefaultElement::None, DefaultElement::Hash([1; 32]), DefaultElement::CID(cid)];
		let decoded: Vec<DefaultElement> = Decode::decode(&mut &elems.encode()[..]).unwrap();
		assert_eq!(decoded, elems);
	}

	#[test]
	fn test_validate_catches_invalid_bool() {
		let invalid = vec![2, 2];
		let decoded =
			DefaultElement::decode(&mut &invalid[..]).expect("decoded despite invalid flag");
		assert!(decoded.validate().is_err());
	}

	#[test]
	fn test_try_from_vec_raw() {
		let data = vec![1u8; 8];
		let element = DefaultElement::try_from(data.clone()).expect("bounded raw");
		assert!(element.is_raw());
		assert_eq!(element.as_raw(), Some(&data[..]));
	}

	#[test]
	fn test_fuzz_raw_sizes() {
		for &len in &[0usize, 1, 10, 63, 64, 512, 1024] {
			let data = vec![0xFF; len];
			let bounded: BoundedVec<u8, ConstU32<1024>> = data.clone().try_into().unwrap();
			let elem = DefaultElement::Raw(bounded);
			let round = DefaultElement::decode(&mut &elem.encode()[..]).unwrap();
			assert_eq!(round, elem);
		}
	}
}
