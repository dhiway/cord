use origin_primitives::identifier::DecodedIdentifier;
use origin_primitives::Ss58Identifier;
use subxt::utils::AccountId32;

/// Canonical account type for Origin runtimes.
pub type OriginAccountId = AccountId32;

/// Shared identifier aliases (all pallet-facing IDs are Ss58Identifier).
pub type EntityToken = Ss58Identifier;
pub type RegistryId = Ss58Identifier;
pub type PacketId = Ss58Identifier;
pub type TokenId = Ss58Identifier;

/// Decoded token identifier returned by token view functions.
pub type TokenDecodedId = DecodedIdentifier;
