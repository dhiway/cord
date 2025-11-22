#![allow(missing_docs)]
#![doc = include_str!("../README.md")]

//! Origin SDK (view-only, dynamic Subxt).

pub mod client;
pub mod error;
pub mod flavors;
pub mod metadata;
pub mod origin_client;
pub mod params;

pub use client::{
    BatchCall, Client, ConnectionConfig, EventFilter, EventWatcher, NonceManager, NonceState,
    NonceStrategy, OriginSigner, RetryPolicy, SubxtSignerAdapter, TransactionClient, ViewApi,
    DEFAULT_RPC_ENDPOINT,
};
pub use error::Error;
pub use flavors::ChainFlavor;
pub use origin_client::{
    ClientConfig as DynamicClientConfig, DynamicEvent, EventStream, OriginClient,
    OriginClientBuilder, SubmitRetryPolicy, WsConfig,
};
pub use params::config::{OriginConfig, OriginHubConfig};
