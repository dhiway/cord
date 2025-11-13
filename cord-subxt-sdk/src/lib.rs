#![allow(missing_docs)]
#![doc = include_str!("../README.md")]

//! Origin SDK core library. Provides a lightweight facade over Subxt for
//! connecting to Orb/Origin/OriginHub chains, composing extrinsics, and
//! calling runtime view functions using JSON payloads.

pub mod api;
pub mod client;
pub mod demo;
pub mod entity;
pub mod error;
pub mod flavors;
pub mod params;
pub mod query;
mod runtime_helpers;
pub mod scale;
pub mod state;
pub mod tx;
pub mod types;
pub mod utils;

pub use client::Client;
pub use error::Error;
pub use flavors::ChainFlavor;
pub use scale::MetadataResolver;
