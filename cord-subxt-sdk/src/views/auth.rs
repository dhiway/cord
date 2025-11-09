use crate::error::Result;
use codec::Encode;
use serde::{Deserialize, Serialize};
use subxt::config::PolkadotConfig as C;
#[allow(unused_imports)]
use subxt::tx::Signer as _;

/// Supported signature schemes for view authorizations.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum SignatureScheme {
	Sr25519,
	Ed25519,
	Ecdsa,
}

/// Payload that authorises runtime view calls.
#[derive(Clone, Serialize, Deserialize)]
pub struct ViewAuthorization {
	pub account_ss58: String,
	pub account_id: subxt::utils::AccountId32,
	pub scheme: SignatureScheme,
	pub message: Vec<u8>,
	pub signature: Vec<u8>,
}

pub struct AuthorizationBuilder;

impl AuthorizationBuilder {
	/// Construct a random "cord:view" payload.
	pub fn random_message() -> Vec<u8> {
		let mut out = b"cord:view:v1|".to_vec();
		let mut rnd = [0u8; 48];
		let _ = getrandom::getrandom(&mut rnd);
		out.extend_from_slice(&rnd);
		out
	}

	/// Sign a payload using the provided Subxt signer.
	pub fn from_signer<S: subxt::tx::Signer<C>>(
		signer: &S,
		scheme: SignatureScheme,
		message: Option<&[u8]>,
	) -> Result<ViewAuthorization> {
		let msg = message.map(|m| m.to_vec()).unwrap_or_else(Self::random_message);
		let sig = signer.sign(&msg);
		Ok(ViewAuthorization {
			account_ss58: signer.account_id().to_string(),
			account_id: signer.account_id(),
			scheme,
			message: msg,
			signature: sig.encode(),
		})
	}
}
