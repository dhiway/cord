// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod fungibles;
#[cfg(feature = "genesis")]
pub mod genesis;
pub mod labels;
pub mod members_notifier_subscriber;
pub mod pgas;
pub mod traits;

pub mod utils {
	use alloc::vec::Vec;
	use codec::MaxEncodedLen;
	use sp_core::U256;
	use sp_runtime::traits::{Get, TypedGet};

	/// Convert a type implementing `TypedGet` into a type implementing `Get`.
	pub struct TypedGetToGet<T: TypedGet>(core::marker::PhantomData<T>);
	impl<T: TypedGet> Get<<T as TypedGet>::Type> for TypedGetToGet<T> {
		fn get() -> T::Type {
			T::get()
		}
	}

	/// A u32 encoded in big-endian format.
	///
	/// Unlike SCALE's default little-endian encoding for primitives, this type uses big-endian
	/// encoding so that lexicographic ordering of encoded bytes matches numeric ordering.
	/// This is useful for storage keys where iteration order matters.
	#[derive(PartialEq, Eq, Clone, Copy, Debug)]
	pub struct BigEndianU32(pub u32);

	impl codec::Encode for BigEndianU32 {
		fn encode(&self) -> Vec<u8> {
			self.0.to_be_bytes().to_vec()
		}

		fn encode_to<T: codec::Output + ?Sized>(&self, dest: &mut T) {
			dest.write(&self.0.to_be_bytes());
		}

		fn size_hint(&self) -> usize {
			core::mem::size_of::<u32>()
		}
	}

