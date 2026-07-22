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

//! Private deterministic bridge from finalized topology evidence to peer wire context.
//!
//! This module owns no transport, route, worker or confirmation authority. It retains the exact
//! topology snapshot and endpoint bytes so later private orchestration cannot silently re-resolve
//! mutable provider state.

use crate::{
	chain::{
		ChainError, ReplicationProviderExclusion, ReplicationProviderSnapshot,
		ReplicationTopologySnapshot,
	},
	peer::{PeerContextV1, PeerMmrCommitmentV1},
};

/// One exact provider identity retained for a private replication session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReplicationSessionPeerV1 {
	provider: [u8; 32],
	order: u8,
	endpoint: Vec<u8>,
	endpoint_hash: [u8; 32],
	service_key: [u8; 32],
	service_key_version: u64,
}

impl ReplicationSessionPeerV1 {
	pub(crate) fn provider(&self) -> [u8; 32] {
		self.provider
	}

	pub(crate) fn order(&self) -> u8 {
		self.order
	}

	pub(crate) fn endpoint(&self) -> &[u8] {
		&self.endpoint
	}

	pub(crate) fn endpoint_hash(&self) -> [u8; 32] {
		self.endpoint_hash
	}

	pub(crate) fn service_key(&self) -> [u8; 32] {
		self.service_key
	}

	pub(crate) fn service_key_version(&self) -> u64 {
		self.service_key_version
	}
}

/// Owned, exact topology and wire context for one ordered source-to-target exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReplicationSessionV1 {
	topology: ReplicationTopologySnapshot,
	context: PeerContextV1,
	source: ReplicationSessionPeerV1,
	target: ReplicationSessionPeerV1,
	target_may_confirm: bool,
}

impl ReplicationSessionV1 {
	/// Derive one fail-closed session from an exact finalized topology snapshot.
	pub(crate) fn from_topology(
		topology: ReplicationTopologySnapshot,
		local_provider: [u8; 32],
		local_service_key: [u8; 32],
		source_provider: [u8; 32],
		target_provider: [u8; 32],
		commitment: PeerMmrCommitmentV1,
	) -> Result<Self, ChainError> {
		topology.validate(local_provider, local_service_key)?;
		commitment
			.validate()
			.map_err(|error| rejected(format!("replication MMR commitment is invalid: {error}")))?;
		if source_provider == target_provider {
			return Err(rejected("replication source and target must be distinct"));
		}
		if local_provider != source_provider && local_provider != target_provider {
			return Err(rejected(
				"local replication provider is neither the session source nor target",
			));
		}
		let governed = topology
			.governed_finalized_checkpoint
			.ok_or_else(|| rejected("governed replication checkpoint is unavailable"))?;
		let source_snapshot = topology
			.providers
			.iter()
			.find(|provider| provider.provider == source_provider)
			.ok_or_else(|| rejected("replication source is not an ordered bucket member"))?;
		let target_snapshot = topology
			.providers
			.iter()
			.find(|provider| provider.provider == target_provider)
			.ok_or_else(|| rejected("replication target is not an ordered bucket member"))?;
		if !source_snapshot.usable || !source_snapshot.exclusions.is_empty() {
			return Err(rejected("replication source is not fully usable"));
		}
		ensure_participation_evidence(source_snapshot, governed, "source")?;
		let confirmation_invalid_only = target_snapshot.exclusions.as_slice()
			== [ReplicationProviderExclusion::ConfirmationInvalid];
		if !target_snapshot.usable && !confirmation_invalid_only {
			return Err(rejected("replication target has a non-repairable exclusion"));
		}
		if target_snapshot.usable && !target_snapshot.exclusions.is_empty() {
			return Err(rejected("replication target usability evidence is inconsistent"));
		}
		ensure_participation_evidence(target_snapshot, governed, "target")?;

		let source = session_peer(source_snapshot, "source")?;
		let target = session_peer(target_snapshot, "target")?;
		let context = PeerContextV1::new(
			topology.genesis_hash,
			topology.finalized_hash,
			topology.finalized_number,
			topology.bucket_id,
			source.provider,
			target.provider,
			source.service_key_version,
			source.service_key,
			target.service_key_version,
			target.service_key,
			source.endpoint_hash,
			target.endpoint_hash,
			commitment,
		)
		.map_err(|error| rejected(format!("replication peer context is invalid: {error}")))?;
		Ok(Self {
			topology,
			context,
			source,
			target,
			target_may_confirm: !confirmation_invalid_only,
		})
	}

