//! Subxt config for Origin networks (Origin base & hub).
//!
//! Keeps the SDK dynamic (no codegen) while ensuring hashes, headers, accounts,
//! and signatures align with Origin primitives.

use origin_primitives::{AccountId, BlockNumber, Signature};
use sp_runtime::MultiAddress;
use subxt::config::{
	substrate::{BlakeTwo256, SubstrateExtrinsicParams, SubstrateHeader},
	Config,
};

/// Chain config bound to Origin primitives.
#[derive(Debug, Clone, Copy, Default)]
pub struct OriginConfig;

impl Config for OriginConfig {
	type AccountId = AccountId;
	type Address = MultiAddress<Self::AccountId, u32>;
	type Signature = Signature;
	type Hasher = BlakeTwo256;
	type Header = SubstrateHeader<BlockNumber, BlakeTwo256>;
	type ExtrinsicParams = SubstrateExtrinsicParams<Self>;
	type AssetId = ();
}

/// Convenience alias for online client bound to `OriginConfig`.
pub type Client = subxt::OnlineClient<OriginConfig>;
