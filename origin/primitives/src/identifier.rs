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

// # CORD Identifier (Ss58Identifier)

// Note: This module is part of cord-origin-primitives and should be imported by all higher-level
// modules that need to interact with identifiers.

extern crate alloc;
use alloc::{format, string::String, vec::Vec};
use blake2::{Blake2b512, Digest};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::{convert::TryFrom, marker::PhantomData};
use frame_support::{ensure, traits::ConstU32, BoundedVec};
use hex;
use scale_decode::{visitor, DecodeAsType, IntoVisitor, TypeResolver};
use scale_info::TypeInfo;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Constant prefix used in checksum calculation.
const PREFIX: &[u8] = b"SS58PRE";

/// Identifier constant for Origin network (relay / origin chain)
pub const ORIGIN_IDENT: u16 = 0;
/// Identifier constant for CORD (non-origin / standalone)
pub const DEV_IDENT: u16 = 29;

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
	/// Blake2b-512 over the global PREFIX and the provided bytes, as raw bytes.
	pub fn ss58hash(data: &[u8]) -> Vec<u8> {
		let mut context = Blake2b512::new();
		context.update(PREFIX);
		context.update(data);
		context.finalize().to_vec()
	}

	/// Construct an Ss58Identifier from a 32-byte digest, a network id, and a pallet id.
	/// The resulting identifier is Base58-encoded.
	pub fn to_encoded<I>(data: I, nid: u16, pid: u16, ori: u8) -> Result<Self, IdentifierError>
	where
		I: AsRef<[u8]> + Into<Vec<u8>>,
	{
		let input = data.as_ref();
		ensure!(input.len() == 32, IdentifierError::InvalidDigestLength);

		let ident14: u16 = if ori == 0 { DEV_IDENT } else { ORIGIN_IDENT };
		let nid14: u16 = nid & 0b0011_1111_1111_1111;
		let pid14: u16 = pid & 0b0011_1111_1111_1111;
		let ori6: u8 = ori & 0b0011_1111;

		let mut preimage = Vec::with_capacity(2 + 32 + 1 + 2 + 2);
		Self::compact_encode_to(ident14, &mut preimage)?;
		preimage.extend_from_slice(input);
		preimage.push(ori6);
		Self::compact_encode_to(nid14, &mut preimage)?;
		Self::compact_encode_to(pid14, &mut preimage)?;

		// checksum
		let h = Self::ss58hash(&preimage);
		preimage.extend(&h[0..2]);

		// base58 encode & wrap
		Ok(Self(
			Vec::<u8>::from(bs58::encode(preimage).into_string())
				.try_into()
				.map_err(|_| IdentifierError::InvalidIdentifier)?,
		))
	}

	/// Decode the Ss58Identifier back into its structured components.
	pub fn to_decoded(&self) -> Result<DecodedIdentifier, IdentifierError> {
		let decoded =
			bs58::decode(&self.0).into_vec().map_err(|_| IdentifierError::InvalidFormat)?;

		let len = decoded.len();
		ensure!((38..=41).contains(&len), IdentifierError::InvalidIdentifierLength);

		let (body, chk) = decoded.split_at(len - 2);
		let expect = Self::ss58hash(body);
		ensure!(chk == &expect[..2], IdentifierError::InvalidChecksum);

		let mut off = 0usize;

		let (_ident, id_len) = Self::compact_decode(&body[off..])?;
		off += id_len;

		ensure!(body.len() >= off + 32, IdentifierError::InvalidIdentifierLength);
		let digest = &body[off..off + 32];
		off += 32;

		ensure!(body.len() > off, IdentifierError::InvalidIdentifierLength);
		let origin = match body[off] {
			0 => false,
			1 => true,
			_ => return Err(IdentifierError::InvalidMode),
		};
		off += 1;

		let (nid, nlen) = Self::compact_decode(&body[off..])?;
		off += nlen;

		let (pid, plen) = Self::compact_decode(&body[off..])?;
		off += plen;

		ensure!(off == body.len(), IdentifierError::InvalidIdentifierLength);

		Ok(DecodedIdentifier {
			origin,
			network: nid,
			pallet: pid,
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
			// Single-byte form: top two bits 00, payload in low 6 bits.
			0..=63 => Ok((data[0] as u16, 1)),
			64..=127 => {
				ensure!(data.len() >= 2, IdentifierError::InvalidPrefix);

				let b0 = data[0];
				let b1 = data[1];
				let mid = b0 & 0b0011_1111;
				let low = (b1 >> 6) & 0b0000_0011;
				let high = b1 & 0b0011_1111;
				let value = ((high as u16) << 8) | ((mid as u16) << 2) | (low as u16);
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
		if value > 0x3FFF {
			return Err(IdentifierError::CompactValueOutOfRange);
		}
		let x = value & 0b0011_1111_1111_1111;
		match x {
			0..=63 => buf.push(x as u8),
			64..=16_383 => {
				let first = ((x & 0b0000_0000_1111_1100) as u8) >> 2;
				let second = ((x >> 8) as u8) | ((x & 0b0000_0000_0000_0011) as u8) << 6;
				buf.extend_from_slice(&[first | 0b0100_0000, second]);
			},
			_ => return Err(IdentifierError::CompactValueOutOfRange),
		}
		Ok(())
	}

	/// Returns a reference to the underlying bytes of the identifier.
	pub fn as_bytes(&self) -> &[u8] {
		self.0.as_slice()
	}

	/// Render the identifier as a best-effort SS58 string.
	pub fn to_string_lossy(&self) -> String {
		let bytes = self.as_ref();
		if let Ok(text) = core::str::from_utf8(bytes) {
			if !text.trim().is_empty() {
				return text.to_owned();
			}
		}
		for offset in 0..bytes.len() {
			if let Ok(text) = core::str::from_utf8(&bytes[offset..]) {
				if text.chars().all(|ch| !ch.is_control()) {
					let trimmed = text.trim();
					if !trimmed.is_empty() {
						return trimmed.to_owned();
					}
				}
			}
		}
		format!("0x{}", hex::encode(bytes))
	}
}

impl TryFrom<Vec<u8>> for Ss58Identifier {
	type Error = IdentifierError;

	fn try_from(v: Vec<u8>) -> Result<Self, Self::Error> {
		let s = String::from_utf8(v).map_err(|_| IdentifierError::InvalidFormat)?;
		Ss58Identifier::try_from(s)
	}
}

impl TryFrom<String> for Ss58Identifier {
	type Error = IdentifierError;
	fn try_from(s: String) -> Result<Self, Self::Error> {
		let decoded = bs58::decode(&s).into_vec().map_err(|_| IdentifierError::InvalidFormat)?;
		if !(38..=41).contains(&decoded.len()) {
			return Err(IdentifierError::InvalidIdentifierLength);
		}
		let (body, chk) = decoded.split_at(decoded.len() - 2);
		let expect = Ss58Identifier::ss58hash(body);
		if chk != &expect[..2] {
			return Err(IdentifierError::InvalidChecksum);
		}
		let bytes: Vec<u8> = s.into_bytes();
		let bv: BoundedVec<u8, ConstU32<64>> =
			BoundedVec::try_from(bytes).map_err(|_| IdentifierError::InvalidIdentifier)?;
		Ok(Ss58Identifier(bv))
	}
}

impl AsRef<[u8]> for Ss58Identifier {
	fn as_ref(&self) -> &[u8] {
		self.0.as_slice()
	}
}

impl Serialize for Ss58Identifier {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&self.to_string_lossy())
	}
}

