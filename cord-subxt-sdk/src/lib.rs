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

pub use client::{Client, ConnectionConfig, RetryPolicy, DEFAULT_RPC_ENDPOINT};
pub use error::Error;
pub use flavors::ChainFlavor;
pub use scale::MetadataResolver;

#[cfg(test)]
mod type_checks {
	use crate::api::runtime;

	#[test]
	fn verify_meta_tx_types_exist() {
		use runtime::runtime_types::pallet_meta_tx::MetaTx;
		use runtime::runtime_types::sp_runtime::generic::Era;
		let _ = core::any::TypeId::of::<MetaTx<
			runtime::runtime_types::cord_orb_runtime::RuntimeCall,
			runtime::runtime_types::cord_orb_runtime::MetaTxExtension,
		>>();
		let _ = Era::Immortal;
	}
}
