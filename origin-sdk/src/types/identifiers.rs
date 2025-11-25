pub use origin_primitives::{identifier::DecodedIdentifier, Ss58Identifier};
/// Helper to render an identifier as a human SS58/base58 string.
pub fn ss58_to_string(id: &Ss58Identifier) -> String {
	id.to_string_lossy()
}

/// Render an AccountId32 with a given ss58 prefix (default Origin = 29).
pub fn account_to_ss58(account: &subxt::utils::AccountId32, prefix: u16) -> String {
	use sp_core::crypto::{AccountId32 as CoreAccountId32, Ss58AddressFormat, Ss58Codec};
	let fmt = Ss58AddressFormat::custom(prefix);
	let core = CoreAccountId32::from(account.0);
	core.to_ss58check_with_version(fmt)
}
