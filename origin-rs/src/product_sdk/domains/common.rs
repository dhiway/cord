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

use serde::{Deserialize, Serialize};

use crate::product_sdk::{Finality, NativeError, NativeErrorCode};

pub const DOMAIN_CONTRACT_VERSION: u16 = 1;
pub const MAX_PAGE_SIZE: u32 = 100;

pub type BlockNumber = u32;
pub type DomainResult<T> = Result<T, NativeError>;

pub trait Validate {
	fn validate(&self) -> DomainResult<()>;
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Hash32(String);

impl Hash32 {
	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = value.into();
		if !is_hash32(&value) {
			return Err(invalid("expected a 0x-prefixed 32-byte lowercase hex value"));
		}
		Ok(Self(value))
	}

	pub fn from_bytes(value: [u8; 32]) -> Self {
		Self(format!("0x{}", hex::encode(value)))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl Validate for Hash32 {
	fn validate(&self) -> DomainResult<()> {
		if is_hash32(&self.0) {
			Ok(())
		} else {
			Err(invalid("invalid 32-byte hash"))
		}
	}
}

macro_rules! hash_type {
	($name:ident) => {
		#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
		#[serde(transparent)]
		pub struct $name(pub Hash32);

		impl $name {
			pub fn new(value: impl Into<String>) -> DomainResult<Self> {
				Hash32::new(value).map(Self)
			}

			pub fn as_hash(&self) -> &Hash32 {
				&self.0
			}
		}

		impl Validate for $name {
			fn validate(&self) -> DomainResult<()> {
				self.0.validate()
			}
		}
	};
}

hash_type!(SchemaId);
hash_type!(AttestationId);
hash_type!(NameId);
hash_type!(SubjectCommitment);
hash_type!(PayloadCommitment);
hash_type!(StatusCommitment);
hash_type!(ContentCommitment);
hash_type!(ContentHash);
hash_type!(UniquenessCommitment);
hash_type!(RegistrationCommitment);
hash_type!(ReservationReference);
hash_type!(ContainerId);
hash_type!(AgreementId);
hash_type!(ChallengeId);
hash_type!(ProofCommitment);
hash_type!(ProviderReference);
hash_type!(DriveId);
hash_type!(BucketId);
hash_type!(ObjectId);

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct OperationId(String);

impl OperationId {
	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = value.into().to_lowercase();
		let bytes = value.strip_prefix("0x").ok_or_else(|| invalid("invalid operation id"))?;
		if bytes.len() != 32 || !bytes.bytes().all(|byte| byte.is_ascii_hexdigit()) {
			return Err(invalid("operation id must be a 16-byte 0x-prefixed value"));
		}
		Ok(Self(value))
	}

	pub fn from_bytes(value: [u8; 16]) -> Self {
		Self(format!("0x{}", hex::encode(value)))
	}

	pub fn as_bytes(&self) -> DomainResult<[u8; 16]> {
		hex::decode(&self.0[2..])
			.map_err(|_| invalid("invalid operation id"))?
			.try_into()
			.map_err(|_| invalid("invalid operation id"))
	}
}

impl Validate for OperationId {
	fn validate(&self) -> DomainResult<()> {
		Self::new(self.0.clone()).map(|_| ())
	}
}

/// Canonical native Entity identifier referenced by Orbis Names.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SubjectId(String);

impl SubjectId {
	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = value.into();
		origin_primitives::identifier::Ss58Identifier::try_from(value.clone())
			.map_err(|_| invalid("invalid native Entity SubjectId"))?;
		Ok(Self(value))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl Validate for SubjectId {
	fn validate(&self) -> DomainResult<()> {
		Self::new(self.0.clone()).map(|_| ())
	}
}

/// Commons resource storage-reservation identifier.
///
/// The runtime type is `u64`, but the SDK JSON contract uses a canonical decimal string so IDs
/// above JavaScript's safe-integer range cannot be rounded by mobile or web hosts.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ReservationId(String);

impl ReservationId {
	pub fn from_u64(value: u64) -> Self {
		Self(value.to_string())
	}

	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = value.into();
		let parsed = value
			.parse::<u64>()
			.map_err(|_| invalid("invalid decimal u64 reservation ID"))?;
		if parsed.to_string() != value {
			return Err(invalid("reservation ID must be a canonical decimal u64 string"));
		}
		Ok(Self(value))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}

