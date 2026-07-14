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

extern crate alloc;
use alloc::{format, string::String, vec::Vec};
use blake2::{Blake2b512, Digest};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::convert::TryFrom;
use frame_support::{ensure, traits::ConstU32, BoundedVec};
use scale_info::TypeInfo;

/// Constant prefix used in checksum calculation.
const PREFIX: &[u8] = b"IDENTPRE";
/// Single-byte identifier version
pub const IDENTIFIER_VERSION: u8 = 2;
/// Default Relay
pub const DEFAULT_RELAY: u16 = 29;

/// Identifier errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierError {
	/// The identifier format is invalid.
	InvalidFormat,
	/// The prefix is invalid or unrecognized.
	InvalidPrefix,
	/// The identifier is not valid.
	InvalidIdentifier,
	/// The checksum validation failed.
	InvalidChecksum,
	/// The identifier length is not valid.
	InvalidIdentifierLength,
	/// The provided digest length is invalid. Expected 32 bytes.
	InvalidDigestLength,
	/// The value is out of the expected range for compact encoding.
	CompactValueOutOfRange,
	/// A compact‐encoded value used the wrong byte‐length form.
	InvalidCompactEncoding,
	/// The origin‐mode flag was not 0 or 1.
	InvalidMode,
}

/// The Ss58Identifier type is a persistent identifier built from a bounded vector of bytes.
/// The capacity (here, 64) must be chosen such that the final Base58-encoded value fits the system
/// constraints.
#[derive(
	Clone, Eq, PartialEq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug,
)]
pub struct Ss58Identifier(pub(crate) BoundedVec<u8, ConstU32<64>>);

impl Ss58Identifier {
	/// Compute the ss58 hash: Blake2b512 over PREFIX concatenated with the provided data.
	fn ss58hash(data: &[u8]) -> [u8; 2] {
		let mut hasher = Blake2b512::new();
		hasher.update(PREFIX);
		hasher.update(data);
		let digest = hasher.finalize();
		let mut checksum = [0u8; 2];
		checksum.copy_from_slice(&digest[..2]);
		checksum
	}

	/// Construct an Ss58Identifier from a 32-byte digest, a network id, and a pallet id.
	/// The resulting identifier is Base58-encoded.
	pub fn to_encoded<I>(input: I, nid: u16, pid: u16, ori: u8) -> Result<Self, IdentifierError>
	where
		I: AsRef<[u8]> + Into<Vec<u8>>,
	{
		let data = input.as_ref();
		ensure!(data.len() == 32, IdentifierError::InvalidDigestLength);

		let mut buffer = Vec::with_capacity(42);
		buffer.push(IDENTIFIER_VERSION);
		Self::compact_encode_to(DEFAULT_RELAY & 0x3FFF, &mut buffer)?;
		buffer.push(ori);
		Self::compact_encode_to(nid & 0x3FFF, &mut buffer)?;
		buffer.extend_from_slice(data);
		Self::compact_encode_to(pid & 0x3FFF, &mut buffer)?;
		buffer.extend_from_slice(&Self::ss58hash(&buffer));

		let bs58_bytes = bs58::encode(&buffer).into_vec();
		let bv = BoundedVec::<u8, ConstU32<64>>::try_from(bs58_bytes)
			.map_err(|_| IdentifierError::InvalidIdentifier)?;

		Ok(Ss58Identifier(bv))
	}

