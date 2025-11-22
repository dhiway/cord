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

// Common authorization structures shared by pallets.

use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use core::fmt;
use scale_info::TypeInfo;
use sp_io::hashing::twox_128;
use sp_runtime::RuntimeDebug;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

/// Shared authorization details for read-only query calls.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Authorization<AccountId, Payload, Signature> {
	pub account: AccountId,
	pub payload: Payload,
	pub signature: Signature,
}

impl<AccountId, Payload, Signature> fmt::Debug for Authorization<AccountId, Payload, Signature>
where
	Payload: AsRef<[u8]>,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Authorization")
			.field("payload_len", &self.payload.as_ref().len())
			.finish()
	}
}

/// Helper to compute the xxHash-128 signature hash used for replay protection.
pub fn authorization_signature_hash<AccountId, Signature>(
	account: &AccountId,
	payload: &[u8],
	signature: &Signature,
) -> [u8; 16]
where
	AccountId: Encode,
	Signature: Encode,
{
	let mut encoded: Vec<u8> = Vec::with_capacity(
		account
			.encoded_size()
			.saturating_add(payload.len())
			.saturating_add(signature.encoded_size()),
	);
	account.encode_to(&mut encoded);
	encoded.extend_from_slice(payload);
	signature.encode_to(&mut encoded);
	twox_128(&encoded)
}

/// Size of the `reference_block` trailer encoded into authorization payloads.
pub const AUTHORIZATION_VALID_UNTIL_BYTES: usize = core::mem::size_of::<u32>();

/// Extract the trailing reference block number from a payload.
pub fn extract_valid_until(payload: &[u8]) -> Option<u32> {
	if payload.len() < AUTHORIZATION_VALID_UNTIL_BYTES {
		return None;
	}
	let idx = payload.len() - AUTHORIZATION_VALID_UNTIL_BYTES;
	let bytes: [u8; AUTHORIZATION_VALID_UNTIL_BYTES] = payload[idx..].try_into().ok()?;
	Some(u32::from_le_bytes(bytes))
}

impl<AccountId, Payload, Signature> MaxEncodedLen for Authorization<AccountId, Payload, Signature>
where
	AccountId: MaxEncodedLen,
	Payload: MaxEncodedLen,
	Signature: MaxEncodedLen,
{
	fn max_encoded_len() -> usize {
		AccountId::max_encoded_len()
			.saturating_add(Payload::max_encoded_len())
			.saturating_add(Signature::max_encoded_len())
	}
}

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum AuthorizationError {
	Unauthorized,
	NotFound,
	InvalidInput,
	TooLarge,
	Expired,
	Internal,
	DecodeFailed,
}

/// Ensure an authorization produced at `reference_block` remains valid for the provided TTL.
///
/// `current_block`: current block number.
/// `reference_block`: block number at which the auth was minted / signed.
/// `max_ttl`: maximum allowed age in blocks.
pub fn ensure_authorization_ttl(
	current_block: u32,
	reference_block: u32,
	max_ttl: u32,
) -> Result<(), AuthorizationError> {
	let expires_at = reference_block.saturating_add(max_ttl);
	if current_block >= expires_at {
		Err(AuthorizationError::Expired)
	} else {
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use codec::Encode;

	#[test]
	fn signature_hash_depends_on_inputs_and_is_deterministic() {
		let account_a: u8 = 1;
		let account_b: u8 = 2;
		let payload: &[u8] = b"payload-bytes";
		let signature: &[u8] = b"sig";

		let hash_a = authorization_signature_hash(&account_a, payload, &signature);
		let hash_b = authorization_signature_hash(&account_b, payload, &signature);
		let hash_p = authorization_signature_hash(&account_a, b"different", &signature);

		assert_ne!(hash_a, hash_b, "changing account must change hash");
		assert_ne!(hash_a, hash_p, "changing payload must change hash");
		assert_eq!(hash_a, authorization_signature_hash(&account_a, payload, &signature));
	}

	#[test]
	fn extract_valid_until_reads_trailing_u32_le() {
		let mut payload: Vec<u8> = b"body".encode();
		let trailer: u32 = 0xA1B2C3D4;
		payload.extend_from_slice(&trailer.to_le_bytes());

		let got = extract_valid_until(&payload);
		assert_eq!(got, Some(trailer));
	}

	#[test]
	fn extract_valid_until_returns_none_when_payload_too_short() {
		assert_eq!(extract_valid_until(&[]), None);
		assert_eq!(extract_valid_until(&[1, 2, 3]), None);
	}

	#[test]
	fn ttl_allows_recent_authorization() {
		let current: u32 = 10;
		let reference: u32 = 5;
		let ttl: u32 = 10;
		assert!(ensure_authorization_ttl(current, reference, ttl).is_ok());
	}

	#[test]
	fn ttl_rejects_at_boundary_and_beyond() {
		let reference: u32 = 5;
		let ttl: u32 = 5;
		let expired_at_boundary = ensure_authorization_ttl(reference + ttl, reference, ttl);
		assert!(matches!(expired_at_boundary, Err(AuthorizationError::Expired)));
		let expired_later = ensure_authorization_ttl(reference + ttl + 1, reference, ttl);
		assert!(matches!(expired_later, Err(AuthorizationError::Expired)));
	}
}
