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

//! Redacted typed provider recovery outcomes derived from durable local and Commons state.

use serde::{Deserialize, Serialize};

use crate::{
	storage::CheckpointDutyInventory, CheckpointDutyMode, CheckpointDutyPhase, CheckpointDutyRole,
	IntegritySummary, PROTOCOL_VERSION,
};

/// Stable recovery outcome for provider operators and application diagnostics.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRecoveryOutcome {
	/// Local state could not be authenticated or read.
	LocalStateUnavailable,
	/// No complete finalized Commons duty snapshot is installed yet.
	ControlSnapshotUnavailable,
	/// Local content is quarantined and must be repaired before it can be served.
	RepairRequired,
	/// Commons selected this provider as the deterministic fallback initiator.
	FailoverReady,
	/// Commons selected another provider and this provider must await that transition.
	FailoverPending,
	/// Fallback promotion finalized and the promoted checkpoint must complete.
	PromotionPending,
	/// Commons reports insufficient quorum or no eligible initiator.
	Blocked,
	/// Local bytes and the installed Commons control view require no recovery action.
	Ready,
}

/// Explicit next action paired with [`ProviderRecoveryOutcome`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRecoveryAction {
	/// Operator intervention is required before retrying.
	EscalateLocalState,
	/// Wait for the bounded finalized duty scan to install a complete snapshot.
	AwaitControlSnapshot,
	/// Run or resume the authenticated local repair journal.
	RepairLocalData,
	/// Submit or resume the one Commons-authorized fallback transition.
	InitiateFallback,
	/// Wait for the selected fallback provider and finalized Commons observation.
	AwaitFallback,
	/// Complete or resume the promoted checkpoint lifecycle.
	CompletePromotedCheckpoint,
	/// Wait for the selected provider to complete the promoted checkpoint lifecycle.
	AwaitPromotedCheckpoint,
	/// Governance or provider membership must restore a viable quorum.
	EscalateControlPlane,
	/// No recovery action is required.
	None,
}

/// Redacted bounded summary of one completely installed Commons duty snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderControlRecoveryStatus {
	/// Finalized block number that fixed the complete duty inventory.
	pub finalized_number: u32,
	/// Governed checkpoint that fixed the duty inventory.
	pub snapshot_checkpoint: u32,
	/// Total local duties in the bounded inventory.
	pub duties: u32,
	/// Duties where this provider is the primary.
	pub primary_duties: u32,
	/// Duties where this provider is a replica.
	pub replica_duties: u32,
	/// Duties this provider alone may initiate in the installed view.
	pub initiator_duties: u32,
	/// Duties currently in deterministic replica fallback.
	pub failover_duties: u32,
	/// Duties completing a promoted checkpoint.
	pub promotion_pending_duties: u32,
	/// Duties with insufficient quorum or no eligible initiator.
	pub blocked_duties: u32,
}

/// Developer-consumable provider recovery state without object, bucket, duty, proof or key IDs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRecoveryStatus {
	/// Provider protocol version.
	pub version: u16,
	/// Stable typed outcome.
	pub outcome: ProviderRecoveryOutcome,
	/// Stable typed next action.
	pub action: ProviderRecoveryAction,
	/// Whether retrying after the named action or finalized-state advance is meaningful.
	pub retryable: bool,
	/// Whether all installed objects are currently admitted for verified reads.
	pub byte_plane_ready: bool,
	/// Redacted installed-object count when local state was readable.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub installed_objects: Option<u64>,
	/// Redacted verified-read-ready object count when local state was readable.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub ready_objects: Option<u64>,
	/// Redacted quarantined object count when local state was readable.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub quarantined_objects: Option<u64>,
	/// Complete redacted Commons snapshot summary, absent until one is durably installed.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub control: Option<ProviderControlRecoveryStatus>,
}

pub(crate) fn status(
	integrity: Option<IntegritySummary>,
	inventory: Option<CheckpointDutyInventory>,
	local_state_available: bool,
) -> ProviderRecoveryStatus {
	let control = inventory.as_ref().map(control_status);
	let (outcome, action, retryable) =
		classify(integrity.as_ref(), control.as_ref(), local_state_available);
	ProviderRecoveryStatus {
		version: PROTOCOL_VERSION,
		outcome,
		action,
		retryable,
		byte_plane_ready: integrity.as_ref().is_some_and(|summary| summary.ready),
		installed_objects: integrity.as_ref().map(|summary| summary.installed_objects),
		ready_objects: integrity.as_ref().map(|summary| summary.ready_objects),
		quarantined_objects: integrity.as_ref().map(|summary| summary.quarantined_objects),
		control,
	}
}