impl<'de> Deserialize<'de> for Ss58Identifier {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		Ss58Identifier::try_from(s).map_err(|_| serde::de::Error::custom("invalid Ss58 identifier"))
	}
}

pub struct Ss58IdentifierVisitor<R>(PhantomData<R>);

impl<R: TypeResolver> visitor::Visitor for Ss58IdentifierVisitor<R> {
	type Value<'scale, 'resolver> = Ss58Identifier;
	type Error = scale_decode::Error;
	type TypeResolver = R;

	fn unchecked_decode_as_type<'scale, 'resolver>(
		self,
		input: &mut &'scale [u8],
		type_id: <Self::TypeResolver as TypeResolver>::TypeId,
		types: &'resolver Self::TypeResolver,
	) -> visitor::DecodeAsTypeResult<Self, Result<Self::Value<'scale, 'resolver>, Self::Error>> {
		let result = visitor::decode_with_visitor(input, type_id, types, Vec::<u8>::into_visitor())
			.and_then(|bytes| {
				BoundedVec::<u8, ConstU32<64>>::try_from(bytes)
					.map(Ss58Identifier)
					.map_err(|_| {
						scale_decode::Error::from(codec::Error::from(
							"ss58 identifier exceeds 64 bytes",
						))
					})
			});
		visitor::DecodeAsTypeResult::Decoded(result)
	}
}

