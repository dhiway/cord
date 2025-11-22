use crate::params::config::OriginConfig;
use async_trait::async_trait;
use sp_core::{ecdsa, ed25519, sr25519, Pair};
use sp_runtime::{traits::IdentifyAccount, MultiSignature, MultiSigner};
use subxt::{
	tx::Signer as SubxtSigner,
	utils::{AccountId32, MultiSignature as SubxtMultiSignature},
};

/// Minimal signer abstraction that works for both on-chain extrinsics and off-chain
/// meta-transaction payloads.
#[async_trait]
pub trait OriginSigner: Send + Sync + Clone + 'static {
	fn account_id(&self) -> AccountId32;
	async fn sign_payload(&self, payload: &[u8]) -> MultiSignature;
}

/// Multi-crypto signer that mirrors Substrate's `MultiSigner` (sr25519, ed25519, ecdsa).
#[derive(Clone)]
pub enum MultiKeySigner {
	Sr25519(sr25519::Pair),
	Ed25519(ed25519::Pair),
	Ecdsa(ecdsa::Pair),
}

impl MultiKeySigner {
	pub fn from_sr25519(pair: sr25519::Pair) -> Self {
		Self::Sr25519(pair)
	}

	pub fn from_ed25519(pair: ed25519::Pair) -> Self {
		Self::Ed25519(pair)
	}

	pub fn from_ecdsa(pair: ecdsa::Pair) -> Self {
		Self::Ecdsa(pair)
	}

	pub fn from_seed(seed: &str, scheme: &str) -> Result<Self, crate::error::Error> {
		let scheme = if scheme.is_empty() { "sr25519" } else { scheme };
		match scheme {
			"sr25519" => Ok(Self::from_sr25519(
				sr25519::Pair::from_string(seed, None)
					.map_err(|e| crate::error::Error::Signer(format!("invalid seed: {e}")))?,
			)),
			"ed25519" => Ok(Self::from_ed25519(
				ed25519::Pair::from_string(seed, None)
					.map_err(|e| crate::error::Error::Signer(format!("invalid seed: {e}")))?,
			)),
			"ecdsa" => Ok(Self::from_ecdsa(
				ecdsa::Pair::from_string(seed, None)
					.map_err(|e| crate::error::Error::Signer(format!("invalid seed: {e}")))?,
			)),
			other => Err(crate::error::Error::Signer(format!(
				"unsupported key scheme '{other}', expected sr25519|ed25519|ecdsa"
			))),
		}
	}

	fn multisigner(&self) -> MultiSigner {
		match self {
			Self::Sr25519(p) => MultiSigner::from(p.public()),
			Self::Ed25519(p) => MultiSigner::from(p.public()),
			Self::Ecdsa(p) => MultiSigner::from(p.public()),
		}
	}
}

#[async_trait]
impl OriginSigner for MultiKeySigner {
	fn account_id(&self) -> AccountId32 {
		let account: sp_runtime::AccountId32 = self.multisigner().into_account();
		let bytes: [u8; 32] = *account.as_ref();
		AccountId32::from(bytes)
	}

	async fn sign_payload(&self, payload: &[u8]) -> MultiSignature {
		match self {
			Self::Sr25519(p) => MultiSignature::from(p.sign(payload)),
			Self::Ed25519(p) => MultiSignature::from(p.sign(payload)),
			Self::Ecdsa(p) => MultiSignature::from(p.sign(payload)),
		}
	}
}

/// Simple sr25519 keypair-backed signer for local testing.
#[derive(Clone)]
pub struct LocalSigner {
	pair: sr25519::Pair,
}

impl LocalSigner {
	pub fn from_sr25519_pair(pair: sr25519::Pair) -> Self {
		Self { pair }
	}

	pub fn from_seed(seed: &str) -> Result<Self, crate::error::Error> {
		let pair = sr25519::Pair::from_string(seed, None)
			.map_err(|e| crate::error::Error::Signer(format!("invalid seed: {e}")))?;
		Ok(Self { pair })
	}
}

#[async_trait]
impl OriginSigner for LocalSigner {
	fn account_id(&self) -> AccountId32 {
		AccountId32(self.pair.public().0)
	}

	async fn sign_payload(&self, payload: &[u8]) -> MultiSignature {
		let sig = self.pair.sign(payload);
		MultiSignature::from(sig)
	}
}

/// Explicit sr25519 signer wrapper (alias of LocalSigner for clarity).
pub type Sr25519Signer = LocalSigner;

/// Placeholder MetaTx signer; in a full implementation this can wrap DID/HSM signers.
#[derive(Clone)]
pub struct MetaTxSigner<S: OriginSigner> {
	inner: S,
}

impl<S: OriginSigner> MetaTxSigner<S> {
	pub fn new(inner: S) -> Self {
		Self { inner }
	}
}

#[async_trait]
impl<S: OriginSigner> OriginSigner for MetaTxSigner<S> {
	fn account_id(&self) -> AccountId32 {
		self.inner.account_id()
	}

	async fn sign_payload(&self, payload: &[u8]) -> MultiSignature {
		self.inner.sign_payload(payload).await
	}
}

/// Adapter to plug an [`OriginSigner`] into Subxt transaction flows.
#[derive(Clone)]
pub struct SubxtSignerAdapter<S: OriginSigner> {
	inner: S,
}

impl<S: OriginSigner> SubxtSignerAdapter<S> {
	pub fn new(inner: S) -> Self {
		Self { inner }
	}

	pub fn into_inner(self) -> S {
		self.inner
	}
}

impl<S: OriginSigner> SubxtSigner<OriginConfig> for SubxtSignerAdapter<S> {
	fn account_id(&self) -> AccountId32 {
		self.inner.account_id()
	}

	fn sign(&self, payload: &[u8]) -> subxt::utils::MultiSignature {
		// The async contract on OriginSigner lets us support HSMs/wallets later;
		// for now we opportunistically block on the current runtime.
		let handle = tokio::runtime::Handle::current();
		match handle.block_on(self.inner.sign_payload(payload)) {
			MultiSignature::Ed25519(sig) => SubxtMultiSignature::Ed25519(sig.0),
			MultiSignature::Sr25519(sig) => SubxtMultiSignature::Sr25519(sig.0),
			MultiSignature::Ecdsa(sig) => SubxtMultiSignature::Ecdsa(sig.0),
		}
	}
}