	pub fn as_u64(&self) -> DomainResult<u64> {
		self.validate()?;
		self.0.parse().map_err(|_| invalid("invalid decimal u64 reservation ID"))
	}
}

impl Validate for ReservationId {
	fn validate(&self) -> DomainResult<()> {
		Self::new(self.0.clone()).map(|_| ())
	}
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct AccountId(String);

impl AccountId {
	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = value.into();
		if value.is_empty()
			|| value.len() > 128
			|| !value.bytes().all(|byte| byte.is_ascii_graphic())
		{
			return Err(invalid("invalid account identifier"));
		}
		Ok(Self(value))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl Validate for AccountId {
	fn validate(&self) -> DomainResult<()> {
		Self::new(self.0.clone()).map(|_| ())
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageRequest {
	pub cursor: Option<u32>,
	pub limit: u32,
}

impl PageRequest {
	pub fn new(cursor: Option<u32>, limit: u32) -> DomainResult<Self> {
		let page = Self { cursor, limit };
		page.validate()?;
		Ok(page)
	}
}

impl Validate for PageRequest {
	fn validate(&self) -> DomainResult<()> {
		if self.limit > MAX_PAGE_SIZE {
			return Err(invalid("page limit must be between 0 and 100"));
		}
		Ok(())
	}
}

/// A transport-neutral runtime API invocation pinned to one finalized Orbis block.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedQuery<Q> {
	pub version: u16,
	pub finalized_block_hash: Hash32,
	pub query: Q,
}

/// Versioned result decoded from one runtime API call at the requested finalized block.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedValue<T> {
	pub version: u16,
	pub finalized_block_hash: Hash32,
	pub value: Option<T>,
}

impl<T> FinalizedValue<T> {
	pub fn validate_envelope(&self) -> DomainResult<()> {
		ensure_version(self.version)?;
		self.finalized_block_hash.validate()
	}
}

/// Versioned bounded page decoded from one runtime API call.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedPage<T> {
	pub version: u16,
	pub finalized_block_hash: Hash32,
	pub items: Vec<T>,
	pub next_cursor: Option<u32>,
}

impl<T> FinalizedPage<T> {
	pub fn validate_envelope(&self) -> DomainResult<()> {
		ensure_version(self.version)?;
		self.finalized_block_hash.validate()?;
		if self.items.len() > MAX_PAGE_SIZE as usize {
			return Err(invalid("runtime response exceeded the 100-item page contract"));
		}
		if self.items.is_empty() && self.next_cursor.is_some() {
			return Err(invalid("empty runtime response cannot advance a cursor"));
		}
		Ok(())
	}
}

impl<Q: Validate> FinalizedQuery<Q> {
	pub fn new(finalized_block_hash: Hash32, query: Q) -> DomainResult<Self> {
		let request = Self { version: DOMAIN_CONTRACT_VERSION, finalized_block_hash, query };
		request.validate()?;
		Ok(request)
	}

	pub fn validate(&self) -> DomainResult<()> {
		ensure_version(self.version)?;
		self.finalized_block_hash.validate()?;
		self.query.validate()
	}
}

/// A command intent that a transport must submit and observe through finalization.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitAndFinalize<C> {
	pub version: u16,
	pub intent_id: String,
	pub signer: AccountId,
	pub finality: Finality,
	pub command: C,
}

impl<C: Validate> SubmitAndFinalize<C> {
	pub fn new(intent_id: impl Into<String>, signer: AccountId, command: C) -> DomainResult<Self> {
		let intent = Self {
			version: DOMAIN_CONTRACT_VERSION,
			intent_id: intent_id.into(),
			signer,
			finality: Finality::SubmitAndFinalize,
			command,
		};
		intent.validate()?;
		Ok(intent)
	}

	pub fn validate(&self) -> DomainResult<()> {
		ensure_version(self.version)?;
		if self.intent_id.len() < 16 || self.intent_id.len() > 128 {
			return Err(invalid("intent_id must contain between 16 and 128 bytes"));
		}
		if self.finality != Finality::SubmitAndFinalize {
			return Err(invalid("native command must use submit-and-finalize finality"));
		}
		self.signer.validate()?;
		self.command.validate()
	}
}

pub(crate) fn ensure_version(version: u16) -> DomainResult<()> {
	if version == DOMAIN_CONTRACT_VERSION {
		Ok(())
	} else {
		Err(NativeError::new(
			NativeErrorCode::UnsupportedRuntime,
			"unsupported native domain contract version",
		))
	}
}

pub(crate) fn ensure_bytes(value: &[u8], min: usize, max: usize, field: &str) -> DomainResult<()> {
	if value.len() < min || value.len() > max {
		Err(invalid(format!("{field} must contain between {min} and {max} bytes")))
	} else {
		Ok(())
	}
}

pub(crate) fn invalid(message: impl Into<String>) -> NativeError {
	NativeError::new(NativeErrorCode::InvalidInput, message)
}

pub(crate) fn expired(message: impl Into<String>) -> NativeError {
	NativeError::new(NativeErrorCode::Expired, message)
}

fn is_hash32(value: &str) -> bool {
	value.len() == 66
		&& value.starts_with("0x")
		&& value.as_bytes()[2..]
			.iter()
			.all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hashes_pages_and_command_finality_fail_closed() {
		assert!(Hash32::new(format!("0x{}", "ab".repeat(32))).is_ok());
		assert!(Hash32::new(format!("0x{}", "AB".repeat(32))).is_err());
		assert!(PageRequest::new(None, 100).is_ok());
		assert!(PageRequest::new(None, 0).is_ok());
		assert!(PageRequest::new(None, 101).is_err());
	}
}
