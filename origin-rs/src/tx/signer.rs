//! Flexible keypair helpers with multi-algorithm support.

use crate::params::config::CordConfig;
use sp_core::{
	crypto::{Pair as _, SecretStringError},
	ed25519, sr25519,
};
use sp_runtime::MultiSignature as RuntimeMultiSignature;
use std::{fmt, str::FromStr};
use subxt::config::PolkadotConfig;
use subxt::tx::Signer as SubxtSigner;
use subxt::utils::{AccountId32, MultiSignature as SubxtMultiSignature};

/// Supported cryptographic algorithms for SDK keypairs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAlgorithm {
	Sr25519,
	Ed25519,
}

impl Default for KeyAlgorithm {
	fn default() -> Self {
		KeyAlgorithm::Sr25519
	}
}

/// Convenience enum for common development accounts.
#[derive(Clone, Copy, Debug)]
pub enum DevAccount {
	Alice,
	Bob,
	Charlie,
	Dave,
	Eve,
	Ferdie,
}

impl DevAccount {
	fn uri(self) -> &'static str {
		match self {
			DevAccount::Alice => "//Alice",
			DevAccount::Bob => "//Bob",
			DevAccount::Charlie => "//Charlie",
			DevAccount::Dave => "//Dave",
			DevAccount::Eve => "//Eve",
			DevAccount::Ferdie => "//Ferdie",
		}
	}
}

/// Errors encountered while constructing keypairs.
#[derive(Debug)]
pub enum SigningError {
	InvalidSecret(String),
	UnsupportedSeedLength,
}

impl fmt::Display for SigningError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SigningError::InvalidSecret(msg) => write!(f, "invalid secret: {msg}"),
			SigningError::UnsupportedSeedLength => write!(f, "seed must be exactly 32 bytes"),
		}
	}
}

impl std::error::Error for SigningError {}

impl From<SecretStringError> for SigningError {
	fn from(err: SecretStringError) -> Self {
		SigningError::InvalidSecret(err.to_string())
	}
}

/// Unified wrapper over supported Substrate keypair types.
#[derive(Clone)]
pub struct Keypair {
	inner: KeypairInner,
}

#[derive(Clone)]
enum KeypairInner {
	Sr25519(sr25519::Pair),
	Ed25519(ed25519::Pair),
}

impl Keypair {
	/// Create a dev keypair (defaults to sr25519).
	pub fn dev(account: DevAccount) -> Self {
		Self::dev_with(account, KeyAlgorithm::default())
	}

	/// Create a dev keypair using the selected algorithm.
	pub fn dev_with(account: DevAccount, alg: KeyAlgorithm) -> Self {
		Self::from_secret_uri(alg, account.uri(), None).expect("dev key URIs are always valid")
	}

	/// Construct from a `//path` or mnemonic URI supported by `sp_core`.
	pub fn from_secret_uri(
		alg: KeyAlgorithm,
		uri: &str,
		password: Option<&str>,
	) -> Result<Self, SigningError> {
		match alg {
			KeyAlgorithm::Sr25519 => {
				let pair = sr25519::Pair::from_string(uri, password)?;
				Ok(Self { inner: KeypairInner::Sr25519(pair) })
			},
			KeyAlgorithm::Ed25519 => {
				let pair = ed25519::Pair::from_string(uri, password)?;
				Ok(Self { inner: KeypairInner::Ed25519(pair) })
			},
		}
	}

	/// Construct from a raw 32-byte seed.
	pub fn from_seed(alg: KeyAlgorithm, seed: &[u8]) -> Result<Self, SigningError> {
		if seed.len() != 32 {
			return Err(SigningError::UnsupportedSeedLength);
		}
		match alg {
			KeyAlgorithm::Sr25519 => {
				let pair = sr25519::Pair::from_seed_slice(seed)?;
				Ok(Self { inner: KeypairInner::Sr25519(pair) })
			},
			KeyAlgorithm::Ed25519 => {
				let mut bytes = [0u8; 32];
				bytes.copy_from_slice(seed);
				let pair = ed25519::Pair::from_seed(&bytes);
				Ok(Self { inner: KeypairInner::Ed25519(pair) })
			},
		}
	}

	/// Parse from a JSON-compatible secret URI string (e.g. `"//Alice"`).
	pub fn from_str_with_alg(alg: KeyAlgorithm, uri: &str) -> Result<Self, SigningError> {
		Self::from_secret_uri(alg, uri, None)
	}

	/// Return the algorithm used by this keypair.
	pub fn algorithm(&self) -> KeyAlgorithm {
		match &self.inner {
			KeypairInner::Sr25519(_) => KeyAlgorithm::Sr25519,
			KeypairInner::Ed25519(_) => KeyAlgorithm::Ed25519,
		}
	}

	/// Compute the SS58 account identifier.
	pub fn account_id(&self) -> AccountId32 {
		match &self.inner {
			KeypairInner::Sr25519(pair) => AccountId32::from(pair.public().0),
			KeypairInner::Ed25519(pair) => AccountId32::from(pair.public().0),
		}
	}

	/// Sign an arbitrary payload and return the resulting `MultiSignature`.
	pub fn sign_message(&self, payload: &[u8]) -> RuntimeMultiSignature {
		self.sign_internal(payload)
	}

	fn sign_internal(&self, payload: &[u8]) -> RuntimeMultiSignature {
		match &self.inner {
			KeypairInner::Sr25519(pair) => RuntimeMultiSignature::from(pair.sign(payload)),
			KeypairInner::Ed25519(pair) => RuntimeMultiSignature::from(pair.sign(payload)),
		}
	}

	fn sign_for_subxt(&self, payload: &[u8]) -> SubxtMultiSignature {
		match &self.inner {
			KeypairInner::Sr25519(pair) => SubxtMultiSignature::Sr25519(pair.sign(payload).0),
			KeypairInner::Ed25519(pair) => SubxtMultiSignature::Ed25519(pair.sign(payload).0),
		}
	}
}

impl FromStr for Keypair {
	type Err = SigningError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Keypair::from_secret_uri(KeyAlgorithm::default(), s, None)
	}
}

impl SubxtSigner<CordConfig> for Keypair {
	fn account_id(&self) -> AccountId32 {
		self.account_id()
	}

	fn sign(&self, signer_payload: &[u8]) -> SubxtMultiSignature {
		self.sign_for_subxt(signer_payload)
	}
}

impl SubxtSigner<PolkadotConfig> for Keypair {
	fn account_id(&self) -> AccountId32 {
		self.account_id()
	}

	fn sign(&self, signer_payload: &[u8]) -> SubxtMultiSignature {
		self.sign_for_subxt(signer_payload)
	}
}

/// Return Alice's well-known dev keypair (sr25519 by default).
pub fn dev_alice() -> Keypair {
	Keypair::dev(DevAccount::Alice)
}

/// Return Bob's well-known dev keypair (sr25519 by default).
pub fn dev_bob() -> Keypair {
	Keypair::dev(DevAccount::Bob)
}

/// Create a dev keypair for the selected account using the requested algorithm.
pub fn dev_account_with(account: DevAccount, algorithm: KeyAlgorithm) -> Keypair {
	Keypair::dev_with(account, algorithm)
}
