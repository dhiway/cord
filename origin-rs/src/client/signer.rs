use async_trait::async_trait;
use crate::params::config::OriginConfig;
use sp_runtime::MultiSignature;
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