	pub(crate) fn topology(&self) -> &ReplicationTopologySnapshot {
		&self.topology
	}

	pub(crate) fn context(&self) -> &PeerContextV1 {
		&self.context
	}

	pub(crate) fn source(&self) -> &ReplicationSessionPeerV1 {
		&self.source
	}

	pub(crate) fn target(&self) -> &ReplicationSessionPeerV1 {
		&self.target
	}

	/// False for a repair target admitted solely due to `ConfirmationInvalid`.
	pub(crate) fn target_may_confirm(&self) -> bool {
		self.target_may_confirm
	}
}

fn ensure_participation_evidence(
	provider: &ReplicationProviderSnapshot,
	governed_checkpoint: u32,
	role: &str,
) -> Result<(), ChainError> {
	if !provider.record_present
		|| provider.endpoint.is_none()
		|| provider.endpoint_hash.is_none()
		|| provider.active_service_key.is_none()
		|| provider.active_service_key_version.is_none()
		|| !provider.status_active
		|| !provider.organization_valid
		|| provider.authority_validated_at != Some(governed_checkpoint)
		|| provider.overdue_challenges != 0
		|| !provider.eligible
	{
		return Err(rejected(format!("replication {role} lacks complete participation evidence")));
	}
	Ok(())
}

fn session_peer(
	provider: &ReplicationProviderSnapshot,
	role: &str,
) -> Result<ReplicationSessionPeerV1, ChainError> {
	let endpoint = provider
		.endpoint
		.clone()
		.ok_or_else(|| rejected(format!("replication {role} endpoint is unavailable")))?;
	let endpoint_hash = provider
		.endpoint_hash
		.ok_or_else(|| rejected(format!("replication {role} endpoint hash is unavailable")))?;
	let service_key = provider
		.active_service_key
		.ok_or_else(|| rejected(format!("replication {role} service key is unavailable")))?;
	let service_key_version = provider.active_service_key_version.ok_or_else(|| {
		rejected(format!("replication {role} service-key version is unavailable"))
	})?;
	if endpoint.is_empty()
		|| endpoint_hash == [0; 32]
		|| service_key == [0; 32]
		|| service_key_version == 0
	{
		return Err(rejected(format!("replication {role} evidence is invalid")));
	}
	Ok(ReplicationSessionPeerV1 {
		provider: provider.provider,
		order: provider.order,
		endpoint,
		endpoint_hash,
		service_key,
		service_key_version,
	})
}

fn rejected(message: impl Into<String>) -> ChainError {
	ChainError::Rejected(message.into())
}

#[cfg(test)]
mod tests {
	use codec::Encode;
	use sp_crypto_hashing::blake2_256;

	use super::*;

	fn provider(value: u8, order: u8, primary: bool) -> ReplicationProviderSnapshot {
		let endpoint = format!("https://provider-{value}.invalid").into_bytes();
		ReplicationProviderSnapshot {
			provider: [value; 32],
			order,
			primary,
			record_present: true,
			endpoint_hash: Some(blake2_256(&endpoint)),
			endpoint: Some(endpoint),
			active_service_key: Some([value + 10; 32]),
			active_service_key_version: Some(u64::from(value)),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(8),
			overdue_challenges: 0,
			eligible: true,
			usable: true,
			exclusions: Vec::new(),
			confirmed_checkpoint: None,
		}
	}

