#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

//! Origin SDK core library. Provides a lightweight facade over Subxt for
//! connecting to Orb/Origin/OriginHub chains, composing extrinsics, and
//! calling runtime view functions using JSON payloads.

pub mod client;
pub mod error;
pub mod flavors;
pub mod params;
pub mod state;
pub mod tx;
pub mod types;
pub mod views;

pub use client::Client;
pub use error::Error;
pub use flavors::ChainFlavor;
