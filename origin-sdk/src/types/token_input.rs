use codec::{Decode, Encode};
use origin_primitives::element::ElementView;
use scale_info::TypeInfo;

/// Bounds mirror entity/registry raw length.
pub type MaxRawDataLength = crate::types::entity_input::MaxRawDataLength;
pub type TokenElementInput = crate::types::entity_input::ElementInput;

/// Token attribute update using bounded Element.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct TokenAttributeInput {
	pub key: Vec<u8>,
	pub value: TokenElementInput,
}

impl TokenAttributeInput {
	pub fn from_view(
		key: &[u8],
		view: &ElementView,
	) -> Result<Self, crate::types::error::OriginSdkError> {
		let value = crate::schema::entity::element_from_view(view)?;
		Ok(Self { key: key.to_vec(), value })
	}
}
