use crate::types::core::{EntityToken, OriginAccountId};
use origin_primitives::entity::{
	AccountUnbindEntryView, AttributeHistoryEntryView, EntityInfoView, EntityOverview,
	EntityStateView,
};

pub use origin_primitives::{AttributeValueView, Authorization, ElementType, ElementView, EventBlockView};

pub type EntityInfoViewSdk = EntityInfoView;
pub type EntityStateViewSdk = EntityStateView<OriginAccountId>;
pub type EntityOverviewSdk = EntityOverview;
pub type AccountUnbindEntryViewSdk = AccountUnbindEntryView<OriginAccountId>;
pub type AttributeHistoryEntryViewSdk = AttributeHistoryEntryView;

// --- Extrinsic input mirrors ---
use codec::{Decode, Encode};
use frame_support::traits::ConstU32;
use origin_primitives::attribute::{Attribute, Attributes, AttributesError, Element};
use scale_info::TypeInfo;

/// Compile-time bounds mirrored from runtime defaults.
pub type MaxRawDataLength = ConstU32<4096>;
pub type MaxAdditionalAttributes = ConstU32<32>;
pub type ElementInput = Element<MaxRawDataLength>;
pub type AttributesInput = Attributes<MaxRawDataLength, MaxAdditionalAttributes>;
pub type AttributeUpdateInput = (Attribute, ElementInput);

/// SDK-side mirror of pallet-entity `EntityInfo` used for extrinsic inputs.
///
/// Field order matches pallet: display, web, email, attributes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct EntityInfoInput {
	pub display: ElementInput,
	pub web: ElementInput,
	pub email: ElementInput,
	pub attributes: Option<AttributesInput>,
}

impl EntityInfoInput {
	/// Construct from already-flattened values (raw bytes expected by the pallet).
	pub fn new(
		display: ElementInput,
		web: ElementInput,
		email: ElementInput,
		attributes: Option<AttributesInput>,
	) -> Self {
		Self { display, web, email, attributes }
	}
}

/// Convenience builder for attribute collection.
pub fn build_attributes(
	entries: &[(Attribute, ElementInput)],
) -> Result<AttributesInput, AttributesError> {
	AttributesInput::try_collect(entries.iter().cloned())
}

/// Optional nested UX-friendly view used by the schema layer.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityNestedView {
	pub token: EntityToken,
	pub display: Option<String>,
	pub web: Option<String>,
	pub email: Option<String>,
	pub attributes: std::collections::BTreeMap<String, String>,
}
