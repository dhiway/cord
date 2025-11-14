use crate::error::{Error, Result};
use cord_primitives::{
	authorization::append_valid_until,
	view_api::{AuthorizationPayload, AuthorizationRequest, AUTHORIZATION_MAX_BYTES},
};
use serde::{Deserialize, Serialize};
use sp_core::{ecdsa, ed25519, sr25519};
use sp_runtime::AccountId32;
use std::convert::{TryFrom, TryInto};
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
pub struct Authorization {
	pub account_ss58: String,
	pub account_id: subxt::utils::AccountId32,
	pub scheme: SignatureScheme,
	pub message: Vec<u8>,
	pub signature: Vec<u8>,
}

pub struct AuthorizationBuilder;

pub const DEFAULT_VIEW_AUTH_TTL: u32 = 30;

impl AuthorizationBuilder {
	/// Construct a random "cord:view" payload.
	pub fn random_message() -> Vec<u8> {
		let mut out = b"cord:view:v1|".to_vec();
		let mut rnd = [0u8; 48];
		let _ = getrandom::getrandom(&mut rnd);
		out.extend_from_slice(&rnd);
		out
	}

	/// Sign a payload using the provided Subxt signer, inferring the signature scheme automatically.
	pub fn from_signer<S: subxt::tx::Signer<C>>(
		signer: &S,
		valid_until: u32,
		message: Option<&[u8]>,
	) -> Result<Authorization> {
		let msg = message.map(|m| m.to_vec()).unwrap_or_else(Self::random_message);
		let payload = append_valid_until(msg, valid_until);
		let sig = signer.sign(&payload);
		let (scheme, sig_bytes) = match sig {
			subxt::utils::MultiSignature::Ed25519(inner) => {
				(SignatureScheme::Ed25519, inner.as_ref().to_vec())
			},
			subxt::utils::MultiSignature::Sr25519(inner) => {
				(SignatureScheme::Sr25519, inner.as_ref().to_vec())
			},
			subxt::utils::MultiSignature::Ecdsa(inner) => {
				(SignatureScheme::Ecdsa, inner.as_ref().to_vec())
			},
		};
		Ok(Authorization {
			account_ss58: signer.account_id().to_string(),
			account_id: signer.account_id(),
			scheme,
			message: payload,
			signature: sig_bytes,
		})
	}
}

impl Authorization {
	pub fn as_request(&self) -> Result<AuthorizationRequest> {
		let payload = AuthorizationPayload::try_from(self.message.clone()).map_err(|_| {
			Error::Params(format!("view payload exceeds {} bytes", AUTHORIZATION_MAX_BYTES))
		})?;
		let account = AccountId32::new(*self.account_id.as_ref());
		let signature =
			match self.scheme {
				SignatureScheme::Sr25519 => {
					let raw: [u8; 64] =
						self.signature.as_slice().try_into().map_err(|_| {
							Error::Params("sr25519 signature must be 64 bytes".into())
						})?;
					cord_primitives::Signature::from(sr25519::Signature::from_raw(raw))
				},
				SignatureScheme::Ed25519 => {
					let raw: [u8; 64] =
						self.signature.as_slice().try_into().map_err(|_| {
							Error::Params("ed25519 signature must be 64 bytes".into())
						})?;
					cord_primitives::Signature::from(ed25519::Signature::from_raw(raw))
				},
				SignatureScheme::Ecdsa => {
					let raw: [u8; 65] =
						self.signature.as_slice().try_into().map_err(|_| {
							Error::Params("ecdsa signature must be 65 bytes".into())
						})?;
					cord_primitives::Signature::from(ecdsa::Signature::from_raw(raw))
				},
			};
		Ok(AuthorizationRequest { account, payload, signature })
	}
}