fn classify(
	integrity: Option<&IntegritySummary>,
	control: Option<&ProviderControlRecoveryStatus>,
	local_state_available: bool,
) -> (ProviderRecoveryOutcome, ProviderRecoveryAction, bool) {
	if !local_state_available || integrity.is_none() {
		return (
			ProviderRecoveryOutcome::LocalStateUnavailable,
			ProviderRecoveryAction::EscalateLocalState,
			false,
		);
	}
	let Some(integrity) = integrity else {
		return (
			ProviderRecoveryOutcome::LocalStateUnavailable,
			ProviderRecoveryAction::EscalateLocalState,
			false,
		);
	};
	if !integrity.ready {
		return (
			ProviderRecoveryOutcome::RepairRequired,
			ProviderRecoveryAction::RepairLocalData,
			true,
		);
	}
	let Some(control) = control else {
		return (
			ProviderRecoveryOutcome::ControlSnapshotUnavailable,
			ProviderRecoveryAction::AwaitControlSnapshot,
			true,
		);
	};
	if control.blocked_duties > 0 {
		return (
			ProviderRecoveryOutcome::Blocked,
			ProviderRecoveryAction::EscalateControlPlane,
			false,
		);
	}
	if control.promotion_pending_duties > 0 {
		return if control.initiator_duties > 0 {
			(
				ProviderRecoveryOutcome::PromotionPending,
				ProviderRecoveryAction::CompletePromotedCheckpoint,
				true,
			)
		} else {
			(
				ProviderRecoveryOutcome::PromotionPending,
				ProviderRecoveryAction::AwaitPromotedCheckpoint,
				true,
			)
		};
	}
	if control.failover_duties > 0 {
		return if control.initiator_duties > 0 {
			(ProviderRecoveryOutcome::FailoverReady, ProviderRecoveryAction::InitiateFallback, true)
		} else {
			(ProviderRecoveryOutcome::FailoverPending, ProviderRecoveryAction::AwaitFallback, true)
		};
	}
	(ProviderRecoveryOutcome::Ready, ProviderRecoveryAction::None, false)
}

fn control_status(inventory: &CheckpointDutyInventory) -> ProviderControlRecoveryStatus {
	let mut status = ProviderControlRecoveryStatus {
		finalized_number: inventory.finalized_number,
		snapshot_checkpoint: inventory.snapshot_checkpoint,
		duties: saturating_u32(inventory.duties.len()),
		primary_duties: 0,
		replica_duties: 0,
		initiator_duties: 0,
		failover_duties: 0,
		promotion_pending_duties: 0,
		blocked_duties: 0,
	};
	for duty in &inventory.duties {
		match duty.role {
			CheckpointDutyRole::Primary => status.primary_duties += 1,
			CheckpointDutyRole::Replica => status.replica_duties += 1,
		}
		status.initiator_duties += u32::from(duty.may_initiate);
		status.failover_duties += u32::from(matches!(
			duty.phase,
			CheckpointDutyPhase::ReplicaFallback | CheckpointDutyPhase::ReplicaFallbackPromotion
		));
		status.promotion_pending_duties +=
			u32::from(duty.mode == CheckpointDutyMode::PromotionPending);
		status.blocked_duties += u32::from(matches!(
			duty.phase,
			CheckpointDutyPhase::BlockedInsufficientFallbackQuorum |
				CheckpointDutyPhase::Unavailable
		));
	}
	status
}

