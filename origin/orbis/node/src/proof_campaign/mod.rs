// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

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
