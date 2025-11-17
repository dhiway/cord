#![allow(missing_docs)]
#![doc = include_str!("../README.md")]

//! Origin SDK core library. Provides a lightweight facade over Subxt for
//! connecting to Origin/OriginHub chains, composing extrinsics, and
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
pub mod metadata;
pub mod tx;
pub mod types;
pub mod utils;

// Expose both generated runtimes; keep `runtime` as the hub-default for
// backward compatibility while allowing simultaneous use of both modules.
pub use crate::api::runtime_hub as runtime;
pub use crate::api::{runtime_hub, runtime_origin};

pub use client::{Client, ConnectionConfig, RetryPolicy, DEFAULT_RPC_ENDPOINT};
pub use error::Error;
pub use flavors::ChainFlavor;
pub use params::config::{OriginConfig, OriginHubConfig};
pub use scale::MetadataResolver;

#[cfg(test)]
mod type_checks {
	use crate::api::runtime;

	#[test]
	fn verify_meta_tx_types_exist() {
		use runtime::runtime_types::{pallet_meta_tx::MetaTx, sp_runtime::generic::era::Era};
		let _ = core::any::TypeId::of::<MetaTx<(), ()>>();
		let _ = Era::Immortal;
	}
}
