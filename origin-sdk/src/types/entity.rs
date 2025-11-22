use codec::Decode;
pub use origin_primitives::{AttributeValueView, Authorization, Element, ElementType, ElementView};

#[derive(Clone, Debug, Decode)]
pub struct AttributeView {
	pub key: String,
	pub value: AttributeValueView,
}

#[derive(Clone, Debug, Decode)]
pub struct EntityStateView {
	pub id: String,
	pub attributes: Vec<AttributeView>,
}

#[derive(Clone, Debug, Decode)]
pub struct EntityOverview {
	pub id: String,
	pub summary: Vec<u8>,
}
