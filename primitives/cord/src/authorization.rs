// Common authorization structures shared by pallets.

use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use core::fmt;
use scale_info::TypeInfo;
use sp_io::hashing::blake2_128;

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

/// Helper to compute the Blake2-128 signature hash used for replay protection.
pub fn authorization_signature_hash<AccountId, Signature>(
	account: &AccountId,
	payload: &[u8],
	signature: &Signature,
) -> [u8; 16]
where
	AccountId: Encode,
	Signature: Encode,
{
	let mut encoded: Vec<u8> = account.encode();
	encoded.extend_from_slice(payload);
	encoded.extend(signature.encode());
	blake2_128(&encoded)
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
