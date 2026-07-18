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

//! Stable, bounded and redacted provider failure observations.

/// A stable provider failure code. Values deliberately carry no request, identity, capability,
/// content or peer material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderFailureCode {
	CheckpointDutyIntakeFailed,
	CheckpointLifecycleFailed,
	CheckpointQuorumActionFailed,
	CheckpointQuorumJoinFailed,
	CheckpointQuorumSelectionFailed,
	CheckpointQuorumTickFailed,
	ManifestDeletionFailed,
	ProviderHttpConnectionFailed,
	ReplicationCoordinatorFailed,
	ReplicationDiscoveryRejected,
	ReplicationIntentFailed,
}

impl ProviderFailureCode {
	pub(crate) const fn code(self) -> &'static str {
		match self {
			Self::CheckpointDutyIntakeFailed => "PROVIDER_CHECKPOINT_DUTY_INTAKE_FAILED",
			Self::CheckpointLifecycleFailed => "PROVIDER_CHECKPOINT_LIFECYCLE_FAILED",
			Self::CheckpointQuorumActionFailed => "PROVIDER_CHECKPOINT_QUORUM_ACTION_FAILED",
			Self::CheckpointQuorumJoinFailed => "PROVIDER_CHECKPOINT_QUORUM_JOIN_FAILED",
			Self::CheckpointQuorumSelectionFailed => "PROVIDER_CHECKPOINT_QUORUM_SELECTION_FAILED",
			Self::CheckpointQuorumTickFailed => "PROVIDER_CHECKPOINT_QUORUM_TICK_FAILED",
			Self::ManifestDeletionFailed => "PROVIDER_MANIFEST_DELETION_FAILED",
			Self::ProviderHttpConnectionFailed => "PROVIDER_HTTP_CONNECTION_FAILED",
			Self::ReplicationCoordinatorFailed => "PROVIDER_REPLICATION_COORDINATOR_FAILED",
			Self::ReplicationDiscoveryRejected => "PROVIDER_REPLICATION_DISCOVERY_REJECTED",
			Self::ReplicationIntentFailed => "PROVIDER_REPLICATION_INTENT_FAILED",
		}
	}

	pub(crate) const fn action(self) -> &'static str {
		match self {
			Self::CheckpointDutyIntakeFailed => "inspect-finalized-duty-cursor",
			Self::CheckpointLifecycleFailed => "inspect-checkpoint-journal",
			Self::CheckpointQuorumActionFailed => "inspect-quorum-confirmation",
			Self::CheckpointQuorumJoinFailed => "inspect-quorum-worker-health",
			Self::CheckpointQuorumSelectionFailed => "inspect-quorum-scheduler",
			Self::CheckpointQuorumTickFailed => "inspect-quorum-tick",
			Self::ManifestDeletionFailed => "inspect-deletion-duty-journal",
			Self::ProviderHttpConnectionFailed => "inspect-provider-listener",
			Self::ReplicationCoordinatorFailed => "inspect-replication-resume-journal",
			Self::ReplicationDiscoveryRejected => "inspect-finalized-topology",
			Self::ReplicationIntentFailed => "inspect-replication-intent",
		}
	}
}

pub(crate) fn emit_failure(code: ProviderFailureCode) {
	eprintln!("{}", failure_line(code));
}

fn failure_line(code: ProviderFailureCode) -> String {
	format!("provider_failure code={} action={}", code.code(), code.action())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn failure_lines_are_stable_actionable_and_redacted() {
		let line = failure_line(ProviderFailureCode::CheckpointQuorumTickFailed);
		assert_eq!(
			line,
			"provider_failure code=PROVIDER_CHECKPOINT_QUORUM_TICK_FAILED action=inspect-quorum-tick"
		);
		for forbidden in ["cid", "account", "subject", "profile", "proof", "token", "capability"] {
			assert!(!line.to_ascii_lowercase().contains(forbidden));
		}
	}

	#[test]
	fn every_code_has_a_bounded_action() {
		let codes = [
			ProviderFailureCode::CheckpointDutyIntakeFailed,
			ProviderFailureCode::CheckpointLifecycleFailed,
			ProviderFailureCode::CheckpointQuorumActionFailed,
			ProviderFailureCode::CheckpointQuorumJoinFailed,
			ProviderFailureCode::CheckpointQuorumSelectionFailed,
			ProviderFailureCode::CheckpointQuorumTickFailed,
			ProviderFailureCode::ManifestDeletionFailed,
			ProviderFailureCode::ProviderHttpConnectionFailed,
			ProviderFailureCode::ReplicationCoordinatorFailed,
			ProviderFailureCode::ReplicationDiscoveryRejected,
			ProviderFailureCode::ReplicationIntentFailed,
		];
		let mut names = codes.iter().map(|code| code.code()).collect::<Vec<_>>();
		names.sort_unstable();
		names.dedup();
		assert_eq!(names.len(), codes.len());
		assert!(codes.iter().all(|code| code.action().len() <= 48));
	}
}
