use crate::types::core::{PacketId, RegistryId};
use origin_primitives::{
	packet::{PacketAttributeView, PacketMetadataView, PacketStateView},
	registry::{LookupSpec, RegistryAttributeView},
};
pub use origin_primitives::registry::{RegistryPermissions, RegistryStateView, RegistryStatus};

pub type RegistryStateViewSdk = RegistryStateView;
pub type RegistryAttributeViewSdk = RegistryAttributeView;
pub type LookupSpecViewSdk = LookupSpec;

pub type PacketStateViewSdk = PacketStateView;
pub type PacketMetadataViewSdk = PacketMetadataView;
pub type PacketAttributeViewSdk = PacketAttributeView;

/// Handy aliases for registry- and packet-scoped identifiers.
pub type RegistryIdSdk = RegistryId;
pub type PacketIdSdk = PacketId;
