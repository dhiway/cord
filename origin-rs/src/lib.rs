#![allow(missing_docs)]
#![doc = include_str!("../README.md")]

//! Origin SDK core library. Provides a lightweight facade over Subxt for
//! connecting to Origin/OriginHub chains, composing extrinsics, and
//! calling runtime view functions using JSON payloads.

pub mod api_dynamic;
pub mod client;
pub mod demo;
pub mod entity;
pub mod error;
pub mod flavors;
pub mod metadata;
pub mod origin_client;
pub mod params;
pub mod query;
pub mod scale;
pub mod state;
pub mod tx;
pub mod types;
pub mod utils;
pub mod dyn_helpers;

pub use api_dynamic::{DynamicApis, DynamicEntityApi, DynamicRegisterApi, DynamicTokenApi};
pub use client::{Client, ConnectionConfig, RetryPolicy, DEFAULT_RPC_ENDPOINT};
pub use error::Error;
pub use flavors::ChainFlavor;
pub use origin_client::{
	ClientConfig as DynamicClientConfig, DynamicEvent, EventStream, NonceStrategy, OriginClient,
	OriginClientBuilder, SubmitRetryPolicy, WsConfig,
};
pub use params::config::{OriginConfig, OriginHubConfig};
pub use scale::MetadataResolver;
