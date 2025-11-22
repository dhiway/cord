use codec::Encode;

/// Encode helper using SCALE.
pub fn encode_args<T: Encode>(value: &T) -> Vec<u8> {
	value.encode()
}
