//! Origin SDK – dynamic Subxt client for Origin runtimes.
//!
//! This crate is currently scaffolding the async view/extrinsic pipeline; the
//! modules are stubbed so we can iterate quickly without coupling to runtime
//! pallets.

pub mod client;
pub mod extrinsic;
pub mod types;
pub mod util;

pub use client::OriginClient;
pub use types::error::OriginSdkError;

/// Convenient re-exports for application crates.
pub mod prelude {
	pub use crate::client::OriginClient;
	pub use crate::types::error::OriginSdkError;
}