impl IntoVisitor for Ss58Identifier {
	type AnyVisitor<R: TypeResolver> = Ss58IdentifierVisitor<R>;

	fn into_visitor<R: TypeResolver>() -> Self::AnyVisitor<R> {
		Ss58IdentifierVisitor(PhantomData)
	}
}

/// Represents the structured components of an identifier after decoding.
#[derive(
	Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize, DecodeAsType,
)]
pub struct DecodedIdentifier {
	// Origin mode
	pub origin: bool,
	/// Network identifier.
	pub network: u16,
	/// Pallet (or module) identifier.
	pub pallet: u16,
	/// The digest (genesis hash) in hexadecimal format.
	pub genesis: String,
}

#[cfg(test)]
mod tests {
	use super::*;
	use alloc::vec::Vec;
	use core::convert::{TryFrom, TryInto};

	// Test constants
	const TEST_ORI: u8 = 1; // 0 = standalone, 1 = origin/para

	/// Helper: produce a valid 32‐byte digest.
	fn valid_digest() -> [u8; 32] {
		let s = "e673a596cb3f5a1a8143f194cc87dee9ab6072ffea2f6e1eb906f78c31f84a0b";
		let v = hex::decode(s).expect("valid hex");
		v.try_into().expect("slice with incorrect length")
	}

	#[test]
	fn encode_decode_roundtrip() {
		let digest = valid_digest();
		let nid: u16 = 100;
		let pid: u16 = 5;

		let identifier = Ss58Identifier::to_encoded(digest, nid, pid, TEST_ORI)
			.expect("encoding should succeed");
		let decoded = identifier.to_decoded().expect("decoding should succeed");

		assert_eq!(decoded.origin, TEST_ORI != 0, "origin should match flag");
		assert_eq!(decoded.network, nid, "network id should match");
		assert_eq!(decoded.pallet, pid, "pallet id should match");
		assert_eq!(decoded.genesis, format!("0x{}", hex::encode(digest)), "digest should match");
	}

	#[test]
	fn encode_decode_prints_values_origin_ident() {
		// run with: cargo +nightly test -p cord-primitives -Z unstable-options -- --show-output
		let digest = valid_digest();
		let nid: u16 = 100;
		let pid: u16 = 64;

		let id = Ss58Identifier::to_encoded(digest, nid, pid, TEST_ORI).expect("encode ok");
		let s = String::from_utf8(id.0.clone().into()).expect("utf8 base58 string");
		println!("Encoded (ORIGIN, ori=1): {}", s);

		let dec = Ss58Identifier::try_from(s)
			.expect("string→id ok")
			.to_decoded()
			.expect("decode ok");
		println!(
			"Decoded:\n  origin: {}\n  network: {}\n  pallet: {}\n  genesis: {}",
			dec.origin, dec.network, dec.pallet, dec.genesis
		);

		assert_eq!(dec.origin, true);
		assert_eq!(dec.network, nid);
		assert_eq!(dec.pallet, pid);
	}

	#[test]
	fn encode_decode_prints_values_cord_ident() {
		// run with: cargo +nightly test -p cord-primitives -Z unstable-options -- --show-output
		let digest = valid_digest();
		let nid: u16 = 200;
		let pid: u16 = 65;

		// origin = 0 → should encode with DEV_IDENT
		let id = Ss58Identifier::to_encoded(digest, nid, pid, 0).expect("encode ok");
		let s = String::from_utf8(id.0.clone().into()).expect("utf8 base58 string");
		println!("Encoded (CORD, ori=0): {}", s);

		let dec = Ss58Identifier::try_from(s)
			.expect("string→id ok")
			.to_decoded()
			.expect("decode ok");

		println!(
			"Decoded (CORD):\n  origin: {}\n  network: {}\n  pallet: {}\n  genesis: {}",
			dec.origin, dec.network, dec.pallet, dec.genesis
		);

		assert_eq!(dec.origin, false, "origin should be false for ori=0");
		assert_eq!(dec.network, nid);
		assert_eq!(dec.pallet, pid);
		assert_eq!(dec.genesis, format!("0x{}", hex::encode(digest)));
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
		raw[last_index] ^= 0xFF; // flip last bit => checksum wrong

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

		assert_eq!(dec.origin, TEST_ORI != 0);
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

		assert_eq!(dec.origin, TEST_ORI != 0);
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
		// origin=false, nid=0, pid=0
		let digest = [0u8; 32];
		let id = Ss58Identifier::to_encoded(digest, 0, 0, 0).unwrap();
		let dec = id.to_decoded().unwrap();
		assert_eq!(dec.origin, false);
		assert_eq!(dec.network, 0);
		assert_eq!(dec.pallet, 0);
		assert_eq!(dec.genesis, format!("0x{}", hex::encode(digest)));
	}

