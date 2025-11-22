use async_trait::async_trait;
use sp_core::{ecdsa, ed25519, sr25519, Pair};
use sp_runtime::{traits::IdentifyAccount, MultiSignature, MultiSigner};
use subxt::utils::{AccountId32, MultiSignature as SubxtMultiSignature};
use tokio::task;

/// Generic signing interface for Origin SDK (async to allow HSM/wallet flows).
#[async_trait]
pub trait Signer: Send + Sync + 'static {
	fn account_id(&self) -> AccountId32;
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

	/// Build from a secret URI seed; `scheme` may be "sr25519", "ed25519", or "ecdsa".
	pub fn from_seed(seed: &str, scheme: &str) -> Result<Self, String> {
		let scheme = if scheme.is_empty() { "sr25519" } else { scheme };
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
	fn account_id(&self) -> AccountId32 {
		let account: sp_runtime::AccountId32 = self.multisigner().into_account();
		let bytes: [u8; 32] = account.into();
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

/// Simple sr25519 signer convenience wrapper.
#[derive(Clone)]
pub struct Sr25519Signer(MultiKeySigner);

impl Sr25519Signer {
	#[allow(dead_code)]
	pub fn from_seed(seed: &str) -> Result<Self, String> {
		MultiKeySigner::from_seed(seed, "sr25519").map(Self)
	}
}

#[async_trait]
impl Signer for Sr25519Signer {
	fn account_id(&self) -> AccountId32 {
		self.0.account_id()
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
	fn account_id(&self) -> subxt::utils::AccountId32 {
		self.inner.account_id()
	}

	fn sign(&self, payload: &[u8]) -> SubxtMultiSignature {
		let inner = self.inner.clone();
		let sig = task::block_in_place(|| {
			let handle = tokio::runtime::Handle::current();
			handle.block_on(inner.sign_payload(payload))
		});
		match sig {
			MultiSignature::Ed25519(s) => SubxtMultiSignature::Ed25519(s.0),
			MultiSignature::Sr25519(s) => SubxtMultiSignature::Sr25519(s.0),
			MultiSignature::Ecdsa(s) => SubxtMultiSignature::Ecdsa(s.0),
		}
	}
}
