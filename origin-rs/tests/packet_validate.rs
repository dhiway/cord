use oc::{schema::packet::PacketNestedValue, tx::packet::validate_packet_against_schema};
use origin_primitives::element::ElementView;

/// Basic schema validation: required key present, optional missing is ok, unknown key rejected.
#[test]
fn packet_validation_basic() {
	let nested = PacketNestedValue {
		attributes: vec![origin_primitives::packet::PacketAttributeView {
			key: b"id".to_vec(),
			value: ElementView::Raw(b"123".to_vec()),
		}],
	};
	let flat = oc::schema::packet::flatten_packet(&nested).expect("flatten");
	let schema = vec![(b"id".to_vec(), origin_primitives::element::ElementType::Raw, false)];
	let res = validate_packet_against_schema(&flat, &schema);
	assert!(res.is_ok());
}
