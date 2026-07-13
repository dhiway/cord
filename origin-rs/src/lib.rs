//! Origin SDK – dynamic Subxt client for Origin runtimes.
//!
//! Dynamic-only client (no runtime codegen) with view-first reads and async
//! extrinsic pipeline tuned for Origin pallets.

pub mod client;
pub mod config;
pub mod extrinsic;
pub mod p1_campaign;
pub mod product_sdk;
pub mod query;
pub mod schema;
pub mod tx;
pub mod types;
pub mod util;

pub use client::{signer::OriginSigner, OriginClient};
pub use types::{
	account::{
		account_id_from_subxt, account_id_to_ss58, account_id_to_ss58_subxt, origin_ss58_format,
		ss58_to_account_id, AccountError, CryptoScheme, OriginAccount, OriginPair,
		ORIGIN_SS58_PREFIX,
	},
	error::OriginSdkError,
};

/// Convenient re-exports for application crates.
pub mod prelude {
	pub use crate::{
		client::OriginClient,
		config::{OrbisConfig, OriginConfig},
		query::Query,
		tx::Tx,
		types::error::OriginSdkError,
	};
}
