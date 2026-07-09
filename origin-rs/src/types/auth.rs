//! Helpers for building view authorizations.

use codec::Encode;
use origin_primitives::{authorization::Authorization, AccountId};
use sp_crypto_hashing::twox_128;

/// Default TTL anchor: use the reference block the node reports (latest).
pub const DEFAULT_TTL_BLOCKS: u32 = 50;

/// Build the authorization payload expected by Origin pallets:
/// twox_128(nonce || pallet || "::" || function || account || reference_block)
/// || account || reference_block_le.
pub fn build_view_payload(
	account: &AccountId,
	pallet: &str,
	function: &str,
	reference_block: u32,
) -> Vec<u8> {
	let nonce = reference_block.to_le_bytes(); // deterministic, block-based
	let account_bytes = account.encode();
	let mut preimage = Vec::with_capacity(
		nonce
			.len()
			.saturating_add(pallet.len())
			.saturating_add(function.len())
			.saturating_add(account_bytes.len())
			.saturating_add(core::mem::size_of::<u32>())
			.saturating_add(2),
	);
	preimage.extend_from_slice(&nonce);
	preimage.extend_from_slice(pallet.as_bytes());
	preimage.extend_from_slice(b"::");
	preimage.extend_from_slice(function.as_bytes());
	preimage.extend_from_slice(&account_bytes);
	preimage.extend_from_slice(&reference_block.to_le_bytes());

	let digest = twox_128(&preimage);
	let mut payload = Vec::with_capacity(digest.len() + account_bytes.len() + 4);
	payload.extend_from_slice(&digest);
	payload.extend_from_slice(&account_bytes);
	payload.extend_from_slice(&reference_block.to_le_bytes());
	payload
}

/// Build a full view authorization by signing the payload.
pub async fn build_authorization(
	signer: &dyn crate::client::signer::Signer,
	pallet: &str,
	function: &str,
	reference_block: u32,
) -> Authorization<origin_primitives::AccountId, Vec<u8>, origin_primitives::Signature> {
	let account = signer.account_id();
	let payload = build_view_payload(&account, pallet, function, reference_block);
	let signature = signer.sign_payload(&payload).await;
	Authorization { account, payload, signature }
}
