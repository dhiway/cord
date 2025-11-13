//! Signer helpers and dev key shortcuts.

pub use subxt_signer::sr25519;

/// Return Alice's well-known dev key for quick examples.
pub fn dev_alice() -> sr25519::Keypair {
	sr25519::dev::alice()
}

/// Return Bob's well-known dev key.
pub fn dev_bob() -> sr25519::Keypair {
	sr25519::dev::bob()
}
