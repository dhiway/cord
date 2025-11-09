use thiserror::Error;

/// Convenient alias for SDK result types.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Unified error surface for the Origin SDK.
#[derive(Debug, Error)]
pub enum Error {
	/// Transport/client level failures (network, WebSocket, etc.).
	#[error("transport: {0}")]
	Transport(String),
	/// RPC protocol or method errors.
	#[error("rpc: {0}")]
	Rpc(String),
	/// SCALE codec or metadata mismatches.
	#[error("codec: {0}")]
	Codec(String),
	/// Signer/key management related failures.
	#[error("signer: {0}")]
	Signer(String),
	/// Invalid or unsupported parameters supplied by the caller.
	#[error("params: {0}")]
	Params(String),
	/// Decoding runtime view output failed.
	#[error("view-decode: {0}")]
	ViewDecode(String),
	/// Referenced data was not found.
	#[error("not-found: {0}")]
	NotFound(String),
	/// Operation timed out.
	#[error("timeout")]
	Timeout,
}

impl From<subxt::Error> for Error {
	fn from(err: subxt::Error) -> Self {
		match err {
			subxt::Error::Codec(e) => Error::Codec(e.to_string()),
			subxt::Error::Rpc(e) => Error::Rpc(e.to_string()),
			subxt::Error::Metadata(e) => Error::Codec(e.to_string()),
			subxt::Error::MetadataDecoding(e) => Error::Codec(e.to_string()),
			subxt::Error::Decode(e) => Error::Codec(e.to_string()),
			subxt::Error::Encode(e) => Error::Codec(e.to_string()),
			subxt::Error::Transaction(e) => Error::Signer(e.to_string()),
			subxt::Error::Extrinsic(e) => Error::Signer(e.to_string()),
			subxt::Error::Block(e) => Error::Signer(e.to_string()),
			subxt::Error::StorageAddress(e) => Error::Params(e.to_string()),
			subxt::Error::Runtime(e) => Error::Rpc(e.to_string()),
			subxt::Error::Other(e) => Error::Params(e),
			subxt::Error::Io(e) => Error::Transport(e.to_string()),
			subxt::Error::Serialization(e) => Error::Codec(e.to_string()),
			subxt::Error::Unknown(bytes) => Error::Codec(format!("unknown error: {bytes:?}")),
			#[cfg(feature = "unstable-light-client")]
			subxt::Error::LightClient(e) => Error::Transport(e.to_string()),
		}
	}
}

impl From<subxt::ext::scale_decode::Error> for Error {
	fn from(err: subxt::ext::scale_decode::Error) -> Self {
		Error::Codec(err.to_string())
	}
}

impl From<subxt::ext::scale_encode::Error> for Error {
	fn from(err: subxt::ext::scale_encode::Error) -> Self {
		Error::Codec(err.to_string())
	}
}
