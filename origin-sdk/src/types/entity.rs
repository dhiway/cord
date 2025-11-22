pub use origin_primitives::{AttributeValueView, Authorization, Element, ElementType, ElementView};

#[derive(Clone, Debug)]
pub struct AttributeView {
	pub key: String,
	pub value: AttributeValueView,
}

#[derive(Clone, Debug)]
pub struct EntityStateView {
	pub id: String,
	pub attributes: Vec<AttributeView>,
}

#[derive(Clone, Debug)]
pub struct EntityOverview {
	pub id: String,
	pub summary: serde_json::Value,
}
