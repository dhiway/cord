use crate::error::Result;

/// Supported runtime "flavors" that the SDK can target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChainFlavor {
	/// Auto-detect based on metadata/spec name.
	Auto,
	/// Origin relay chain runtime.
	Origin,
	/// OriginHub parachain runtime.
	OriginHub,
}

impl Default for ChainFlavor {
	fn default() -> Self {
		ChainFlavor::Auto
	}
}

impl ChainFlavor {
	pub fn ss58_prefix(self) -> u16 {
		match self {
			Self::Auto | Self::Origin | Self::OriginHub => 42,
		}
	}
}

/// Inspect the connected chain and infer the correct [`ChainFlavor`].
/// Infer flavor from a metadata snapshot (no network calls).
pub fn detect_flavor_from_metadata(metadata: &subxt::Metadata) -> Result<ChainFlavor> {
	let has_register = metadata.pallet_by_name("Register").is_some();
	let has_entity = metadata.pallet_by_name("Entity").is_some();
	let has_token = metadata.pallet_by_name("Token").is_some();
	let has_meta = metadata.pallet_by_name("MetaTx").is_some();

	// Origin hub (parachain) exposes full user pallets including Token/Register/Entity.
	if has_token && has_register && has_entity {
		return Ok(ChainFlavor::OriginHub);
	}

	// Origin relay runtime supports meta transactions but omits Register/Entity.
	if has_meta && !has_register && !has_entity {
		return Ok(ChainFlavor::Origin);
	}

	// Fallback to OriginHub-compatible API.
	Ok(ChainFlavor::OriginHub)
}
