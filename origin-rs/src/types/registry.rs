use crate::types::core::{PacketId, RegistryId};
pub use origin_primitives::registry::{RegistryPermissions, RegistryStateView, RegistryStatus};
use origin_primitives::{
	packet::{PacketAttributeView, PacketMetadataView, PacketStateView},
	registry::{LookupSpec, RegistryAttributeView},
};

pub type RegistryStateViewSdk = RegistryStateView;
pub type RegistryAttributeViewSdk = RegistryAttributeView;
pub type LookupSpecViewSdk = LookupSpec;

pub type PacketStateViewSdk = PacketStateView;
pub type PacketMetadataViewSdk = PacketMetadataView;
pub type PacketAttributeViewSdk = PacketAttributeView;

/// Handy aliases for registry- and packet-scoped identifiers.
pub type RegistryIdSdk = RegistryId;
pub type PacketIdSdk = PacketId;

// --- Extrinsic input mirrors ---
use codec::{Decode, Encode};
use frame_support::{traits::ConstU32, BoundedVec};
use origin_primitives::{
	attribute::{Attribute, Element},
	element::ElementType,
	registry::RegistryKind,
};
use scale_info::TypeInfo;

pub type MaxRawDataLength = ConstU32<4096>;
pub type MaxAdditionalAttributes = ConstU32<32>;
pub type RegistryInfoInput = Element<MaxRawDataLength>;
pub type MaxDelegateRoles = ConstU32<16>;

/// Mirror of pallet-register `AttributeSpec`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct RegistryAttributeInput {
	pub key: Attribute,
	pub kind: ElementType,
	pub optional: bool,
}

/// Mirror of pallet-register `LookupSpec` (Single/Combo).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum RegistryLookupInput {
	Single(Attribute),
	Combo(BoundedVec<Attribute, MaxAdditionalAttributes>),
}

/// Input struct for `Register::create_registry`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct RegistryCreateInput {
	pub info: RegistryInfoInput,
	pub kind: RegistryKind,
	pub attributes: BoundedVec<RegistryAttributeInput, MaxAdditionalAttributes>,
	pub token_spec: RegistryLookupInput,
	pub lookup_specs: BoundedVec<RegistryLookupInput, MaxAdditionalAttributes>,
}

/// Input for granting delegate permissions (strongly typed).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct DelegatePermissionsInput {
	pub registry: origin_primitives::Ss58Identifier,
	pub delegate: subxt::utils::AccountId32,
	/// Roles requested for the delegate.
	pub roles: BoundedVec<origin_primitives::registry::RegistryPermissions, MaxDelegateRoles>,
}

/// Input for removing delegate permissions.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct RemoveDelegatePermissionsInput {
	pub registry: origin_primitives::Ss58Identifier,
	pub delegate: origin_primitives::Ss58Identifier,
}
