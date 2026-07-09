use sp_core::{
	crypto::{AccountId32, Ss58AddressFormat, Ss58Codec},
	ecdsa, ed25519, sr25519, Pair,
};
use thiserror::Error;

/// Origin human-readable SS58 prefix (Cord = 29).
pub const ORIGIN_SS58_PREFIX: u16 = 29;

#[inline]
pub fn origin_ss58_format() -> Ss58AddressFormat {
	Ss58AddressFormat::custom(ORIGIN_SS58_PREFIX)
}

/// Supported crypto schemes for Origin accounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CryptoScheme {
	Sr25519,
	Ed25519,
	Ecdsa,
}

impl Default for CryptoScheme {
	fn default() -> Self {
		CryptoScheme::Sr25519
	}
}

#[derive(Debug, Error)]
pub enum AccountError {
	#[error("invalid SS58 address: {0}")]
	InvalidSs58(String),
	#[error("invalid derivation or seed: {0}")]
	Derivation(String),
	#[error("unsupported dev URI; expected //Name style")]
	InvalidDevUri,
	#[error("unsupported scheme for this operation")]
	UnsupportedScheme,
}

/// Scheme-tagged keypair container.
#[derive(Clone)]
pub enum OriginPair {
	Sr25519(sr25519::Pair),
	Ed25519(ed25519::Pair),
	Ecdsa(ecdsa::Pair),
}

impl OriginPair {
	pub fn scheme(&self) -> CryptoScheme {
		match self {
			OriginPair::Sr25519(_) => CryptoScheme::Sr25519,
			OriginPair::Ed25519(_) => CryptoScheme::Ed25519,
			OriginPair::Ecdsa(_) => CryptoScheme::Ecdsa,
		}
	}

	pub fn public_key_bytes(&self) -> [u8; 32] {
		match self {
			OriginPair::Sr25519(p) => p.public().0,
			OriginPair::Ed25519(p) => p.public().0,
			OriginPair::Ecdsa(p) => {
				// For ECDSA we derive AccountId32 via Blake2-256 of the 33-byte pubkey, matching
				// Substrate convention.
				sp_crypto_hashing::blake2_256(&p.public().0)
			},
		}
	}

	pub fn account_id(&self) -> AccountId32 {
		AccountId32::from(self.public_key_bytes())
	}
}

/// High-level Origin account wrapper with helpers.
#[derive(Clone)]
pub struct OriginAccount {
	pair: OriginPair,
}

impl OriginAccount {
	/// Generate a random account (default scheme sr25519) returning `(account, mnemonic)`.
	pub fn generate() -> (Self, String) {
		Self::generate_with_scheme(CryptoScheme::Sr25519)
	}

	pub fn generate_with_scheme(scheme: CryptoScheme) -> (Self, String) {
		match scheme {
			CryptoScheme::Sr25519 => {
				let (p, phrase, _) = sr25519::Pair::generate_with_phrase(None);
				(Self { pair: OriginPair::Sr25519(p) }, phrase)
			},
			CryptoScheme::Ed25519 => {
				let (p, phrase, _) = ed25519::Pair::generate_with_phrase(None);
				(Self { pair: OriginPair::Ed25519(p) }, phrase)
			},
			CryptoScheme::Ecdsa => {
				let (p, phrase, _) = ecdsa::Pair::generate_with_phrase(None);
				(Self { pair: OriginPair::Ecdsa(p) }, phrase)
			},
		}
	}

	/// Build from a dev URI (`//Alice`, `//Bob`), sr25519 only.
	pub fn from_dev(uri: &str) -> Result<Self, AccountError> {
		if !uri.starts_with("//") {
			return Err(AccountError::InvalidDevUri);
		}
		let p = sr25519::Pair::from_string(uri, None)
			.map_err(|e| AccountError::Derivation(e.to_string()))?;
		Ok(Self { pair: OriginPair::Sr25519(p) })
	}

	/// Restore from mnemonic/derivation URI with optional scheme (default sr25519).
	pub fn from_uri(uri: &str, scheme: Option<CryptoScheme>) -> Result<Self, AccountError> {
		match scheme.unwrap_or_default() {
			CryptoScheme::Sr25519 => sr25519::Pair::from_string(uri, None)
				.map(|p| Self { pair: OriginPair::Sr25519(p) })
				.map_err(|e| AccountError::Derivation(e.to_string())),
			CryptoScheme::Ed25519 => ed25519::Pair::from_string(uri, None)
				.map(|p| Self { pair: OriginPair::Ed25519(p) })
				.map_err(|e| AccountError::Derivation(e.to_string())),
			CryptoScheme::Ecdsa => ecdsa::Pair::from_string(uri, None)
				.map(|p| Self { pair: OriginPair::Ecdsa(p) })
				.map_err(|e| AccountError::Derivation(e.to_string())),
		}
	}

	pub fn scheme(&self) -> CryptoScheme {
		self.pair.scheme()
	}

	pub fn account_id(&self) -> AccountId32 {
		self.pair.account_id()
	}

	pub fn ss58(&self) -> String {
		account_id_to_ss58(&self.account_id())
	}

	pub fn pair(&self) -> &OriginPair {
		&self.pair
	}
}

/// Encode AccountId32 to SS58 with Origin prefix.
pub fn account_id_to_ss58(acc: &AccountId32) -> String {
	acc.to_ss58check_with_version(origin_ss58_format())
}

/// Decode SS58 (any prefix) into AccountId32.
pub fn ss58_to_account_id(addr: &str) -> Result<AccountId32, AccountError> {
	let (acc, _) = AccountId32::from_ss58check_with_version(addr)
		.map_err(|e| AccountError::InvalidSs58(e.to_string()))?;
	Ok(acc)
}

/// Convert a `subxt::utils::AccountId32` into the chain's `AccountId32`.
pub fn account_id_from_subxt(acc: &subxt::utils::AccountId32) -> AccountId32 {
	AccountId32::from(acc.0)
}

/// Format a `subxt::utils::AccountId32` to SS58 (prefix 29).
pub fn account_id_to_ss58_subxt(acc: &subxt::utils::AccountId32) -> String {
	account_id_to_ss58(&account_id_from_subxt(acc))
}
