use crate::error::{Error, Result};
use scale_decode::DecodeAsType;

/// Decode SCALE bytes into the requested `T` using the metadata's portable registry.
pub fn decode_with_metadata<T: DecodeAsType>(
	metadata: &subxt::Metadata,
	type_id: u32,
	bytes: &[u8],
) -> Result<T> {
	let registry = metadata.types();
	let mut cursor = bytes;
	T::decode_as_type(&mut cursor, type_id, registry).map_err(|e| Error::ViewDecode(e.to_string()))
}
