//! Domain-first SDK facade (v2).
//!
//! Provides typed, ergonomic entry points while keeping Subxt dynamic usage internal.

pub mod client;
pub mod entities;
pub mod error;
pub mod packets;
pub mod registers;
pub mod tokens;
pub mod types;
pub(crate) mod wire;

pub use client::OriginClient;
pub use error::OriginError;
pub use types::*;