	/// Decode the Ss58Identifier back into its structured components.
	pub fn to_decoded(&self) -> Result<DecodedIdentifier, IdentifierError> {
		let decoded =
			bs58::decode(&self.0).into_vec().map_err(|_| IdentifierError::InvalidFormat)?;

		let len = decoded.len();
		ensure!(len >= 38 && len <= 42, IdentifierError::InvalidIdentifierLength);

		let checksum_start = len - 2;
		let provided = &decoded[checksum_start..];
		let expected = Self::ss58hash(&decoded[..checksum_start]);
		ensure!(provided == expected, IdentifierError::InvalidChecksum);
		let mut offset = 0;
		let data = &decoded[..checksum_start];
		let version = data[0];
		offset += 1;
		let (prefix, rid_len) = Self::compact_decode(&data[offset..])?;
		offset += rid_len;
		let origin = data[offset];
		ensure!(origin == 0 || origin == 1, IdentifierError::InvalidMode);
		offset += 1;

		let (network, nid_len) = Self::compact_decode(&data[offset..])?;
		offset += nid_len;
		ensure!(data.len() >= offset + 32, IdentifierError::InvalidIdentifierLength);

		let digest = data[offset..offset + 32].to_vec();
		offset += 32;
		let (pallet, _) = Self::compact_decode(&data[offset..])?;

		Ok(DecodedIdentifier {
			version,
			prefix,
			origin,
			network,
			pallet,
			genesis: format!("0x{}", hex::encode(digest)),
		})
	}

	/// Decodes a compact-encoded u16 value from the provided data slice.
	/// Returns the decoded value and the number of bytes consumed.
	fn compact_decode(data: &[u8]) -> Result<(u16, usize), IdentifierError> {
		if data.is_empty() {
			return Err(IdentifierError::InvalidPrefix);
		}
		match data[0] {
			0..=63 => Ok((data[0] as u16, 1)),
			64..=127 => {
				ensure!(data.len() >= 2, IdentifierError::InvalidPrefix);
				let mid = data[0] & 0b0011_1111;
				let low = data[1] >> 6;
				let high = data[1] & 0b0011_1111;
				let value = ((high as u16) << 8) | ((mid as u16) << 2) | (low as u16);
				// Must use two‐byte form only for values >= 64
				if value < 64 || value > 0x3FFF {
					return Err(IdentifierError::InvalidCompactEncoding);
				}
				Ok((value, 2))
			},
			_ => Err(IdentifierError::InvalidPrefix),
		}
	}

	/// Compactly encodes a u16 value into 1 or 2 bytes.
	fn compact_encode_to(value: u16, buf: &mut Vec<u8>) -> Result<(), IdentifierError> {
		match value {
			0..=63 => buf.push(value as u8),
			64..=16_383 => {
				let first = (((value & 0b0000_0000_1111_1100) >> 2) as u8) | 0b0100_0000;
				let second = ((value >> 8) as u8) | (((value & 0b11) as u8) << 6);
				buf.push(first);
				buf.push(second);
			},
			_ => return Err(IdentifierError::CompactValueOutOfRange),
		}
		Ok(())
	}

	/// Returns a reference to the underlying bytes of the identifier.
	pub fn as_bytes(&self) -> &[u8] {
		self.0.as_slice()
	}
}

impl TryFrom<Vec<u8>> for Ss58Identifier {
	type Error = IdentifierError;
	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		let bounded = BoundedVec::<u8, ConstU32<64>>::try_from(value)
			.map_err(|_| IdentifierError::InvalidIdentifierLength)?;
		let identifier = Ss58Identifier(bounded);
		// Validate by attempting to decode.
		identifier.to_decoded()?;
		Ok(identifier)
	}
}

impl TryFrom<String> for Ss58Identifier {
	type Error = IdentifierError;
	fn try_from(s: String) -> Result<Self, Self::Error> {
		if s.len() > 64 {
			return Err(IdentifierError::InvalidIdentifierLength);
		}
		Ss58Identifier::try_from(s.into_bytes())
	}
}

impl AsRef<[u8]> for Ss58Identifier {
	fn as_ref(&self) -> &[u8] {
		self.0.as_slice()
	}
}

/// Represents the structured components of an identifier after decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedIdentifier {
	// SS58 identifier version
	pub version: u8,
	// Origin mode - 0 = false, 1 = true
	pub origin: u8,
	/// Network identifier.
	pub network: u16,
	/// Pallet (or module) identifier.
	pub pallet: u16,
	// SS58 prefix from the runtime
	pub prefix: u16,
	/// The digest (genesis hash) in hexadecimal format.
	pub genesis: String,
}

