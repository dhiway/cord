use crate::types::core::TokenDecodedId;
use origin_primitives::{entity::EventBlockView, token::TokenStateEventView};
use sp_core::H256;

pub type TokenStateEventViewSdk = TokenStateEventView<H256>;
pub type TokenTimelineViewSdk = origin_primitives::token::TokenTimelineView<H256>;
pub type TokenLookupView = TokenDecodedId;
pub type TokenEventBlockViewSdk = EventBlockView;

// --- Extrinsic input mirrors ---
use codec::{Decode, Encode};
use origin_primitives::element::ElementView;
use scale_info::TypeInfo;

/// Bounds mirror entity/registry raw length.
pub type MaxRawDataLength = crate::types::entity::MaxRawDataLength;
pub type TokenElementInput = crate::types::entity::ElementInput;

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
