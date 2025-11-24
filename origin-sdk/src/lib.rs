//! Origin SDK – dynamic Subxt client for Origin runtimes.
//!
//! Dynamic-only client (no runtime codegen) with view-first reads and async
//! extrinsic pipeline tuned for Origin pallets.

pub mod client;
pub mod config;
pub mod extrinsic;
pub mod query;
pub mod schema;
pub mod types;
pub mod util;

pub use client::OriginClient;
pub use types::error::OriginSdkError;

/// Convenient re-exports for application crates.
pub mod prelude {
	pub use crate::{
		client::OriginClient, config::OriginConfig, query::Query, types::error::OriginSdkError,
	};
}