#[cfg(test)]
mod tests {
	use super::*;
	use alloc::vec::Vec;
	use core::convert::TryFrom;

	// Test constants for SS58 prefix and origin‐mode:
	const TEST_RID: u16 = 29;
	const TEST_ORI: u8 = 1; // 0 = standalone, 1 = parachain

	/// Helper: produce a valid 32‐byte digest.
	fn valid_digest() -> [u8; 32] {
		[0xAB; 32]
	}

	#[test]
	fn encode_decode_roundtrip() {
		let digest = valid_digest();
		let nid: u16 = 100;
		let pid: u16 = 5;
		let identifier = Ss58Identifier::to_encoded(digest.clone(), nid, pid, TEST_ORI)
			.expect("encoding should succeed");
		let decoded = identifier.to_decoded().expect("decoding should succeed");

		// check all fields
		assert_eq!(decoded.version, IDENTIFIER_VERSION, "version should match constant");
		assert_eq!(decoded.prefix, TEST_RID, "SS58 prefix should match");
		assert_eq!(decoded.origin, TEST_ORI, "origin-mode should match");
		assert_eq!(decoded.network, nid, "network id should match");
		assert_eq!(decoded.pallet, pid, "pallet id should match");
		assert_eq!(decoded.genesis, format!("0x{}", hex::encode(digest)), "digest should match");
	}

	#[test]
	fn fails_invalid_digest_length() {
		let short = vec![0u8; 31];
		let nid = 1u16;
		let pid = 2u16;
		let result = Ss58Identifier::to_encoded(short, nid, pid, TEST_ORI);
		assert!(result.is_err(), "should reject non-32-byte digest");
	}

	#[test]
	fn fails_tampered_checksum() {
		let digest = valid_digest();
		let nid = 1u16;
		let pid = 2u16;
		let id = Ss58Identifier::to_encoded(digest, nid, pid, TEST_ORI).expect("encode ok");

		let mut raw = bs58::decode(&id.0).into_vec().expect("base58 decode");
		let last_index = raw.len() - 1;
		raw[last_index] ^= 0xFF;

		// Re-encode and attempt to parse
		let tampered = bs58::encode(&raw).into_string();
		assert!(Ss58Identifier::try_from(tampered).is_err(), "tampered checksum must fail");
	}

	#[test]
	fn try_from_vec_success() {
		let digest = valid_digest();
		let nid = 10u16;
		let pid = 20u16;
		let id = Ss58Identifier::to_encoded(digest, nid, pid, TEST_ORI).expect("encode ok");
		let raw: Vec<u8> = id.0.clone().into();
		let id2 = Ss58Identifier::try_from(raw).expect("vec→id ok");
		let dec = id2.to_decoded().expect("decode ok");

		assert_eq!(dec.version, IDENTIFIER_VERSION, "version should match constant");
		assert_eq!(dec.prefix, TEST_RID);
		assert_eq!(dec.origin, TEST_ORI);
		assert_eq!(dec.network, nid);
		assert_eq!(dec.pallet, pid);
	}

	#[test]
	fn try_from_string_success() {
		let digest = valid_digest();
		let nid = 11u16;
		let pid = 22u16;
		let id = Ss58Identifier::to_encoded(digest, nid, pid, TEST_ORI).expect("encode ok");
		let s = String::from_utf8(id.0.clone().into()).expect("utf8 base58");
		let id2 = Ss58Identifier::try_from(s).expect("string→id ok");
		let dec = id2.to_decoded().expect("decode ok");

		assert_eq!(dec.version, IDENTIFIER_VERSION, "version should match constant");
		assert_eq!(dec.prefix, TEST_RID);
		assert_eq!(dec.origin, TEST_ORI);
		assert_eq!(dec.network, nid);
		assert_eq!(dec.pallet, pid);
	}

