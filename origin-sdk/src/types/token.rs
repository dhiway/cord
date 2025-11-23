use sp_core::H256;

pub type TokenStateEvent = origin_primitives::TokenStateEventView<H256>;
pub type TokenTimelineView = origin_primitives::TokenTimelineView<H256>;
pub type TokenLookupView = origin_primitives::identifier::DecodedIdentifier;
