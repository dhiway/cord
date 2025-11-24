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
