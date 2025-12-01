use async_trait::async_trait;
use sp_core::{ecdsa, ed25519, sr25519, Pair};
use sp_runtime::{traits::IdentifyAccount, MultiSignature, MultiSigner};
use tokio::task;

use crate::types::{OriginAccount, OriginPair};

/// Generic signing interface for Origin SDK (async to allow HSM/wallet flows).
#[async_trait]
pub trait Signer: Send + Sync + 'static {
	fn account_id(&self) -> origin_primitives::AccountId;
	fn account_identifier(&self) -> MultiSigner;
	async fn sign_payload(&self, payload: &[u8]) -> MultiSignature;
}

/// Multi-crypto signer covering sr25519, ed25519, and ecdsa.
#[derive(Clone)]
pub enum MultiKeySigner {
	Sr25519(sr25519::Pair),
	Ed25519(ed25519::Pair),
	Ecdsa(ecdsa::Pair),
}

impl MultiKeySigner {
	#[allow(dead_code)]
	pub fn from_sr25519(pair: sr25519::Pair) -> Self {
		Self::Sr25519(pair)
	}

	#[allow(dead_code)]
	pub fn from_ed25519(pair: ed25519::Pair) -> Self {
		Self::Ed25519(pair)
	}

	#[allow(dead_code)]
	pub fn from_ecdsa(pair: ecdsa::Pair) -> Self {
		Self::Ecdsa(pair)
	}

	/// Build from a secret URI seed using sr25519 by default.
	pub fn from_seed(seed: &str) -> Result<Self, String> {
		Self::from_seed_with_scheme(seed, None)
	}

	/// Build from a secret URI seed; `scheme` may be "sr25519", "ed25519", or "ecdsa".
	pub fn from_seed_with_scheme(seed: &str, scheme: Option<&str>) -> Result<Self, String> {
		let scheme = scheme.unwrap_or("sr25519");
		match scheme {
			"sr25519" => sr25519::Pair::from_string(seed, None)
				.map(Self::Sr25519)
				.map_err(|e| format!("invalid seed: {e}")),
			"ed25519" => ed25519::Pair::from_string(seed, None)
				.map(Self::Ed25519)
				.map_err(|e| format!("invalid seed: {e}")),
			"ecdsa" => ecdsa::Pair::from_string(seed, None)
				.map(Self::Ecdsa)
				.map_err(|e| format!("invalid seed: {e}")),
			other => Err(format!("unsupported key scheme '{other}'")),
		}
	}

	/// Build from an OriginAccount (covers all supported schemes).
	pub fn from_origin_account(acc: &OriginAccount) -> Result<Self, String> {
		match acc.pair() {
			OriginPair::Sr25519(p) => Ok(Self::Sr25519(p.clone())),
			OriginPair::Ed25519(p) => Ok(Self::Ed25519(p.clone())),
			OriginPair::Ecdsa(p) => Ok(Self::Ecdsa(p.clone())),
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
impl Signer for MultiKeySigner {
	fn account_id(&self) -> origin_primitives::AccountId {
		self.multisigner().into_account()
	}

	fn account_identifier(&self) -> MultiSigner {
		self.multisigner()
	}

	async fn sign_payload(&self, payload: &[u8]) -> MultiSignature {
		match self {
			Self::Sr25519(p) => MultiSignature::from(p.sign(payload)),
			Self::Ed25519(p) => MultiSignature::from(p.sign(payload)),
			Self::Ecdsa(p) => MultiSignature::from(p.sign(payload)),
		}
	}
}

/// Simple sr25519 signer convenience wrapper.
#[derive(Clone)]
pub struct Sr25519Signer(MultiKeySigner);

impl Sr25519Signer {
	#[allow(dead_code)]
	pub fn from_seed(seed: &str) -> Result<Self, String> {
		MultiKeySigner::from_seed(seed).map(Self)
	}
}

#[async_trait]
impl Signer for Sr25519Signer {
	fn account_id(&self) -> origin_primitives::AccountId {
		self.0.account_id()
	}

	fn account_identifier(&self) -> MultiSigner {
		self.0.account_identifier()
	}

	async fn sign_payload(&self, payload: &[u8]) -> MultiSignature {
		self.0.sign_payload(payload).await
	}
}

/// Adapter to plug async Signer into Subxt (blocking on current runtime).
#[derive(Clone)]
pub struct SubxtSignerAdapter {
	inner: std::sync::Arc<dyn Signer>,
}

impl SubxtSignerAdapter {
	pub fn new(inner: std::sync::Arc<dyn Signer>) -> Self {
		Self { inner }
	}
}

impl subxt::tx::Signer<crate::client::OriginConfig> for SubxtSignerAdapter {
	fn account_id(&self) -> origin_primitives::AccountId {
		self.inner.account_id()
	}

	fn sign(&self, payload: &[u8]) -> MultiSignature {
		let inner = self.inner.clone();
		let sig = task::block_in_place(|| {
			let handle = tokio::runtime::Handle::current();
			handle.block_on(inner.sign_payload(payload))
		});
		sig
	}
}

/// Preferred signer for SDK users; wraps MultiKeySigner and is buildable from OriginAccount.
#[derive(Clone)]
pub struct OriginSigner(MultiKeySigner);

impl OriginSigner {
	pub fn from_account(acc: &OriginAccount) -> Result<Self, String> {
		MultiKeySigner::from_origin_account(acc).map(Self)
	}

	pub fn account_id(&self) -> origin_primitives::AccountId {
		self.0.account_id()
	}
}

impl TryFrom<&OriginAccount> for OriginSigner {
	type Error = String;
	fn try_from(value: &OriginAccount) -> Result<Self, Self::Error> {
		OriginSigner::from_account(value)
	}
}

#[async_trait]
impl Signer for OriginSigner {
	fn account_id(&self) -> origin_primitives::AccountId {
		self.0.account_id()
	}

	fn account_identifier(&self) -> MultiSigner {
		self.0.account_identifier()
	}

	async fn sign_payload(&self, payload: &[u8]) -> MultiSignature {
		self.0.sign_payload(payload).await
	}
}
