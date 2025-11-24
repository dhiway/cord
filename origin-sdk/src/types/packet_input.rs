use codec::{Decode, Encode};
use frame_support::traits::ConstU32;
use origin_primitives::attribute::{Attribute, Attributes};
use origin_primitives::element::Elum;
use scale_info::TypeInfo;

/// Bounds mirror runtime constants (see pallet-register).
pub type MaxRawDataLength = ConstU32<4096>;
pub type MaxAdditionalAttributes = ConstU32<32>;

/// Element used for packet attributes in extrinsic inputs.
pub type PacketElementInput = Elum<MaxRawDataLength>;

/// Attribute collection used by packet extrinsics.
pub type PacketAttributesInput = Attributes<MaxRawDataLength, MaxAdditionalAttributes>;

/// Single attribute entry for packets.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct PacketAttributeInput {
	pub key: Attribute,
	pub value: PacketElementInput,
}
