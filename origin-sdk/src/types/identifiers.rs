pub use origin_primitives::Ss58Identifier;

/// Decoded identifier placeholder for SDK-friendly formats.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DecodedIdentifier {
	pub raw: Ss58Identifier,
	pub encoded: String,
}
