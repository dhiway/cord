use thiserror::Error;

/// Unified SDK error type.
#[derive(Debug, Error)]
pub enum OriginSdkError {
	#[error("connection error: {0}")]
	Connection(String),
	#[error("metadata error: {0}")]
	Metadata(String),
	#[error("encode error: {0}")]
	Encode(String),
	#[error("decode error: {0}")]
	Decode(String),
	#[error("view error: {0}")]
	View(String),
	#[error("transaction error: {0}")]
	Tx(String),
	#[error("nonce error: {0}")]
	Nonce(String),
	#[error("meta-tx error: {0}")]
	MetaTx(String),
	#[error("timeout")]
	Timeout,
	#[error("invalid input: {0}")]
	InvalidInput(String),
	#[error("unimplemented: {0}")]
	Unimplemented(String),
}

impl From<subxt::Error> for OriginSdkError {
	fn from(err: subxt::Error) -> Self {
		OriginSdkError::Tx(err.to_string())
	}
}

impl From<subxt::ext::scale_decode::Error> for OriginSdkError {
	fn from(err: subxt::ext::scale_decode::Error) -> Self {
		OriginSdkError::Decode(err.to_string())
	}
}

impl From<subxt::ext::scale_encode::Error> for OriginSdkError {
	fn from(err: subxt::ext::scale_encode::Error) -> Self {
		OriginSdkError::Encode(err.to_string())
	}
}