	#[test]
	fn compact_roundtrip_boundaries() {
		for &v in &[63u16, 64u16, 16_383u16, 16_384u16] {
			let mut buf = Vec::new();
			let res = Ss58Identifier::compact_encode_to(v, &mut buf);

			if v <= 16_383 {
				res.expect("compact encode should succeed");
				let (decoded, len) =
					Ss58Identifier::compact_decode(&buf).expect("compact decode should succeed");
				assert_eq!(decoded, v);
				assert_eq!(len, buf.len());
			} else {
				assert!(matches!(res, Err(IdentifierError::CompactValueOutOfRange)));
			}
		}
	}

	#[test]
	fn decode_minimal_values() {
		let digest = [0u8; 32];
		let id = Ss58Identifier::to_encoded(digest, 0, 0, 0).unwrap();
		let dec = id.to_decoded().unwrap();
		assert_eq!(dec.version, IDENTIFIER_VERSION, "version should match constant");
		assert_eq!(dec.prefix, 29);
		assert_eq!(dec.origin, 0);
		assert_eq!(dec.network, 0);
		assert_eq!(dec.pallet, 0);
	}

	#[test]
	fn decode_maximal_values() {
		const MAX: u16 = 0x3FFF;
		let digest = [0xFF; 32];
		let id = Ss58Identifier::to_encoded(digest, MAX, MAX, 1).unwrap();
		let dec = id.to_decoded().unwrap();
		assert_eq!(dec.version, IDENTIFIER_VERSION, "version should match constant");
		assert_eq!(dec.prefix, 29);
		assert_eq!(dec.origin, 1);
		assert_eq!(dec.network, MAX);
		assert_eq!(dec.pallet, MAX);
	}

	#[test]
	fn reject_invalid_mode() {
		let mut buf = Vec::new();
		Ss58Identifier::compact_encode_to(1, &mut buf).unwrap();
		buf.push(2); // bad ori
		Ss58Identifier::compact_encode_to(1, &mut buf).unwrap();
		buf.extend_from_slice(&[0u8; 32]);
		Ss58Identifier::compact_encode_to(1, &mut buf).unwrap();
		buf.extend_from_slice(&Ss58Identifier::ss58hash(&buf));
		let bad = bs58::encode(&buf).into_vec();
		assert!(Ss58Identifier::try_from(bad).is_err());
	}

	#[test]
	fn reject_truncated() {
		let digest = valid_digest();
		let id = Ss58Identifier::to_encoded(digest, 5, 5, 0).unwrap();
		let mut raw = bs58::decode(&id.0).into_vec().unwrap();
		raw.truncate(10);
		assert!(Ss58Identifier::try_from(raw).is_err());
	}

	#[test]
	fn reject_noncanonical_compact() {
		let bad = vec![0b0100_0010, 0b0000_0000];
		match Ss58Identifier::compact_decode(&bad) {
			Err(IdentifierError::InvalidCompactEncoding) => { /* pass */ },
			other => panic!("Expected InvalidCompactEncoding, got {:?}", other),
		}
	}
	#[test]
	fn encoded_length_within_bounds() {
		// Ensure that the Base58-encoded identifier never exceeds the BoundedVec capacity
		let digest = valid_digest();
		// use maximal values to force longest buffer
		const MAX: u16 = 0x3FFF;
		let id = Ss58Identifier::to_encoded(digest, MAX, MAX, TEST_ORI).unwrap();
		let len = id.0.len();
		assert!(len <= 64, "encoded length {} exceeds capacity 64", len);
	}

	#[test]
	fn encoded_length_minimal() {
		// Ensure the minimal identifier still produces a reasonable length
		let digest = [0u8; 32];
		let id = Ss58Identifier::to_encoded(digest, 0, 0, 0).unwrap();
		let len = id.0.len();
		assert!(len >= 50, "encoded length {} lower than expected minimal bound", len);
	}
}
