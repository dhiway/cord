use codec::Encode;
use origin_primitives::{element::ElementView, AttributeValueView, EntityInfoView};
use origin_sdk::schema::entity::to_entity_input;

/// Ensure EntityInfoInput SCALE matches ElementView encoding expectations.
#[test]
fn entity_info_input_matches_view_bytes() {
	let nested = origin_sdk::schema::entity::EntityNestedValue {
		display: ElementView::Raw(b"display".to_vec()),
		web: ElementView::Raw(b"web".to_vec()),
		email: ElementView::Raw(b"mail".to_vec()),
		attributes: Some(vec![AttributeValueView {
			key: b"id".to_vec(),
			value: ElementView::U64(42),
		}]),
	};
	let input = to_entity_input(&nested).expect("convert entity input");

	// Round-trip ElementView conversions.
	assert_eq!(ElementView::from(&input.display), nested.display);
	assert_eq!(ElementView::from(&input.web), nested.web);
	assert_eq!(ElementView::from(&input.email), nested.email);

	let attrs = input.attributes.as_ref().expect("attributes");
	let (key, val) = attrs.iter().next().expect("one attr");
	assert_eq!(key.as_slice(), b"id");
	assert_eq!(ElementView::from(val), ElementView::U64(42));
}
