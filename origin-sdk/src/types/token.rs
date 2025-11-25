use crate::types::core::TokenDecodedId;
use origin_primitives::{entity::EventBlockView, token::TokenStateEventView};
use sp_core::H256;

pub type TokenStateEventViewSdk = TokenStateEventView<H256>;
pub type TokenTimelineViewSdk = origin_primitives::token::TokenTimelineView<H256>;
pub type TokenLookupView = TokenDecodedId;
pub type TokenEventBlockViewSdk = EventBlockView;
