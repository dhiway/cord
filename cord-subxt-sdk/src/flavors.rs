use crate::{error::Result, params::config::CordConfig};

/// Supported runtime "flavors" that the SDK can target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChainFlavor {
	/// Auto-detect based on metadata/spec name.
	Auto,
	/// The Orb development/runtime (default local node).
	Orb,
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

/// Inspect the connected chain and infer the correct [`ChainFlavor`].
pub async fn detect_flavor(api: &subxt::OnlineClient<CordConfig>) -> Result<ChainFlavor> {
	// Inspect metadata for pallets that only exist on specific flavors.
	let metadata = api.metadata();
	if metadata.pallet_by_name("Register").is_some() && metadata.pallet_by_name("Entity").is_some()
	{
		Ok(ChainFlavor::Origin)
	} else if metadata.pallet_by_name("Token").is_some() {
		Ok(ChainFlavor::OriginHub)
	} else {
		Ok(ChainFlavor::Orb)
	}
}
