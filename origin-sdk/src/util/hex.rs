/// Hex encoding helper.
pub fn encode_hex(bytes: impl AsRef<[u8]>) -> String {
	hex::encode(bytes)
}

/// Hex decoding helper.
pub fn decode_hex(data: &str) -> Result<Vec<u8>, hex::FromHexError> {
	hex::decode(data)
}
