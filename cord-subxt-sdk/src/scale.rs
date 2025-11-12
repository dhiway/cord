use crate::error::{Error, Result};
use scale_value::{scale, Value};
use subxt::Metadata;

pub mod decode;
pub mod value;

pub use value::{
	decode_dev_attribute_map, decode_dev_attributes, decode_dev_element, decode_element_view,
};

/// Convenience helpers for working with runtime metadata and SCALE values.
pub struct MetadataResolver<'a> {
	metadata: &'a Metadata,
}

impl<'a> MetadataResolver<'a> {
	/// Construct a resolver referencing the provided metadata snapshot.
	pub fn new(metadata: &'a Metadata) -> Self {
		Self { metadata }
	}

	/// Borrow the underlying metadata.
	pub fn metadata(&self) -> &'a Metadata {
		self.metadata
	}

	/// Borrow the portable registry associated with this metadata.
	pub fn registry(&self) -> &'a scale_info::PortableRegistry {
		self.metadata.types()
	}

	/// Decode SCALE bytes as a dynamic [`Value`] using the supplied type id.
	pub fn decode_value(&self, type_id: u32, bytes: &[u8]) -> Result<Value<u32>> {
		let mut cursor = bytes;
		scale::decode_as_type(&mut cursor, type_id, self.registry())
			.map_err(|e| Error::Codec(e.to_string()))
	}

	/// Encode a dynamic [`Value`] into SCALE bytes using the supplied type id.
	pub fn encode_value(&self, type_id: u32, value: &Value<u32>) -> Result<Vec<u8>> {
		let mut out = Vec::new();
		scale::encode_as_type(value, type_id, self.registry(), &mut out)
			.map_err(|e| Error::Codec(e.to_string()))?;
		Ok(out)
	}

	/// Find the first type id for which `predicate` returns `true`.
	pub fn find_type<F>(&self, mut predicate: F) -> Option<u32>
	where
		F: FnMut(&scale_info::PortableType) -> bool,
	{
		self.registry().types.iter().find(|ty| predicate(ty)).map(|ty| ty.id)
	}

	/// Find a type by matching the full path segments.
	pub fn find_type_by_path<'b>(&self, path: &[&'b str]) -> Option<u32> {
		self.find_type(|ty| {
			ty.ty.path.segments.iter().map(|seg| seg.as_str()).eq(path.iter().copied())
		})
	}

	/// Find a type by the final path segment (type name).
	pub fn find_type_by_name(&self, name: &str) -> Option<u32> {
		self.find_type(|ty| ty.ty.path.segments.last().map(|seg| seg.as_str()) == Some(name))
	}
}
