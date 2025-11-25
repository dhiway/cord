use crate::types::core::{EntityToken, OriginAccountId};
use origin_primitives::entity::{
	AccountUnbindEntryView, AttributeHistoryEntryView, EntityInfoView, EntityOverview,
	EntityStateView,
};

pub use origin_primitives::{
	AttributeValueView, Authorization, Element, ElementType, ElementView, EventBlockView,
};

pub type EntityInfoViewSdk = EntityInfoView;
pub type EntityStateViewSdk = EntityStateView<OriginAccountId>;
pub type EntityOverviewSdk = EntityOverview;
pub type AccountUnbindEntryViewSdk = AccountUnbindEntryView<OriginAccountId>;
pub type AttributeHistoryEntryViewSdk = AttributeHistoryEntryView;

/// Optional nested UX-friendly view used by the schema layer.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityNestedView {
	pub token: EntityToken,
	pub display: Option<String>,
	pub web: Option<String>,
	pub email: Option<String>,
	pub attributes: std::collections::BTreeMap<String, String>,
}
