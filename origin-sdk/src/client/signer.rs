use subxt::utils::{AccountId32, MultiSignature};

/// Generic signing interface for Origin SDK.
pub trait Signer: Send + Sync {
	/// Account identifier for extrinsics.
	fn account_id(&self) -> AccountId32;

	/// Sign an arbitrary payload.
	fn sign(&self, payload: &[u8]) -> MultiSignature;
}
