use thiserror::Error;

use crate::error::Error as LegacyError;

/// Errors raised while decoding dynamic values into typed primitives.
#[derive(Debug, Error)]
pub enum DecodeError {
	#[error("missing field `{0}`")]
	MissingField(&'static str),
	#[error("unexpected type in {ctx}: {ty}")]
	UnexpectedType { ctx: &'static str, ty: String },
	#[error("dynamic decode error: {0}")]
	Dynamic(String),
}

/// High-level error surface for the new SDK facade.
#[derive(Debug, Error)]
pub enum OriginError {
	#[error("rpc error: {0}")]
	Rpc(#[from] LegacyError),
	#[error("decode error: {0}")]
	Decode(#[from] DecodeError),
	#[error("not found")]
	NotFound,
	#[error("{0}")]
	Unsupported(&'static str),
}

pub type Result<T> = core::result::Result<T, OriginError>;
