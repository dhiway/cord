//! Schema transform layer for Origin SDK.
//!
//! These helpers make the nested ↔ flat mapping explicit so we can
//! keep the SDK aligned with pallet expectations while still offering
//! ergonomic nested structures to application developers.
//!
//! Current implementation keeps the flat and nested shapes identical;
//! dedicated flatten/expand logic can be filled in as the pallet types
//! are mirrored in the SDK.

pub mod entity;
pub mod packet;
pub mod registry;
