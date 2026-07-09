//! Types and helpers for the Origin blob-store pallets.

use codec::{Decode, Encode};
use origin_primitives::{authorization::Authorization, AccountId, Signature};
use sp_core::H256;

/// Payload that is signed by the blob owner and supplied by a Publisher.
///
/// This mirrors `pallet-blob-store::RegisterBlobAuthorization`.
///
/// Note: `reference_block` must be the final field so the chain can extract it as a trailing
/// `u32` for TTL checks.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct RegisterBlobAuthorization {
	pub domain: [u8; 8],
	pub publisher: AccountId,
	pub blob_id: H256,
	pub root_hash: H256,
	pub size_bytes: u64,
	pub encoding: u8,
	pub nonce: u64,
	pub reference_block: u32,
}

impl RegisterBlobAuthorization {
	pub const DOMAIN: [u8; 8] = *b"ORIGBLOB";

	pub fn new(
		publisher: AccountId,
		blob_id: H256,
		root_hash: H256,
		size_bytes: u64,
		encoding: u8,
		nonce: u64,
		reference_block: u32,
	) -> Self {
		Self {
			domain: Self::DOMAIN,
			publisher,
			blob_id,
			root_hash,
			size_bytes,
			encoding,
			nonce,
			reference_block,
		}
	}

	pub fn encode_payload(&self) -> Vec<u8> {
		self.encode()
	}
}

/// Build a blob registration authorization and sign it with the owner key.
pub async fn build_register_blob_authorization(
	signer: &dyn crate::client::signer::Signer,
	publisher: &AccountId,
	blob_id: H256,
	root_hash: H256,
	size_bytes: u64,
	encoding: u8,
	nonce: u64,
	reference_block: u32,
) -> Authorization<AccountId, Vec<u8>, Signature> {
	let account = signer.account_id();
	let payload = RegisterBlobAuthorization::new(
		publisher.clone(),
		blob_id,
		root_hash,
		size_bytes,
		encoding,
		nonce,
		reference_block,
	)
	.encode_payload();
	let signature = signer.sign_payload(&payload).await;
	Authorization { account, payload, signature }
}
