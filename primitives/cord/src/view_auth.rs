// Common view-authorization structures shared by pallets.

use alloc::vec::Vec;
use codec::{Decode, Encode};
use core::fmt;
use scale_info::TypeInfo;
use sp_io::hashing::blake2_128;

/// Shared authorization details for read-only view calls.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct ViewAuthorization<AccountId, Payload, Signature> {
	pub account: AccountId,
	pub payload: Payload,
	pub signature: Signature,
}

impl<AccountId, Payload, Signature> fmt::Debug for ViewAuthorization<AccountId, Payload, Signature>
where
	Payload: AsRef<[u8]>,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ViewAuthorization")
			.field("payload_len", &self.payload.as_ref().len())
			.finish()
	}
}

/// Helper to compute the Blake2-128 signature hash used for replay protection.
pub fn view_signature_hash<AccountId, Signature>(
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