	#[test]
	fn decode_maximal_values() {
		const MAX: u16 = 0x3FFF; // 14-bit max
		let digest = [0xFF; 32];
		let id = Ss58Identifier::to_encoded(digest, MAX, MAX, 1).unwrap();
		let dec = id.to_decoded().unwrap();
		assert_eq!(dec.origin, true);
		assert_eq!(dec.network, MAX);
		assert_eq!(dec.pallet, MAX);
		assert_eq!(dec.genesis, format!("0x{}", hex::encode(digest)));
	}

	#[test]
	fn reject_invalid_mode() {
		// Build a valid id first
		let digest = valid_digest();
		let nid = 1u16;
		let pid = 2u16;
		let id = Ss58Identifier::to_encoded(digest, nid, pid, 1).unwrap();

		let mut raw = bs58::decode(&id.0).into_vec().unwrap();
		assert!(raw.len() >= 38);
		let body_len = raw.len() - 2;

		let body = &raw[..body_len];
		let (_ident_val, id_len) = Ss58Identifier::compact_decode(body).unwrap();

		let ori_off = id_len + 32;
		assert!(ori_off < body_len, "origin offset out of range");

		raw[ori_off] = 2;

		let new_chk = Ss58Identifier::ss58hash(&raw[..body_len]);
		raw[body_len..].copy_from_slice(&new_chk[..2]);

		let s = bs58::encode(&raw).into_string();
		let id2 = Ss58Identifier::try_from(s).expect("bounded container ok");
		match id2.to_decoded() {
			Err(IdentifierError::InvalidMode) => { /* pass */ },
			other => panic!("Expected InvalidMode, got {:?}", other),
		}
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
		// Use maximal values to force longest body → ensure bounded vec capacity is respected
		let digest = valid_digest();
		const MAX: u16 = 0x3FFF;
		let id = Ss58Identifier::to_encoded(digest, MAX, MAX, TEST_ORI).unwrap();
		let len = id.0.len();
		assert!(len <= 64, "encoded length {} exceeds capacity 64", len);
	}

	#[test]
	fn encoded_length_minimal() {
		let digest = [0u8; 32];
		let id = Ss58Identifier::to_encoded(digest, 0, 0, 0).unwrap();
		let len = id.0.len();
		// base58 inflates; minimal tends to be ~50+. If this ever regresses far below, flag it.
		assert!(len >= 38, "encoded length {} unexpectedly small", len);
		assert!(len <= 64, "encoded length {} exceeds capacity", len);
	}

	#[test]
	fn ident_is_cord_when_ori_is_zero() {
		// ori = 0 → DEV_IDENT on wire
		let digest = valid_digest();
		let nid: u16 = 100;
		let pid: u16 = 5;
		let id = Ss58Identifier::to_encoded(digest, nid, pid, 0).expect("encode ok");

		// decode base58 to raw bytes; split off checksum
		let raw = bs58::decode(&id.0).into_vec().expect("b58 decode");
		assert!((38..=41).contains(&raw.len()));
		let body_len = raw.len() - 2;
		let body = &raw[..body_len];

		// compact-decode ident at the start of body
		let (ident, _len) = Ss58Identifier::compact_decode(body).expect("compact decode ident");

		assert_eq!(ident, DEV_IDENT, "ident must be DEV_IDENT when ori=0");
	}

