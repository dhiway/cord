//! Disposable, feature-gated Orbis proof-retention fault campaign.
//!
//! This module is compiled only for the dedicated campaign binary.  It is intentionally not
//! reachable from the production `origin-omni-node` entry point.

pub mod config;
pub mod proposer;
pub mod provider;
pub mod service;

pub use config::{Campaign, FaultKind};

pub const RECEIPT_PREFIX: &str = "ORIGIN_ORBIS_PROOF_CAMPAIGN_RECEIPT ";

pub fn receipt(campaign: &Campaign, phase: &str, detail: serde_json::Value) {
	let value = serde_json::json!({
		"schema": "cord.orbis.proof-campaign/v1",
		"mode": campaign.fault.as_str(),
		"target_block": campaign.target,
		"expected_genesis_hash": format!("{:?}", campaign.expected_genesis_hash),
		"phase": phase,
		"detail": detail,
	});
	log::info!(target: "orbis-proof-campaign", "{RECEIPT_PREFIX}{value}");
}
