use codec::Decode;
pub use origin_primitives::{AttributeValueView, Authorization, Element, ElementType, ElementView};
use subxt::utils::AccountId32;

#[derive(Clone, Debug, Decode)]
pub struct EventBlockView {
	pub height: u32,
	pub index: u32,
}

#[derive(Clone, Debug, Decode)]
pub struct AttributeHistoryEntryView {
	pub key: Vec<u8>,
	pub version: u64,
	pub old_value: Vec<u8>,
	pub block: EventBlockView,
}

#[derive(Clone, Debug, Decode)]
pub struct EntityInfoView {
	pub display: ElementView,
	pub web: ElementView,
	pub email: ElementView,
	pub attributes: Option<Vec<AttributeValueView>>,
}

#[derive(Clone, Debug, Decode)]
pub struct EntityStateView {
	pub info: EntityInfoView,
	pub nym: Option<Vec<u8>>,
	pub linked_accounts: Vec<AccountId32>,
	pub history: Vec<AttributeHistoryEntryView>,
}

#[derive(Clone, Debug, Decode)]
pub struct EntityOverview {
	pub info: EntityInfoView,
	pub nym: Option<Vec<u8>>,
}

impl From<EntityStateView> for EntityOverview {
	fn from(state: EntityStateView) -> Self {
		Self { info: state.info, nym: state.nym }
	}
}