	#[test]
	fn ident_is_origin_when_ori_is_one() {
		// ori != 0 → ORIGIN_IDENT on wire
		let digest = valid_digest();
		let nid: u16 = 100;
		let pid: u16 = 5;
		let id = Ss58Identifier::to_encoded(digest, nid, pid, 1).expect("encode ok");

		let raw = bs58::decode(&id.0).into_vec().expect("b58 decode");
		assert!((38..=41).contains(&raw.len()));
		let body_len = raw.len() - 2;
		let body = &raw[..body_len];

		let (ident, _len) = Ss58Identifier::compact_decode(body).expect("compact decode ident");
		assert_eq!(ident, ORIGIN_IDENT, "ident must be ORIGIN_IDENT when ori!=0");
	}

	#[test]
	fn flipping_ori_changes_ident_and_checksum_but_not_digest_or_ids() {
		let digest = valid_digest();
		let nid: u16 = 3210;
		let pid: u16 = 789;

		let id_cord = Ss58Identifier::to_encoded(digest, nid, pid, 0).expect("encode ok");
		let id_origin = Ss58Identifier::to_encoded(digest, nid, pid, 1).expect("encode ok");

		// decode both to raw
		let raw_c = bs58::decode(&id_cord.0).into_vec().expect("b58 decode cord");
		let raw_o = bs58::decode(&id_origin.0).into_vec().expect("b58 decode origin");

		// split checksum
		let (body_c, _chk_c) = raw_c.split_at(raw_c.len() - 2);
		let (body_o, _chk_o) = raw_o.split_at(raw_o.len() - 2);

		// ident
		let (ident_c, len_c) = Ss58Identifier::compact_decode(body_c).expect("ident cord");
		let (ident_o, len_o) = Ss58Identifier::compact_decode(body_o).expect("ident origin");
		assert_eq!(ident_c, DEV_IDENT);
		assert_eq!(ident_o, ORIGIN_IDENT);

		// digest (must be the same)
		let dig_c = &body_c[len_c..len_c + 32];
		let dig_o = &body_o[len_o..len_o + 32];
		assert_eq!(dig_c, dig_o, "digest must be same regardless of ori");

		// origin (must differ)
		let ori_c = body_c[len_c + 32];
		let ori_o = body_o[len_o + 32];
		assert_eq!(ori_c, 0);
		assert_eq!(ori_o, 1);

		// nid/pid (must be same): re-decode
		let (nid_c, nlen_c) =
			Ss58Identifier::compact_decode(&body_c[len_c + 33..]).expect("nid cord");
		let (nid_o, nlen_o) =
			Ss58Identifier::compact_decode(&body_o[len_o + 33..]).expect("nid origin");
		assert_eq!(nid_c, nid_o);
		let pid_off_c = len_c + 33 + nlen_c;
		let pid_off_o = len_o + 33 + nlen_o;
		let (pid_c, _) = Ss58Identifier::compact_decode(&body_c[pid_off_c..]).expect("pid cord");
		let (pid_o, _) = Ss58Identifier::compact_decode(&body_o[pid_off_o..]).expect("pid origin");
		assert_eq!(pid_c, pid_o);
	}

	#[test]
	fn to_decoded_reports_bool_origin_and_correct_ident_for_each_flag() {
		let digest = valid_digest();
		let nid: u16 = 42;
		let pid: u16 = 7;

		for &(ori, expect_ident, expect_bool) in
			&[(0u8, DEV_IDENT, false), (1u8, ORIGIN_IDENT, true)]
		{
			let id = Ss58Identifier::to_encoded(digest, nid, pid, ori).expect("encode ok");
			// quick wire check: ident
			let raw = bs58::decode(&id.0).into_vec().expect("b58 decode");
			let (body, _) = raw.split_at(raw.len() - 2);
			let (ident, _) = Ss58Identifier::compact_decode(body).expect("ident");
			assert_eq!(ident, expect_ident);

			// semantic decode
			let dec = id.to_decoded().expect("decode ok");
			assert_eq!(dec.origin, expect_bool);
			assert_eq!(dec.network, nid);
			assert_eq!(dec.pallet, pid);
			assert_eq!(dec.genesis, format!("0x{}", hex::encode(digest)));
		}
	}

	#[test]
	fn ident_constants_are_within_compact14_domain() {
		assert!(DEV_IDENT <= 0x3FFF);
		assert!(ORIGIN_IDENT <= 0x3FFF);
	}
}
