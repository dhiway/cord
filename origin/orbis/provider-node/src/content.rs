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

//! Canonical Origin/Orbis stored-content addressing and bounds.

use std::{fmt, str::FromStr};

use cid::{multibase::Base, multihash::Multihash, CidGeneric};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

/// Raw CID multicodec required by the provider byte plane.
pub const RAW_CODEC: u64 = 0x55;
/// BLAKE2b-256 multihash code required by the provider byte plane.
pub const BLAKE2B_256_CODE: u64 = 0xb220;
/// Stored chunk size.
pub const CHUNK_BYTES: usize = 262_144;
/// Maximum stored chunks per object.
pub const MAX_CHUNKS: usize = 256;
/// Maximum stored object length.
pub const MAX_STORED_BYTES: u64 = 67_108_864;
/// Maximum verified range response.
pub const MAX_RANGE_BYTES: u64 = 4_194_304;
/// Maximum unacknowledged ingress chunks.
pub const INGRESS_WINDOW_CHUNKS: usize = 4;
/// Maximum unacknowledged ingress bytes.
pub const INGRESS_WINDOW_BYTES: usize = 1_048_576;
/// Maximum durable idempotency records retained by one streaming store.
pub const MAX_STREAMING_OPERATIONS: usize = 8_192;

macro_rules! fixed_identifier {
	($name:ident, $bytes:expr, $description:literal) => {
		#[doc = $description]
		#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
		pub struct $name([u8; $bytes]);

		impl $name {
			/// Construct an identifier from its exact binary representation.
			pub const fn from_bytes(bytes: [u8; $bytes]) -> Self {
				Self(bytes)
			}

			/// Parse exact lowercase hexadecimal without a prefix.
			pub fn parse(value: &str) -> Result<Self, ContentError> {
				if value.len() != $bytes * 2 || value.bytes().any(|byte| byte.is_ascii_uppercase())
				{
					return Err(ContentError::SchemaInvalid);
				}
				let decoded = hex::decode(value).map_err(|_| ContentError::SchemaInvalid)?;
				let bytes = decoded.try_into().map_err(|_| ContentError::SchemaInvalid)?;
				Ok(Self(bytes))
			}

			/// Return the fixed binary representation.
			pub const fn as_bytes(&self) -> &[u8; $bytes] {
				&self.0
			}
		}

		impl fmt::Display for $name {
			fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
				formatter.write_str(&hex::encode(self.0))
			}
		}

		impl Serialize for $name {
			fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
				serializer.serialize_str(&self.to_string())
			}
		}

		impl<'de> Deserialize<'de> for $name {
			fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
				let value = String::deserialize(deserializer)?;
				Self::parse(&value).map_err(D::Error::custom)
			}
		}
	};
}

fixed_identifier!(OperationId, 16, "Exact 128-bit idempotency operation identifier.");
fixed_identifier!(BucketId, 32, "Exact 256-bit canonical bucket identifier.");

/// Canonical CIDv1 base32lower/raw/BLAKE2b-256 address.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCid {
	text: String,
	digest: [u8; 32],
}

impl CanonicalCid {
	/// Parse an exact canonical textual CID.
	pub fn parse(value: &str) -> Result<Self, ContentError> {
		if !value.starts_with('b') || value.bytes().any(|byte| byte.is_ascii_uppercase()) {
			return Err(ContentError::SchemaInvalid);
		}
		let cid = CidGeneric::<32>::from_str(value).map_err(|_| ContentError::SchemaInvalid)?;
		if cid.version() != cid::Version::V1
			|| cid.codec() != RAW_CODEC
			|| cid.hash().code() != BLAKE2B_256_CODE
			|| cid.hash().size() != 32
		{
			return Err(ContentError::SchemaInvalid);
		}
		let canonical = cid
			.to_string_of_base(Base::Base32Lower)
			.map_err(|_| ContentError::SchemaInvalid)?;
		if canonical != value {
			return Err(ContentError::SchemaInvalid);
		}
		let digest = cid.hash().digest().try_into().map_err(|_| ContentError::SchemaInvalid)?;
		Ok(Self { text: canonical, digest })
	}

	/// Construct the canonical address for a BLAKE2b-256 digest.
	pub fn from_digest(digest: [u8; 32]) -> Self {
		let hash = Multihash::<32>::wrap(BLAKE2B_256_CODE, &digest)
			.expect("32-byte digest fits the fixed multihash");
		let cid = CidGeneric::<32>::new_v1(RAW_CODEC, hash);
		let text = cid.to_string_of_base(Base::Base32Lower).expect("CIDv1 supports base32lower");
		Self { text, digest }
	}

	/// Return the canonical textual address.
	pub fn as_str(&self) -> &str {
		&self.text
	}

	/// Return the complete-object digest carried by the CID.
	pub const fn digest(&self) -> [u8; 32] {
		self.digest
	}
}

impl fmt::Display for CanonicalCid {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.text)
	}
}

/// Stored-content contract failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ContentError {
	/// Input is malformed or uses a non-canonical schema representation.
	#[error("WIRE_SCHEMA_INVALID")]
	SchemaInvalid,
	/// Object exceeds the stored-byte or chunk bound.
	#[error("STORAGE_OBJECT_TOO_LARGE")]
	ObjectTooLarge,
	/// Durable provider operation recovery table reached its fixed bound.
	#[error("PROVIDER_RECOVERY_TABLE_FULL")]
	ProviderRecoveryTableFull,
	/// Chunk sequence is not contiguous.
	#[error("STORAGE_CHUNK_OUT_OF_ORDER")]
	ChunkOutOfOrder,
	/// A non-final chunk exceeds the fixed 256 KiB limit.
	#[error("STORAGE_CHUNK_TOO_LARGE")]
	ChunkTooLarge,
	/// Finalization was attempted with missing bytes or chunks.
	#[error("STORAGE_CHUNK_MISSING")]
	ChunkMissing,
	/// Reconstructed length differs from the descriptor.
	#[error("STORAGE_LENGTH_MISMATCH")]
	LengthMismatch,
	/// Complete bytes do not match the expected CID.
	#[error("STORAGE_CID_MISMATCH")]
	CidMismatch,
	/// Existing idempotency key was reused with changed input.
	#[error("STORAGE_IDEMPOTENCY_CONFLICT")]
	IdempotencyConflict,
	/// Requested range is invalid or exceeds 4 MiB.
	#[error("STORAGE_RANGE_INVALID")]
	RangeInvalid,
	/// Stored or staged bytes failed verification.
	#[error("STORAGE_INTEGRITY_FAILED")]
	IntegrityFailed,
	/// Operation or object does not exist.
	#[error("STORAGE_NOT_FOUND")]
	NotFound,
	/// Durable filesystem or journal operation failed.
	#[error("STORAGE_IO_FAILED: {0}")]
	Io(String),
}