fn saturating_u32(value: usize) -> u32 {
	u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn integrity(ready: bool) -> IntegritySummary {
		IntegritySummary {
			installed_objects: 1,
			ready_objects: u64::from(ready),
			quarantined_objects: u64::from(!ready),
			ready,
			last_detection_sequence: u64::from(!ready),
		}
	}

	fn duty(
		provider: u8,
		role: CheckpointDutyRole,
		phase: CheckpointDutyPhase,
		mode: CheckpointDutyMode,
		may_initiate: bool,
	) -> crate::CheckpointDuty {
		crate::CheckpointDuty {
			duty_id: format!("0x{}", hex::encode([0x11; 32])),
			bucket_id: format!("0x{}", hex::encode([0x22; 32])),
			provider: format!("0x{}", hex::encode([provider; 32])),
			role,
			service_key_version: 1,
			service_key: format!("0x{}", hex::encode([provider.saturating_add(1); 32])),
			snapshot_checkpoint: 40,
			snapshot_hash: format!("0x{}", hex::encode([0x33; 32])),
			due_at: 41,
			grace_until: 42,
			phase,
			mode,
			may_sign: true,
			may_initiate,
			encoded_duty: "0x00".into(),
			duty_fingerprint: format!("0x{}", hex::encode([0x44; 32])),
		}
	}

	fn inventory(duty: crate::CheckpointDuty) -> CheckpointDutyInventory {
		CheckpointDutyInventory {
			finalized_hash: format!("0x{}", hex::encode([0x55; 32])),
			finalized_number: 50,
			snapshot_checkpoint: 40,
			duties: vec![duty],
		}
	}

	#[test]
	fn three_provider_journey_has_one_initiator_and_typed_bounded_outcomes() {
		let failed_primary = status(
			Some(integrity(false)),
			Some(inventory(duty(
				1,
				CheckpointDutyRole::Primary,
				CheckpointDutyPhase::Primary,
				CheckpointDutyMode::Standard,
				true,
			))),
			true,
		);
		let fallback = |provider, may_initiate| {
			status(
				Some(integrity(true)),
				Some(inventory(duty(
					provider,
					CheckpointDutyRole::Replica,
					CheckpointDutyPhase::ReplicaFallback,
					CheckpointDutyMode::Standard,
					may_initiate,
				))),
				true,
			)
		};
		let selected = fallback(2, true);
		let observer = fallback(3, false);

		assert_eq!(failed_primary.outcome, ProviderRecoveryOutcome::RepairRequired);
		assert_eq!(failed_primary.action, ProviderRecoveryAction::RepairLocalData);
		assert_eq!(selected.outcome, ProviderRecoveryOutcome::FailoverReady);
		assert_eq!(selected.action, ProviderRecoveryAction::InitiateFallback);
		assert_eq!(observer.outcome, ProviderRecoveryOutcome::FailoverPending);
		assert_eq!(observer.action, ProviderRecoveryAction::AwaitFallback);
		assert_eq!(
			[selected, observer]
				.iter()
				.map(|item| item.control.as_ref().unwrap().initiator_duties)
				.sum::<u32>(),
			1
		);
	}

	#[test]
	fn promoted_checkpoint_distinguishes_writer_from_observer_and_ready_is_terminal() {
		let promotion = |provider, may_initiate| {
			status(
				Some(integrity(true)),
				Some(inventory(duty(
					provider,
					CheckpointDutyRole::Replica,
					CheckpointDutyPhase::ReplicaFallbackPromotion,
					CheckpointDutyMode::PromotionPending,
					may_initiate,
				))),
				true,
			)
		};
		assert_eq!(promotion(2, true).action, ProviderRecoveryAction::CompletePromotedCheckpoint);
		assert_eq!(promotion(3, false).action, ProviderRecoveryAction::AwaitPromotedCheckpoint);

		let ready = status(
			Some(integrity(true)),
			Some(inventory(duty(
				2,
				CheckpointDutyRole::Primary,
				CheckpointDutyPhase::NotDue,
				CheckpointDutyMode::Standard,
				false,
			))),
			true,
		);
		assert_eq!(ready.outcome, ProviderRecoveryOutcome::Ready);
		assert_eq!(ready.action, ProviderRecoveryAction::None);
		assert!(!ready.retryable);
	}

	#[test]
	fn unavailable_control_and_local_state_have_explicit_retry_semantics() {
		let waiting = status(Some(integrity(true)), None, true);
		assert_eq!(waiting.outcome, ProviderRecoveryOutcome::ControlSnapshotUnavailable);
		assert_eq!(waiting.action, ProviderRecoveryAction::AwaitControlSnapshot);
		assert!(waiting.retryable);

		let unavailable = status(None, None, false);
		assert_eq!(unavailable.outcome, ProviderRecoveryOutcome::LocalStateUnavailable);
		assert_eq!(unavailable.action, ProviderRecoveryAction::EscalateLocalState);
		assert!(!unavailable.retryable);
		assert!(!unavailable.byte_plane_ready);

		let blocked = status(
			Some(integrity(true)),
			Some(inventory(duty(
				2,
				CheckpointDutyRole::Replica,
				CheckpointDutyPhase::BlockedInsufficientFallbackQuorum,
				CheckpointDutyMode::Standard,
				false,
			))),
			true,
		);
		assert_eq!(blocked.outcome, ProviderRecoveryOutcome::Blocked);
		assert_eq!(blocked.action, ProviderRecoveryAction::EscalateControlPlane);
		assert!(!blocked.retryable);
		assert_eq!(blocked.control.unwrap().blocked_duties, 1);
	}
}
