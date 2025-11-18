use subxt::dynamic::{DecodedValue, Value};

/// Internal helper trait for decoding dynamic values into typed primitives.
pub(crate) trait DynamicDecode: Sized {
	type Error;

	fn decode_from_dynamic(value: &DecodedValue) -> Result<Self, Self::Error>;
}

/// Internal helper trait for encoding typed primitives into dynamic SCALE values.
pub(crate) trait DynamicEncode {
	fn encode_to_dynamic(&self) -> Value;
}
