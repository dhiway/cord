use codec::Decode;
use sp_core::H256;

use super::entity::EventBlockView;

#[derive(Clone, Debug, Decode)]
pub struct TokenStateEvent {
	pub action: Vec<u8>,
	pub digest: H256,
	pub seal: EventBlockView,
}

/// Timeline view result: a list of state events plus an optional cursor.
pub type TokenTimelineView = (Vec<TokenStateEvent>, Option<u32>);

/// Decoded form of an Origin identifier (see `origin_primitives::identifier::DecodedIdentifier`).
pub type TokenLookupView = origin_primitives::identifier::DecodedIdentifier;