	impl codec::Decode for BigEndianU32 {
		fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
			let mut bytes = [0u8; 4];
			input.read(&mut bytes)?;
			Ok(Self(u32::from_be_bytes(bytes)))
		}
	}

	impl MaxEncodedLen for BigEndianU32 {
		fn max_encoded_len() -> usize {
			core::mem::size_of::<u32>()
		}
	}

	impl codec::EncodeLike for BigEndianU32 {}
	impl codec::DecodeWithMemTracking for BigEndianU32 {}

	impl scale_info::TypeInfo for BigEndianU32 {
		type Identity = Self;

		fn type_info() -> scale_info::Type {
			scale_info::Type::new(
				// Path: the type name visible in metadata, making it clear this is not a regular
				// u32.
				scale_info::Path::new("BigEndianU32", module_path!()),
				// Type parameters: none, this is not a generic type.
				Vec::new(),
				// Type definition: a fixed-size array of 4 bytes, matching the encoded
				// representation.
				scale_info::TypeDefArray::new(4, scale_info::MetaType::new::<u8>()),
				// Documentation visible in metadata.
				Vec::from([
					"A u32 encoded in big-endian format for correct lexicographic ordering.",
				]),
			)
		}
	}

	impl From<u32> for BigEndianU32 {
		fn from(v: u32) -> Self {
			Self(v)
		}
	}

	impl From<BigEndianU32> for u32 {
		fn from(v: BigEndianU32) -> Self {
			v.0
		}
	}

	/// A u64 encoded in big-endian format.
	///
	/// Unlike SCALE's default little-endian encoding for primitives, this type uses big-endian
	/// encoding so that lexicographic ordering of encoded bytes matches numeric ordering.
	/// This is useful for storage keys where iteration order matters.
	#[derive(PartialEq, Eq, Clone, Copy, Debug)]
	pub struct BigEndianU64(pub u64);

	impl codec::Encode for BigEndianU64 {
		fn encode(&self) -> Vec<u8> {
			self.0.to_be_bytes().to_vec()
		}

		fn encode_to<T: codec::Output + ?Sized>(&self, dest: &mut T) {
			dest.write(&self.0.to_be_bytes());
		}

		fn size_hint(&self) -> usize {
			core::mem::size_of::<u64>()
		}
	}

	impl codec::Decode for BigEndianU64 {
		fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
			let mut bytes = [0u8; 8];
			input.read(&mut bytes)?;
			Ok(Self(u64::from_be_bytes(bytes)))
		}
	}

	impl MaxEncodedLen for BigEndianU64 {
		fn max_encoded_len() -> usize {
			core::mem::size_of::<u64>()
		}
	}

	impl codec::EncodeLike for BigEndianU64 {}
	impl codec::DecodeWithMemTracking for BigEndianU64 {}

	impl scale_info::TypeInfo for BigEndianU64 {
		type Identity = Self;

		fn type_info() -> scale_info::Type {
			scale_info::Type::new(
				scale_info::Path::new("BigEndianU64", module_path!()),
				Vec::new(),
				scale_info::TypeDefArray::new(8, scale_info::MetaType::new::<u8>()),
				Vec::from([
					"A u64 encoded in big-endian format for correct lexicographic ordering.",
				]),
			)
		}
	}

	impl From<u64> for BigEndianU64 {
		fn from(v: u64) -> Self {
			Self(v)
		}
	}

	impl From<BigEndianU64> for u64 {
		fn from(v: BigEndianU64) -> Self {
			v.0
		}
	}

	/// A `U256` encoded as 32 big-endian bytes.
	///
	/// Like [`BigEndianU32`] but for the 256-bit `U256` type: SCALE's default
	/// little-endian encoding for `U256` sorts the wrong way for storage
	/// iteration, so this wrapper re-encodes via `U256::to_big_endian`. The
	/// numeric ordering of the wrapped `U256` matches the lexicographic
	/// ordering of the 32 encoded bytes, which is what storage tries with
	/// `Identity`-hashed keys want.
	///
	/// All non-encoding behavior (`Ord`, conversions, arithmetic) is deferred
	/// to the inner `U256` — this wrapper purely changes the codec.
	#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Default)]
	pub struct BigEndianU256(pub U256);

	impl codec::Encode for BigEndianU256 {
		fn encode(&self) -> Vec<u8> {
			self.0.to_big_endian().to_vec()
		}

		fn encode_to<T: codec::Output + ?Sized>(&self, dest: &mut T) {
			dest.write(&self.0.to_big_endian());
		}

		fn size_hint(&self) -> usize {
			32
		}
	}

	impl codec::Decode for BigEndianU256 {
		fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
			let mut bytes = [0u8; 32];
			input.read(&mut bytes)?;
			Ok(Self(U256::from_big_endian(&bytes)))
		}
	}

	impl MaxEncodedLen for BigEndianU256 {
		fn max_encoded_len() -> usize {
			32
		}
	}

	impl codec::EncodeLike for BigEndianU256 {}
	impl codec::DecodeWithMemTracking for BigEndianU256 {}

	impl scale_info::TypeInfo for BigEndianU256 {
		type Identity = Self;

		fn type_info() -> scale_info::Type {
			scale_info::Type::new(
				scale_info::Path::new("BigEndianU256", module_path!()),
				Vec::new(),
				scale_info::TypeDefArray::new(32, scale_info::MetaType::new::<u8>()),
				Vec::from([
					"A U256 encoded in big-endian format for correct lexicographic ordering.",
				]),
			)
		}
	}

	impl From<U256> for BigEndianU256 {
		fn from(v: U256) -> Self {
			Self(v)
		}
	}

	impl From<BigEndianU256> for U256 {
		fn from(v: BigEndianU256) -> Self {
			v.0
		}
	}

	impl From<[u8; 32]> for BigEndianU256 {
		fn from(bytes: [u8; 32]) -> Self {
			Self(U256::from_big_endian(&bytes))
		}
	}

	/// Draw a random `u32` from the offchain random source.
	///
	/// Only callable from an offchain context (e.g. the offchain worker), where the random seed
	/// externality is available.
	pub fn ocw_random_u32() -> u32 {
		let seed = sp_io::offchain::random_seed();
		u32::from_le_bytes([seed[0], seed[1], seed[2], seed[3]])
	}

	#[cfg(test)]
	mod tests {
		use super::*;
		use codec::{Decode, Encode};

		#[test]
		fn big_endian_encoded_order_matches_numeric_order() {
			let values = [0u32, 1, 255, 256, 512, 65536, u32::MAX];
			let encoded = values.iter().map(|&v| BigEndianU32(v).encode()).collect::<Vec<_>>();

			// Encoded bytes must be sorted.
			for w in encoded.windows(2) {
				assert!(
					w[0] < w[1],
					"BigEndianU32 encoded order must match numeric order: {:?} < {:?}",
					w[0],
					w[1],
				);
			}
		}

		#[test]
		fn big_endian_u32_encodes_in_big_endian() {
			// 0x01020304 in big-endian should be [0x01, 0x02, 0x03, 0x04]
			let value = BigEndianU32(0x01020304);
			let encoded = value.encode();
			assert_eq!(encoded, vec![0x01, 0x02, 0x03, 0x04]);
		}

		#[test]
		fn big_endian_u32_decodes_from_big_endian() {
			let bytes = [0x01, 0x02, 0x03, 0x04];
			let decoded = BigEndianU32::decode(&mut &bytes[..]).unwrap();
			assert_eq!(decoded.0, 0x01020304);
		}

		#[test]
		fn big_endian_u32_lexicographic_order_matches_numeric_order() {
			// This is the key property: encoded bytes should sort in the same order as the numbers.
			// This is critical for storage iteration to return values in ascending numeric order.
			let values = [0u32, 1, 255, 256, 257, 65535, 65536, u32::MAX - 1, u32::MAX];
			let mut encoded: Vec<_> = values.iter().map(|&v| BigEndianU32(v).encode()).collect();
			let original_order = encoded.clone();

			// Sort lexicographically
			encoded.sort();

			// Should be unchanged because big-endian encoding preserves numeric order
			assert_eq!(
				encoded, original_order,
				"Lexicographic order of encoded BigEndianU32 must match numeric order"
			);
		}

		#[test]
		fn big_endian_u32_max_encoded_len() {
			assert_eq!(BigEndianU32::max_encoded_len(), 4);
		}

		#[test]
		fn le_u32_encoded_order_does_not_match_numeric_order() {
			// This demonstrates the bug that BigEndianU32 fixes.
			let a = 1u32.encode(); // [01, 00, 00, 00]
			let b = 256u32.encode(); // [00, 01, 00, 00]

			// Numerically 1 < 256, but LE-encoded 256 sorts before 1.
			assert!(b < a, "LE u32 encoding must sort 256 before 1 (the bug)");
		}

		#[test]
		fn encode_to_matches_encode() {
			for v in [0u32, 1, 256, u32::MAX] {
				let via_encode = BigEndianU32(v).encode();
				let mut via_encode_to = Vec::new();
				BigEndianU32(v).encode_to(&mut via_encode_to);
				assert_eq!(via_encode, via_encode_to);
			}
		}

		#[test]
		fn roundtrip() {
			for v in [0u32, 1, 255, 256, 65535, u32::MAX] {
				let encoded = BigEndianU32(v).encode();
				let decoded = BigEndianU32::decode(&mut &encoded[..]).unwrap();
				assert_eq!(decoded.0, v);
			}
		}

		#[test]
		fn big_endian_u64_roundtrip_and_order() {
			let values = [0u64, 1, 255, 256, u32::MAX as u64, u64::MAX - 1, u64::MAX];
			let encoded: Vec<_> = values.iter().map(|&v| BigEndianU64(v).encode()).collect();
			for w in encoded.windows(2) {
				assert!(w[0] < w[1], "BigEndianU64 must sort in numeric order");
			}
			for v in values {
				let bytes = BigEndianU64(v).encode();
				let decoded = BigEndianU64::decode(&mut &bytes[..]).unwrap();
				assert_eq!(decoded.0, v);
			}
			assert_eq!(BigEndianU64::max_encoded_len(), 8);
		}

		fn u256_values() -> [U256; 8] {
			[
				U256::zero(),
				U256::one(),
				U256::from(u64::MAX),
				U256::from(1u64) << 128,
				U256::from(1u64) << 200,
				U256::MAX - U256::one(),
				U256::MAX,
				U256::from(1234567890u64) * U256::from(987654321u64),
			]
		}

		#[test]
		fn big_endian_u256_roundtrip() {
			for v in u256_values() {
				let encoded = BigEndianU256(v).encode();
				let decoded = BigEndianU256::decode(&mut &encoded[..]).unwrap();
				assert_eq!(decoded.0, v);
			}
		}

		#[test]
		fn big_endian_u256_encodes_in_big_endian() {
			// 1u256 must encode as 0x00..0001 (MSB first).
			let encoded = BigEndianU256(U256::one()).encode();
			assert_eq!(encoded.len(), 32);
			let mut expected = vec![0u8; 32];
			expected[31] = 1;
			assert_eq!(encoded, expected);
		}

		#[test]
		fn big_endian_u256_lexicographic_order_matches_numeric_order() {
			let mut vs = u256_values();
			// Sort numerically.
			vs.sort();
			let encoded: Vec<_> = vs.iter().map(|v| BigEndianU256(*v).encode()).collect();
			let mut sorted = encoded.clone();
			sorted.sort();
			assert_eq!(
				encoded, sorted,
				"BigEndianU256 encoded order must match numeric order of the inner U256"
			);
		}

		#[test]
		fn big_endian_u256_max_encoded_len() {
			assert_eq!(BigEndianU256::max_encoded_len(), 32);
		}
	}
}
