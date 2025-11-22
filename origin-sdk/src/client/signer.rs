use subxt::utils::{AccountId32, MultiSignature};
use sp_core::{ecdsa, ed25519, sr25519, Pair};
use sp_runtime::{traits::IdentifyAccount, MultiSigner};

/// Generic signing interface for Origin SDK.
pub trait Signer: Send + Sync {
	/// Account identifier for extrinsics.
	fn account_id(&self) -> AccountId32;

	/// Sign an arbitrary payload.
	fn sign(&self, payload: &[u8]) -> MultiSignature;
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
		match scheme {
			"sr25519" | "" => sr25519::Pair::from_string(seed, None)
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
}

impl Signer for MultiKeySigner {
	fn account_id(&self) -> AccountId32 {
		let multisigner: MultiSigner = match self {
			Self::Sr25519(p) => MultiSigner::from(p.public()),
			Self::Ed25519(p) => MultiSigner::from(p.public()),
			Self::Ecdsa(p) => MultiSigner::from(p.public()),
		};
		let account: sp_runtime::AccountId32 = multisigner.into_account();
		let bytes: [u8; 32] = account.into();
		AccountId32::from(bytes)
	}

	fn sign(&self, payload: &[u8]) -> MultiSignature {
		match self {
			Self::Sr25519(p) => MultiSignature::Sr25519(p.sign(payload).into()),
			Self::Ed25519(p) => MultiSignature::Ed25519(p.sign(payload).into()),
			Self::Ecdsa(p) => MultiSignature::Ecdsa(p.sign(payload).into()),
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

impl Signer for Sr25519Signer {
	fn account_id(&self) -> AccountId32 {
		self.0.account_id()
	}

	fn sign(&self, payload: &[u8]) -> MultiSignature {
		self.0.sign(payload)
	}
}