	fn topology() -> ReplicationTopologySnapshot {
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: [1; 32],
			finalized_hash: [2; 32],
			finalized_number: 10,
			governed_finalized_checkpoint: Some(8),
			bucket_id: [3; 32],
			bucket_version: 4,
			primary: [4; 32],
			replicas: vec![[5; 32], [6; 32]],
			providers: vec![provider(4, 0, true), provider(5, 1, false), provider(6, 2, false)],
			current_checkpoint: None,
			snapshot_hash: [0; 32],
		};
		reseal(&mut topology);
		topology
	}

	fn commitment() -> PeerMmrCommitmentV1 {
		PeerMmrCommitmentV1::new([20; 32], 0, 1, 0).unwrap()
	}

	fn reseal(topology: &mut ReplicationTopologySnapshot) {
		topology.snapshot_hash = [0; 32];
		let mut input = b"cord/provider/replication-topology/v1".to_vec();
		topology.encode_to(&mut input);
		topology.snapshot_hash = blake2_256(&input);
	}

	fn set_exclusion(
		topology: &mut ReplicationTopologySnapshot,
		index: usize,
		exclusion: ReplicationProviderExclusion,
	) {
		let provider = &mut topology.providers[index];
		provider.usable = false;
		provider.exclusions = vec![exclusion];
		match exclusion {
			ReplicationProviderExclusion::MissingProvider => {
				provider.record_present = false;
				provider.endpoint = None;
				provider.endpoint_hash = None;
				provider.active_service_key = None;
				provider.active_service_key_version = None;
			},
			ReplicationProviderExclusion::Inactive => provider.status_active = false,
			ReplicationProviderExclusion::GovernedCheckpointUnavailable => {
				topology.governed_finalized_checkpoint = None;
			},
			ReplicationProviderExclusion::OrgInvalid => provider.organization_valid = false,
			ReplicationProviderExclusion::AuthorityUnvalidated => {
				provider.authority_validated_at = None;
			},
			ReplicationProviderExclusion::OverdueChallenge => provider.overdue_challenges = 1,
			ReplicationProviderExclusion::RuntimeIneligible => provider.eligible = false,
			ReplicationProviderExclusion::InvalidEndpoint => {
				let endpoint = b"invalid-endpoint".to_vec();
				provider.endpoint_hash = Some(blake2_256(&endpoint));
				provider.endpoint = Some(endpoint);
			},
			ReplicationProviderExclusion::InvalidServiceKey => {
				provider.active_service_key = None;
				provider.active_service_key_version = None;
			},
			ReplicationProviderExclusion::ConfirmationInvalid => {},
		}
		reseal(topology);
	}

	#[test]
	fn local_source_local_target_and_remote_source_preserve_exact_topology() {
		let topology = topology();
		let local_source = ReplicationSessionV1::from_topology(
			topology.clone(),
			[4; 32],
			[14; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.unwrap();
		assert_eq!(local_source.context().finalized(), ([2; 32], 10));
		assert_eq!(local_source.context().bucket(), [3; 32]);
		assert_eq!(local_source.context().source_provider(), [4; 32]);
		assert_eq!(local_source.context().target_provider(), [5; 32]);
		assert_eq!(local_source.topology().genesis_hash, [1; 32]);
		assert_eq!(local_source.topology().bucket_version, 4);
		assert_eq!(local_source.topology().governed_finalized_checkpoint, Some(8));
		assert_eq!(local_source.topology().snapshot_hash, topology.snapshot_hash);
		assert_eq!(local_source.source().order(), 0);
		assert_eq!(local_source.target().order(), 1);
		assert_eq!(local_source.source().endpoint(), b"https://provider-4.invalid");
		assert_eq!(local_source.target().endpoint(), b"https://provider-5.invalid");
		assert!(local_source.target_may_confirm());

		let local_target = ReplicationSessionV1::from_topology(
			topology.clone(),
			[5; 32],
			[15; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.unwrap();
		assert_eq!(local_target.source().provider(), [4; 32]);
		assert_eq!(local_target.target().provider(), [5; 32]);
		assert_eq!(
			local_target.source().endpoint_hash(),
			blake2_256(b"https://provider-4.invalid")
		);
		let remote_source = ReplicationSessionV1::from_topology(
			topology,
			[6; 32],
			[16; 32],
			[4; 32],
			[6; 32],
			commitment(),
		)
		.unwrap();
		assert_eq!(remote_source.source().provider(), [4; 32]);
		assert_eq!(remote_source.target().provider(), [6; 32]);
	}

	#[test]
	fn confirmation_invalid_is_target_only_and_never_confirms() {
		let mut topology = topology();
		set_exclusion(&mut topology, 1, ReplicationProviderExclusion::ConfirmationInvalid);
		let target = ReplicationSessionV1::from_topology(
			topology.clone(),
			[5; 32],
			[15; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.unwrap();
		assert!(!target.target_may_confirm());
		assert!(ReplicationSessionV1::from_topology(
			topology,
			[6; 32],
			[16; 32],
			[5; 32],
			[6; 32],
			commitment(),
		)
		.is_err());
	}

	#[test]
	fn every_other_source_and_target_exclusion_is_rejected() {
		let exclusions = [
			ReplicationProviderExclusion::MissingProvider,
			ReplicationProviderExclusion::Inactive,
			ReplicationProviderExclusion::GovernedCheckpointUnavailable,
			ReplicationProviderExclusion::OrgInvalid,
			ReplicationProviderExclusion::AuthorityUnvalidated,
			ReplicationProviderExclusion::OverdueChallenge,
			ReplicationProviderExclusion::RuntimeIneligible,
			ReplicationProviderExclusion::InvalidEndpoint,
			ReplicationProviderExclusion::InvalidServiceKey,
		];
		for exclusion in exclusions {
			let mut excluded_source = topology();
			set_exclusion(&mut excluded_source, 1, exclusion);
			assert!(ReplicationSessionV1::from_topology(
				excluded_source,
				[6; 32],
				[16; 32],
				[5; 32],
				[6; 32],
				commitment(),
			)
			.is_err());

			let mut excluded_target = topology();
			set_exclusion(&mut excluded_target, 1, exclusion);
			assert!(ReplicationSessionV1::from_topology(
				excluded_target,
				[4; 32],
				[14; 32],
				[4; 32],
				[5; 32],
				commitment(),
			)
			.is_err());
		}
	}

	#[test]
	fn missing_evidence_topology_tamper_and_wrong_local_binding_fail_closed() {
		let mut missing_endpoint = topology();
		missing_endpoint.providers[1].endpoint = None;
		reseal(&mut missing_endpoint);
		assert!(ReplicationSessionV1::from_topology(
			missing_endpoint,
			[4; 32],
			[14; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.is_err());
		let mut missing_key = topology();
		missing_key.providers[1].active_service_key = None;
		reseal(&mut missing_key);
		assert!(ReplicationSessionV1::from_topology(
			missing_key,
			[4; 32],
			[14; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.is_err());
		let mut missing_version = topology();
		missing_version.providers[1].active_service_key_version = None;
		reseal(&mut missing_version);
		assert!(ReplicationSessionV1::from_topology(
			missing_version,
			[4; 32],
			[14; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.is_err());
		let mut zero_version = topology();
		zero_version.providers[1].active_service_key_version = Some(0);
		reseal(&mut zero_version);
		assert!(ReplicationSessionV1::from_topology(
			zero_version,
			[4; 32],
			[14; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.is_err());

		let mut tampered = topology();
		tampered.bucket_version += 1;
		assert!(ReplicationSessionV1::from_topology(
			tampered,
			[4; 32],
			[14; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.is_err());
		assert!(ReplicationSessionV1::from_topology(
			topology(),
			[4; 32],
			[99; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.is_err());
		assert!(ReplicationSessionV1::from_topology(
			topology(),
			[99; 32],
			[14; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.is_err());
		assert!(ReplicationSessionV1::from_topology(
			topology(),
			[6; 32],
			[16; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.is_err());
	}

	#[test]
	fn active_rotation_versions_and_keys_are_bound_exactly() {
		let mut topology = topology();
		topology.providers[0].active_service_key = Some([41; 32]);
		topology.providers[0].active_service_key_version = Some(7);
		topology.providers[1].active_service_key = Some([42; 32]);
		topology.providers[1].active_service_key_version = Some(9);
		reseal(&mut topology);
		let session = ReplicationSessionV1::from_topology(
			topology,
			[4; 32],
			[41; 32],
			[4; 32],
			[5; 32],
			commitment(),
		)
		.unwrap();
		assert_eq!(session.source().service_key(), [41; 32]);
		assert_eq!(session.source().service_key_version(), 7);
		assert_eq!(session.target().service_key(), [42; 32]);
		assert_eq!(session.target().service_key_version(), 9);
	}
}
