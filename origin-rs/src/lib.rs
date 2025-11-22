#![allow(missing_docs)]
#![doc = include_str!("../README.md")]

//! Origin SDK (view-only, dynamic Subxt).

pub mod client;
pub mod config;
pub mod domain;
pub mod error;
pub mod events;
pub mod extrinsic;
mod flavors;
pub mod metadata;
pub mod origin_client;
pub mod params;
pub mod types;
pub mod util;

pub use client::{
	BatchCall, Client, ConnectionConfig, EventFilter, EventWatcher, NonceManager, NonceState,
	NonceStrategy, OriginSigner, RetryPolicy, SubxtSignerAdapter, TransactionClient, ViewApi,
	DEFAULT_RPC_ENDPOINT,
};
pub use config::*;
pub use domain::{EntityClient, PacketClient, RegistryClient, TokenClient};
pub use error::Error;
pub use extrinsic::{BatchBuilder, CallFactory, DynamicCall, ExtrinsicBuilder, MetaTxClient};
pub use origin_client::{
	ClientConfig as DynamicClientConfig, DynamicEvent, EventStream, OriginClient,
	OriginClientBuilder, SubmitRetryPolicy, WsConfig,
};
pub use params::config::{OriginConfig, OriginHubConfig};
pub use types::*;
