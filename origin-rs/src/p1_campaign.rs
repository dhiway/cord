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

//! Typed evidence contract and fixed SCALE vectors for the P1 live campaign.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CONTROL_CASES: &[&str] = &[
	"AC7-VALIDATOR-REMOVE",
	"AC7-VALIDATOR-ADMIT",
	"AC7-COLLATOR-REMOVE",
	"AC7-COLLATOR-ADMIT",
	"AC7-KEY-ROTATION",
	"AC7-ORIGIN-UPGRADE",
	"AC7-ORBIS-UPGRADE",
	"AC7-TX-PAUSE-RECOVERY",
	"AC7-SAFE-MODE-RECOVERY",
	"AC7-COMPROMISE-RECOVERY",
];

pub const BROKER_PRE_RESTART_CASES: &[&str] = &[
	"AC8-BOOTSTRAP",
	"AC8-REQUEST",
	"AC8-RESERVE",
	"AC8-ASSIGN",
	"AC8-RENEW",
	"AC8-RESIZE-DOWN",
	"AC8-RESIZE-UP",
	"AC8-DELAYED",
	"AC8-DUPLICATE",
	"AC8-OUT-OF-ORDER",
	"AC8-RECEIPT-REORDER",
	"AC8-SESSION",
	"AC8-RELEASE",
];

pub const BROKER_POST_RESTART_CASES: &[&str] = &["AC8-FULL-RESTART", "AC8-RESTART-RECOVERY"];

#[derive(Clone, Debug, Deserialize)]
pub struct ScenarioManifest {
	pub schema: String,
	pub campaign_id: String,
	pub para_id: u32,
	pub phases: BTreeMap<String, Vec<Scenario>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Scenario {
	pub id: String,
	pub action: String,
	pub requires: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DriverEvidence {
	pub schema: &'static str,
	pub campaign_id: String,
	pub phase: String,
	pub status: CaseStatus,
	pub cases: BTreeMap<String, CaseRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaseStatus {
	Pass,
	Failed,
	CapabilityGap,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseRecord {
	pub status: CaseStatus,
	pub input_hashes: BTreeMap<String, String>,
	pub output_hashes: BTreeMap<String, String>,
	pub finalized_blocks: Vec<FinalizedBlock>,
	pub events: Vec<EventRecord>,
	pub assertions: Vec<AssertionRecord>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub failure: Option<FailureRecord>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FinalizedBlock {
	pub chain: String,
	pub number: u32,
	pub hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventRecord {
	pub chain: String,
	pub block_hash: String,
	pub pallet: String,
	pub variant: String,
	pub fields: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AssertionRecord {
	pub name: String,
	pub passed: bool,
	pub observed: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FailureRecord {
	pub kind: String,
	pub stage: String,
	pub message: String,
	pub missing_runtime_surface: Option<String>,
}

impl CaseRecord {
	pub fn capability_gap(stage: &str, message: &str, surface: &str) -> Self {
		Self {
			status: CaseStatus::CapabilityGap,
			input_hashes: BTreeMap::new(),
			output_hashes: BTreeMap::new(),
			finalized_blocks: Vec::new(),
			events: Vec::new(),
			assertions: vec![AssertionRecord {
				name: "required runtime surface exists".into(),
				passed: false,
				observed: surface.into(),
			}],
			failure: Some(FailureRecord {
				kind: "capability-gap".into(),
				stage: stage.into(),
				message: message.into(),
				missing_runtime_surface: Some(surface.into()),
			}),
		}
	}

	pub fn validate_pass(&self) -> Result<(), &'static str> {
		if self.status != CaseStatus::Pass {
			return Err("status is not pass");
		}
		if self.input_hashes.is_empty() || self.output_hashes.is_empty() {
			return Err("input/output hash ledger is empty");
		}
		if self.finalized_blocks.is_empty() || self.events.is_empty() {
			return Err("finalized block/event evidence is empty");
		}
		if self.assertions.is_empty() || self.assertions.iter().any(|item| !item.passed) {
			return Err("assertions are empty or contain a failure");
		}
		if self.failure.is_some() {
			return Err("pass record contains failure");
		}
		Ok(())
	}
}

pub fn expected_cases(phase: &str) -> Option<&'static [&'static str]> {
	match phase {
		"control" => Some(CONTROL_CASES),
		"broker-pre-restart" => Some(BROKER_PRE_RESTART_CASES),
		"broker-post-restart" => Some(BROKER_POST_RESTART_CASES),
		_ => None,
	}
}

/// Exact Origin `CoretimeControl.submit_request` call encoding.
pub fn provider_submit_call(id: u64, count: u16) -> Vec<u8> {
	let mut call = vec![221, 1];
	call.extend_from_slice(&id.to_le_bytes());
	call.extend_from_slice(&count.to_le_bytes());
	call
}

/// Exact Orbis `CoretimeControl.acknowledge` call encoding.
pub fn orbis_acknowledge_call(id: u64, count: u16, status_index: u8) -> Vec<u8> {
	let mut call = vec![221, 2];
	call.extend_from_slice(&id.to_le_bytes());
	call.extend_from_slice(&count.to_le_bytes());
	call.push(status_index);
	call
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fixed_case_partition_is_complete_and_unique() {
		let all = CONTROL_CASES
			.iter()
			.chain(BROKER_PRE_RESTART_CASES)
			.chain(BROKER_POST_RESTART_CASES)
			.copied()
			.collect::<std::collections::BTreeSet<_>>();
		assert_eq!(all.len(), 25);
	}

	#[test]
	fn stack_owned_control_call_vectors_are_stable() {
		assert_eq!(
			provider_submit_call(0x0102_0304_0506_0708, 0x090a),
			vec![221, 1, 8, 7, 6, 5, 4, 3, 2, 1, 10, 9]
		);
		assert_eq!(
			orbis_acknowledge_call(0x0102_0304_0506_0708, 0x090a, 3),
			vec![221, 2, 8, 7, 6, 5, 4, 3, 2, 1, 10, 9, 3]
		);
	}

	#[test]
	fn capability_gap_can_never_validate_as_pass() {
		let record = CaseRecord::capability_gap("delay", "no queue hold", "test-only UMP hold");
		assert!(record.validate_pass().is_err());
	}
}
