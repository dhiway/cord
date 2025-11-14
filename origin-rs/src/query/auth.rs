use crate::error::{Error, Result};
use cord_primitives::view_api::{AuthorizationPayload, AuthorizationRequest, AUTHORIZATION_MAX_BYTES};
use serde::{Deserialize, Serialize};
use sp_core::{ecdsa, ed25519, hashing::twox_128, sr25519};
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
	pub payload: Vec<u8>,
	pub signature: Vec<u8>,
}

pub struct AuthorizationBuilder;

pub const DEFAULT_VIEW_AUTH_TTL: u32 = 30;

impl AuthorizationBuilder {
	/// Construct a random 48-byte nonce.
	pub fn random_nonce() -> Vec<u8> {
		let mut out = vec![0u8; 48];
		let _ = getrandom::getrandom(&mut out);
		out
	}

	/// Hash a pallet/view pair into a 16-byte context tag.
	pub fn view_context(pallet: &str, view: &str) -> [u8; 16] {
		let mut label = Vec::with_capacity(pallet.len() + view.len() + 2);
		label.extend_from_slice(pallet.as_bytes());
		label.extend_from_slice(b"::");
		label.extend_from_slice(view.as_bytes());
		twox_128(&label)
	}

	/// Default context used by legacy helpers.
	pub fn default_context() -> [u8; 16] {
		Self::view_context("cord", "view")
	}

	fn compose_payload(
		account: &subxt::utils::AccountId32,
		context: &[u8],
		nonce: &[u8],
		reference_block: u32,
	) -> Vec<u8> {
		let account_bytes: &[u8] = account.as_ref();
		let mut preimage = Vec::with_capacity(
			nonce.len() + context.len() + account_bytes.len() + core::mem::size_of::<u32>(),
		);
		preimage.extend_from_slice(nonce);
		preimage.extend_from_slice(context);
		preimage.extend_from_slice(account_bytes);
		preimage.extend_from_slice(&reference_block.to_le_bytes());
		let digest = twox_128(&preimage);
		let mut payload = Vec::with_capacity(digest.len() + account_bytes.len() + 4);
		payload.extend_from_slice(&digest);
		payload.extend_from_slice(account_bytes);
		payload.extend_from_slice(&reference_block.to_le_bytes());
		payload
	}

	fn sign_payload<S: subxt::tx::Signer<C>>(
		signer: &S,
		payload: Vec<u8>,
	) -> Result<Authorization> {
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
			payload,
			signature: sig_bytes,
		})
	}

	/// Generate a ready-to-use authorization request for a specific pallet view.
	pub fn generate_view_authorization<S: subxt::tx::Signer<C>>(
		signer: &S,
		context: &[u8; 16],
		reference_block: u32,
		nonce: Option<&[u8]>,
	) -> Result<AuthorizationRequest> {
		let account = signer.account_id();
		let nonce_vec = nonce.map(|n| n.to_vec()).unwrap_or_else(Self::random_nonce);
		let payload = Self::compose_payload(&account, context, &nonce_vec, reference_block);
		Self::sign_payload(signer, payload)?.as_request()
	}

	/// Legacy helper retained for compatibility; prefer [`generate_view_authorization`].
	#[allow(dead_code)]
	#[deprecated(note = "use generate_view_authorization with an explicit context")]
	pub fn from_signer<S: subxt::tx::Signer<C>>(
		signer: &S,
		reference_block: u32,
		nonce: Option<&[u8]>,
	) -> Result<Authorization> {
		let ctx = Self::default_context();
		let account = signer.account_id();
		let nonce_vec = nonce.map(|n| n.to_vec()).unwrap_or_else(Self::random_nonce);
		let payload = Self::compose_payload(&account, &ctx, &nonce_vec, reference_block);
		Self::sign_payload(signer, payload)
	}
}

impl Authorization {
	pub fn as_request(&self) -> Result<AuthorizationRequest> {
		let payload = AuthorizationPayload::try_from(self.payload.clone()).map_err(|_| {
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
