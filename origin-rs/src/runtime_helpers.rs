use crate::api::runtime;
use origin_primitives::packet::ElementType;

pub(crate) type RuntimeBoundedVec<T> =
	runtime::runtime_types::bounded_collections::bounded_vec::BoundedVec<T>;

pub(crate) type RuntimeIdentifier =
	runtime::runtime_types::origin_primitives::identifier::Ss58Identifier;

pub(crate) fn identifier_bytes(identifier: &RuntimeIdentifier) -> &[u8] {
	bounded_slice(&identifier.0)
}

pub(crate) fn identifier_string(identifier: &RuntimeIdentifier) -> String {
	String::from_utf8_lossy(identifier_bytes(identifier)).into_owned()
}

pub(crate) fn bounded_slice<T>(bounded: &RuntimeBoundedVec<T>) -> &[T] {
	bounded.0.as_slice()
}

pub(crate) fn bounded_iter<T>(bounded: &RuntimeBoundedVec<T>) -> core::slice::Iter<'_, T> {
	bounded.0.iter()
}

pub(crate) fn bounded_cloned<T: Clone>(bounded: &RuntimeBoundedVec<T>) -> Vec<T> {
	bounded.0.clone()
}

pub(crate) fn bounded_bytes_vec(bounded: &RuntimeBoundedVec<u8>) -> Vec<u8> {
	bounded_cloned(bounded)
}

pub(crate) fn attribute_optional(
	flags: &runtime::runtime_types::pallet_register::register::AttributeFlags,
) -> bool {
	flags.bits & 0b0000_0001 != 0
}

pub(crate) fn element_type_to_sdk(
	kind: &runtime::runtime_types::origin_primitives::element::ElementType,
) -> ElementType {
	use runtime::runtime_types::origin_primitives::element::ElementType as Rt;
	match kind {
		Rt::None => ElementType::None,
		Rt::Raw => ElementType::Raw,
		Rt::Bool => ElementType::Bool,
		Rt::U64 => ElementType::U64,
		Rt::U128 => ElementType::U128,
		Rt::Hash => ElementType::Hash,
		Rt::Token => ElementType::Token,
		Rt::Cid => ElementType::Cid,
	}
}
